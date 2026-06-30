# Apple Keychain Signing Backend Runbook

本文档定义 `apple-keychain-v1` 设备签名 backend 的平台验证边界。读者是后续实现 macOS / iOS Keychain bridge、`ime-crypto` backend 接线、管理 UI 设备页面和审阅同步隐私边界的开发者。本文不包含 Swift / Objective-C 源码、FFI 导出接口、App Sandbox entitlement 配置、输入法安装流程、Flutter 页面或真实用户同步开放步骤；平台私钥抽象见 `docs/adr/0004-platform-private-key-storage-backend.md`。

## 当前结论

- `apple-keychain-v1` 是第一批真实平台私钥 backend 的优先验证对象。
- 默认 Rust workspace 继续只启用 `test-memory-v1` 和 `unavailable` backend，不访问 Keychain；macOS backend 只在显式 `apple-keychain` feature 下编译。
- `apple-keychain-v1` 已完成 feature-gated 接线和 ignored smoke 测试骨架；真实 Keychain smoke 尚未执行，不能视为平台验证通过。
- `apple-keychain-v1` 用于真实远端对象前必须通过本 runbook 的创建、加载、签名、删除 / 撤销、锁屏 / 权限、备份迁移和日志脱敏验证。
- 未验证 Secure Enclave 前，不承诺 `hardware_backed = true`。
- 未验证 user presence 前，不承诺 `user_presence_required = true`。
- 未验证 iCloud Keychain / 设备迁移语义前，不承诺 `backup_migratable = true`。
- 真实用户同步仍不得因为 macOS Keychain 可用就绕过外部 TLS、认证、备份恢复演练或客户端合并写回边界。

## Backend 标识与职责

稳定 backend id：

```text
apple-keychain-v1
```

职责：

- 平台 bridge 创建 Ed25519 设备签名私钥，并保存到 Apple Keychain 或等价 Apple 安全存储。
- 平台 bridge 返回 `DeviceSigningPublicKey`、`DeviceSigningKeyHandle` metadata 和签名结果。
- Rust core 只接收 canonical bytes、public key、handle metadata 和 signature bytes。
- FFI / CLI / Flutter / 平台壳不得获得私钥 bytes、seed、Keychain item secret 或可导出 key backup。

当前不做：

- 不把 Apple Keychain 调用直接散落到同步、userdb、ranker 或平台壳。
- 不通过 `security` 命令行工具实现生产 backend。
- 不在默认 `cargo test` 中访问用户真实 Keychain。
- 不把 Keychain account、access group 或 label 设计成包含系统用户名、设备真实名称、词库内容、input code 或本机绝对路径。

## 建议桥接边界

平台层可以使用 Apple Security framework 的 Keychain / SecKey API：

- `SecKeyCreateRandomKey`：创建设备签名私钥。
- `SecKeyCopyPublicKey`：读取公钥。
- `SecKeyCreateSignature`：对 canonical bytes 签名。
- `SecItemCopyMatching`：按 opaque key id 加载 key item。
- `SecItemDelete`：删除或撤销本机 key item。

Rust core 仍只看到抽象方法：

```text
create_signing_key(device_id, signing_key_id, created_at_ms) -> DeviceSigningPublicKey
handle(device_id, signing_key_id) -> DeviceSigningKeyHandle
public_key(handle) -> DeviceSigningPublicKey
sign(handle, canonical_bytes) -> DeviceSignature
delete_or_revoke(handle, revoked_at_ms)
backend_status() -> DevicePrivateKeyStoreStatus
```

桥接规则：

- `sign` 只能接收 canonical bytes，不接收 plaintext payload、SQLite row、HTTP request body 或 userdb JSON。
- `handle` 中的 `signing_key_id` 必须是 RadishLex 生成的 opaque id，不是 Keychain 可读 label。
- Keychain item lookup 所需 account / application tag / access group 是本地实现细节，不进入 Go server metadata。
- 如果平台 API 需要用户交互，应返回 `private_key_user_presence_required` 或 `private_key_access_denied`，不得回退到 `test-memory-v1`。
- 如果 Keychain 被锁定、权限缺失或 item 损坏，应返回明确错误，不创建新 key 顶替旧设备身份。

## 能力声明

首版 macOS software-protected Keychain backend 建议能力：

```text
storage_backend = apple-keychain-v1
exportable = false
hardware_backed = false
user_presence_required = false
backup_migratable = false
```

规则：

- `exportable = false` 只表示 RadishLex backend 不提供私钥导出路径；如果平台策略允许系统迁移或备份，应通过 `backup_migratable` 单独表达。
- `hardware_backed = true` 只允许在 Secure Enclave 路径有实际设备验证后启用。
- `user_presence_required = true` 只允许在本机验证签名确实需要用户确认或生物识别后启用。
- `backup_migratable = true` 只允许在 iCloud Keychain、Time Machine、设备迁移或 app container 迁移语义有实测记录后启用。
- iOS Keyboard Extension 与 containing app 的 Keychain access group 必须单独验证；未验证前不能假定键盘扩展可访问 manager 创建的 key。

## Key ID 与 Keychain Item

建议映射：

```text
device_id: RadishLex opaque device id
signing_key_id: RadishLex opaque signing key id
keychain_service: org.radishlex.sync.signing
keychain_account_or_tag: signing_key_id
keychain_label: RadishLex Device Signing Key
```

