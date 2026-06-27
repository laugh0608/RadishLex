# RadishLex 同步 Payload 草案

本文档定义当前 Rust 侧同步 payload 草案、数据分级映射和验证口径，读者是后续实现 `ime-sync`、`ime-crypto`、同步 CLI、Go server 和管理 UI 的开发者。本文不包含加密算法实现、设备授权流程完整协议、HTTP API、Go server 数据库 migration 或远端同步客户端实现；客户端加密边界见 `docs/crypto-boundary.md`。

## 当前定位

当前只落地 Rust 本地同步对象边界模型，不连接后端，不生成明文上传文件，也不实现网络同步。`ime-sync` 的作用是把 `sync preflight` 已验证的本地分类边界转成可测试的 Rust API，并让上传草案元数据从 `ime-crypto` envelope 派生，避免同步层和加密层字段语义漂移；`ime-userdb` 当前提供 Rust 内部 P2 plaintext payload 只读迭代器，并已在 integration test 中完成本地 envelope 加密、解密和 sync draft 派生验证：

- P2 数据可以进入后续端到端加密对象。
- P1 明细事件默认只能留在本地。
- 导入批次属于本地审计信息，不作为同步 payload。

`ime-ffi` 当前暴露 `radishlex_userdb_sync_preflight` 这类状态摘要入口：调用方必须显式传入 SQLite 路径，返回值只包含 P2 / P1 / 本地审计计数和 `plaintext_payload = false`，不返回同步 payload、P1 明细事件或数据库句柄。`ime-ffi` 的 userdb add / delete / list 入口只用于用户明确管理 P2 词条，不作为同步 payload 生成器。

FFI preflight summary 字段含义：

```text
schema_version
plaintext_payload
syncable_user_terms
syncable_ranker_weights
syncable_deleted_terms
local_selection_events
local_negative_feedback
local_import_batches
```

- `plaintext_payload` 当前必须为 `false` / `0`。
- `syncable_user_terms`、`syncable_ranker_weights`、`syncable_deleted_terms` 是后续可进入 P2 加密对象的本地计数。
- `local_selection_events` 和 `local_negative_feedback` 是 P1 明细事件计数，只能本地保留。
- `local_import_batches` 是本地审计计数，不进入同步 payload。
- 该 summary 只用于状态展示和进入真实同步前的边界检查，不是上传计划，不包含对象内容、hash、密文大小或版本号。

## 数据来源映射

| 本地来源 | 分级 | 后续同步对象 | 当前策略 |
| --- | --- | --- | --- |
| `user_terms` | P2 | `dictionary.user_terms` | 可进入加密对象 |
| `deleted_terms` | P2 | `dictionary.deleted_terms` | 必须同步删除意图 |
| `ranker_weights` | P2 | `ranker.weights` | 可进入加密对象 |
| `selection_events` | P1 | 无 | 默认本地保留 |
| `negative_feedback` | P1 | 无 | 默认本地保留 |
| `import_batches` | 本地审计 | 无 | 默认本地保留 |

P1 原始事件后续可以被压缩成 P2 权重摘要，但原始事件本身不进入同步 payload。

## 对象类型

当前 `ime-sync` 定义的后续同步对象类型：

```text
dictionary.user_terms
dictionary.deleted_terms
ranker.weights
settings.profile
settings.schema
backup.snapshot
```

本阶段只验证类型和边界，当前已定义 `dictionary.user_terms`、`ranker.weights` 与 `dictionary.deleted_terms` 的 plaintext payload 字段序列化，并已证明它们可以进入 `ime-crypto` envelope 后派生成 `EncryptedSyncObjectDraft`。真正写入服务端前必须补齐设备授权、key management 和冲突语义，服务端只能看到对象类型、设备 ID、key id、key epoch、algorithm、nonce、版本、密文大小、ciphertext hash 和时间戳。

## Plaintext Payload

`ime-userdb` 当前提供 `UserDb::p2_plaintext_payloads()`，返回 Rust 内部只读迭代器。该入口不是 CLI / FFI / 文件导出入口，不返回 P1 原始事件、本地审计批次、SQLite handle 或可上传明文文件。

通用字段顺序：

```text
payload_schema_version
object_type
```

`dictionary.user_terms` payload：

```text
terms[]
  input_code
  text
  reading
  source
  weight
  status
  created_at_ms
  updated_at_ms
  last_used_at_ms
```

规则：

- 只包含 `active` / `suppressed` 用户词条。
- 不包含 SQLite rowid、selection event id、session id、context kind、negative feedback reason 或 import batch source。
- `reading` 使用稳定字符串表达，未知时为空字符串。

`ranker.weights` payload：

```text
weights[]
  input_code
  text
  reading
  frequency
  recency_score
  negative_score
  context_kind
  updated_at_ms
```

规则：

- 只来自 `ranker_weights` 摘要表，该表由 P1 本地选择事件和负反馈明细压缩更新。
- `context_kind` 是稳定场景分类，用于摘要级合并和 explain，不包含窗口标题、正文、App 原始内容或上下文统计分布。
- 不包含 SQLite rowid、selection event id、session id、candidate index、candidate count、negative feedback reason、selection event 原始行、negative feedback 原始行或 import batch source。
- `frequency`、`recency_score` 和 `negative_score` 必须为非负摘要值；`reading` 使用稳定字符串表达，未知时为空字符串。

