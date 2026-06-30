# Sync Server Production Deployment Runbook

本文档定义 RadishLex Go sync server 进入真实用户部署前必须确认的部署边界、备份恢复、升级回滚和停止线。读者是维护自部署 sync server 的开发者、部署者和审阅隐私边界的协作者。本文不包含 Flutter 同步 UI、平台输入法接入、真实平台私钥存储 backend、云厂商专用 Terraform / Helm、完整监控系统或公网开放许可；容器入口细节见 `docs/runbooks/sync-server-compose.md`，恢复码业务流程见 `docs/production-recovery-flow.md`。

## 当前结论

- 部署态入口仍是 `deploy/sync-server/docker-compose.yaml` 加唯一 env 示例 `deploy/sync-server/.env.example`，不新增第二个 env 文件。
- 部署态只提供同机 HTTP upstream `http://127.0.0.1:7319`；外部 TLS 必须在反向代理、VPN 或等价网络边界完成，Go server 通过单用户 bearer access token 执行首个内建访问门禁。
- 本地验证入口仍是显式 `-f deploy/sync-server/docker-compose.local.yaml`，通过 Caddy internal TLS 提供 `https://localhost:7319`，不新增第二个对外端口。
- Go server 已验证密文对象上传下载、设备授权、版本冲突、日志脱敏、Docker Compose 本地 / 部署态启动 smoke 和 Rust userdb 两客户端真实 Go HTTP 同步；这些证据仍不等于可以开放真实用户同步。
- 真实用户同步前仍缺少备份演练、升级回滚演练、外部 TLS 真实验证、平台私钥存储 backend 和用户可用同步 UI；生产访问认证已有单用户 bearer token 实现证据，但部署者仍必须设置真实 token 并复验失败响应。

## 部署拓扑

推荐拓扑：

```text
Client
  -> HTTPS / access control
  -> external reverse proxy
  -> http://127.0.0.1:7319
  -> sync-server container
  -> RADISHLEX_SYNC_DATA_PATH/sync-server.sqlite
  -> RADISHLEX_SYNC_DATA_PATH/objects/
```

规则：

- `RADISHLEX_SYNC_BIND` 默认保持 `127.0.0.1`。如果反代不在同机，只能改为受控内网地址；不能直接改成公网监听。
- `RADISHLEX_SYNC_PORT` 默认保持 `7319`。本地 HTTPS 和部署态 HTTP upstream 都使用这个对外端口，不引入备用端口。
- `RADISHLEX_SYNC_LISTEN` 在容器内可以保持 `0.0.0.0:7319`，外部暴露范围由 Compose 端口绑定控制。
- `RADISHLEX_SYNC_DATA_PATH` 必须是宿主机持久化目录，包含 SQLite metadata 和 encrypted blob dir；不要把真实数据写入仓库。
- `RADISHLEX_SYNC_PUBLIC_URL` 当前只作为部署说明字段，不代表 server 已实现公网认证或客户端发现。

## 外部 TLS

`deploy/sync-server/nginx.prod.conf` 只是 TLS 终止骨架，不是可直接公网开放的完整生产配置。上线前至少确认：

- 证书链和私钥路径指向目标域名的真实证书。
- HTTP 入口重定向到 HTTPS。
- 只启用 TLS 1.2 / TLS 1.3 或更高安全基线。
- `client_max_body_size` 不小于 `RADISHLEX_SYNC_MAX_OBJECT_BYTES`，且不会允许超过服务端对象大小门禁的请求通过。
- `proxy_read_timeout` / `proxy_send_timeout` 与服务端超时策略一致，不让长时间挂起请求占满连接。
- 反代日志不得记录请求体、响应体、encrypted payload bytes、signature bytes、wrapped material bytes 或恢复材料。

停止线：

- 如果只有裸 HTTP upstream，没有外部 TLS，不得开放给真实用户。
- 如果证书、域名、上游地址和对象大小限制没有被部署者逐项确认，不得把该配置当作生产配置。

## 认证与访问控制

Go sync server 当前的设备签名、join request 和 object manifest 验签只证明同步域内设备身份，不等同于部署访问认证。部署访问认证由 bearer access token 和外部网络 / TLS 边界共同承担。

