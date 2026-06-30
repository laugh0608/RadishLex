# Sync Server Docker Compose Runbook

本文档说明如何用 Docker Compose 复验 RadishLex Go sync server 的容器运行边界，读者是准备检查本地 HTTPS 入口、部署态 HTTP 上游、SQLite / encrypted blob 持久化和日志脱敏的维护者与协作者。本文不包含完整公网部署、生产认证、备份恢复策略、Flutter 同步 UI、平台壳联调、真实平台私钥存储 backend 或真实用户数据导入。

## 前提

- Compose 只负责自部署 sync server 的容器入口，不替代 Go / Rust 自动化测试。
- 本地容器验证默认访问 `https://localhost:7443`，由 `sync-gateway` 使用 Caddy internal TLS 反代到内部 `sync-server:7319`。
- 部署态默认访问 `http://127.0.0.1:7319`，用于同机外部 `Nginx / Traefik / Caddy` 终止 TLS 后转发；如需内网监听，应在私有部署 env 中显式修改 bind。
- 当前 server 仍缺少生产认证、备份策略、平台私钥 backend 和用户可用同步 UI，不应直接开放给真实用户或公网。
- 不写入真实用户词、input code、reading、联系人、P1 事件、ranker 明细或真实恢复材料。
- AI 默认不启动或保留长期运行服务；需要人工明确执行 `docker compose up`。

## 文件入口

- `compose.yaml`：统一 Compose 入口，包含内部 `sync-server` 和对外 `sync-gateway`。
- `deploy/sync-server/env/local.env`：本地 HTTPS 验证配置。
- `deploy/sync-server/env/production.env`：部署态 HTTP 上游配置模板。
- `deploy/sync-server/caddy/local.Caddyfile`：本地 `https://localhost:7443` Caddy internal TLS 入口。
- `deploy/sync-server/caddy/production.Caddyfile`：部署态 HTTP 入口。
- `server/sync-server/Dockerfile`：Go sync server 多阶段构建。
- `server/sync-server/.dockerignore`：限制 Docker build context。

Compose 默认服务：

- `sync-server`：容器内监听 `0.0.0.0:7319`，不直接发布主机端口。
- `sync-gateway`：唯一对外入口，按 env 文件选择 HTTPS 或 HTTP。
- `sync-server-data`：保存 SQLite metadata 与 encrypted blob dir。
- `sync-gateway-data` / `sync-gateway-config`：保存 Caddy internal CA 和运行配置。

## 本地 HTTPS 验证

在仓库根目录执行：

```sh
docker compose --env-file deploy/sync-server/env/local.env config
docker compose --env-file deploy/sync-server/env/local.env up --build
```

后台运行：

```sh
docker compose --env-file deploy/sync-server/env/local.env up --build -d
```

本地入口：

```text
https://localhost:7443
```

Caddy internal TLS 证书默认不被宿主机信任。命令行 smoke 可使用 `curl -k`；浏览器或真实客户端验证如需无警告访问，应只在本机开发场景信任 Caddy 生成的本地 CA，不要把该 CA 作为生产证书。

## 部署态 HTTP 上游

部署态与兄弟 Radish 项目保持同类边界：容器入口只提供 HTTP，上游 TLS 由外部反向代理终止。

建议先复制模板到未提交的本机 env 文件，再按目标主机调整：

```sh
cp deploy/sync-server/env/production.env .env.radishlex-sync-production
docker compose --env-file .env.radishlex-sync-production config
docker compose --env-file .env.radishlex-sync-production up --build -d
```

默认部署态入口：

```text
http://127.0.0.1:7319
```

关键变量：