`dictionary.deleted_terms` payload：

```text
tombstones[]
  input_code
  text
  reading
  deleted_at_ms
  reason
```

规则：

- 只表达当前 deleted term identity 和最新 tombstone 意图。
- plaintext term identity 只允许作为加密前的本地 payload 字段，后续必须进入 `ime-crypto` envelope；不得作为服务端可见 object id、hash 或日志字段。
- 不导出 P1 原始选择事件或负反馈明细。

## 加密对象外壳

`EncryptedSyncObjectDraft` 表示后续可上传对象的外壳元数据：

```text
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
encrypted_payload_len
ciphertext_hash
created_at_ms
updated_at_ms
```

规则：

- `object_id`、`owner_device_id`、`key_id`、`algorithm` 和 `ciphertext_hash` 不能为空；该字段是密文或密文加 AAD 的 hash，不能是 plaintext hash。
- `schema_version`、`algorithm`、`key_epoch` 和 `nonce` 长度必须与 `ime-crypto` 当前 envelope 语义一致。
- `version` 从 1 开始。
- `base_version` 必须小于 `version`。
- `encrypted_payload_len` 必须大于 0。
- `updated_at_ms` 不得早于 `created_at_ms`。
- 该结构从 `ime-crypto::EncryptedObjectEnvelope` 派生，只保存密文长度，不保存 plaintext payload 或 encrypted payload bytes。
- 该结构不包含明文用户词、明文选择事件或明文负反馈。

## 冲突与删除方向

后续合并策略必须按对象类型区分：

- `dictionary.user_terms`：按 `input_code + text + reading` 合并，更新时间和删除 tombstone 参与冲突判断。
- `dictionary.deleted_terms`：删除意图优先，旧设备和旧备份不得复活用户已删除词条。
- `ranker.weights`：按 `input_code + text + reading + context_kind` 合并，允许按版本做摘要级冲突处理。
- `settings.*`：可以先使用 last-write-wins，后续管理 UI 再提供显式冲突提示。
- `backup.snapshot`：作为完整快照，不参与细粒度合并。

## 当前实现

已落地：

- `SyncObjectType`：后续对象类型枚举。
- `PayloadSource`：本地表来源分类。
- `LocalDataClass`：P1 本地、P2 加密同步、本地审计分级。
- `SyncPayloadPlan`：把本地来源分为可同步和本地保留。
- `EncryptedSyncObjectDraft`：从 `ime-crypto::EncryptedObjectEnvelope` 派生加密对象外壳元数据，校验 `schema_version`、`key_id`、`key_epoch`、`algorithm`、`nonce`、`ciphertext_hash` 和版本关系。
- `UserDb::p2_plaintext_payloads()`：导出 `dictionary.user_terms`、`ranker.weights` 与 `dictionary.deleted_terms` 的 Rust 内部 plaintext payload bytes，测试固定字段顺序、JSON string escaping、空库行为和 P1 / 本地审计阻断。
- userdb P2 payload 本地加密装配测试：用合成 key / device id 把 `dictionary.user_terms`、`ranker.weights` 与 `dictionary.deleted_terms` payload 加密为 `ime-crypto::EncryptedObjectEnvelope`，验证可解密回原 bytes、nonce 不重复，并派生 `ime-sync::EncryptedSyncObjectDraft` 元数据。

未落地：

- `settings.profile`、`settings.schema` 和 `backup.snapshot` plaintext payload 字段序列化。
- 生产级 userdb P2 plaintext payload 与 `ime-crypto` envelope 组装入口；当前只有 integration test，不暴露 CLI / FFI / 后端入口。
- 签名、设备授权、恢复码、设备撤销、密钥轮换和 key management；`ime-crypto` 当前已落地本地 AEAD / HKDF / ciphertext hash / envelope 测试。
- HTTP API、Go server 存储和冲突合并执行器。

## 验证口径

当前验证入口：

```text
cargo test -p radishlex-ime-sync
cargo test -p radishlex-ime-userdb
cargo test -p radishlex-ime-cli
./scripts/check-repo.sh
```

必须持续满足：

- P2 来源能映射到同步对象类型。
- P1 来源没有同步对象类型。
- 本地审计来源没有同步对象类型。
- 加密对象外壳拒绝空 ID、空设备 ID、空 key id、未知 algorithm、非法 nonce 长度、空 `ciphertext_hash`、0 版本、0 payload 大小和非法版本关系。
- `EncryptedSyncObjectDraft::from_crypto_envelope` 必须先验证 `ime-crypto` envelope，损坏的 crypto envelope 不能进入同步草案。
- userdb P2 plaintext payload 必须固定字段顺序、稳定 JSON escaping，不包含 P1 原始选择事件、负反馈 reason、上下文统计或本地 import batch 审计字段；`ranker.weights` 只能包含 P1 明细压缩后的 P2 权重摘要。
- userdb P2 payload 本地加密装配测试必须验证 envelope 可解密回原 bytes，`EncryptedSyncObjectDraft` 只保留密文长度和 ciphertext hash 等元数据，不携带 plaintext bytes。
