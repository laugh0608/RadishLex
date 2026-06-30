# Sync Server Docker Compose Runbook

本文档说明如何用 Docker Compose 在本机启动 RadishLex Go sync server，读者是准备复验自部署运行边界的维护者和协作者。本文不包含公网部署、TLS / 反向代理、Flutter 同步 UI、平台壳联调、真实平台私钥存储 backend 或真实用户数据导入。

## 前提

- 只用于本机开发和自部署边界验证。
- 默认只绑定 `127.0.0.1:7319`，不暴露到公网。
- 当前 server 仍缺少生产认证、TLS、备份策略和平台私钥 backend，不应直接作为真实用户同步入口。
- 不写入真实用户词、input code、reading、联系人、P1 事件、ranker 明细或真实恢复材料。
- AI 默认不启动或保留长期运行服务；需要人工明确执行 `docker compose up`。

## 文件入口

- `compose.yaml`：本机 Compose 入口。
- `server/sync-server/Dockerfile`：Go sync server 多阶段构建。
- `server/sync-server/.dockerignore`：限制 Docker build context。

Compose 默认配置：

- 镜像：`radishlex-sync-server:dev`
- 容器监听：`0.0.0.0:7319`
- 主机绑定：`127.0.0.1:7319`
- SQLite metadata：`/var/lib/radishlex/sync-server/sync-server.sqlite`
- encrypted blob dir：`/var/lib/radishlex/sync-server/objects`
- Compose named volume：`sync-server-data`
- 最大对象大小：`16777216`
- recovery latest 每小时读取上限：`12`

## 启动

在仓库根目录执行：

```sh
docker compose up --build sync-server
```

后台运行：

```sh
docker compose up --build -d sync-server
```

查看日志：

```sh
docker compose logs -f sync-server
```

停止但保留 SQLite 和 encrypted blobs：

```sh
docker compose down
```

停止并删除本机 Compose volume：

```sh
docker compose down -v
```

`docker compose down -v` 会删除 SQLite metadata 和 encrypted blob 数据，只能在确认不需要保留本机测试数据时执行。

## 配置覆盖

如需改端口或路径，优先用本机 override 文件，不提交包含本机绝对路径或真实数据路径的配置：

```yaml
# compose.override.yaml
services:
  sync-server:
    ports:
      - "127.0.0.1:87319:7319"
    environment:
      RADISHLEX_SYNC_MAX_OBJECT_BYTES: "33554432"
```

不要把服务改为 `0.0.0.0:7319:7319` 后直接暴露到公网。公网部署需要先补 TLS、认证、备份、日志留存、升级和恢复策略。

## 验证

Compose 配置只负责启动服务，不替代自动化测试。修改 Compose、Dockerfile、runtime 或 storage 后至少运行：

```sh
go test ./...
./scripts/check-repo.sh
```

如需确认容器实际可启动，由开发者人工执行：

```sh
docker compose up --build sync-server
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

- 如果 Compose 需要公网监听才能完成本机验证，应停止并回退配置。
- 如果 container log、SQLite 表、blob 路径或测试 fixture 出现真实明文输入数据，应停止并回退设计。
- 如果需要把同步主密钥、恢复码明文、平台私钥或 P1 原始事件放入 environment、volume、日志或 image layer，应停止并回退设计。
- 如果真实用户同步需要 Docker Compose 之外的 TLS、认证、备份、恢复或平台私钥 backend，而这些边界尚未补齐，不应开放给真实用户使用。
