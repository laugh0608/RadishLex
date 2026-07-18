# RadishLex ime-crypto 边界设计

本文档定义 `ime-crypto` 必须遵守的客户端加密、密钥、对象 envelope、删除同步和验证边界，读者是实现 `ime-crypto`、扩展 `ime-sync`、设计同步客户端和审阅 Go server 接口的开发者。本文不包含当前批次状态、完整设备配对协议、Go server migration、Flutter 页面设计或真实上传下载编排；实时进度见 `docs/status/current.md`，Go server 边界见 `docs/sync-server-api-storage.md`，生产恢复流程见 `docs/production-recovery-flow.md`，平台私钥边界见 `docs/adr/0004-platform-private-key-storage-backend.md`。

## 稳定定位

- `ime-crypto` 只负责客户端密钥、加解密、签名、恢复材料、算法版本和错误脱敏，不负责 UI、HTTP orchestration 或服务端业务。
- 只有 P2 规范化 payload 可以进入加密对象；P0/P1 原始数据、平台句柄和 provider 原始错误不得进入同步对象。
- Rust remote client 只接收已加密 object 和 signed manifest；Go server 不接触明文或客户端密钥。
- 合成、loopback、短生命周期服务和跨语言测试可在 M1/M2 期间继续验证实现，但真实用户同步入口必须等 M3 安全退出标准满足后开放。
- 平台输入和本地 manager 不依赖同步，可按 M1/M2 独立推进；平台签名 backend、恢复和设备授权不得因此被伪装为生产可用。
- 完成记录、平台设备矩阵和当前 blocker 写入 devlog、状态入口或对应 runbook，不在本文重复维护。

当前依赖选型：

- `chacha20poly1305`：提供 XChaCha20Poly1305 AEAD 和系统随机 nonce。
- `hkdf` + `sha2`：提供 HKDF-SHA256 对象密钥派生和 SHA-256 ciphertext hash。
- `argon2`：提供恢复码 Argon2id KDF；当前锁定版本为 `0.5.3`，主要传递依赖包括 `password-hash`、`blake2` 和 `base64ct`。
- `ed25519-dalek`：提供纯 Rust Ed25519 签名和验签；当前锁定版本为 `2.2.0`，许可为 `BSD-3-Clause`，当前只用于本地 test-memory signing key store 与签名对象测试。
- `p256`：提供 P-256 SEC1/P1363 编码校验和 ECDSA/SHA-256 验签；当前锁定版本为 `0.13.2`，关闭默认 PEM feature，只启用 `ecdsa` / `std`，许可为 `Apache-2.0 OR MIT`。

上述 AEAD、KDF、HKDF、hash 与 P-256 依赖来自 RustCrypto 生态，当前采用 MIT OR Apache-2.0 兼容许可口径；`ed25519-dalek` 采用 `BSD-3-Clause`。后续仍需要在移动端和低内存平台记录 Argon2id 恢复耗时。

## 职责分工

`ime-userdb` 负责：

- 保存本地 P1 / P2 数据和删除 tombstone。
- 提供可复验的 P2 来源计数和后续导出迭代入口。
- 继续阻止普通导入或旧事件复活 deleted tombstone。

`ime-sync` 负责：

- 定义同步对象类型、版本、base version 和冲突语义。
- 判断本地来源是否允许进入 P2 加密同步。
- 后续把 P2 记录整理成规范化 plaintext payload 字节，并通过 `SyncEnvelopeAssembler` 组装为密文 envelope 和同步草案元数据。

`ime-crypto` 负责：

- 管理本地密钥、设备密钥、对象密钥和恢复材料的模型。
- 对允许同步的 plaintext payload 执行加密、解密和完整性校验。
- 生成密文长度、密文 hash、nonce、算法标识和 key id 等 envelope 字段。
- 使用 AAD 绑定对象元数据，避免密文被换绑到其他对象或版本。

Go server 只负责：

- 存储密文对象和同步元数据。
- 做版本冲突检测和设备公钥登记。
- 不解密、不排序、不解析用户词、不保存明文事件。

## 数据准入

允许进入 `ime-crypto` 的 P2 plaintext payload：

- `dictionary.user_terms`
- `dictionary.deleted_terms`
- `ranker.weights`
- `settings.profile`
- `settings.schema`
- `backup.snapshot`

禁止进入 `ime-crypto` 的数据：

- P0 输入内容。
- P1 原始选择事件。
- 负反馈详细 reason 列表。
- 应用上下文统计和窗口正文。
- `import_batches` 本地审计记录。
- Rime 私有对象、SQLite handle、平台窗口句柄。
- 任何通过 FFI 导出的明文同步 payload。

