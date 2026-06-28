# ADR 0003: 设备签名与私钥存储边界

本文档用于固定 RadishLex 真实远端同步前的设备签名、签名对象、私钥存储抽象、错误语义和测试口径，读者是后续实现 `ime-crypto`、`ime-sync`、Go sync server、管理 UI 设备页面和平台密钥存储接线的开发者与审阅者。本文不包含生产签名代码、系统 Keychain / Keystore / DPAPI / Secret Service 接入、HTTP API、Flutter 页面设计或真实平台授权交互；平台私钥存储 backend 决策见 `docs/adr/0004-platform-private-key-storage-backend.md`。

## 状态

Accepted

## 背景

当前 `ime-crypto` 已落地本地 AEAD envelope、HKDF-SHA256 object key 派生、device wrapping、recovery material 和恢复码 KDF；`ime-sync` 已落地设备生命周期、加入请求、授权包、撤销记录、对象版本冲突和客户端合并模型。进入真实远端同步前，还需要回答两个问题：

- 服务端只保存密文和元数据时，客户端如何确认某个对象版本、设备授权包、撤销记录或恢复记录确实来自授权设备。
- 设备私钥如何表达存储边界，避免后续平台壳、Flutter manager 或 FFI 直接接触可导出的私钥字节。

AEAD 可以证明持有对象 key 的客户端能解密 payload，并能发现密文 / AAD 篡改；它不能单独表达“哪个设备创建了这条记录”“该设备当时是否 active”“服务端是否替换了签名来源”。签名层用于绑定设备身份、对象元数据、恢复记录和设备生命周期操作。

## 参考依据

- RFC 8032 定义 Ed25519 / Ed448 数字签名算法；RadishLex v1 选择 Ed25519 作为设备签名算法，保留后续增加算法 profile 的空间。

## 决策

RadishLex v1 设备签名使用 Ed25519，算法标识为：

```text
signature_algorithm: ed25519-v1
signature_schema_version: 1
public_key_len: 32
signature_len: 64
```

规则：

- 使用 RFC 8032 的 Pure Ed25519，不默认使用 prehash 变体。
- 签名 key 只用于签名，不用于 key agreement、密钥包装或对象加密。
- 后续如需使用 X25519、P-256 或平台硬件密钥，必须新增独立算法 profile，不得复用 Ed25519 signing key 做加密用途。
- 签名验证必须先解析签名对象的 `signature_schema_version`、`signature_algorithm`、`signature_key_id`、`signer_device_id` 和 `domain_id`，再按 canonical bytes 验签。
- 未知算法、未知签名 schema、签名 key 不匹配、签名设备非 active、签名时间晚于撤销时间、key epoch 不允许、canonical bytes 不匹配或 signature 长度错误，都必须返回明确错误。

## 设备 key 分工

后续 Rust 模型应把设备 key usage 明确分开：

```text
DeviceSigningKey
  device_id
  signing_key_id
  signature_algorithm
  public_key
  created_at_ms
  revoked_at_ms

DeviceKeyAgreementKey
  device_id
  key_agreement_key_id
  key_agreement_algorithm
  public_key
  created_at_ms
  revoked_at_ms
```

当前代码中的 `DeviceKeyDescriptor` 仍是早期泛化描述；签名模型已新增独立 `DeviceSigningKeyHandle` / `DeviceSigningPublicKey`，后续 key agreement 模型也应使用显式 key usage，避免继续用 `DeviceKeyPair` 表达所有设备密钥职责。

服务端可保存设备签名公钥、key agreement 公钥、key id、算法、创建时间和撤销时间；不得保存私钥、可导出的私钥备份、同步主密钥或恢复码明文。

## 需要签名的对象

### SignedSyncObjectManifest

用于证明某个加密对象版本由指定 active 设备产生。签名覆盖对象 envelope 元数据，不覆盖 plaintext。

