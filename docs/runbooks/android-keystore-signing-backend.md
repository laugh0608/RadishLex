# Android Keystore Signing Backend Runbook

本文档定义 `android-keystore-v1` 设备签名 backend 的平台验证边界。读者是后续实现 Android Kotlin bridge、NDK / JNI 调用层、`ime-crypto` backend 接线、管理 UI 设备页面和审阅同步隐私边界的开发者。本文不包含 Kotlin 源码、Gradle 配置、FFI 导出接口、键盘 UI、系统输入法安装流程或真实用户同步开放步骤；平台私钥抽象见 `docs/adr/0004-platform-private-key-storage-backend.md`，设备签名协议见 `docs/adr/0003-device-signing-key-storage.md`。

## 当前结论

- `android-keystore-v1` 是 Apple Keychain 阻塞后优先补验证边界的下一类平台 backend。
- Android Keystore 的目标优势是让 key material 不进入 app 进程，并在设备支持时绑定到 TEE / Secure Element；硬件支持与可用算法组合必须通过设备实测确认。
- RadishLex Phase 3 设备签名协议仍是 `ed25519-v1`；`android-keystore-v1` 进入实现前必须证明 Android Keystore provider 能创建、加载并使用非导出 Ed25519 signing key，或明确记录 `unsupported_signature_algorithm`。
- 当前不引入 P-256，不改变 Go / Rust Ed25519 verifier，不把 seed 作为普通 secret 存入 Keystore / SharedPreferences 后取回 Rust 签名。
- `android-keystore-v1` 未完成创建、加载、签名、删除、锁屏 / 权限、备份迁移和日志脱敏验证前，不得声明生产可用。
- Android IME 输入热路径不调用同步签名；同步签名只允许由管理 / sync client 层在明确后台同步或用户操作中触发。

## 官方参考入口

- Android Keystore system: <https://developer.android.com/privacy-and-security/keystore>
- `KeyProperties`: <https://developer.android.com/reference/android/security/keystore/KeyProperties>
- `KeyGenParameterSpec`: <https://developer.android.com/reference/android/security/keystore/KeyGenParameterSpec>
- `KeyInfo`: <https://developer.android.com/reference/android/security/keystore/KeyInfo>

以上链接只作为 API 行为入口。实际可用性必须由 RadishLex 的 gated smoke 和设备矩阵记录确认，不能只凭文档推断。

## Backend 标识与职责

稳定 backend id：

```text
android-keystore-v1
```

职责：

- Android bridge 使用 `AndroidKeyStore` provider 创建设备签名 key。
- Kotlin / JNI bridge 只返回 public key、signature bytes、opaque `signing_key_id` 和 capability metadata。
- Rust core 只接收 canonical bytes、public key、handle metadata 和 signature bytes。
- Go server 继续只验证设备公钥、签名和设备状态，不知道 Android keystore alias 或任何私钥材料。

当前不做：

- 不在 Rust core 中直接调用 Android Keystore。
- 不让 InputMethodService、键盘 UI 或候选生成路径持有私钥材料。
- 不把 Keystore alias 设计成用户可读设备名、账号、本机路径或输入内容。
- 不把 Ed25519 seed 存入普通 app storage、SharedPreferences、SQLite、文件或可导出的 backup。
- 不在 `android-keystore-v1` 内静默降级到 `test-memory-v1`、软件 seed 或其他签名算法。

## 建议桥接边界

Android bridge 首选路径：

```text
KeyPairGenerator.getInstance("Ed25519", "AndroidKeyStore")
KeyGenParameterSpec.Builder(signing_key_id, PURPOSE_SIGN | PURPOSE_VERIFY)
KeyStore.getInstance("AndroidKeyStore")
Signature.getInstance("Ed25519")
```

验证规则：

- 如果 `Ed25519` + `AndroidKeyStore` 不可用，返回 `unsupported_signature_algorithm` 或 `unsupported_storage_backend`，不得创建 fallback key。
- 如果只能通过普通 JCA provider 创建可导出 Ed25519 key，不属于 `android-keystore-v1`。
- 如果平台要求 `KeyProperties` 常量而当前 SDK 不暴露 Ed25519 常量，应记录 API level / build tools / 设备结果，不绕过验证。
- 如果需要切到 P-256 或其他算法，必须先补签名算法 ADR、迁移计划和 Go / Rust verifier 变更。

Rust 边界仍保持：

```text
create_signing_key(device_id, signing_key_id, created_at_ms) -> DeviceSigningPublicKey
handle(device_id, signing_key_id) -> DeviceSigningKeyHandle
public_key(handle) -> DeviceSigningPublicKey
sign(handle, canonical_bytes) -> DeviceSignature
delete_or_revoke(handle, revoked_at_ms)
backend_status() -> DevicePrivateKeyStoreStatus
```

## 能力声明

首轮 smoke 通过前，`android-keystore-v1` 不应声明可生产签名。

通过基础 smoke 但未证明硬件绑定时：

```text
storage_backend = android-keystore-v1
exportable = false
hardware_backed = false
user_presence_required = false
backup_migratable = false
```

只有在 `KeyInfo` / attestation / 设备实测证明 key material 绑定到 TEE / Secure Element 后，才允许：

```text
hardware_backed = true
```

只有在用户认证策略实测覆盖锁屏、生物识别 / PIN、超时窗口和后台限制后，才允许：

```text
user_presence_required = true
```

只有在 Android Auto Backup、device transfer、app reinstall、work profile / personal profile 行为实测后，才允许：

```text
backup_migratable = true
```

## Key ID 与 Keystore Alias

建议映射：

```text
device_id: RadishLex opaque device id
signing_key_id: RadishLex opaque signing key id
keystore_provider: AndroidKeyStore
keystore_alias: org.radishlex.sync.signing.<signing_key_id>
```

