# RadishLex Sync Server

本文说明 Go 同步服务的职责、运行配置、目录结构和开发验证入口，面向维护自部署服务、存储实现和 Rust 客户端联调的开发者。本文不复制完整 API 字段定义、生产发布步骤或阶段进度；协议参考见 [Sync Server API/Storage](../../docs/sync-server-api-storage.md)，部署操作见 [同步服务部署说明](../../deploy/sync-server/README.md)。

## 服务边界

同步服务默认不可信，只保存客户端提交的密文对象和完成协议所需的公开 metadata。它负责：

- domain、device join / authorization / revocation 和 lifecycle sequence。
- 设备签名 profile、公钥绑定和 wrapped epoch record。
- recovery record rotation、recovered device activation 与 recovery revocation。
- 密文对象版本、乐观并发、发现 cursor、blob 完整性和审计事件。

它不负责输入转换、候选排序、客户端合并、明文解密或密钥托管，也不得接收明文用户词库、原始输入事件、恢复码或 master key。

## 存储模型

运行时把数据拆为两类：

- SQLite metadata：domain/device lifecycle、签名 profile、对象版本、hash、blob reference 和 recovery metadata。
- blob directory：客户端已经加密的 payload bytes。

进程启动时在事务中应用 embedded migration，并把 SQLite `PRAGMA user_version` 维护为 schema v9。metadata 文件、blob 目录和中间路径必须是受控的普通文件 / 目录；符号链接、宽松权限或无法确认的路径会失败关闭。SQLite metadata 和 blob 数据必须作为同一个备份 / 恢复单元处理。

服务端会校验协议 metadata、签名链、hash、版本和 lifecycle 约束，但这些校验不改变“客户端才是明文与合并真相源”的边界。

## 代码结构

```text
cmd/radishlex-sync-server/  进程装配与退出码
internal/api/               HTTP 路由、DTO、鉴权和审计映射
internal/config/            环境变量、默认值和配置校验
internal/runtime/           SQLite/blob 装配、权限检查和 HTTP server
internal/storage/           Store 接口、memory/SQLite 实现和密码 metadata 校验
migrations/                 embedded SQLite schema 与兼容迁移
```

`api` 不得绕过 `storage.Store` 直接操作 SQLite 或 blob path；`cmd` 不承载协议逻辑；明文数据模型不得加入任何一层。

## 运行配置

| 环境变量 | 默认值 | 说明 |
| --- | --- | --- |
| `RADISHLEX_SYNC_LISTEN` | `127.0.0.1:7319` | Go HTTP upstream 监听地址 |
| `RADISHLEX_SYNC_METADATA_PATH` | `data/sync-server.sqlite` | SQLite metadata 路径 |
| `RADISHLEX_SYNC_BLOB_DIR` | `data/objects` | 密文 blob 目录 |
| `RADISHLEX_SYNC_MAX_OBJECT_BYTES` | `16777216` | 单对象 payload 上限 |
| `RADISHLEX_SYNC_RECOVERY_READS_PER_HOUR` | `12` | recovery read 限速 |
| `RADISHLEX_SYNC_ACCESS_TOKEN` | 空 | 可选 bearer token；非空时至少 32 字节且无空白 |

Go 进程本身提供 HTTP upstream。面向真实网络时必须在受控反向代理终止 TLS，并配置 bearer token 或等价的代理 / 网络访问控制；不得把未保护的 upstream 直接暴露到公网。

## API 与日志

API 以 `/api/v1` 为前缀，覆盖 domain、device lifecycle、wrapped epoch、recovery 和 object version。路径、字段、状态码、签名 canonical 和存储不变量统一见 [Sync Server API/Storage](../../docs/sync-server-api-storage.md)，客户端不得从本 README 推导协议。

审计日志只记录 request ID、route、公开协议 ID、结果码、状态码、byte count 和延迟。日志不得包含 bearer token、请求 / 响应体、密文 bytes、签名、公钥 bytes、wrapped material、恢复材料或任何 plaintext payload。

## 开发验证

在本目录执行 Go 单元、API、migration、storage conformance 和 runtime smoke：

```bash
go test ./...
```

仓库级跨语言和本地 HTTPS 验证从仓库根执行：

```bash
cargo test -p radishlex-ime-userdb --test two_client_go_http_sync
./scripts/check-sync-server-local-https.sh
```

本地启动、备份恢复、升级回滚和外部 TLS 验证应使用 [同步服务部署说明](../../deploy/sync-server/README.md) 与对应 runbook，避免手工拼接数据目录、token 或证书参数。
