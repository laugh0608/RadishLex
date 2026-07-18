# ADR 0006: 设备签名算法 Profile

本文档固定 RadishLex M3 设备签名算法的可演进边界，读者是实现 `ime-crypto`、`ime-sync`、Go sync server、Apple 私钥 backend 与后续设备管理入口的开发者和审阅者。本文不开放真实用户同步，不定义恢复码或设备授权 UI；算法 profile 决策本身不构成 Secure Enclave 或硬件保护证据，平台事实必须由 gated smoke、runbook 和逐字段资格评审取得。

## 状态

Accepted

## 背景

ADR 0003 固定了 `ed25519-v1`、`radishlex-signature-v1` canonical bytes 与设备私钥抽象。该 profile 已被 Rust、Go 与现有合成同步链验证，必须继续兼容。Apple Keychain 与 Android Keystore 的现有 spike 尚未证明平台能创建不可导出的 Ed25519 私钥；把 Ed25519 seed 存入 generic password item、普通文件、SQLite 或 settings 再取回 Rust 签名，会改变保护语义并突破现有 backend 停止线。

Apple Security framework 提供 P-256 ECDSA 的平台原生 `SecKey` 路径，因此 M3 需要在不替换历史 Ed25519 设备、不改变对象加密算法、也不降低 backend 失败语义的前提下增加第二个签名 profile。

## 决策

RadishLex 接受 P-256 作为新的生产候选设备签名 profile，稳定算法标识为：

```text
signature_algorithm: ecdsa-p256-sha256-v1
signature_schema_version: 1
curve: NIST P-256 / secp256r1 / prime256v1
hash: SHA-256
public_key_encoding: SEC1 uncompressed point
public_key_len: 65
signature_encoding: IEEE P1363 fixed-width r || s
signature_len: 64
```

`ecdsa-p256-sha256-v1` 是“ECDSA P-256 + SHA-256 + 固定编码”的完整协议标识，不允许用同一 id 表达 DER 签名、压缩点、SHA-384、prehash 输入或其他曲线。现有 `ed25519-v1` 继续保持 ADR 0003 已固定的 32-byte RFC 8032 public key 与 64-byte PureEd25519 signature。

P-256 目前只是生产候选 profile。2026-07-18 已取得普通 DPK 软件 key 的 manager 产品进程创建、重载、签名、Rust/Go 验签和删除证据；评审确认该 key 可由平台 API 导出，因此 `apple-keychain-p256-v1` 不具备生产资格。协议 profile 可以由后续 Secure Enclave backend 复用，但基础 smoke 不证明 Secure Enclave、hardware-backed、user presence 或 backup migration。

## Public Key 编码

`ecdsa-p256-sha256-v1` public key 使用 SEC1/X9.63 未压缩点：

```text
0x04 || X[32] || Y[32]
```

规则：

- 长度必须恰好为 65，首字节必须为 `0x04`。
- verifier 必须确认坐标构成 P-256 曲线上的有效非无穷远点。
- 不接受 33-byte compressed point、DER SubjectPublicKeyInfo、PEM、JWK 或 Apple 内部对象序列化。
- 服务端设备记录必须显式保存 `signing_algorithm` 并与 public key 一起绑定；不得只靠长度推断算法。
- 已存在且没有算法列的服务端历史设备只允许由一次显式 migration 标记为 `ed25519-v1`。新建 domain、join request 与设备记录必须显式传入算法，不做请求级默认或降级。

## Signature 编码

`ecdsa-p256-sha256-v1` signature 使用固定 64-byte IEEE P1363 编码：

```text
r[32] || s[32]
```

规则：

- `r`、`s` 均为 32-byte unsigned big-endian 标量。
- verifier 必须拒绝长度错误、零值、超出曲线阶或其他非法标量编码。
- Apple `SecKeyCreateSignature` 返回的 X9.62 DER 只允许在 Apple backend 内部严格解析并立即转换为 P1363；DER bytes 不进入 Rust 公共签名模型、Go API、fixture 或持久化 metadata。
- P-256 验签对 canonical bytes 执行 SHA-256；调用方传入的是完整 `radishlex-signature-v1` canonical bytes，不是调用方自行计算的 digest。
- v1 不要求 deterministic ECDSA，也不把 signature bytes 当作对象身份；同一 key 与同一消息产生不同但有效的签名仍可接受。

## Canonical Bytes

两个算法 profile 复用现有 `radishlex-signature-v1` canonical builder：

- domain separator、record type、字段顺序、field name、8-byte big-endian 长度与结尾 `0x00` 不变。
- `signature_schema_version` 保持 `1`。
- `signature_algorithm` 是被签字段，必须写入实际 profile id；因此把一个有效签名的算法标签改成另一 profile 会改变 canonical bytes 并验签失败。
- `sync_object_manifest`、`device_authorization`、`device_revocation` 和 `recovery_record` 的既有字段顺序不变，历史 `ed25519-v1` canonical bytes 与签名继续逐字兼容。
- 禁止以 JSON、HTTP body、Debug、SQLite row 或平台对象序列化替代 canonical builder。

