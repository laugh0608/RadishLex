# ADR 0007: Apple Secure Enclave P-256 Backend

本文档固定 RadishLex 在 Apple 平台使用 Secure Enclave 保存设备签名私钥的稳定 backend、访问控制、能力声明和迁移边界。读者是实现 `ime-crypto`、`ime-ffi`、manager 产品验证入口和审阅 M3 生产签名门禁的开发者。本文不开放真实同步，不定义恢复码或设备管理 UI，也不把仓库实现当作产品环境证据；实机步骤与证据矩阵见 Secure Enclave runbook。

## 状态

Accepted

## 背景

ADR 0006 已固定 `ecdsa-p256-sha256-v1`、65-byte SEC1 uncompressed public key、64-byte P1363 signature 和既有 `radishlex-signature-v1` canonical bytes。`apple-keychain-p256-v1` 的 provisioning-backed manager 产品生命周期已经通过，但普通 data protection keychain 软件私钥可由平台 API 导出，因此只能作为运行时能力，不能取得生产资格。

Apple Security framework 提供独立的 Secure Enclave token。该 token 只允许在设备内生成 256-bit EC private key，private key operation 需要 `kSecAccessControlPrivateKeyUsage`，硬件绑定 private key 不支持 external representation。这一风险模型不能隐藏在 `apple-keychain-p256-v1` 内，也不能把既有软件 key 原地升级成硬件 key。

## 决策

新增稳定 backend id：

```text
apple-secure-enclave-p256-v1
```

它只绑定：

```text
signature_algorithm = ecdsa-p256-sha256-v1
key_type = kSecAttrKeyTypeECSECPrimeRandom
key_size = 256
token = kSecAttrTokenIDSecureEnclave
accessibility = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
access_control = kSecAccessControlPrivateKeyUsage
```

规则：

- private key 只能由 `SecKeyCreateRandomKey` 在 Secure Enclave 内生成；不支持 import、seed、generic password、普通文件、SQLite、settings 或 `test-memory-v1` fallback。
- 创建、查询和删除使用独立 application tag/service，并在 macOS 明确选择 data protection keychain；不得查询普通 DPK 或 legacy file-based keychain item。
- public key 仍通过 `SecKeyCopyPublicKey` 与 public external representation 取得；private external representation 只允许在 gated qualification smoke 中作为“必须失败”的布尔验证，不返回或记录任何 bytes。
- signature 继续使用 `kSecKeyAlgorithmECDSASignatureMessageX962SHA256`；DER 只在 native backend 内严格转换为 64-byte P1363。
- 本版本不增加 `userPresence`、`biometryAny` 或 `devicePasscode` access-control flag。正常解锁会话签名不应弹出逐次认证 UI，`user_presence_required=false`。如未来产品要求每次或定期确认，必须新增 backend/version 并重新取得产品证据。
- unsupported hardware、token unavailable、locked、denied、missing、corrupted 和 user-presence 错误必须失败关闭；不得创建普通 DPK key顶替同一设备身份。

## 能力与资格分层

仓库实现完成后、产品证据取得前，状态必须为：

```text
storage_backend = apple-secure-enclave-p256-v1
signature_algorithm = ecdsa-p256-sha256-v1
compiled = true                 # 仅 macOS + apple-keychain feature
available = false
can_create_signing_keys = false
can_sign = false
product_qualified = false
user_sync_enabled = false
exportable = false
hardware_backed = false
user_presence_required = false
backup_migratable = false
```

`exportable=false` 是该 backend 的平台 API 设计约束；产品 qualification smoke 仍必须实际确认 private external representation 失败。`hardware_backed` 在真实产品进程创建、重载、签名和 token 评审完成前保持 false，不能仅根据代码包含 `kSecAttrTokenIDSecureEnclave` 改为 true。

2026-07-18 同一 qualification bundle 的产品 lifecycle 已完成创建、重载、签名、Rust/Go 验签、private external representation 失败、删除、missing 和 cleanup。评审据此开放 `available/can_create_signing_keys/can_sign/hardware_backed=true`；`product_qualified/user_sync_enabled/user_presence_required/backup_migratable=false` 继续保持，直至独立失败矩阵和上层总门禁完成。

产品资格只能在同一冻结产品 bundle 中同时取得以下证据后评审：

- 签名 identity、application identifier、Keychain access group、entitlement 与 provisioning profile 一致。
- 创建、跨 store/进程重载同一 public key、签名、Rust/Go 验签、删除和 fresh missing 全部通过。
- private external representation 明确失败，且验证路径不返回、打印或持久化 private bytes。
- unsupported、denied、locked 与 cleanup 语义能够区分，失败时不回退其他 backend。
- `backend_status()` 与实测字段一致，日志只有固定分类、数值 OSStatus 和布尔摘要。

即使上述条件支持 `runtime_available/product_qualified/hardware_backed=true`，`user_sync_enabled` 仍由 M3 上层 orchestration、设备生命周期和部署总门禁控制，不能自动开放。

## Key Identity 与迁移

- Secure Enclave key 使用新的 `signing_key_id`、backend id 和 application tag；不得保留原 `apple-keychain-p256-v1` key id 后替换 public key。
- 普通 DPK 软件设备若未来迁移，必须通过设备 key rotation 或重新授权登记新 public key；服务端历史对象和旧签名继续按原 key/algorithm 验证。
- Secure Enclave key 只在创建它的设备上可用，默认 `backup_migratable=false`。备份、系统迁移或硬件更换后 missing 必须进入重新授权/恢复流程，不能静默生成同一设备身份。
- 删除 Secure Enclave item 不等于服务端设备撤销；跨设备真相仍是 signed revocation 与服务端 active/revoked 状态。

## FFI 与产品进程边界

- `ime-ffi` 可以导出独立的 Secure Enclave status 与 gated validation ABI；它只返回固定 capability/lifecycle flags、错误分类和数值 OSStatus。
- private key、public key bytes、canonical bytes、signature bytes、access-control object、query dictionary 和 CFError 文本不得进入 Dart、Flutter method channel、settings、diagnostics 或 InputMethodKit。
- manager Release executable 是资格 smoke 的 host identity；bundle 内 dylib 不能获得超出宿主进程签名和 entitlement 的权限。
- 普通 manager 启动不得创建、加载或签名 Secure Enclave key。所有 smoke 必须同时经过显式脚本参数、产品命令行参数和 native 环境门。

## 验证与停止线

- 默认 `cargo test`、manager product check 和仓库门禁不得访问 Keychain 或 Secure Enclave。
- feature 测试必须覆盖 backend/algorithm 绑定、evidenced runtime 与关闭的产品 gate、Debug 脱敏、DER/P1363、错误映射和无 fallback。
- 实际 Secure Enclave/Keychain 访问、产品进程启动、系统锁定或权限变更必须分别获得授权。
- 在资格证据完成前，不得把 `available`、`can_create_signing_keys`、`can_sign`、`product_qualified` 或 `hardware_backed` 改为 true。
- 不把“当前 Mac 支持 Secure Enclave”扩写成所有 macOS、虚拟机、CI、Intel Mac 或 iOS extension 均支持。
- 不开放真实同步、恢复码、设备授权/撤销 UI 或 Flutter 同步真相源。

## 参考

- [ADR 0004: 平台私钥存储 Backend 边界](0004-platform-private-key-storage-backend.md)
- [ADR 0006: 设备签名算法 Profile](0006-device-signature-algorithm-profiles.md)
- [平台私钥 Backend 策略](../platform-private-key-backend-strategy.md)
- [Apple Secure Enclave P-256 Runbook](../runbooks/apple-secure-enclave-p256-backend.md)