P1 原始事件后续只能先在本机压缩为 P2 权重摘要，再由 P2 对象进入加密同步；原始事件行本身不能同步。

## 密钥模型

当前设计至少区分：

- `ProfileRootKey`：本设备本地保护的根材料，用于解锁同步密钥或恢复流程。
- `SyncMasterKey`：用户同步域的主密钥材料，用于派生对象加密密钥和设备包装密钥。
- `DeviceKeyPair`：设备加入同步域时使用的非对称密钥对。
- `DeviceWrappingKey`：旧设备或恢复流程为新设备包装同步密钥时使用。
- `ObjectKey`：按对象类型、对象 ID、版本和用途派生的对象加密密钥。

当前 Rust 模型边界：

- `SyncMasterKeyMaterial::derive_device_wrapping_key` 已按 recipient device id、wrapping key id 和 key epoch 派生设备包装 key；同一同步域内不同设备或不同 epoch 得到不同 key。
- `DeviceKeyDescriptor` 只记录设备 ID、公钥 ID、device key id 和 key epoch，并要求来源 key role 为 `DeviceKeyPair`；它不保存私钥，也不选择具体非对称算法。
- `DeviceWrappingRecord` 只记录 recipient device id、wrapping key id、key epoch、包装密文和创建时间，并要求来源 key role 为 `DeviceWrapping`；Debug 输出会隐藏 `encrypted_key`。
- `RecoveryCode` 解析 `RLX1` 恢复码、Crockford Base32 secret 和短校验段；Debug 输出不打印恢复码 secret。
- `RecoveryKdfProfile` 固定 `argon2id-v1`、`0x13`、64 MiB memory、3 iterations、4 parallelism、16 byte salt 和 32 byte output 的当前 profile，并拒绝弱化参数。
- `RecoveryMaterial` 记录 recovery id、domain id、key epoch、KDF 参数、salt、envelope algorithm、envelope nonce、恢复密文和时间戳；Debug 输出只显示 `salt_len` / `envelope_nonce_len`，不打印 `encrypted_recovery_key`。
- 设备签名与私钥存储边界已由 ADR 0003/0006 固定并在 Rust 模型中落地：`ed25519-v1` 与 `ecdsa-p256-sha256-v1` 按显式 profile 共存，签名 key 只用于签名，签名对象覆盖 object manifest、device authorization、device revocation 和 recovery record。普通测试可使用合成 `test-memory-v1`，平台 status 均保留生产门禁；`apple-keychain-p256-v1` 已通过命令行基础生命周期 Keychain smoke，但产品进程访问、失败矩阵和 capability status 尚未闭环，既有 Apple/Android Ed25519 阻塞也未解除。
- 当前模型用于固定字段、校验、AAD 绑定、恢复记录解密、签名验签和日志边界；生产设备私钥存储 backend、非对称包装算法和生产恢复 UI / API 仍需补齐后再进入远端同步。

规则：

- 服务端账号密码不得直接解密用户数据。
- 新设备加入必须通过已有设备授权或恢复码。
- 撤销设备后，后续对象必须使用新同步密钥或新 key epoch；旧设备不应能解密撤销后的新对象。
- 历史对象是否重加密是独立策略；如果不重加密，UI 和文档必须说明旧设备在被撤销前已经取得的历史密钥无法被技术上追回。
- 恢复码必须按 `docs/adr/0002-recovery-code-kdf.md` 使用 Argon2id KDF；当前已覆盖参数校验、错误码、AAD 绑定和 Debug 敏感字段阻断。

## 加密对象 Envelope

后续 `ime-crypto` 应输出类似结构：

```text
EncryptedObjectEnvelope
  schema_version
  object_id
  object_type
  owner_device_id
  key_id
  key_epoch
  algorithm
  nonce
  version
  base_version
  encrypted_payload
  ciphertext_hash
  created_at_ms
  updated_at_ms
```

`ciphertext_hash` 必须是密文或密文加 AAD 的 hash，不得是 plaintext payload hash。服务端可以用它做对象完整性和去重辅助，但不能通过 hash 猜测用户词。

当前 integration test 中的 object id、owner device id、key id、版本号和 nonce 都是合成 fixture，用于验证 envelope、AAD、解密和 sync draft 派生。它们不代表生产对象 ID 命名、nonce 分配或设备身份策略。

AAD 至少绑定：

```text
schema_version
object_id
object_type
owner_device_id
key_id
key_epoch
version
base_version
created_at_ms
updated_at_ms
```

解密时 AAD 不匹配、nonce 重复风险、算法未知、key id 不存在或认证标签失败，都必须返回明确错误，不得输出部分 plaintext。

## Payload 规范化

