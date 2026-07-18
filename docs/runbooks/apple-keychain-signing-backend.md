# Apple Keychain Signing Backend Runbook

本文档定义 `apple-keychain-v1` 与 `apple-keychain-p256-v1` 设备签名 backend 的平台验证边界。读者是后续实现 macOS / iOS Keychain bridge、`ime-crypto` backend 接线、管理 UI 设备页面和审阅同步隐私边界的开发者。本文不包含 FFI 导出接口、App Sandbox entitlement 配置、输入法安装流程、Flutter 页面或真实用户同步开放步骤；平台私钥抽象见 ADR 0004，算法 profile 见 ADR 0006。

## 当前结论

- `apple-keychain-v1` 保留为历史 Ed25519 候选；真实创建已证明当前路径不支持 `ed25519-v1`，其 production gate 继续关闭。
- ADR 0006 已新增 `ecdsa-p256-sha256-v1` 和独立 `apple-keychain-p256-v1`，作为当前优先验证的 Apple 非导出候选。
- 默认 Rust workspace 继续只启用 `test-memory-v1` 和 `unavailable` backend，不访问 Keychain；macOS backend 只在显式 `apple-keychain` feature 下编译。
- `apple-keychain-v1` 已完成 feature-gated 接线和 ignored smoke 测试骨架；真实 Keychain smoke 已执行但未通过，不能视为平台验证通过。
- 2026-06-30 smoke 在沙盒和提权真实环境均阻塞于 Ed25519 Keychain key 创建阶段，错误为 `UnsupportedSignatureAlgorithm { algorithm: "ed25519-v1" }`；测试未进入签名成功或用户可用同步路径。
- `apple-keychain-v1` 的 `backend_status` 在平台策略未解决前必须阻断生产签名；普通 feature 测试只验证编译和状态门禁，不创建 Keychain item。
- `apple-keychain-p256-v1` 的 repository spike 已覆盖 SecKey 参数、public key encoding、DER -> P1363、Rust/Go 验签和结构化错误映射；命令行基础生命周期 gated smoke 已通过，产品进程访问、失败矩阵和 capability status 未闭环前继续保持关闭。
- 当前策略保留 `ed25519-v1` 设备签名协议，不把 Ed25519 seed 作为 generic password / data item 存入 Keychain 后取回 Rust 签名，也不把该软件保护方案伪装成 `apple-keychain-v1`。
- `apple-keychain-v1` 用于真实远端对象前必须通过本 runbook 的创建、加载、签名、删除 / 撤销、锁屏 / 权限、备份迁移和日志脱敏验证。
- 未验证 Secure Enclave 前，不承诺 `hardware_backed = true`。
- 未验证 user presence 前，不承诺 `user_presence_required = true`。
- 未验证 iCloud Keychain / 设备迁移语义前，不承诺 `backup_migratable = true`。
- 真实用户同步仍不得因为 macOS Keychain 可用就绕过外部 TLS、认证、备份恢复演练或客户端合并写回边界。

## Backend 标识与职责

稳定 backend id：

```text
apple-keychain-v1
apple-keychain-p256-v1
```

职责：

- `apple-keychain-v1` 只表达 Ed25519 原生 SecKey 路径；`apple-keychain-p256-v1` 只表达 P-256 原生 SecKey 路径。
- 平台 backend 创建永久 signing key 并保存到 Apple Keychain；RadishLex 不请求 private external representation。
- 平台 bridge 返回 `DeviceSigningPublicKey`、`DeviceSigningKeyHandle` metadata 和签名结果。
- Rust core 只接收 canonical bytes、public key、handle metadata 和 signature bytes。
- FFI / CLI / Flutter / 平台壳不得获得私钥 bytes、seed、Keychain item secret 或可导出 key backup。

当前不做：

- 不把 Apple Keychain 调用直接散落到同步、userdb、ranker 或平台壳。
- 不在任一 Apple backend 内静默降级为 Keychain 保存 seed、Rust 取出 seed 签名的软件保护路径，也不在 P-256 失败后尝试 Ed25519 或 `test-memory-v1`。
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

两个 Apple backend 当前都使用保守能力；P-256 基础生命周期证据不会自动改变以下声明：

```text
storage_backend = apple-keychain-p256-v1
signature_algorithm = ecdsa-p256-sha256-v1
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

P-256 keychain_service: org.radishlex.sync.signing.p256
P-256 keychain_label: RadishLex P-256 Device Signing Key
```

限制：

- `keychain_account_or_tag` 不包含系统用户名、设备名、host name、本机路径或用户输入内容。
- `keychain_label` 只使用固定产品字符串，不拼接用户词或设备可识别名称。
- 如果需要 Apple access group，必须用 bundle / team 配置固定，不写入用户数据。
- committed fixture 只能使用合成 `device_id` 和 `signing_key_id`。

