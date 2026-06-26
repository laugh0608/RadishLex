# RadishLex 同步 Payload 草案

本文档定义当前 Rust 侧同步 payload 草案、数据分级映射和验证口径，读者是后续实现 `ime-sync`、`ime-crypto`、同步 CLI、Go server 和管理 UI 的开发者。本文不包含加密算法实现、设备授权流程完整协议、HTTP API、Go server 数据库 migration 或远端同步客户端实现。

## 当前定位

当前只落地 `crates/ime-sync/` 的同步对象边界模型，不连接后端，不生成明文上传文件，也不实现加密。它的作用是把 `sync preflight` 已验证的本地分类边界转成可测试的 Rust API：

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

本阶段只验证类型和边界，不定义完整字段序列化格式。真正写入服务端前必须先经过 `ime-crypto` 加密，服务端只能看到对象类型、设备 ID、版本、密文大小、hash 和时间戳。

## 加密对象外壳

`EncryptedSyncObjectDraft` 表示后续可上传对象的外壳元数据：

```text
object_id
object_type
owner_device_id
version
base_version
encrypted_payload_len
payload_hash
created_at_ms
updated_at_ms
```

规则：

- `object_id`、`owner_device_id` 和 `payload_hash` 不能为空。
- `version` 从 1 开始。
- `base_version` 必须小于 `version`。
- `encrypted_payload_len` 必须大于 0。
- `updated_at_ms` 不得早于 `created_at_ms`。
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
- `EncryptedSyncObjectDraft`：加密对象外壳元数据与校验。

未落地：

- 明文 payload 字段序列化。
- 加密、签名、hash 计算和 key management。
- 设备授权、恢复码、设备撤销和密钥轮换。
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
- 加密对象外壳拒绝空 ID、空设备 ID、空 hash、0 版本、0 payload 大小和非法版本关系。
