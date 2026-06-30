# Sync Server Docker Compose Runbook

本文档说明如何用 Docker Compose 复验 RadishLex Go sync server 的容器运行边界，读者是准备检查本地 HTTPS、部署态 HTTP 上游、SQLite / encrypted blob 持久化和日志脱敏的维护者与协作者。本文不包含完整公网部署、生产认证、备份恢复策略、Flutter 同步 UI、平台壳联调、真实平台私钥存储 backend 或真实用户数据导入。

## 前提

- Compose 只负责自部署 sync server 的容器入口，不替代 Go / Rust 自动化测试。
- 本地容器验证使用 `deploy/sync-server/docker-compose.local.yaml`，默认访问 `https://localhost:7319`。
- 部署态使用 `deploy/sync-server/docker-compose.yaml`，容器只暴露 HTTP 上游 `http://127.0.0.1:7319`，外部 `Nginx / Traefik / Caddy` 负责 TLS 终止。
- 当前 server 仍缺少生产认证、备份策略、平台私钥 backend 和用户可用同步 UI，不应直接开放给真实用户或公网。
- 不写入真实用户词、input code、reading、联系人、P1 事件、ranker 明细或真实恢复材料。
- AI 默认不启动或保留长期运行服务；需要人工明确执行 `docker compose up`。

## 文件入口

- `deploy/sync-server/docker-compose.local.yaml`：本地容器验证入口，包含内部 `sync-server` 和本地 HTTPS `sync-gateway`。
- `deploy/sync-server/docker-compose.yaml`：部署态入口，只暴露 `sync-server` HTTP 上游。
- `deploy/sync-server/.env.example`：唯一 env 示例；真实部署复制为 `.env` 后修改。
- `deploy/sync-server/caddy/local.Caddyfile`：本地 `https://localhost:7319` Caddy internal TLS 入口。
- `deploy/sync-server/nginx.prod.conf`：生产外部 Nginx TLS 终止示例。
- `server/sync-server/Dockerfile`：Go sync server 多阶段构建。
- `server/sync-server/.dockerignore`：限制 Docker build context。

## 本地 HTTPS 验证

在仓库根目录执行：

```sh
docker compose -f deploy/sync-server/docker-compose.local.yaml config

docker compose -f deploy/sync-server/docker-compose.local.yaml up --build
```

后台运行：

```sh
docker compose -f deploy/sync-server/docker-compose.local.yaml up --build -d
```

本地入口：

```text
https://localhost:7319
```

本地 HTTPS 由 Caddy internal TLS 提供，因为 Go sync server 当前只实现 HTTP API。该 Caddy 入口只存在于本地 compose 文件中，不进入部署态 compose。Caddy internal TLS 证书默认不被宿主机信任；命令行 smoke 可使用 `curl -k`，浏览器或真实客户端验证如需无警告访问，应只在本机开发场景信任 Caddy 生成的本地 CA。

## 部署态 HTTP 上游

部署态与兄弟 Radish 项目保持同类边界：容器入口只提供 HTTP，上游 TLS 由外部反向代理终止。

建议复制模板到未提交的 `.env`，再按目标主机调整：

```sh
cd deploy/sync-server
cp .env.example .env
docker compose --env-file .env config
docker compose --env-file .env up --build -d
```

默认部署态上游：

```text
http://127.0.0.1:7319
```

关键变量：

- `COMPOSE_PROJECT_NAME`：控制容器和 volume 前缀。
- `RADISHLEX_SYNC_IMAGE`：镜像名；当前阶段默认仍支持本地 build。
- `RADISHLEX_SYNC_BIND`：默认 `127.0.0.1`，适合同机外部反代；如果反代不在同机，应改为受控内网地址，不要直接改成公网监听。
- `RADISHLEX_SYNC_PORT`：唯一对外端口，默认 `7319`；本地 compose 在同一端口提供 HTTPS，部署态 compose 在同一端口提供 HTTP 上游。
- `RADISHLEX_SYNC_DATA_PATH`：部署态宿主机持久化目录，默认 `../../DeployData/SyncServer`。
- `RADISHLEX_SYNC_METADATA_PATH`：SQLite metadata 容器内路径。
- `RADISHLEX_SYNC_BLOB_DIR`：encrypted blob 容器内目录。
- `RADISHLEX_SYNC_MAX_OBJECT_BYTES`：单个 encrypted object 上限。
- `RADISHLEX_SYNC_RECOVERY_READS_PER_HOUR`：recovery latest 读取限速。