Plaintext payload 后续必须有稳定 schema：

- 包含 `payload_schema_version` 和 `object_type`。
- 字段顺序、字符串编码、空 reading 表达和时间戳单位必须稳定。
- `ranker.weights` 只能来自 P1 本地事件压缩后的 P2 权重摘要，不得包含原始选择事件、负反馈 reason、上下文统计或本地审计批次。
- 不依赖 SQLite rowid 作为跨设备身份。
- 不包含测试机路径、平台窗口信息、Rime session id 或调试日志。
- 测试 fixture 只能使用合成词和虚构设备 ID。

对象 ID 不得包含明文用户词、input code、reading 或上下文。若需要 term identity，优先放在 encrypted payload 内；如必须生成跨对象稳定 identifier，应使用同步域密钥派生的 keyed identifier，而不是公开 hash。

当前 `ime-userdb` 的 `stable_hash_hex` 只用于本地 tombstone 查询，不具备同步安全属性，不能作为服务端可见对象 ID、payload hash 或跨设备安全标识。

## 删除与冲突

`dictionary.deleted_terms` 是 P2 对象，必须参与加密同步。

规则：

- 删除 tombstone 优先于旧选择事件、旧导入和旧设备上传。
- 普通 dictionary import 不能复活 deleted tombstone。
- 用户显式手动添加可以作为恢复意图，但需要生成新的版本并清理对应删除意图。
- 冲突合并时必须覆盖旧设备复活、离线删除、备份恢复和重复导入场景。

## 实施顺序

1. 已补 `ime-crypto` crate 的 envelope、key role、nonce、ciphertext hash、AAD 绑定和错误模型测试。
2. 已确认并落地 XChaCha20Poly1305、HKDF-SHA256、SHA-256 和系统随机数依赖。
3. 已补合成 plaintext payload 的加密 / 解密 / 篡改失败测试，不读取真实 userdb。
4. 已将 `ime-sync::EncryptedSyncObjectDraft` 对齐 `ime-crypto` envelope，保持 `schema_version`、`key_id`、`key_epoch`、`algorithm`、`nonce`、`ciphertext_hash` 和版本语义不回退。
5. 已接入 userdb 的 `dictionary.user_terms` 与 `dictionary.deleted_terms` P2 plaintext payload 只读迭代器，仍不连接后端，不暴露 FFI 明文 payload。
6. 已接入 `ranker.weights` P2 plaintext payload schema，来源限制为 P1 本地事件压缩后的权重摘要，不导出 selection event、negative feedback、上下文统计或本地审计明细。
7. 已把 userdb P2 plaintext payload 接入 `ime-sync::SyncEnvelopeAssembler`，由该组装边界调用 `ime-crypto` 生成本地 envelope 并派生 `ime-sync::EncryptedSyncObjectDraft`；当前覆盖 `dictionary.user_terms`、`ranker.weights` 与 `dictionary.deleted_terms`，仍不暴露 FFI 明文 payload。
8. 已补 `ime-sync` remote object client DTO / transport trait 和 std-only `http://` HTTP transport，上传入口只接收 `AssembledSyncObject` 和 `SignedSyncObjectManifest`，用于验证 Go server JSON / base64 metadata、binary payload 下载、真实 HTTP request / response 传递和错误映射。
9. 已补 `docs/sync-key-management.md`，固定设备授权、恢复码、设备撤销、key epoch、服务端可见元数据和冲突边界。
10. 已在 Rust 侧补 device key descriptor、device wrapping key / record、recovery material、key epoch 和 device authorization 草案模型；测试覆盖设备包装 key 按设备和 epoch 派生、包装密文不进入 Debug 明文、授权设备和接收设备都必须 active、撤销后新对象使用新 `key_epoch` 且旧 epoch key 不能解密。
11. 已补 `ime-sync` 客户端解密后合并模型，覆盖删除 tombstone 压过旧 user terms / ranker weights、旧 epoch 上传不能复活删除词、显式恢复清理 tombstone 和恢复前旧权重不复活。
12. 已补 `ime-sync::SyncEnvelopeAssembler`，固定 Rust 内部 P2 payload 到 envelope 的组装边界，覆盖 sync master 派生 object key、nonce 复用阻断、draft 派生和 Debug 明文阻断。
13. 已补 `docs/adr/0002-recovery-code-kdf.md`，固定恢复码 Argon2id KDF、格式、恢复记录字段、失败限速和验证口径。
14. 已在 `ime-crypto` 补 `RecoveryCode`、`RecoveryKdfProfile`、`RecoveryWrappingKeyMaterial` 和 `RecoveryMaterial` 恢复记录加解密测试，覆盖恢复码格式 / 校验段、KDF 参数校验、同码同 salt 稳定派生、salt 变化、错误恢复码失败、AAD 变更失败和 Debug 脱敏。
15. 已补 `docs/adr/0003-device-signing-key-storage.md`，固定 Ed25519 设备签名、签名对象、canonical bytes、私钥存储抽象、错误语义和验证口径。
16. 已按 ADR 落地签名 / 设备密钥存储 Rust 模型，覆盖 Ed25519 test-memory signer、signed sync object manifest、signed recovery record、signed device authorization 和 signed device revocation。
17. 已补真实 userdb P2 payload 解析到 merge input 的接线。
18. 已补合并结果写回真实 userdb 的执行器。
19. 已补 Go server API / storage 边界设计、生产恢复流程设计和平台私钥存储 backend ADR。
20. 已补平台私钥存储 backend capability / unavailable backend 的 Rust 模型和测试，覆盖生产签名门禁、backend mismatch、unavailable 不回退和 revoked key 阻断。
21. 已按 `docs/sync-server-api-storage.md` 起步 Go server metadata / storage / API / runtime 验证模型，覆盖签名验签、device wrapping bytes、recovery wrapped material、object version、版本冲突、Rust envelope hash / length、错误语义、脱敏审计和单用户 bearer access token 门禁。
22. 已补 Docker Compose 本地 / 部署态入口、生产部署 runbook、Rust HTTP transport 直连 Go server 测试和 Rust userdb 两客户端真实 Go HTTP 同步测试；这些只验证密文对象、签名、版本和日志边界，不开放用户可用同步。
23. 已接线 `apple-keychain-v1` feature-gated macOS backend 和 smoke 入口；真实 smoke 阻塞于 `ed25519-v1` 创建，backend status 在 smoke 通过前阻断生产签名，不回退到 seed 存储或 `test-memory-v1`。
24. 已补 sync server 备份恢复、外部 TLS 反代和升级回滚实现级验证证据；进入用户可用同步前仍需补目标部署运行证据和可用平台私钥 backend，Apple 原生非导出 Ed25519 支持矩阵需单独调查。
25. 已补 `android-keystore-v1` 平台 runbook、`android-keystore` feature、不可用状态门禁、Rust bridge wrapper、bridge contract、raw JNI glue、合成 bridge 单测、ignored smoke 入口、仓库内 Kotlin bridge source、Gradle harness、`@JvmStatic` facade、gated instrumented smoke、provider diagnostics、smoke 记录模板和设备矩阵记录，固定 Android Keystore Ed25519 创建 / 加载 / 签名 / 删除、锁屏 / 权限、备份迁移、IME 生命周期和日志脱敏验证边界；Android target build 已通过 `./scripts/check-android-target.sh` 复验 `radishlex-ime-crypto --features android-keystore --target aarch64-linux-android`；Android Gradle harness 已在 Pixel 9 Pro API 35 AVD 上执行真实 smoke 和 provider diagnostics，并在 Pixel 10 Pro API 37 AVD 上执行 provider diagnostics，结果均为 `unsupported_signature_algorithm`，不解除生产签名门禁。