## Rust 与 Go 验签边界

Rust 和 Go 都必须先完成 metadata 与编码校验，再进入密码学 verifier：

1. `signature_schema_version` 必须受支持。
2. 签名算法必须在 allowlist 中，并与登记设备的 `signing_algorithm` 完全一致。
3. key id、signer device id、授权时间与撤销时间必须匹配。
4. public key 与 signature 必须通过对应 profile 的严格编码校验。
5. 按 record type 生成 canonical bytes，再分派 Ed25519 或 ECDSA P-256/SHA-256 verifier。

稳定负向语义：

| 场景 | Rust 分类 | Go 对外分类 |
| --- | --- | --- |
| 未知或未启用算法 | `unsupported_signature_algorithm` | `invalid_signature`，detail `unsupported_signature_algorithm` |
| 记录算法与设备算法不一致 | `signature_algorithm_mismatch` | `invalid_signature`，detail `signature_algorithm_mismatch` |
| public key 长度、点或编码非法 | `invalid_signing_public_key` | `invalid_signature`，detail `invalid_signing_public_key` |
| signature 长度、DER/P1363 标量非法 | `invalid_signature_encoding` | `invalid_signature`，detail `invalid_signature_encoding` |
| canonical bytes 被篡改或签名不匹配 | `signature_verification_failed` | `invalid_signature`，detail `signature_verification_failed` |
| key 在签名时已撤销或尚未生效 | `signature_key_not_active` | `invalid_signature`，detail `signature_key_not_active` |
| 设备状态不是 active | 由上层设备状态校验拒绝 | `forbidden_device` |

Go 继续保留既有顶层 `invalid_signature` / `forbidden_device` API 兼容性；detail 只使用固定 allowlist，不包含 public key、signature、canonical bytes 或平台错误正文。Rust 错误同样不得携带这些实际 bytes。

共享 test vector 必须由 Rust 与 Go 读取同一 committed fixture，至少覆盖两个 profile 的正确签名，以及错误算法、错误 public key、篡改 canonical bytes、非法编码、签名不匹配和 revoked key。fixture 只使用合成 key 与合成 record，不包含用户输入、token、恢复码或真实设备标识。

## 共存、协商与历史迁移

- `ed25519-v1` 与 `ecdsa-p256-sha256-v1` 同时进入 verifier allowlist；这是共存，不是全域切换。
- 每个 signing key 只绑定一个算法。key id、public key、algorithm 三者必须作为不可拆分的设备身份 metadata；禁止对同一 key id 原地更换算法或 public key。
- signer 使用自己的登记算法签署对象和设备操作；同一同步域可以同时存在 Ed25519 与 P-256 active 设备。
- 服务端不根据客户端偏好“选择较弱算法”，也不在失败时尝试另一 verifier。请求算法与设备登记算法不一致立即失败。
- 历史 Ed25519 设备、对象、授权、撤销与恢复记录无需重签；服务端 schema migration 只为历史设备和 join request 补显式 `ed25519-v1` metadata。
- 现有设备如需改用 P-256，必须创建新的 signing key id，并通过后续明确的 key rotation / 设备重新授权流程登记；不能在原记录上替换 public key。
- 新设备 capability negotiation 最终必须让授权设备确认 recipient algorithm 与 public key commitment。当前产品设备加入入口仍关闭；在相应 canonical schema 完成该绑定与跨设备测试前，不开放真实设备授权 UI。
- 撤销按设备与 key lifetime 执行，与算法无关。被撤销的 Ed25519 或 P-256 key 都不能签署新对象，也不能通过更换请求算法绕过状态检查。

## Apple Backend

新的独立 backend id 为：

```text
apple-keychain-p256-v1
```

它表示 Apple Security framework 中普通 data protection keychain 软件 P-256 `SecKey` 路径，与阻塞中的 Ed25519 `apple-keychain-v1` 及后续 Secure Enclave backend 分离。RadishLex 不提供私钥导出接口，但平台本身允许导出该软件 key，必须标记 `exportable=true`。规则：