`deploy/sync-server/nginx.prod.conf` 是生产外部反代示例，默认把 `https://sync.radishlex.example.com` 转发到 `127.0.0.1:7319`。真实部署前必须替换域名、证书路径和上游地址。

`.env.example` 不是完整生产上线方案。真实用户同步前仍必须补齐认证策略、备份恢复、升级回滚、外部 TLS 证书、监控告警、平台私钥 backend 和人工恢复演练。

## 日志与停止

本地查看日志：

```sh
docker compose -f deploy/sync-server/docker-compose.local.yaml logs -f sync-server sync-gateway
```

部署态查看日志：

```sh
cd deploy/sync-server
docker compose --env-file .env logs -f sync-server
```

停止但保留数据：

```sh
docker compose -f deploy/sync-server/docker-compose.local.yaml down
```

停止并删除本地 Compose volume：

```sh
docker compose -f deploy/sync-server/docker-compose.local.yaml down -v
```

`docker compose down -v` 会删除本地 SQLite metadata、encrypted blob 数据和 Caddy internal CA，只能在确认不需要保留本机测试数据时执行。部署态使用 `RADISHLEX_SYNC_DATA_PATH` 持久化到宿主机目录，清理前必须先确认备份和恢复策略。

## 配置覆盖

需要修改部署参数时优先复制 env 示例到未提交的 `.env`，不提交包含本机绝对路径、真实域名秘密或真实数据路径的配置。示例：

```dotenv
RADISHLEX_SYNC_BIND=127.0.0.1
RADISHLEX_SYNC_PORT=7319
RADISHLEX_SYNC_DATA_PATH=/srv/radishlex-sync
RADISHLEX_SYNC_MAX_OBJECT_BYTES=33554432
```

不要把 `RADISHLEX_SYNC_BIND` 改成公网地址后直接暴露服务。公网入口应先由外部 HTTPS 反代、认证、备份和日志策略兜住，且当前阶段仍不面向真实用户开放。

## 验证

修改 Compose、Dockerfile、Caddyfile、env、Nginx 示例、runtime 或 storage 后至少运行：

```sh
docker compose -f deploy/sync-server/docker-compose.local.yaml config

docker compose -f deploy/sync-server/docker-compose.yaml \
  --env-file deploy/sync-server/.env.example \
  config

go test ./...
./scripts/check-repo.sh
```

### 容器实际启动 smoke

容器启动 smoke 需要 Docker daemon 可用。执行前确认没有其他服务占用 `127.0.0.1:7319`，执行后必须 `down`，不能保留长期运行服务。

本地 HTTPS 入口：

```sh
docker compose -f deploy/sync-server/docker-compose.local.yaml up --build -d
curl -k -i https://localhost:7319/api/v1/domains/smoke-compose/state
docker compose -f deploy/sync-server/docker-compose.local.yaml down -v
```

预期 `curl` 能经由 Caddy internal TLS 到达 `sync-server`，返回 sync-server 的结构化 `404 not_found` JSON，而不是 TLS 握手失败、连接拒绝或 Caddy upstream 错误。

部署态 HTTP 上游可直接使用 `.env.example` 做配置解析；实际启动时建议用仓库外临时 env 覆盖 `RADISHLEX_SYNC_DATA_PATH` 到临时目录，避免在仓库中留下 `DeployData`：

```sh
tmpdir="$(mktemp -d)"
tmpenv="$tmpdir/radishlex-sync.env"
cp deploy/sync-server/.env.example "$tmpenv"
printf '\nRADISHLEX_SYNC_DATA_PATH=%s\n' "$tmpdir/data" >> "$tmpenv"

docker compose -f deploy/sync-server/docker-compose.yaml --env-file "$tmpenv" up --build -d
curl -i http://127.0.0.1:7319/api/v1/domains/smoke-compose/state
docker compose -f deploy/sync-server/docker-compose.yaml --env-file "$tmpenv" down
rm -rf "$tmpdir"
```

预期部署态 `curl` 能直连 `sync-server` HTTP upstream，返回结构化 `404 not_found` JSON。该入口不提供 TLS；真实生产 TLS 必须由外部反代终止。

如果 Docker daemon 或 Docker socket 权限不可用，不要把 `docker compose config` 写成容器启动通过。应记录：

- `docker compose version`
- `docker context ls`
- `docker version --format '{{.Server.Version}}'` 的失败信息
- 本地 HTTPS 和部署态 HTTP 的待复验命令
- 是否执行过 `down` / `down -v`，以及是否留下临时 env、临时数据目录或 Compose volume

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
