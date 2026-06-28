# ADR 0002: 恢复码 KDF 与同步域恢复边界

本文档用于固定 RadishLex 恢复码派生同步域恢复材料的算法、参数、数据边界和后续测试口径，读者是后续实现 `ime-crypto`、`ime-sync`、Go sync server 和管理 UI 同步恢复流程的开发者与审阅者。本文不包含生产恢复码代码、系统 Keychain / Keystore 接入、HTTP API、Flutter 页面设计或真实设备配对 UI。

## 状态

Accepted

## 背景

RadishLex 的服务端默认不可信，不能通过账号密码、对象 ID、hash 或同步元数据解密用户词库。新设备加入同步域有两条路径：

- 已有 active 设备授权新设备。
- 用户输入离线保存的恢复码，恢复同步域材料。

恢复码路径必须能在没有旧设备可用时恢复 `SyncMasterKey` 或等价同步域材料，但服务端不能看到恢复码明文、派生 key 或同步域明文密钥。恢复码也不能成为服务端登录密码或绕过设备状态的万能凭据。

当前已落地：

- `ime-crypto` 本地 envelope、AAD、nonce、ciphertext hash、device wrapping、recovery material 和撤销后 key epoch 解密边界。
- `ime-sync` 设备生命周期、撤销记录、对象版本冲突模型、客户端合并模型和 `SyncEnvelopeAssembler`。
- `ime-userdb` P2 payload 到本地 envelope / sync draft 的受控 Rust 内部装配。

进入远端同步和生产恢复码实现前，需要先固定恢复码 KDF 选择、参数版本、恢复记录字段、失败处理和验证口径。

## 参考依据

- RFC 9106 固定 Argon2id 为必须支持的 Argon2 变体，并给出通用和内存受限配置建议：`t=1, p=4, m=2 GiB` 与 `t=3, p=4, m=64 MiB`。
- OWASP Password Storage Cheat Sheet 推荐优先使用 Argon2id，并给出 19 MiB / 2 iterations / 1 parallelism 作为最低配置线。
- NIST SP 800-63B 要求口令 verifier 使用 salted hashing，记录算法和 cost factor，并对 look-up secret 的离线抗攻击存储和失败限速提出要求。

## 决策

RadishLex v1 恢复码 KDF 采用 Argon2id，恢复码由客户端生成，不允许用户自选。

恢复码格式：

```text
RLX1-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-C
```

规则：

- `RLX1` 是格式版本前缀。
- 主体使用 Crockford Base32 或等价大小写不敏感编码，避开易混字符。
- 恢复码主体至少承载 160 bit 客户端随机 secret。
- `C` 是短校验段，只用于发现抄写错误，不作为安全认证材料。
- UI 可以显示分组、二维码或打印版，但 committed fixture 不得包含真实恢复码。

默认 KDF profile：

```text
kdf_id: argon2id-v1
argon2_version: 0x13
memory_kib: 65536
iterations: 3
parallelism: 4
salt_len: 16
output_len: 32
```

说明：

- 默认参数采用 RFC 9106 的内存受限建议，优先适配桌面和移动恢复路径。
- 恢复流程不是输入热路径，可以接受比常规输入操作更高的延迟。
- 后续可以增加更高成本 profile，例如桌面优先的 256 MiB 或更高内存配置，但不得降低 `argon2id-v1` 的安全下限。
- 如果某个平台无法承受默认 profile，必须新增明确版本化 profile，并记录平台限制、风险和迁移策略；不得静默降级。

恢复 key 派生：

```text
recovery_secret = decode_recovery_code(code)
recovery_wrapping_key = Argon2id(
  password = recovery_secret,
  salt = recovery_salt,
  memory_kib = profile.memory_kib,
  iterations = profile.iterations,
  parallelism = profile.parallelism,
  output_len = 32
)
```

派生出的 `recovery_wrapping_key` 只用于解开恢复记录中的同步域材料包装密文，不能直接作为 `SyncMasterKey`、object key 或服务端认证 token 使用。

## 恢复记录字段

客户端创建恢复码时，服务端可保存恢复记录元数据和密文：

```text
RecoveryRecord
  recovery_id
  domain_id
  key_epoch
  kdf_id
  kdf_version
  salt
  memory_kib
  iterations
  parallelism
  output_len
  envelope_algorithm
  envelope_nonce
  encrypted_recovery_key
  created_at_ms
  updated_at_ms
```

服务端不得保存：

- 恢复码明文。
- `recovery_wrapping_key`。
- `SyncMasterKey` 明文。
- plaintext payload。
- 明文用户词、input code、reading、上下文或候选偏好。

`encrypted_recovery_key` 的 AAD 必须绑定：

```text
recovery_id
domain_id
key_epoch
kdf_id
kdf_version
salt
memory_kib
iterations
parallelism
output_len
envelope_algorithm
envelope_nonce
created_at_ms
updated_at_ms
```