```text
SignedSyncObjectManifest
  signature_schema_version
  signature_algorithm
  signature_key_id
  signer_device_id
  domain_id
  object_id
  object_type
  version
  base_version
  key_id
  key_epoch
  envelope_algorithm
  nonce
  encrypted_payload_len
  ciphertext_hash
  created_at_ms
  updated_at_ms
  signature
```

验证规则：

- `signer_device_id` 必须是当前同步域中 active 设备。
- `signature_key_id` 必须属于 `signer_device_id`。
- `key_epoch` 不得低于服务端 / 客户端已知的撤销边界。
- `ciphertext_hash` 必须仍是 ciphertext 或 ciphertext + AAD hash，不得改为 plaintext hash。
- 签名有效不代表 payload 可被服务端读取；payload 解密仍只发生在客户端。

### SignedDeviceAuthorization

用于证明已有 active 设备授权新设备接收 key epoch 材料。

```text
SignedDeviceAuthorization
  signature_schema_version
  signature_algorithm
  signature_key_id
  authorizer_device_id
  recipient_device_id
  recipient_public_key_id
  join_challenge
  join_short_code
  key_epoch
  wrapping_key_id
  encrypted_key_len
  created_at_ms
  signature
```

验证规则：

- `authorizer_device_id` 必须是 active 设备。
- `recipient_device_id` 必须处于 pending 或刚完成授权的 active 状态。
- `join_challenge` 必须来自待授权设备的加入请求，不能由服务端替换。
- `wrapping_key_id` 与 `encrypted_key_len` 必须和实际包装记录一致。

### SignedDeviceRevocation

用于证明设备撤销和 key epoch 推进来自仍可信的 active 设备。

```text
SignedDeviceRevocation
  signature_schema_version
  signature_algorithm
  signature_key_id
  revoked_by_device_id
  revoked_device_id
  previous_key_epoch
  new_key_epoch
  reason
  revoked_at_ms
  signature
```

验证规则：

- `revoked_by_device_id` 必须在签名时仍是 active 设备。
- `new_key_epoch` 必须大于 `previous_key_epoch`。
- 被撤销设备不得再签发新 `key_epoch` 对象、授权包或恢复记录。

### SignedRecoveryRecord

用于证明恢复记录元数据和恢复包装密文由授权设备创建或轮换。

```text
SignedRecoveryRecord
  signature_schema_version
  signature_algorithm
  signature_key_id
  signer_device_id
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
  encrypted_recovery_key_len
  created_at_ms
  updated_at_ms
  signature
```

验证规则：

- 签名只证明恢复记录来源和元数据完整性，不证明服务端知道恢复码。
- 恢复记录解密仍必须通过 `RecoveryCode` + Argon2id + AEAD。
- 恢复记录不得绕过设备状态、key epoch 或撤销规则。

## Canonical bytes

所有签名对象必须使用稳定 canonical bytes。进入正式协议前，Rust model 可沿用现有 AAD 风格的 length-prefixed field list，但必须满足：

- 字段顺序固定，由 record type 定义。
- 每个字段写入 field name、长度和原始 bytes。
- 空 `base_version` 使用空 bytes，不能省略字段。
- 所有整数使用十进制 ASCII 或统一大端二进制；同一 record type 内不得混用。
- 字符串必须是 UTF-8，且不得经过 JSON pretty print、map iteration 或平台 locale 影响。
- canonical bytes 起始必须包含 domain separator，例如 `radishlex-signature-v1` 和 record type。

禁止临时使用未排序 JSON、Debug 输出、平台对象序列化、SQLite row dump 或 HTTP request 原文作为签名输入。

## 私钥存储边界

后续应引入设备私钥存储抽象，而不是把私钥字节暴露给 FFI 或管理 UI：

```text
DevicePrivateKeyStore
  create_signing_key(device_id, algorithm) -> DeviceSigningPublicKey
  load_signing_key_handle(device_id, signing_key_id) -> DeviceSigningKeyHandle
  sign(handle, canonical_bytes) -> DeviceSignature
  public_key(handle) -> DeviceSigningPublicKey
  delete_or_revoke(handle)
```

`DeviceSigningKeyHandle` 至少应记录：