OIDC / Radish 产品账号体系接入已记录为未来专题，见 `docs/sync-server-oidc-roadmap.md`。当前生产部署 runbook 不要求部署 Radish Auth、Gateway 或兼容 OIDC IdP；若未来改为 OIDC，必须先固定认证策略 ADR、token 验证规则、scope-route 映射、客户端 token 存储和外部 gateway header 防伪边界。

当前固定的首个方案是 Go server 内建单用户 bearer access token：

- 在未提交的部署 `.env` 或等价 secret 注入机制中设置 `RADISHLEX_SYNC_ACCESS_TOKEN`。
- token 必须随机生成，至少 32 bytes，不包含空白字符；不要使用 domain id、device id、object id、恢复码、同步主密钥或平台私钥派生。
- 所有客户端请求必须发送 `Authorization: Bearer <token>`。Go handler 在业务 storage 前校验 token，缺失、重复、格式错误或不匹配时返回 `401 unauthenticated`。
- Rust `HttpSyncRemoteTransport` 继续拒绝 URL credentials、query token 和 fragment token；访问启用 token 的 server 时必须通过 transport 配置 bearer token header。
- 更换 token 只影响部署访问，不改变同步域密钥、设备授权、object version、恢复记录或 tombstone 语义。

可叠加的外层控制：

- 私有网络 / VPN / Tailscale / WireGuard 之类的受控网络，只允许已授权设备访问 upstream。
- 反向代理 mTLS，并配套客户端证书管理、撤销和轮换流程。
- 反向代理 `auth_request` / OIDC / 等价外部认证，并配套客户端支持。

当前不可接受：

- 仅靠随机 URL、domain id、device id 或 object id 作为认证。
- 仅靠服务端设备签名验签来替代访问控制。
- 把 access token、恢复码明文、同步主密钥或平台私钥提交进 Git、写进 image layer、写进 Nginx 示例、写入备份索引或打印进日志。
- 在没有访问控制的情况下把 `RADISHLEX_SYNC_BIND` 改成 `0.0.0.0` 或公网 IP。

部署操作要求：

- `.env` 必须保持未提交，权限建议 `0600`，并排除在备份清单的“非敏感配置”之外。
- `deploy/sync-server/.env.example` 只能保留空 token 占位，不能写真实 token。
- 外部反代必须保留 `Authorization` header 到 upstream，或由外层认证方案明确替换 Go token 门禁；当前 Nginx 示例显式转发该 header。
- 验证失败响应时只记录 HTTP status、`error_code` 和 route，不记录 token。

最小验证：

```sh
curl -i http://127.0.0.1:7319/api/v1/domains/smoke-auth/state
curl -i -H 'Authorization: Bearer <redacted-access-token>' \
  http://127.0.0.1:7319/api/v1/domains/smoke-auth/state
```

预期第一条返回 `401 unauthenticated`，第二条在 domain 不存在时返回结构化 `404 not_found`。这只验证 Go token 门禁和 upstream 到达性，不替代外部 HTTPS 验证。

停止线：

- `RADISHLEX_SYNC_ACCESS_TOKEN` 为空、过短、含空白字符或未完成失败响应验证时，不开放真实用户同步。
- 认证失败、签名失败、rate limit 和 storage 错误不得返回请求体、payload、signature 或恢复材料。

## 数据目录与权限

部署态数据目录由 `RADISHLEX_SYNC_DATA_PATH` 控制。目录中至少包含：

```text
sync-server/sync-server.sqlite
sync-server/objects/
```

要求：

- 目录位于仓库外，不提交到 Git。
- 只允许 sync-server 运行用户和备份任务读取。
- 备份系统应把该目录视为敏感数据：blob 是密文，但 metadata 包含设备 ID、对象 ID、版本、时间和审计事件。
- 日志和备份索引不得把宿主机真实用户名、联系人、输入词、input code 或 reading 写入文件名。
- `docker compose down -v` 只适用于本地 named volume 测试；部署态清理必须针对 `RADISHLEX_SYNC_DATA_PATH` 做人工确认。

## 备份

当前推荐冷备份，先停止服务再复制 SQLite metadata 和 encrypted blob dir，保证二者处于同一个时间点。

流程：

1. 通知用户暂停同步写入。
2. 执行 `docker compose --env-file .env down`。
3. 复制整个 `RADISHLEX_SYNC_DATA_PATH` 到受保护备份位置。
4. 记录镜像 tag、`.env` 非敏感配置、备份时间和恢复演练结果。
5. 执行 `docker compose --env-file .env up -d`。
6. 检查日志只包含允许字段，并确认没有 `storage_unavailable`、hash mismatch 或 migration error。

