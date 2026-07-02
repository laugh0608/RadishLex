# Android Keystore Signing Backend Runbook

本文档定义 `android-keystore-v1` 设备签名 backend 的平台验证边界。读者是后续实现 Android Kotlin bridge、NDK / JNI 调用层、`ime-crypto` backend 接线、管理 UI 设备页面和审阅同步隐私边界的开发者。本文不包含 Kotlin 源码、Gradle 配置、FFI 导出接口、键盘 UI、系统输入法安装流程或真实用户同步开放步骤；平台私钥抽象见 `docs/adr/0004-platform-private-key-storage-backend.md`，设备签名协议见 `docs/adr/0003-device-signing-key-storage.md`。

## 当前结论

- `android-keystore-v1` 是 Apple Keychain 阻塞后优先补验证边界的下一类平台 backend。
- Android Keystore 的目标优势是让 key material 不进入 app 进程，并在设备支持时绑定到 TEE / Secure Element；硬件支持与可用算法组合必须通过设备实测确认。
- RadishLex Phase 3 设备签名协议仍是 `ed25519-v1`；`android-keystore-v1` 进入实现前必须证明 Android Keystore provider 能创建、加载并使用非导出 Ed25519 signing key，或明确记录 `unsupported_signature_algorithm`。
- `ime-crypto` 已有 `android-keystore` feature、`AndroidKeystoreDeviceKeyStore`、capability metadata、生产签名门禁、ignored smoke 入口和 Rust 侧 bridge 包装层；默认 bridge 仍保持不可用，不访问 Android Keystore。
- `platforms/android-ime/keystore-bridge` 已补仓库内 Kotlin bridge source、独立 Android Gradle library harness、JVM/JNI-callable Kotlin facade、gated instrumented smoke 和 smoke 记录模板，固定 `AndroidKeyStore` / `Ed25519` 的创建、加载、公钥读取、签名、删除和错误码映射；当前仍未接 Rust native JNI，也未运行真实 Android Keystore smoke，不代表真实 Android Keystore 已可用。
- 当前 Rust 单元测试只使用合成 bridge 复验 `create -> load public key -> sign -> verify -> delete / revoke` 的模型语义、错误语义和 Debug 脱敏，不代表 Kotlin bridge 已接入 Rust 生产路径。
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
- 当前 feature-gated Rust store 只保留平台 bridge 插入点；Kotlin / Gradle harness 只作为后续 Rust native JNI 和真实设备 smoke 的接线准备，默认不创建 Android Keystore item，也不声明 `available = true`。

## 建议桥接边界

Android bridge 候选路径：

```text
KeyPairGenerator.getInstance("Ed25519", "AndroidKeyStore")
KeyGenParameterSpec.Builder(keystore_alias, PURPOSE_SIGN | PURPOSE_VERIFY)
KeyStore.getInstance("AndroidKeyStore")
Signature.getInstance("Ed25519")
```

验证规则：

- 如果 `Ed25519` + `AndroidKeyStore` 不可用，返回 `unsupported_signature_algorithm` 或 `unsupported_storage_backend`，不得创建 fallback key。
- Android 官方 `KeyProperties` API 当前不提供 `KEY_ALGORITHM_ED25519` 常量；如果只能通过 provider 字符串、普通 JCA provider 或设备私有行为创建 Ed25519 key，必须在 smoke 记录中写清 API level、provider、build tools 和设备结果。
- 如果只能通过普通 JCA provider 创建可导出 Ed25519 key，不属于 `android-keystore-v1`。
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

Rust bridge wrapper 当前状态：

- `AndroidKeystoreDeviceKeyStore` 内部通过私有 `AndroidKeystoreBridge` trait 调用平台能力，默认实现为 unavailable bridge。
- `ime-crypto` 公开 `ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION = 1`、`ANDROID_KEYSTORE_PROVIDER = AndroidKeyStore`、`ANDROID_KEYSTORE_SIGNATURE_ALGORITHM = Ed25519`、bridge operation、bridge error code、alias 构造和 public key / signature 校验 helper，作为 Kotlin / JNI 层接线时必须遵守的 Rust contract。
- bridge 方法同时接收 opaque `signing_key_id` 和内部 `keystore_alias`；错误对象、Debug 和日志只能使用 opaque id 或长度信息，不打印完整 alias、canonical bytes、signature bytes 或 key material。
- Kotlin / JNI 层返回的 public key 必须是 32-byte Ed25519 public key，signature 必须是 64-byte Ed25519 signature；长度或格式不符时映射为 `private_key_corrupted`，不得继续进入 Go server 验签路径。
- Kotlin / JNI 层只允许返回固定 error code：`storage_backend_unavailable`、`unsupported_signature_algorithm`、`unsupported_storage_backend`、`private_key_unavailable`、`private_key_locked`、`private_key_access_denied`、`private_key_user_presence_required`、`private_key_corrupted`。错误消息和日志不应包含 alias、canonical bytes、signature bytes、KeyInfo dump 或 provider exception 原文中的敏感字段。
- 合成 bridge 只存在于 `ime-crypto` 单元测试，用来验证 Rust 模型语义；生产代码不得把合成 bridge、`test-memory-v1` 或软件 seed 作为 `android-keystore-v1` fallback。
- Kotlin / JNI 接线完成前，`backend_status()` 必须继续返回 `available = false`、`can_create_signing_keys = false`、`can_sign = false`。
- 当前 Kotlin source 位于 `platforms/android-ime/keystore-bridge/src/main/kotlin/org/radishlex/android/keystore/RadishLexAndroidKeystoreBridge.kt`，只保留平台调用实现和脱敏 DTO。
- `RadishLexAndroidKeystoreJniBridge` 提供 `@JvmStatic` facade，固定后续 Rust native JNI 可调用的参数形状；它仍未接入 `ime-crypto` 的生产 backend。
- `RadishLexAndroidKeystoreBridgeInstrumentedTest` 提供 gated smoke，只有传入 `-Pradishlex.runAndroidKeystoreSmoke=true` 时才会创建临时 Android Keystore item，并在 `finally` 中删除该 item。
- Kotlin source 从 Android `PublicKey.encoded` 的 X.509 SubjectPublicKeyInfo 中提取 32-byte raw Ed25519 public key，再交给 Rust contract；格式不匹配时返回 `private_key_corrupted`，不得上传到 Go server 验签路径。