限制：

- `signing_key_id` 必须由 RadishLex 生成，不取系统设备名、账号、手机号或用户输入。
- alias 可以包含固定产品前缀和 opaque signing key id，不包含用户名、host name、联系人、词库内容、input code、reading 或本机路径。
- alias 不进入 Go server metadata；服务端只看到 public key、signing key id、backend id 和设备状态。
- committed fixture 只能使用合成 `device_id` 和 `signing_key_id`。

## 创建与加载验证

必须验证：

- 创建 key 后返回 32-byte Ed25519 public key。
- `DeviceSigningKeyHandle` metadata 使用 `android-keystore-v1`。
- 进程重启后能用同一 `signing_key_id` 读取相同 public key。
- 加载不存在 key 返回 `private_key_not_found` 或明确 backend 错误。
- 不支持 Ed25519 时返回 `unsupported_signature_algorithm`，并保持 backend status 不可生产签名。
- Debug / log 不包含 private key、seed、完整 alias query、canonical bytes、signature bytes 或同步主密钥。

## 签名验证

必须验证：

- 同一 handle 对 `radishlex-signature-v1` canonical bytes 生成 Ed25519 signature。
- Rust `DeviceSignature::verify_at` 和 Go server verifier 都能验证该 signature。
- 篡改 canonical bytes、signing key id、signer device id 或 public key 后验签失败。
- revoked key 后续签名失败。
- key locked、user authentication required、access denied、unsupported algorithm、corrupted item 等错误可区分。

签名输入限制：

- 只签 RadishLex canonical bytes。
- 不签 HTTP request body。
- 不签 plaintext userdb payload。
- 不签 P1 原始选择事件、负反馈明细或应用上下文。
- 不在 InputMethodService 热路径中签名。

## 删除与撤销验证

撤销设备时：

1. 客户端先生成 signed device revocation。
2. 服务端接受撤销后拒绝该设备后续写入。
3. Android backend 删除或本地撤销该 key。
4. 后续 `sign(handle, canonical_bytes)` 必须失败。

验证要求：

- `KeyStore.deleteEntry(alias)` 成功后加载 key 返回 not found。
- 如果平台删除失败，RadishLex 本地状态仍必须标记 revoked 并阻止后续签名。
- 清空 userdb、卸载输入方案或关闭键盘不等于撤销设备私钥。
- 管理 UI 后续必须把“停止同步”“撤销当前设备”“清空学习数据”分开。

## 锁屏、权限与生命周期

Android 验证矩阵：

- 正常解锁会话：创建、加载、签名、删除。
- 锁屏后：是否允许签名，或返回 user authentication required / access denied。
- 开启用户认证策略：PIN / 生物识别通过前后签名行为。
- 后台同步：后台限制、电量优化、work profile 和 direct boot 状态。
- App reinstall / data clear：key 是否仍可加载，以及本地 metadata 如何处理。
- InputMethodService 生命周期：键盘服务不触发签名，full sync 仍由管理 / sync client 层执行。

## 备份与迁移

必须记录实测结论：

- Android Auto Backup 是否迁移 Keystore key。
- 设备换机 / OEM transfer 是否迁移 key。
- work profile 到 personal profile 是否可见。
- app uninstall / reinstall 后 key 是否保留。
- 迁移后旧设备撤销语义如何告知用户。

默认策略：

- 未验证前 `backup_migratable = false`。
- 新设备应创建新设备签名 key，通过已有设备授权或恢复码加入。
- 如果平台迁移导致旧 key 在新设备可用，管理 UI 必须提示撤销旧设备或重新授权。

## 日志与数据

允许日志字段：

- backend id
- operation name
- error category
- device id
- signing key id
- result code
- created / revoked timestamp

禁止日志字段：

- private key bytes、seed、KeyStore raw key material。
- 完整 alias query、KeyInfo dump、attestation certificate raw bytes。
- canonical bytes 原文、signature bytes。
- 同步主密钥、object key、恢复码明文。
- 明文用户词、input code、reading、P1 event、ranker 明细。
- Android account、联系人、手机号、设备真实名称或本机文件路径。

## 自动化验证建议

默认 CI：

- 不访问 Android Keystore。
- 只验证 `android-keystore-v1` backend id、capability metadata、status 门禁和 Debug 脱敏。

Android gated smoke：

```text
cargo test -p radishlex-ime-crypto --features android-keystore --test android_keystore_smoke -- --ignored --nocapture
```

或等价 Gradle instrumentation / host-driven smoke。该 smoke 必须使用合成 `device_id` / `signing_key_id`，创建临时 key，完成签名验证后删除 key。运行前必须明确告知会触碰测试设备的 Android Keystore，并获得开发者批准。

设备矩阵至少记录：

- Android version / API level。
- device model。
- security patch level。
- provider behavior。
- Ed25519 create / load / sign / delete 结果。
- `KeyInfo` hardware / security level 结果。
- locked / user authentication 结果。
- backup / migration 结果。

## 停止线

- Ed25519 key 不能由 Android Keystore provider 创建、加载和签名时，不接 `android-keystore-v1` 代码。
- 如果需要导出私钥 bytes 才能完成签名，应停止并回退设计。
- 如果 backend unavailable 时回退到 `test-memory-v1`、软件 seed 或普通 app storage，应停止并回退实现。
- 如果只能使用 P-256 等其他算法，应先补签名算法 ADR 和协议迁移计划。
- 如果 alias、日志或 fixture 包含真实账号、手机号、设备真实名称、本机路径或输入内容，应停止并修正。
- 如果 InputMethodService 热路径必须参与同步签名，应停止并重画 Android 同步边界。