限制：

- `keychain_account_or_tag` 不包含系统用户名、设备名、host name、本机路径或用户输入内容。
- `keychain_label` 只使用固定产品字符串，不拼接用户词或设备可识别名称。
- 如果需要 Apple access group，必须用 bundle / team 配置固定，不写入用户数据。
- committed fixture 只能使用合成 `device_id` 和 `signing_key_id`。

## 创建与加载验证

必须验证：

- 创建 key 后返回 32-byte Ed25519 public key。
- `DeviceSigningKeyHandle` metadata 使用 `apple-keychain-v1`。
- handle Debug 不包含私钥、seed、Keychain query dictionary、account secret 或 access token。
- 重新启动进程后能通过 `device_id + signing_key_id` 加载同一 public key。
- 尝试加载不存在的 key 返回 `private_key_not_found` / `private_key_unavailable` 等明确错误。
- 不支持 Ed25519 的系统返回 `unsupported_signature_algorithm` 或 `unsupported_storage_backend`，不得创建 fallback test key。

## 签名验证

必须验证：

- 同一 handle 对同一 canonical bytes 生成 Ed25519 signature。
- `DeviceSignature::verify_at` 可用返回的 public key 验证签名。
- 篡改 canonical bytes、signing key id 或 signer device id 后验签失败。
- revoked key 后续签名失败。
- access denied、user presence required、Keychain locked 和 item corrupted 的错误可区分。
- 日志和错误不包含 canonical bytes 内容、signature bytes、private key material 或 Keychain query 细节。

签名输入限制：

- 只签 `radishlex-signature-v1` canonical bytes。
- 不签任意 HTTP request body。
- 不签 plaintext userdb payload。
- 不签 P1 原始选择事件或负反馈明细。

## 删除与撤销验证

撤销设备时：

1. 客户端先生成 signed device revocation。
2. 服务端接受撤销后拒绝该设备后续写入。
3. 本机 backend 执行 `delete_or_revoke(handle, revoked_at_ms)`。
4. 后续 `sign(handle, canonical_bytes)` 必须失败。

验证要求：

- `SecItemDelete` 成功后，加载 key 返回 not found。
- 如果平台删除失败，RadishLex 本地状态仍必须把 key 标记 revoked 并阻止后续签名。
- 删除 userdb 不等于删除设备私钥；管理 UI 后续必须把两者分开。

## 锁屏、权限与用户交互

macOS 验证矩阵：

- 正常登录会话：创建、加载、签名、删除。
- Keychain locked 或无法访问：返回 locked / access denied，不回退。
- App Sandbox / hardened runtime 权限缺失：返回 access denied，并记录非敏感错误分类。
- user presence policy 开启时：未满足交互返回 user presence required 或 access denied。

iOS / Keyboard Extension 后续还需验证：

- containing app 创建 key 后，Keyboard Extension 是否可访问同一 access group。
- full access 关闭时，键盘扩展是否应禁止同步签名。
- 锁屏、后台、系统输入法生命周期下签名是否可用。

## 备份与迁移

必须记录实测结论：

- macOS 设备迁移、Time Machine、iCloud Keychain 是否迁移该 key。
- iOS encrypted backup / iCloud Keychain / app reinstall 是否保留该 key。
- 同一 key 迁移到新设备时，设备撤销语义如何告知用户。

默认策略：

- 未验证前 `backup_migratable = false`。
- 新设备应创建新设备签名 key，通过已有设备授权或恢复码加入。
- 如果平台迁移导致旧设备 key 在新设备可用，管理 UI 必须提示用户撤销旧设备或重新授权。

## 日志与数据

允许日志字段：

- backend id
- error category
- device id
- signing key id
- operation name
- result code
- created / revoked timestamp

禁止日志字段：

- private key bytes、seed、SecKey raw data。
- Keychain item secret、完整 query dictionary、access token。
- canonical bytes 原文、signature bytes。
- 同步主密钥、object key、恢复码明文。
- 明文用户词、input code、reading、P1 event、ranker 明细。
- 系统用户名、设备真实名称、本机绝对路径。

## 自动化验证建议

默认 CI：

- 继续只跑 `test-memory-v1` 和 `unavailable`，不访问系统 Keychain。
- 测试 `apple-keychain-v1` capability metadata 构造和 Debug 脱敏。

macOS 本机手动 / gated smoke：

```text
cargo test -p radishlex-ime-crypto --features apple-keychain --test apple_keychain_smoke -- --ignored --nocapture
```

该 smoke 使用合成 `device_id` / `signing_key_id`，创建临时 Keychain item，完成签名验证后删除 item。失败时必须输出阻塞原因和可复验命令，不得把 skip 写成通过。运行前必须明确告知会触碰本机 macOS Keychain，并获得开发者批准。

## 停止线

- Keychain backend 未通过创建、加载、签名、删除和错误语义验证前，不用于真实远端对象上传。
- 如果需要导出私钥 bytes 才能完成签名，应停止并回退设计。
- 如果 backend unavailable 时回退到 `test-memory-v1`，必须停止并回退实现。
- 如果 Keychain label / account / 日志包含真实用户名、设备名称、本机路径或输入内容，必须停止并修正。
- 如果 iOS Keyboard Extension 无法可靠访问同一 key，不能把 iOS 同步签名接入用户可用路径。