## 验证口径

进入真实同步前必须覆盖：

- P1 / 本地审计来源不能进入 crypto payload。
- 同一 key 下 nonce 不重复。
- P2 payload envelope 组装必须从 sync master 派生 object key，并拒绝非 object key descriptor。
- AAD 任一字段变化会导致解密失败。
- 密文或认证标签被篡改会导致解密失败。
- `ciphertext_hash` 不等于 plaintext hash，且不包含明文用户词。
- 删除 tombstone 加密同步后不会被旧对象复活。
- 已解密 userdb P2 payload 写回必须只应用被合并模型接受的记录，并覆盖 tombstone 阻断、显式恢复清理和旧权重阻断。
- 设备撤销后新对象使用新 key epoch。
- 恢复码格式校验、Argon2id 参数下限、恢复记录 AAD、错误恢复码和 Debug 脱敏必须保持测试覆盖。
- 签名 canonical bytes、签名字段篡改、错误设备公钥、非 active 设备签名、撤销签名 key 和私钥 Debug 脱敏必须保持测试覆盖。
- 平台私钥存储 backend unavailable 时必须明确阻断生产签名，不能静默回退到 `test-memory-v1`。

默认验证命令后续应至少包含：

```text
cargo test -p radishlex-ime-crypto
cargo test -p radishlex-ime-sync
cargo test -p radishlex-ime-userdb
./scripts/check-repo.sh
```

在真实设备同步落地前，当前仓库只要求 `ime-crypto` 模型、`ime-sync` payload 草案和 userdb / FFI preflight 继续保持一致。