## 创建与加载验证

必须验证：

- Ed25519 路径创建成功时返回 32-byte public key；不支持时返回明确 `unsupported_signature_algorithm`。
- P-256 路径创建成功时返回 65-byte `0x04 || X || Y` SEC1 uncompressed public key，不接受 compressed point、SPKI 或平台对象序列化。
- `DeviceSigningKeyHandle` metadata 使用与算法一一对应的 backend id 和 `signature_algorithm`。
- handle Debug 不包含私钥、seed、Keychain query dictionary、account secret 或 access token。
- 重新启动进程后能通过 `device_id + signing_key_id` 加载同一 public key。
- 尝试加载不存在的 key 返回 `private_key_not_found` / `private_key_unavailable` 等明确错误。
- 不支持 Ed25519 的系统返回 `unsupported_signature_algorithm` 或 `unsupported_storage_backend`，不得创建 fallback test key。

## 签名验证

必须验证：

- Ed25519 handle 只生成 64-byte Ed25519 signature；P-256 handle 使用 SHA-256/X9.62 签名，并在 backend 内把严格 DER 转成 64-byte P1363 `r || s`。
- `DeviceSignature::verify_at` 可用返回的 public key 验证签名。
- P-256 signature 还必须由 Go verifier 对同一 canonical bytes 验证；server/API 不接受 DER signature。
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
- 如果平台删除失败，backend 返回 locked / denied / unavailable 等明确错误，不能宣称本机 item 已删除；跨设备设备状态仍由已经提交的 signed revocation 失败关闭，不能依赖进程内集合替代服务端撤销真相。
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
- 测试两个 Apple backend 的 capability metadata、算法绑定和 Debug 脱敏。
- 测试 DER -> P1363、locked / denied / missing 固定错误映射，以及两个 backend status 在真实证据前阻断生产签名。

安全的 repository feature 门禁：

```text
cargo test -p radishlex-ime-crypto --features apple-keychain
```

该命令会显示 Keychain integration tests 为 ignored，不会创建系统 key。

macOS 本机手动 / gated smoke：

```text
RADISHLEX_RUN_APPLE_KEYCHAIN_P256_SMOKE=1 \
  cargo test -p radishlex-ime-crypto --features apple-keychain \
  --test apple_keychain_smoke \
  apple_keychain_p256_smoke_creates_reloads_cross_verifies_and_deletes_key \
  -- --ignored --exact --nocapture
```

该 smoke 有 ignored 与环境变量双门，使用合成 `device_id` / `signing_key_id`，创建临时 P-256 Keychain item，跨独立 store 重载 public key，完成 Rust/Go 验签后删除 item并复核 missing。测试带失败路径 cleanup guard，会在异常返回时尝试删除同一合成 item。失败时必须输出阻塞原因和可复验命令，不得把 skip 写成通过。运行前必须明确告知会触碰本机 macOS Keychain，并获得开发者批准。

正常 smoke 不证明 locked / denied。若要锁定 Keychain、改变 app 权限、sandbox/entitlement 或 user-presence policy，必须单独列出系统状态变化和恢复步骤并再次获得授权。

## 2026-07-18 P-256 实机证据

- 环境：arm64 macOS 26.5.2（25F84），仓库分支 `dev`，本轮基线提交 `c8457e9`。
- 经开发者单独授权执行上述唯一 P-256 gated smoke；结果为 `1 passed`，内嵌 Go verifier `TestExternalP256SignatureSmoke` 同步通过。
- 证据覆盖真实 Keychain key 创建、独立 store 重载同一 public key、非导出签名、Rust/Go 验签、删除后当前 store 撤销，以及 fresh store 返回 missing。测试使用合成标识，成功路径删除 item，失败路径 cleanup guard 保持启用。
- 本轮未锁定 Keychain、修改权限、sandbox/entitlement、user-presence policy 或系统输入法设置，也未访问真实同步、userdb 或用户输入数据。
- 该证据不证明 locked/denied 真实映射、产品 bundle 进程可访问、Secure Enclave、hardware-backed、user presence、backup migration 或 production-ready；这些能力继续独立门禁。

## 停止线

- `apple-keychain-p256-v1` 已通过基础生命周期 smoke，但 capability status、产品进程访问和受控失败矩阵完成前，不用于真实远端对象上传。
- 如果需要导出私钥 bytes 才能完成签名，应停止并回退设计。
- 如果 backend unavailable 时回退到 `test-memory-v1`，必须停止并回退实现。
- 如果 Keychain label / account / 日志包含真实用户名、设备名称、本机路径或输入内容，必须停止并修正。
- 如果 iOS Keyboard Extension 无法可靠访问同一 key，不能把 iOS 同步签名接入用户可用路径。
