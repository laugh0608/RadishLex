# Sync Server 本机 Smoke Runbook

本文档说明如何在开发者本机复验 RadishLex Go sync server 的本地启动、脱敏日志和 encrypted object 上传下载路径。读者是准备检查 `server/sync-server` 运行时边界的维护者和协作者。本文不包含 Docker Compose、公网部署、Rust 远端客户端接线、Flutter 同步 UI、平台壳联调或真实用户数据；Docker Compose 容器入口见 `docs/runbooks/sync-server-compose.md`。

## 前提

- 只使用合成 domain、device、object、signature 和 encrypted payload fixture。
- 不使用真实用户词、input code、reading、联系人、P1 事件、ranker 明细或真实恢复材料。
- 不把服务绑定到公网地址；默认使用 `127.0.0.1`。
- 本机 smoke 由人工明确启动和停止；AI 默认不保留后台服务。

## 自动化 Smoke

优先运行自动化 smoke，它会使用临时 SQLite 文件和临时 blob 目录，并通过短生命周期 `httptest` server 走真实 HTTP 请求：

```sh
cd server/sync-server
go test ./internal/runtime -run TestLocalServerSmokeUploadsReadsAndConflicts -count=1
```

该测试覆盖：

- runtime 装配 SQLite metadata store 和 local blob store。
- `POST /api/v1/domains` 创建同步域。
- `POST /api/v1/domains/{domain_id}/join-requests` 创建第二设备 pending join request。
- `POST /api/v1/domains/{domain_id}/join-requests/{join_request_id}/authorization` 用已授权设备签名激活第二设备，并保存 wrapped key bytes。
- `GET /api/v1/domains/{domain_id}/devices/{device_id}` 复验第二设备变为 active。
- `POST /api/v1/domains/{domain_id}/objects/{object_id}/versions` 上传 encrypted object version。
- `GET /api/v1/domains/{domain_id}/objects/{object_id}/versions/{version}` 读取 metadata。
- `GET /api/v1/domains/{domain_id}/objects/{object_id}/versions/{version}/payload` 下载 encrypted payload。
- 第二设备 stale `base_version` 返回 `409 conflict_stale_base_version` 和 latest metadata。
- 第二设备基于最新 `base_version` 上传 v2 后可读取对应 encrypted payload。
- runtime audit log 不包含请求体、signature、wrapped key bytes 或 encrypted payload bytes。

## 人工本机启动

需要人工确认后，可在一个终端启动服务：

```sh
cd server/sync-server
export RADISHLEX_SYNC_LISTEN=127.0.0.1:7319
export RADISHLEX_SYNC_METADATA_PATH="$(mktemp -d)/sync-server.sqlite"
export RADISHLEX_SYNC_BLOB_DIR="$(mktemp -d)/objects"
export RADISHLEX_SYNC_MAX_OBJECT_BYTES=16777216
export RADISHLEX_SYNC_RECOVERY_READS_PER_HOUR=12
go run ./cmd/radishlex-sync-server
```

停止服务使用 `Ctrl-C`。停止后可删除 `RADISHLEX_SYNC_METADATA_PATH` 所在临时目录和 `RADISHLEX_SYNC_BLOB_DIR` 临时目录。

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

- 如果日志、错误响应、SQLite 表、blob 路径或测试 fixture 出现真实明文输入数据，应停止并回退该设计。
- 如果本机 smoke 需要公网监听、Docker Compose、Flutter、平台壳或 Rust 远端客户端才能通过，应停止并重新收窄 server 运行时边界。
- 如果对象上传绕过签名验证、设备状态验证、payload hash / length 验证或 stale base version 冲突语义，应停止并修复服务端边界。