```text
device_id
signing_key_id
signature_algorithm
storage_backend
exportable
hardware_backed
created_at_ms
last_used_at_ms
```

规则：

- 生产 backend 不允许导出私钥 bytes。
- 测试 backend 可以使用合成可导出 key，但必须标记为 `test-memory-v1`，不得进入生产配置。
- `storage_backend` 初期只允许 `test-memory-v1` 或 `unavailable`；Apple Keychain、Android Keystore、Windows CNG、Linux Secret Service 等平台 backend 边界由 ADR 0004 固定，具体实现仍需平台验证。
- FFI 不导出私钥、签名 handle 内部指针、canonical bytes helper 或签名 API，直到平台线程、生命周期和错误语义稳定。
- CLI 不新增生产签名命令；测试命令若后续加入，必须只使用合成 fixture。

## 错误语义

后续 Rust model 至少区分这些错误：

- `unsupported_signature_algorithm`
- `invalid_signature_key`
- `signature_verification_failed`
- `signer_device_not_active`
- `signature_key_revoked`
- `canonical_bytes_mismatch`
- `private_key_unavailable`
- `private_key_locked`
- `private_key_access_denied`
- `private_key_export_blocked`
- `storage_backend_unavailable`

错误日志不得包含私钥 bytes、签名 key seed、系统 keychain item secret、同步主密钥、恢复码或 plaintext payload。签名、公钥和 key id 是公开元数据，但默认日志也应避免大量打印完整签名值。

## Rust 实施口径

当前 Rust 实施口径：

- `ime-crypto` 已新增签名基础类型、签名 key handle/public key、签名对象 canonical bytes 和纯 Rust `test-memory-v1` signer。
- `ime-crypto` 已覆盖 `SignedSyncObjectManifest` 与 `SignedRecoveryRecordManifest`；`ime-sync` 已覆盖 `SignedDeviceAuthorization` 与 `SignedDeviceRevocation`，并接入设备状态校验。
- 暂不接系统 Keychain / Keystore，不引入平台 SDK，不暴露 FFI。
- 当前签名依赖为 `ed25519-dalek = 2.2.0`，许可 `BSD-3-Clause`；当前 test-memory signer 使用合成 seed，不依赖系统 RNG 创建生产 key。
- Go server API 与平台存储 backend 边界已分别由 `docs/sync-server-api-storage.md` 和 ADR 0004 固定；生产 key 创建流程仍需后续 Rust model 和平台验证。

## 验证口径

进入远端同步前必须覆盖：

- 同一 record type 的 canonical bytes 稳定。
- 任一被签字段变化都会导致验签失败。
- 错误设备公钥验签失败。
- 非 active、revoked 或 lost 设备不能签发新对象、授权包、撤销记录或恢复记录。
- `new_key_epoch <= previous_key_epoch` 的撤销签名无效。
- `SignedSyncObjectManifest` 的 `ciphertext_hash`、`encrypted_payload_len`、`object_id`、`version` 和 `key_epoch` 与 envelope 不一致时失败。
- `SignedDeviceAuthorization` 的 join challenge、recipient device 和 wrapping metadata 不一致时失败。
- `SignedRecoveryRecord` 的 KDF 参数、salt、envelope nonce 或 encrypted recovery key 长度变化时失败。
- `DeviceSigningKeyHandle`、错误对象和 Debug 输出不打印私钥、seed、同步主密钥或恢复码。

## 后果

收益：

- 远端同步前先固定设备身份真实性边界。
- 服务端仍不能解密 payload，但客户端可以验证对象和设备操作来源。
- 私钥存储以抽象边界进入设计，避免平台壳和 FFI 直接接触私钥。

代价：

- 后续需要引入签名依赖和更多跨对象测试 fixture。
- 真实平台密钥存储需要分别验证系统权限、锁屏状态、备份 / 迁移、删除和用户交互。
- 设备撤销、恢复记录轮换和历史对象重签名会增加同步协议复杂度。

## 参考链接

- RFC 8032: <https://www.rfc-editor.org/rfc/rfc8032.html>