- `RADISHLEX_SYNC_PUBLIC_BIND`：默认 `127.0.0.1`，适合同机外部反代；如果反代不在同机，应改为受控内网地址，不要直接改成公网监听。
- `RADISHLEX_SYNC_PUBLIC_PORT`：主机暴露端口，默认 `7319`。
- `RADISHLEX_SYNC_GATEWAY_PORT`：Caddy 容器内监听端口，默认与公开端口一致。
- `RADISHLEX_SYNC_CADDYFILE`：选择本地 HTTPS 或部署态 HTTP Caddyfile。
- `RADISHLEX_SYNC_METADATA_PATH`：SQLite metadata 容器内路径。
- `RADISHLEX_SYNC_BLOB_DIR`：encrypted blob 容器内目录。
- `RADISHLEX_SYNC_MAX_OBJECT_BYTES`：单个 encrypted object 上限。
- `RADISHLEX_SYNC_RECOVERY_READS_PER_HOUR`：recovery latest 读取限速。

`production.env` 不是完整生产上线方案。真实用户同步前仍必须补齐认证策略、备份恢复、升级回滚、外部 TLS 证书、监控告警、平台私钥 backend 和人工恢复演练。

## 日志与停止

查看日志：

```sh
docker compose --env-file deploy/sync-server/env/local.env logs -f sync-server sync-gateway
```

停止但保留 SQLite、encrypted blobs 和 Caddy 本地 CA：

```sh
docker compose --env-file deploy/sync-server/env/local.env down
```

停止并删除本机 Compose volume：

```sh
docker compose --env-file deploy/sync-server/env/local.env down -v
```

`docker compose down -v` 会删除 SQLite metadata、encrypted blob 数据和 Caddy internal CA，只能在确认不需要保留本机测试数据时执行。

## 配置覆盖

需要修改部署参数时优先使用 env 文件，不提交包含本机绝对路径、真实域名秘密或真实数据路径的配置。示例：

```dotenv
RADISHLEX_SYNC_PUBLIC_BIND=127.0.0.1
RADISHLEX_SYNC_PUBLIC_PORT=87319
RADISHLEX_SYNC_GATEWAY_PORT=87319
RADISHLEX_SYNC_MAX_OBJECT_BYTES=33554432
```

不要把 `RADISHLEX_SYNC_PUBLIC_BIND` 改成公网地址后直接暴露服务。公网入口应先由外部 HTTPS 反代、认证、备份和日志策略兜住，且当前阶段仍不面向真实用户开放。

## 验证

修改 Compose、Dockerfile、Caddyfile、env、runtime 或 storage 后至少运行：

```sh
docker compose --env-file deploy/sync-server/env/local.env config
docker compose --env-file deploy/sync-server/env/production.env config
go test ./...
./scripts/check-repo.sh
```

如需确认容器实际可启动，由开发者人工执行：

```sh
docker compose --env-file deploy/sync-server/env/local.env up --build
```

当前仓库没有提交依赖 Docker daemon 的 CI 检查；Docker build / run 失败时，应先记录镜像、平台、Go 版本和日志，再判断是配置问题还是本机 Docker 环境问题。

## 允许日志字段

日志允许包含：

- request id
- route name
- domain id
- device id
- object id
- object type
- version
- result code
- HTTP status
- encrypted byte length
- server time
- latency

## 禁止日志字段

日志不得包含：

- 请求体或响应体。
- encrypted payload bytes 或其 base64 表示。
- signature bytes。
- wrapped material bytes。
- recovery material bytes。
- plaintext user term、input code、reading、P1 event、ranker 明细或真实用户上下文。

## 停止线

- 如果本地 HTTPS 验证必须改为公网监听才能通过，应停止并回退配置。
- 如果部署态 HTTP 上游没有外部 TLS / 认证 / 备份边界却准备开放给真实用户，应停止并回退部署计划。
- 如果 container log、SQLite 表、blob 路径或测试 fixture 出现真实明文输入数据，应停止并回退设计。
- 如果需要把同步主密钥、恢复码明文、平台私钥或 P1 原始事件放入 environment、volume、日志或 image layer，应停止并回退设计。
- 如果真实用户同步需要 Docker Compose 之外的 TLS、认证、备份、恢复或平台私钥 backend，而这些边界尚未补齐，不应开放给真实用户使用。