AAD 不匹配、KDF profile 不支持、salt 长度不足、恢复码校验失败、AEAD 认证失败或解密后 key role 不匹配，都必须返回明确错误，不得输出部分同步域材料。

## 恢复流程

1. 第一台设备初始化同步域时生成恢复码。
2. 客户端生成 `recovery_salt`，按当前 KDF profile 派生 `recovery_wrapping_key`。
3. 客户端用 `recovery_wrapping_key` 加密同步域恢复材料，得到 `RecoveryRecord`。
4. 服务端只保存恢复记录元数据和包装密文。
5. 新设备输入恢复码后，从服务端拉取恢复记录。
6. 新设备本地派生 `recovery_wrapping_key`，解开同步域材料。
7. 新设备登记为 pending / active 设备，后续仍要遵守设备状态与 key epoch 规则。

成功使用恢复码后，管理 UI 应提示用户生成新的恢复码。后续可以把旧恢复记录标记为 superseded；是否强制一次性使用由管理 UI 和同步域策略决定，但旧恢复记录不得绕过已撤销设备状态或新 key epoch。

## 失败与限速

恢复码错误可能发生在本地格式校验、KDF 后 AEAD 解密或服务端拉取记录阶段。

规则：

- 客户端必须先做格式和校验段检查，避免明显拼写错误进入高成本 KDF。
- 客户端必须对连续错误增加本地等待时间。
- 服务端应对同一 `recovery_id`、账号或 IP 做失败限速，但不能依赖服务端限速抵御离线攻击。
- 离线攻击防护主要依赖恢复码随机熵、Argon2id 成本参数和恢复记录加密。
- 错误日志不得包含恢复码片段、派生 key、salt 以外的敏感材料或解密出的同步域材料。

## 参数迁移

KDF 参数必须版本化：

- `kdf_id` 标识算法族，例如 `argon2id-v1`。
- `kdf_version` 标识 RadishLex 参数 profile 版本。
- 客户端必须拒绝未知算法或低于当前允许下限的 profile。
- 参数升级时，新恢复记录使用新 profile；旧 profile 的读取保留兼容窗口，并通过管理 UI 提示用户轮换恢复码。
- 降级只允许用于显式兼容旧记录，不允许新建弱 profile。

## Rust 实现口径

当前 Rust 实现口径：

- 已在 `ime-crypto` 中新增 `RecoveryCode`、`RecoveryKdfProfile`、`RecoveryWrappingKeyMaterial` 和恢复码 KDF 参数校验。
- 已继续使用 `RecoveryMaterial` 表达恢复记录元数据、`envelope_nonce`、AAD 和包装密文。
- 已提供 Rust 内部 `RecoveryMaterial::encrypt_sync_master_key` / `decrypt_sync_master_key` 测试路径，用合成恢复码和合成同步主密钥复验恢复记录加解密。
- 不把 KDF 或恢复入口暴露给 FFI，直到所有权、错误语义和敏感数据清理策略固定。
- 不新增 CLI 明文恢复码导出命令；CLI 只能在明确测试模式下使用合成恢复码 fixture。
- 先保持纯 Rust model / test，再考虑 Go server API、管理 UI 恢复流程和系统密钥存储。

## 验证口径

进入生产恢复码实现前必须覆盖：

- 合成恢复码格式解析和校验段验证。
- Argon2id 参数 profile 校验，拒绝未知算法、过短 salt、0 memory、0 iterations、0 parallelism 和过短 output。
- 同一恢复码与同一 salt 派生稳定 key。
- salt 变化会改变派生 key。
- 错误恢复码无法解密 `encrypted_recovery_key`。
- AAD 任一字段变化会导致解密失败。
- `RecoveryMaterial`、错误对象和 Debug 输出不打印恢复码、派生 key 或恢复密文。
- 旧 KDF profile 可按兼容策略读取，但不能用于新建恢复记录。
- 恢复成功后新设备仍必须经过设备状态流程，不能绕过 active / revoked / lost 规则。

## 后果

收益：

- 恢复码安全边界在 Go server 和管理 UI 前固定。
- 服务端继续只保存恢复参数和密文，不获得解密能力。
- 参数版本化为后续算法升级和平台差异留下空间。

代价：

- 恢复码较长，用户必须离线保存。
- 恢复流程比普通登录慢，尤其在移动设备上。
- 当前已引入 RustCrypto `argon2` crate；其直接与当前锁定的主要传递依赖 `password-hash`、`blake2`、`base64ct` 均为 MIT / Apache-2.0 兼容许可口径。后续仍需要在移动端和低内存平台验证恢复耗时。

## 参考链接

- RFC 9106: <https://www.rfc-editor.org/rfc/rfc9106.html>
- OWASP Password Storage Cheat Sheet: <https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html>
- NIST SP 800-63B: <https://pages.nist.gov/800-63-4/sp800-63b.html>
