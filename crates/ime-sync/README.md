# ime-sync 组件说明

本文说明 `radishlex-ime-sync` 的职责、主要接口、数据流和开发验证入口，面向维护 Rust 同步客户端、userdb adapter 与平台产品装载的开发者。本文不记录阶段进度、部署流水或真实用户开关状态；协议细节分别以 [同步 payload](../../docs/sync-payload.md)、[同步 orchestration](../../docs/sync-orchestration.md) 和 [同步密钥管理](../../docs/sync-key-management.md) 为准。

## 组件定位

`ime-sync` 负责客户端侧同步协议和单次同步 cycle 的编排。它连接本地 P2 数据、客户端密码边界和不可信 Go server，但不拥有明文用户数据的长期存储，也不向远端提供明文上传入口。

依赖方向固定为：

```text
ime-userdb adapter
       │
       ▼
SyncLocalRepository ── SyncOrchestrationService ── SyncRemoteClient
       ▲                         │                         │
       │                         ▼                         ▼
local transaction       SyncObjectProcessor       HTTP/HTTPS transport
                                 │
                                 ▼
                       SyncCryptoProvider
```

- `SyncLocalRepository` 负责 cycle lease、cursor、journal、outbox、P2 snapshot 和原子 apply；正式实现位于 `ime-userdb`。
- `SyncOrchestrationService` 只编排阶段、重试、冲突重建和取消，不读取 SQLite，也不持有平台密钥。
- `SyncObjectProcessor` 负责下载对象的验签 / 解密 / 解码前置，以及上传对象的加密和签名。
- `SyncRemoteClient` 把加密 DTO 映射到 Go API；`SyncRemoteTransport` 隔离实际传输。

## 单次同步 cycle

`sync_once` 按以下顺序执行：

1. 获取本地 cycle lease，并执行产品与密码 preflight。
2. 从已提交 cursor 开始发现远端版本，逐个下载、校验 metadata、验签并解密。
3. 在一个本地事务中应用整页记录并推进 cursor；事务失败时不得部分推进。
4. 从本地 P2 snapshot 生成带签名的密文 outbox。
5. 上传 outbox；可重试的 transport 错误复用同一份密文和签名。
6. stale conflict 触发重新发现、合并并 supersede 旧 outbox，不原地篡改已签名对象。
7. 记录完成或结构化失败阶段并释放 lease。

`SyncCycleSummary` 只返回计数、阶段和错误分类，不包含 plaintext payload、token、密钥、签名或证书内容。

## 产品密码装载边界

`DefaultSyncObjectProcessor` 通过 `SyncCryptoProvider` 冻结一次 cycle 使用的密码快照。产品装载使用 `ProductSyncCryptoProvider`，并把三个来源保持独立：

- `SyncTrustedDeviceSource`：只返回已经验证的公开 domain/device lifecycle 和签名公钥。
- `SyncEpochMaterialStore`：只为当前本机 active device 返回当前与必要历史 epoch material。
- `SyncDeviceSigningBackend`：通过不可导出的 opaque handle 读取公开 profile 并签名 canonical bytes。

wrapped epoch 的产品适配由 `ProductWrappedEpochMaterialStore` 完成。它通过 `SyncWrappedEpochMaterialSource` 读取密文记录，再通过独立的 `SyncDeviceKeyAgreementBackend` 解封；domain、device、key id、公钥、epoch、AAD 或密文任一不匹配都会失败关闭。

必须保持以下存储边界：

- Rust-owned SQLite 可以保存可信公开 lifecycle、wrapped epoch record、cursor、journal 与密文 outbox。
- Manager settings 只保存非 secret 配置草案和脱敏状态摘要，不得镜像 lifecycle、wrapped record、cursor、journal、outbox 或任何密码材料。
- SQLite、settings、日志和远端服务不得保存明文 master key 或 shared secret。
- 明文 epoch material 只能进入 Rust 的 `SyncCryptoCycleSnapshot`，随单次 cycle 生命周期释放。
- 下载后的 P2 plaintext 只允许在 Rust 解密、解码和本地事务 apply 链中短暂存在，不得传给 transport 或 Manager。
- revoked/lost device 不得装载 epoch material，也不得签发新 outbox。

## HTTP 与 HTTPS

`HttpSyncRemoteTransport` 支持 `http://` 和严格验证的 `https://`：

- 公共 HTTPS 使用内置 Mozilla trust roots，并验证证书链和主机名。
- 本地资格验证可通过 `with_additional_root_certificate_der` 加入进程内 DER trust anchor；transport 不持久化该证书。
- 不提供跳过证书校验、关闭主机名校验或把 token 放入 URL 的入口。
- bearer token 至少 32 字节且不能包含空白；`Debug` 只暴露“是否配置”。
- `with_device_identity` 只发送协议要求的设备 ID header，不替代设备签名链。

部署拓扑和证书操作见 [同步服务部署说明](../../deploy/sync-server/README.md) 与 [Compose Runbook](../../docs/runbooks/sync-server-compose.md)。

## 模块索引

- `orchestration.rs`：cycle 状态、repository port、journal/outbox 语义和错误分类。
- `service.rs`：`SyncOrchestrationService` 及重试 / conflict 主循环。
- `processor.rs`：默认验签、解密、加密、签名 processor。
- `product_provider.rs`：可信 lifecycle、wrapped material、key-agreement 和 signing 产品端口。
- `remote.rs`、`remote/`：Go API DTO、lifecycle、epoch distribution、recovery 与 object client。
- `http_transport.rs`：HTTP/1.1 与 rustls HTTPS transport。
- `lifecycle.rs`、`epoch_distribution.rs`：签名 lifecycle 与 epoch distribution 校验。
- `assemble.rs`、`merge.rs`、`model.rs`：密文对象组装、确定性合并和 P2 object model。

## 开发验证

组件测试和严格静态检查：

```bash
cargo test -p radishlex-ime-sync
cargo clippy -p radishlex-ime-sync --all-targets -- -D warnings
```

跨 `ime-userdb` 与 Go server 的两客户端链：

```bash
cargo test -p radishlex-ime-userdb --test two_client_go_http_sync
```

本地 Caddy HTTPS、严格证书验证和真实 Rust 客户端链：

```bash
./scripts/check-sync-server-local-https.sh
```

测试必须使用合成 domain/device/object 和临时数据目录，不得读取真实用户 userdb、Keychain 项目或输入历史。