- private key 由 `SecKeyCreateRandomKey` 创建为永久 P-256 key，RadishLex 不调用 private external representation，不提供 seed、generic password item 或软件文件 fallback。
- macOS 创建、查询和删除必须一致使用 data protection keychain；`kSecAttrAccessibleWhenUnlockedThisDeviceOnly` 不得在默认 file-based `SecItem` shim 上被当作已生效。data protection keychain 因 host identity、entitlement 或访问组失败时必须返回 denied/unavailable，不得跨域读取 legacy item。
- public key 通过 `SecKeyCopyPublicKey` 导出为 65-byte SEC1 uncompressed point；签名使用 `kSecKeyAlgorithmECDSASignatureMessageX962SHA256`，DER 只在 backend 内转为 P1363。
- backend status 必须声明算法、backend id、`exportable=true`，并分别报告 `hardware_backed=false`、`user_presence_required=false`、`backup_migratable=false`。这些字段描述普通软件 DPK 事实，不能被产品层改写为更强保护。
- repository-only 实现和默认 feature 测试不得访问 Keychain；只有显式 feature + ignored gated smoke + 环境门禁才可创建合成 key。
- locked、denied、missing、corrupted、unsupported 必须映射为结构化错误；不得创建新 key 顶替丢失身份，也不得回退 `test-memory-v1`。
- key tag 与 label 只含固定 service 和 opaque synthetic/production key id，不包含用户名、设备名称、本机路径、canonical bytes 或用户输入。
- 本地 delete/revoke 成功后必须不能继续加载或签名；服务端 revoked 状态仍是跨设备真相，不能只依赖进程内撤销集合。

状态分四层表达：target 编译、当前运行时能力、产品资格与真实用户同步 gate。2026-07-18 的 provisioning-backed manager 产品进程 DPK 生命周期支持当前 macOS feature build 报告：

```text
compiled = true
available = true
can_create_signing_keys = true
can_sign = true
product_qualified = false
user_sync_enabled = false
exportable = true
hardware_backed = false
user_presence_required = false
backup_migratable = false
```

未启用 feature 或非 macOS target 不能报告上述运行时 true。`product_qualified` 因 `exportable=true` 保持 false；locked/denied 证据不能覆盖这一生产条件。用户同步继续受独立 M3 全链 gate。产品 validation ABI 只返回固定状态、生命周期、错误分类和数值 OSStatus，private key、public key、canonical bytes 与 signature bytes 不进入 Dart。

ADR 0007 新增 `apple-secure-enclave-p256-v1`，复用本 ADR 的 P-256 protocol profile，但使用新的 backend id、signing key id、application tag 和产品证据。它不得把普通 DPK key 原地升级或在 Secure Enclave unavailable 时 fallback。qualification lifecycle 已支持 runtime、hardware-backed 与不可导出结论，ad-hoc denied 和真实设备锁屏 locked 也已验证；当前仅剩真实无 Secure Enclave 环境的 unsupported，随后才可逐字段评审 `product_qualified`。

## 日志与脱敏

允许记录固定 operation、backend id、algorithm id、结果分类、opaque device/key id 的受控形式和非敏感时间。禁止记录：

- private key、seed、public key bytes、signature bytes、canonical bytes 或 digest；
- Security framework query dictionary、完整 application tag、CFError 文本或 Keychain item 内容；
- token、恢复码、wrapped material、payload、用户词、input code、reading、上下文或本机绝对路径。

平台原始状态只映射到固定 allowlist 错误类别。未知平台错误失败关闭为 access denied 或 backend unavailable，不把原始错误正文上送 server、manager 或日志。

## 后果

收益：

- 保留已验证 Ed25519 历史链，同时为 Apple 原生非导出 P-256 能力提供明确生产候选。
- Rust/Go verifier、device metadata 与 backend capability 不再隐含“所有 key 都是 Ed25519”。
- DER、SEC1、P1363 和平台能力边界被固定，避免跨语言宽松解析与静默降级。

代价：

- Go storage/API 需要显式保存 signing algorithm，并迁移历史行。
- Rust 需要第二个密码学 verifier 与更细错误分类。
- Apple backend 需要单独 gated smoke；在真实证据完成前仍不能解除生产门禁或开放用户同步。

## 停止线

- 不把 P-256 实现存在写成 Apple production backend 已通过。
- 不把普通 Keychain key 宣称为 Secure Enclave 或 hardware-backed。
- 不把 DER signature、compressed public key 或算法猜测悄悄接受为 v1 profile。
- 不让未知算法、locked、denied、missing 或 verifier 失败回退到 Ed25519、test memory、普通文件或软件 seed。
- 不开放真实同步、恢复码、设备授权、撤销 UI 或 Flutter 同步真相源，直到 M3 对应全链证据满足。

## 参考

- [ADR 0003: 设备签名与私钥存储边界](0003-device-signing-key-storage.md)
- [ADR 0004: 平台私钥存储 Backend 边界](0004-platform-private-key-storage-backend.md)
- [ADR 0005: Apple 平台签名策略](0005-apple-platform-signing-strategy.md)
- [平台私钥 Backend 策略](../platform-private-key-backend-strategy.md)
- [Apple Keychain Signing Backend Runbook](../runbooks/apple-keychain-signing-backend.md)