Kotlin / JNI bridge contract：

| Rust operation | Kotlin / JNI responsibility | Return value |
| --- | --- | --- |
| `CreateSigningKey` | 使用 `AndroidKeyStore` provider 和 `Ed25519` 算法创建不可导出 signing key，绑定传入的 opaque `signing_key_id` 与内部 `keystore_alias`。 | 32-byte Ed25519 public key |
| `LoadPublicKey` | 只从已存在 alias 读取 public key，不创建新 key，不返回 private material。 | 32-byte Ed25519 public key |
| `Sign` | 只签 Rust 传入的 `radishlex-signature-v1` canonical bytes，不解析 plaintext payload、SQLite row 或 HTTP request。 | 64-byte Ed25519 signature |
| `DeleteSigningKey` | 删除对应 alias，后续 `LoadPublicKey` 和 `Sign` 必须返回不可用类错误。 | empty success |

每个 request 必须携带 `ANDROID_KEYSTORE_BRIDGE_CONTRACT_VERSION = 1`。只有 `Sign` 可以携带 canonical bytes；`CreateSigningKey`、`LoadPublicKey` 和 `DeleteSigningKey` 的 canonical bytes 必须为空。Rust 侧会校验 public key / signature 长度，并把长度错误映射为 `private_key_corrupted`。

错误码映射固定如下：

| Bridge error code | Rust error |
| --- | --- |
| `storage_backend_unavailable` | `StorageBackendUnavailable { backend: "android-keystore-v1" }` |
| `unsupported_signature_algorithm` | `UnsupportedSignatureAlgorithm { algorithm: "ed25519-v1" }` |
| `unsupported_storage_backend` | `UnsupportedStorageBackend { backend: "android-keystore-v1" }` |
| `private_key_unavailable` | `PrivateKeyUnavailable { key_id: signing_key_id }` |
| `private_key_locked` | `PrivateKeyLocked { key_id: signing_key_id }` |
| `private_key_access_denied` | `PrivateKeyAccessDenied { key_id: signing_key_id }` |
| `private_key_user_presence_required` | `PrivateKeyUserPresenceRequired { key_id: signing_key_id }` |
| `private_key_corrupted` | `PrivateKeyCorrupted { key_id: signing_key_id }` |

未知 contract version、未知 operation、未知 error code、非 32-byte public key、非 64-byte signature 或 alias / id 映射不一致，都应按 bridge 接线错误处理，不得降级为软件签名或继续上传到 sync server。

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
- 验证 `android-keystore-v1` backend id、capability metadata、status 门禁、Debug 脱敏、Rust bridge wrapper、bridge contract request / error code / response 校验、合成 bridge 创建 / 签名 / 删除语义、`android-keystore` feature 编译和 ignored smoke 入口。
- 当前 Kotlin / Gradle harness 已入仓，但默认仓库检查仍不执行 Android Gradle、instrumented test、模拟器或真实设备 smoke；默认只覆盖文本卫生、文档预算和 Rust contract 测试。

Android Gradle 编译：

```text
cd platforms/android-ime/keystore-bridge
gradle assembleDebug
```

该命令需要本机 Android SDK、Gradle、AGP / Kotlin plugin 依赖和网络 / 本地缓存支持。它不应创建 Android Keystore item。

Android gated smoke：

```text
cd platforms/android-ime/keystore-bridge
gradle connectedAndroidTest -Pradishlex.runAndroidKeystoreSmoke=true
```

该 smoke 使用合成 `signing_key_id` 和固定产品前缀 alias，创建临时 Android Keystore key，完成 create / load / sign / verify / delete 后删除 key。运行前必须明确告知会触碰测试设备的 Android Keystore，并获得开发者批准。

Rust host smoke 仍保持：

```text
cargo test -p radishlex-ime-crypto --features android-keystore --test android_keystore_smoke -- --ignored --nocapture
```

当前 Rust ignored smoke 只验证平台桥接完成前不能声明生产可用，不触碰 Android Keystore。

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

- Ed25519 key 不能由 Android Keystore provider 创建、加载和签名时，不接可用 Kotlin / JNI bridge，不解除 `android-keystore-v1` 的生产签名门禁。
- 如果需要导出私钥 bytes 才能完成签名，应停止并回退设计。
- 如果 backend unavailable 时回退到 `test-memory-v1`、软件 seed 或普通 app storage，应停止并回退实现。
- 如果只能使用 P-256 等其他算法，应先补签名算法 ADR 和协议迁移计划。
- 如果 alias、日志或 fixture 包含真实账号、手机号、设备真实名称、本机路径或输入内容，应停止并修正。
- 如果 InputMethodService 热路径必须参与同步签名，应停止并重画 Android 同步边界。
