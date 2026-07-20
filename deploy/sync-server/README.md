# Sync Server 部署说明

本文说明 `deploy/sync-server` 中 Compose、TLS 示例和环境配置的用途，面向本地联调和自部署维护者。本文不是生产发布证明，也不记录当前阶段进度；完整操作步骤见 [Compose Runbook](../../docs/runbooks/sync-server-compose.md) 和 [Production Deployment Runbook](../../docs/runbooks/sync-server-production-deployment.md)。

## 两种拓扑

目录提供两套用途不同的 Compose 配置：

| 文件 | 用途 | 对外入口 | 数据 |
| --- | --- | --- | --- |
| `docker-compose.local.yaml` | 本机 HTTPS 联调与自动门禁 | `https://localhost:7319`，仅绑定 loopback | Docker named volume |
| `docker-compose.yaml` | 自部署 HTTP upstream | 默认 `http://127.0.0.1:7319`，交给外部代理终止 TLS | host bind mount |

`docker-compose.local.yaml` 使用 Caddy internal CA，只用于本地资格验证。测试脚本会显式提取临时 CA、转换为 DER 并注入 Rust transport 内存；不得把该 CA 当作公网证书，也不得安装到系统 trust store 伪装生产链路。

`docker-compose.yaml` 不内置公网 TLS。它默认只把 Go upstream 暴露到 `127.0.0.1`，由宿主机 Nginx、Caddy 或 Traefik 提供证书、域名和公网入口。`nginx.prod.conf` 只是拓扑与 header 转发示例，使用前必须替换域名、证书路径并完成外部 TLS 验证。

## 文件说明

- `.env.example`：部署变量模板，不包含可用 secret。
- `docker-compose.yaml`：非 root、read-only filesystem、capability drop 的部署服务。
- `docker-compose.local.yaml`：Go server + Caddy 本地 HTTPS 测试拓扑。
- `caddy/local.Caddyfile`：仅供本地 internal TLS。
- `nginx.prod.conf`：外部 TLS termination 示例。

## Secret 与持久化

- 复制 `.env.example` 为未提交的 `.env`，为 `RADISHLEX_SYNC_ACCESS_TOKEN` 生成至少 32 字节且无空白的随机值。
- token 不得出现在 URL、Compose 文件、日志、截图或 committed 文档中。
- `RADISHLEX_SYNC_DATA_PATH` 必须指向专用持久化目录；不要指向仓库根、用户主目录或共享临时目录。
- 备份和恢复必须同时覆盖 SQLite metadata 与密文 blob，并在隔离目录完成恢复验证。
- 外部反向代理必须转发 `Authorization`；设备身份 header 只属于协议 metadata，不能代替 bearer token 或设备签名。

## 本地 HTTPS 验证

从仓库根执行稳定入口：

```bash
./scripts/check-sync-server-local-https.sh
```

该脚本负责短生命周期 Compose、Caddy CA、严格证书链 / 主机名校验、Go API smoke 和真实 Rust 两客户端同步。成功后会清理容器与临时证书；如果中途失败，按脚本输出和 Compose runbook 检查残留资源。

只检查 Compose 解析而不启动服务：

```bash
docker compose -f deploy/sync-server/docker-compose.local.yaml config
docker compose -f deploy/sync-server/docker-compose.yaml --env-file deploy/sync-server/.env.example config
```

## 自部署操作入口

实际启动、健康检查、备份恢复、升级回滚和外部 TLS 证据不要从本页省略步骤执行，统一进入：

- [Compose Runbook](../../docs/runbooks/sync-server-compose.md)
- [Local Smoke Runbook](../../docs/runbooks/sync-server-local-smoke.md)
- [Production Deployment Runbook](../../docs/runbooks/sync-server-production-deployment.md)

Go 服务的 API、环境变量与存储边界见 [Go 同步服务说明](../../server/sync-server/README.md)；Rust 客户端的 transport 和密码装载边界见 [ime-sync 组件说明](../../crates/ime-sync/README.md)。