规则：

- 不只备份 SQLite，不只备份 `objects/`；二者必须成对恢复。
- 未验证 SQLite online backup 或文件系统快照一致性前，不把运行中的普通文件复制当作可靠备份。
- 备份可以包含密文和 metadata，但仍需要加密存储、访问审计和保留期限。
- 备份中不得包含 `.env` 里的真实秘密、TLS 私钥、管理 token、恢复码明文、同步主密钥或平台私钥。

## 恢复

恢复流程：

1. 停止 sync-server。
2. 把当前数据目录移到隔离位置，避免覆盖后无法回退。
3. 恢复备份中的整个 `RADISHLEX_SYNC_DATA_PATH`。
4. 运行 `docker compose --env-file .env config`，确认数据路径、端口和镜像 tag。
5. 启动 sync-server。
6. 检查日志、domain state API、recovery latest API 和对象 payload 读取路径。
7. 让客户端拉取、解密、合并后重新上传最新对象，确认 tombstone、设备撤销和 key epoch 没有被旧备份破坏。

风险：

- 恢复旧服务端备份可能回退 object version、设备撤销记录或最新 tombstone 可见性；它是灾难恢复手段，不是普通撤销或业务回滚手段。
- 如果旧备份早于设备撤销或恢复记录撤销，必须先复核设备状态和 key epoch，再允许客户端继续写入。
- 如果恢复后出现 hash / length mismatch，应停止服务并回退到恢复前隔离目录或另一个备份。

## 升级与回滚

升级前：

- 固定 `RADISHLEX_SYNC_IMAGE` 为明确 tag，不用浮动 `latest`。
- 执行一次冷备份。
- 记录当前 Git commit、镜像 tag、Go server migration 版本和 `.env` 非敏感配置。
- 运行 `docker compose --env-file .env config`。

升级：

1. 拉取或构建目标镜像。
2. 执行 `docker compose --env-file .env up -d`。
3. 检查启动日志、migration 日志、domain state API 和对象 payload 读取路径。
4. 观察客户端同步是否只出现预期的版本冲突或签名错误，不出现 plaintext、hash drift 或 storage corruption。

回滚：

- 如果只是镜像启动失败且 migration 未运行，可切回旧镜像 tag 后重启。
- 如果 migration 已运行或数据写入已发生，不能只切旧镜像；应停止服务，恢复升级前冷备份，再用旧镜像启动。
- 当前没有承诺 down migration；任何 schema 不兼容都按“恢复备份”处理。

停止线：

- 无备份不升级。
- migration 失败、hash mismatch、blob 缺失或日志出现敏感字段时，停止并回滚。
- 升级后如果客户端需要重新上传合并结果，必须保持客户端解密合并真相源，不允许服务端解析 plaintext payload。

## 验证

部署配置变更至少执行：

```sh
docker compose -f deploy/sync-server/docker-compose.yaml \
  --env-file deploy/sync-server/.env.example \
  config

docker compose -f deploy/sync-server/docker-compose.local.yaml config

git diff --check
./scripts/check-repo.sh
```

需要 Docker daemon 的 build / up / curl smoke 如果被沙盒、Docker socket 或权限限制挡住，应申请真实环境复验。不能把 `config` 通过写成容器实际启动通过。

上线前人工复验：

- 外部 HTTPS 证书和域名。
- 访问控制策略和失败响应。
- 冷备份、恢复到隔离目录、回滚旧镜像。
- 日志脱敏。
- `RADISHLEX_SYNC_DATA_PATH` 权限和备份保留策略。
- 客户端设备授权、恢复记录和对象版本冲突路径。

## 停止线

- 没有外部 TLS，不开放真实用户同步。
- 没有认证 / 访问控制，不开放真实用户同步。
- 没有冷备份和恢复演练，不升级或开放真实用户同步。
- 没有平台私钥存储真实 backend，不进入管理 UI 同步主线。
- 日志、备份、Nginx access log 或错误响应出现明文用户词、input code、reading、P1 事件、signature bytes、wrapped material bytes、恢复码明文或密钥材料时，停止并回退。
- 任何方案要求服务端解密 userdb payload、保存同步主密钥、保存恢复码明文或参与输入热路径时，停止并回退。
