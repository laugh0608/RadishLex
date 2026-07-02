# RadishLex 同步 Payload 草案

本文档定义当前 Rust 侧同步 payload 草案、数据分级映射和验证口径，读者是后续实现 `ime-sync`、`ime-crypto`、同步 CLI、Go server 和管理 UI 的开发者。本文不包含加密算法实现、设备授权流程完整协议、Go server 数据库 migration、生产备份恢复操作细节或用户可用同步设置；客户端加密边界见 `docs/crypto-boundary.md`，Go server API 与 storage 边界见 `docs/sync-server-api-storage.md`。

## 当前定位

当前已落地 Rust 同步对象边界模型、remote client DTO / transport trait、std-only `http://` `HttpSyncRemoteTransport`、可选 bearer access token header、Rust 侧两客户端同步边界测试和 Go server 两客户端真实 HTTP 同步测试，不生成明文上传文件，也不启动或保留长期运行服务。`ime-sync` 的作用是把 `sync preflight` 已验证的本地分类边界转成可测试的 Rust API，并让上传草案元数据从 `ime-crypto` envelope 派生，避免同步层和加密层字段语义漂移；当前 remote client 上传入口只接收已加密 `AssembledSyncObject` 与 `SignedSyncObjectManifest`，不接受 plaintext payload。未来若从 shared token 演进到 OIDC bearer token，也只能改变传输层访问凭证，不改变 payload、envelope、object hash、设备签名或客户端解密合并边界；OIDC 规划见 `docs/sync-server-oidc-roadmap.md`。`ime-userdb` 当前提供 Rust 内部 P2 plaintext payload 只读迭代器，并已通过 `SyncEnvelopeAssembler` 完成本地 envelope 加密、解密和 sync draft 派生验证；解密后的 userdb P2 JSON 已能解析为 `ClientSyncMergeInput` 所需记录，并可把被合并模型接受的 user terms、deleted tombstones 和 ranker weights 写回本地 SQLite；`crates/ime-userdb/tests/two_client_sync.rs` 已覆盖设备 A 加密上传到远端 harness、设备 B 下载二进制密文、解密、合并写回 SQLite、stale conflict latest metadata 和基于 base version 重新组装 v2 上传；`crates/ime-userdb/tests/two_client_go_http_sync.rs` 已通过短生命周期 Go sync server 验证设备 B 授权、三类 P2 对象真实 HTTP 上传 / 下载 / 解密 / 写回、stale conflict 和 v2 重新上传：

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

本阶段只验证类型和边界，当前已定义 `dictionary.user_terms`、`ranker.weights` 与 `dictionary.deleted_terms` 的 plaintext payload 字段序列化，并已证明它们可以进入 `ime-crypto` envelope 后派生成 `EncryptedSyncObjectDraft`。`docs/sync-key-management.md` 已固定设备授权、key management 和冲突语义；`docs/sync-server-api-storage.md` 已固定远端 API、SQLite metadata、对象存储和版本冲突边界；Rust 侧已补同步域、设备状态、加入请求、授权包、撤销记录、key epoch、对象版本冲突草案模型、envelope 组装边界、remote client DTO / transport trait 和 `http://` HTTP transport。服务端只能看到对象类型、设备 ID、key id、key epoch、algorithm、nonce、版本、密文大小、ciphertext hash 和时间戳。

## Plaintext Payload

`ime-userdb` 当前提供 `UserDb::p2_plaintext_payloads()`，返回 Rust 内部只读迭代器。该入口不是 CLI / FFI / 文件导出入口，不返回 P1 原始事件、本地审计批次、SQLite handle 或可上传明文文件。

迭代器规则：

- 只返回非空对象；空库或没有 P2 数据时返回空迭代器。
- 对象输出顺序固定为 `dictionary.user_terms`、`ranker.weights`、`dictionary.deleted_terms`。
- 每个 payload 记录 `object_type`、`record_count` 和 UTF-8 JSON bytes；`record_count` 只用于本地测试和后续组装前校验，不作为服务端可见用户数据明细。
- JSON 字符串使用仓库内稳定 escaping：引号、反斜杠和控制字符转义，普通 UTF-8 文本保持直写；字段顺序由序列化函数固定，不依赖 map 遍历顺序。

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
- 记录排序固定为 `input_code, text, reading`。
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
- 记录排序固定为 `input_code, text, reading, context_kind`。
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
- 记录排序固定为 `input_code, text, reading`；若 `deleted_terms` 表没有可匹配 tombstone，`deleted_at_ms` 回退为 deleted user term 的 `updated_at_ms`，`reason` 回退为 `manual_delete`。
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
- integration test 中使用的 `dictionary-user-terms-device-a`、`ranker-weights-device-a`、`dictionary-deleted-terms-device-a` 等对象 ID 只是合成 fixture；生产对象 ID 不得包含明文 term identity、input code、reading、context 或可公开反查的 hash。

`SyncEnvelopeAssembler` 是当前 Rust 内部 P2 payload envelope 组装边界：

- 输入 `PlaintextSyncPayload`、`SyncObjectAssemblySpec` 和 `SyncMasterKeyMaterial`。
- 按 `object_type + object_id + key_epoch` 派生 object key，并调用 `ime-crypto` 生成 `EncryptedObjectEnvelope`。
- 使用 `NonceTracker` 阻断同一 key / epoch 下的 nonce 复用。
- 输出 `AssembledSyncObject`，包含密文 envelope、`EncryptedSyncObjectDraft` 和本地 `record_count`。
- `PlaintextSyncPayload` 的 Debug 输出必须隐藏 bytes；该入口不导出 CLI / FFI 明文 payload，不写文件，不连接 Go server。

## 远端对象客户端边界

`ime-sync` 的 remote client 只负责把本地已经组装完成的加密对象映射到 Go sync server API。它不是明文 payload 生成器，不决定部署网络拓扑，也不直接启动或管理 Go server。当前 `HttpSyncRemoteTransport` 是 std-only `http://` transport 实现，用于短生命周期测试、受控自部署 upstream 和跨语言验证；生产外部 TLS、反向代理和访问 token 配置仍由部署层负责。

核心类型：

```text
SyncRemoteClient<T: SyncRemoteTransport>
SyncRemoteTransport
SyncRemoteRequest
SyncRemoteResponse
RemoteObjectVersion
RemoteObjectPayload
SyncRemoteError
SyncServerErrorCode
LatestObjectConflictMetadata
HttpSyncRemoteTransport
```

上传入口：

```text
upload_object_version(domain_id, AssembledSyncObject, SignedSyncObjectManifest)
```

规则：

- `AssembledSyncObject` 必须来自 `SyncEnvelopeAssembler` 或等价的加密 envelope 组装路径。
- `SignedSyncObjectManifest` 必须覆盖同一 `domain_id`、`object_id`、`object_type`、`version`、`base_version`、`key_id`、`key_epoch`、algorithm、nonce、payload length、ciphertext hash 和客户端时间戳。
- manifest signer 必须等于 encrypted object 的 `owner_device_id`。
- 上传请求只包含服务端可见 metadata、signature bytes 和 encrypted payload bytes，不包含 plaintext user term、input code、reading、P1 event、ranker 明细或本地 merge 记录。
- `nonce`、`signature` 和 `payload` 在 JSON 请求中使用 Go `encoding/json` 兼容的 base64 字符串；不要改成整数数组。
- `base_version = None` 在 HTTP JSON 中映射为 `0`，表示新对象；metadata 响应中的 `base_version = 0` 映射回 `None`。

读取入口：

```text
object_version(domain_id, object_id, version)
object_payload(domain_id, object_id, version)
```

规则：

- metadata 读取返回 `RemoteObjectVersion`，不携带 payload bytes。
- payload 读取先拉取 metadata，再拉取 `/payload` 二进制响应；客户端必须校验响应长度等于 metadata 的 `encrypted_payload_len`。
- `HttpSyncRemoteTransport` 负责通过短连接 HTTP/1.1 传递 request / response bytes，不解析 plaintext，不记录请求体或响应体；客户端后续在解密前仍应复验 ciphertext hash，当前 remote client 边界已阻断长度不一致的响应。

错误映射：

- Go server 的 `error_code` 映射为 `SyncServerErrorCode`，未知错误码保留为 `Unknown`。
- `conflict_stale_base_version` 必须携带 `LatestObjectConflictMetadata`；该结构只包含 latest version 和 latest ciphertext hash，不包含 payload。
- transport 层错误使用 `SyncRemoteError::Transport`，JSON 或 payload 响应结构错误使用 `SyncRemoteError::InvalidResponse`。
- 请求构造错误使用 `SyncRemoteError::InvalidRequest`，包括 path segment 不合法、版本为 0、manifest 与 encrypted object metadata 不一致等。

脱敏规则：

- `SyncRemoteRequest` 的 `Debug` 不打印 body。
- `RemoteObjectVersion` 的 `Debug` 不打印 nonce 或 signature bytes。
- `RemoteObjectPayload` 的 `Debug` 不打印 payload bytes。
- `SyncRemoteError` 只能包含错误分类、HTTP status、服务端 error code、非敏感 message、retryable、server time 和 latest metadata；不得保存请求体或响应 payload。

## 冲突与删除方向

客户端合并策略必须按对象类型区分。当前 Rust 侧已在 `ime-sync` 中落地 `ClientSyncMergeInput` / `ClientSyncMergeResult` 纯模型，用于表达客户端解密 P2 payload 后的合并决策；`ime-userdb` 已补 `UserDbDecryptedSyncObject` 和 `decode_userdb_sync_objects()`，把已解密 P2 JSON 解析为带 `key_epoch` 的 user terms、deleted terms、ranker weights 记录，再转换为 `ClientSyncMergeInput`。`UserDb::apply_decoded_sync_payload_batch()` 会在本地 SQLite transaction 内执行合并结果写回：先用本机已有 tombstone 过滤普通同步词条和旧权重，再写入被接受的删除 tombstone、被接受的用户词条和被接受的 ranker weight；只有更新时间晚于本机 tombstone 的 `manual_add` 显式恢复可以清理本机删除意图。该入口不连接后端，不生成上传补丁，不暴露 CLI / FFI 明文同步入口。

- `dictionary.user_terms`：按 `input_code + text + reading` 合并，更新时间、`key_epoch` 和删除 tombstone 参与冲突判断；普通同步词条不能清除 tombstone。
- `dictionary.deleted_terms`：删除意图优先，旧设备和旧备份不得复活用户已删除词条；同一 term identity 下以较新的 `key_epoch` / 删除时间作为当前删除意图。
- `ranker.weights`：按 `input_code + text + reading + context_kind` 合并，active tombstone 会阻断同一 term identity 下的旧权重摘要。
- 显式恢复：用户明确重新添加词条时，必须以 `ExplicitRestore` 这类恢复意图表达，并且 `key_epoch` / 更新时间晚于 tombstone；恢复通过后才清除对应删除意图。恢复前的旧 `ranker.weights` 不随词条恢复一起复活。
- `settings.*`：可以先使用 last-write-wins，后续管理 UI 再提供显式冲突提示。
- `backup.snapshot`：作为完整快照，不参与细粒度合并。

## 当前实现

已落地：

- `SyncObjectType`：后续对象类型枚举。
- `PayloadSource`：本地表来源分类。
- `LocalDataClass`：P1 本地、P2 加密同步、本地审计分级。
- `SyncPayloadPlan`：把本地来源分为可同步和本地保留。
- `EncryptedSyncObjectDraft`：从 `ime-crypto::EncryptedObjectEnvelope` 派生加密对象外壳元数据，校验 `schema_version`、`key_id`、`key_epoch`、`algorithm`、`nonce`、`ciphertext_hash` 和版本关系。
- `PlaintextSyncPayload`、`SyncObjectAssemblySpec`、`SyncEnvelopeAssembler` 和 `AssembledSyncObject`：固定 Rust 内部 P2 payload 到 `ime-crypto` envelope 的组装边界，从 sync master 派生 object key，生成密文 envelope 和同步草案，并阻断重复 nonce；不暴露 CLI / FFI 明文 payload。
- `SyncDomain`、`SyncDevice`、`DeviceJoinRequest`、`DeviceAuthorizationPackage`、`DeviceRevocationRecord` 和 `SyncObjectVersion`：固定当前 Rust 侧设备生命周期、授权状态、撤销 epoch 推进和对象版本冲突判断。
- `ClientSyncMergeInput`、`ClientSyncMergeResult`、`DictionaryUserTermMergeRecord`、`DictionaryDeletedTermMergeRecord` 和 `RankerWeightMergeRecord`：固定客户端解密后合并模型，覆盖 tombstone 压过旧 user terms / ranker weights、旧 epoch 上传不能复活删除词、显式恢复清理 tombstone、恢复前旧权重不复活和重复记录按 `key_epoch` / 时间收敛。
- `UserDb::p2_plaintext_payloads()`：导出 `dictionary.user_terms`、`ranker.weights` 与 `dictionary.deleted_terms` 的 Rust 内部 plaintext payload bytes，测试固定字段顺序、JSON string escaping、空库行为和 P1 / 本地审计阻断。
- `UserDbDecryptedSyncObject`、`UserDbDecodedSyncPayloadBatch` 和 `decode_userdb_sync_objects()`：解析已解密的 userdb P2 JSON，严格校验 schema、object type、字段集合和值域，把 `manual_add` 映射为显式恢复意图，并接入 `ClientSyncMergeInput`；解析依赖 `serde_json = 1.0.150`，许可为 MIT OR Apache-2.0。
- `UserDb::apply_decoded_sync_payload_batch()` 和 `UserDbSyncApplySummary`：把已解密 P2 payload batch 经过 `ClientSyncMergeInput` 合并后写回真实 userdb，覆盖 user terms、deleted tombstones、ranker weights、payload tombstone 阻断、本机 tombstone 阻断、显式恢复清理和旧权重阻断；summary 只暴露计数，不暴露明文 term identity。
- userdb P2 payload 本地加密装配测试：通过 `SyncEnvelopeAssembler` 用合成 sync master key / device id 把 `dictionary.user_terms`、`ranker.weights` 与 `dictionary.deleted_terms` payload 加密为 `ime-crypto::EncryptedObjectEnvelope`，验证可解密回原 bytes、nonce 不重复，并派生 `ime-sync::EncryptedSyncObjectDraft` 元数据。
- `SyncRemoteClient`、`SyncRemoteTransport`、`RemoteObjectVersion`、`RemoteObjectPayload`、`SyncRemoteError` 和 `HttpSyncRemoteTransport`：固定 Rust remote client 与 Go object version API 的 DTO / transport 边界，覆盖 JSON base64 byte 字段、metadata 读取、binary payload 下载、stale conflict latest metadata、server error code 映射、HTTP/1.1 request / response 传递、可选 bearer access token header、chunked response 解码、base path 拼接和 Debug 脱敏。
- `two_client_sync` integration test：使用合成 userdb、sync master key、test-memory signing key store 和内存 remote harness，复验 P2 payload 加密上传、另一客户端下载密文后解密、解码、合并写回 SQLite、本机 tombstone 阻断旧远端词条、ranker weight 写回、stale base version 409 latest metadata 映射，以及合并后按 `base_version = 1` 重新上传 v2。
- `two_client_go_http_sync` integration test：使用短生命周期 Go sync server、临时 SQLite metadata、临时 blob dir、真实 `HttpSyncRemoteTransport` 和 test-memory signing key store，复验 domain 创建、设备 B join / signed authorization、`dictionary.user_terms` / `ranker.weights` / `dictionary.deleted_terms` 三类 P2 对象真实 HTTP 上传、另一客户端下载密文后解密 / 解码 / 合并写回 SQLite、stale conflict latest metadata、B 端按 `base_version = 1` 上传 v2、A 端下载 v2 写回，以及 runtime 日志脱敏。

未落地：

- `settings.profile`、`settings.schema` 和 `backup.snapshot` plaintext payload 字段序列化。
- 生产恢复 UI / API、远端密钥轮换执行器、备份快照 payload 字段序列化和用户可用同步设置。
- 真实平台私钥存储 backend 的生产可用状态；当前 `apple-keychain-v1` 已 feature-gated 接线，但真实 smoke 阻塞于 `ed25519-v1` 创建，不能作为可用生产 backend；`android-keystore-v1` 已补 runbook、feature-gated Rust store、Rust bridge wrapper、bridge contract、合成 bridge 单测、ignored smoke 门禁、仓库内 Kotlin / Gradle harness、`@JvmStatic` facade、gated instrumented smoke、provider diagnostics、smoke 记录模板和设备矩阵记录，已补 Rust raw JNI glue，Android target build 已通过 `./scripts/check-android-target.sh`；Pixel 9 Pro API 35 AVD 真实 smoke / provider diagnostics 与 Pixel 10 Pro API 37 AVD provider diagnostics 结果均为 `unsupported_signature_algorithm`。

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
- `SyncEnvelopeAssembler` 必须验证 record count、object id、device id、object key role、version / base version、object key 派生、nonce 复用阻断和 Debug 明文阻断。
- userdb P2 payload 本地加密装配测试必须通过 `SyncEnvelopeAssembler` 验证 envelope 可解密回原 bytes，`EncryptedSyncObjectDraft` 只保留密文长度和 ciphertext hash 等元数据，不携带 plaintext bytes。
- remote client 上传请求必须只由 `AssembledSyncObject` 和 `SignedSyncObjectManifest` 生成，不能接受 plaintext payload、P1 event 或 ranker 明细字段；JSON byte 字段必须保持 Go 兼容 base64。
- remote client 必须拒绝 manifest 与 encrypted object metadata 不一致的上传请求，必须把 stale base version 映射为 latest metadata，且错误 / Debug 输出不得泄漏请求体、signature、nonce 或 payload bytes。
- `HttpSyncRemoteTransport` 必须只支持不含凭据、query 和 fragment 的 `http://` base URL，必须传递 JSON request 和 binary payload response，必须拒绝请求 path 中的 query / fragment；访问启用 Go access token 的 server 时只能通过受控 bearer header 配置，不得把 token 放进 URL、日志或 Debug，且 transport 错误不得包含请求体、payload、nonce、signature、token 或 plaintext payload。
- 设备生命周期模型必须验证 pending / active / revoked 状态转移，授权设备和接收设备都必须 active，撤销记录必须推进 `key_epoch`，对象版本必须能识别 stale base version。
- 客户端合并模型必须验证 `dictionary.deleted_terms` tombstone 能压过旧 `dictionary.user_terms` 和旧 `ranker.weights`，旧 epoch 上传不能靠更晚本机时间复活删除词，显式恢复必须晚于 tombstone，且恢复前的旧权重不随词条恢复一起复活。
- userdb P2 payload 解码必须拒绝 schema / object type 不匹配、未知字段、非法字段类型、`dictionary.user_terms` 中的 deleted 状态、0 key epoch 和负数 / 非有限权重摘要，并能把真实 payload bytes 转成 `ClientSyncMergeInput`。
- userdb P2 payload 写回必须在同一 SQLite transaction 内执行，并覆盖写入 accepted user terms、写入 accepted tombstones、写入 accepted ranker weights、payload tombstone / 本机 tombstone 阻断旧词条与旧权重、显式恢复清理 tombstone，以及 summary 不暴露明文身份。
- Rust 侧两客户端同步边界必须覆盖设备 A 加密上传、设备 B 下载二进制密文、解密、解码、合并写回 userdb、stale conflict latest metadata 映射和基于最新 base version 重新上传；该测试不得启动长期运行 server，也不得引入 plaintext payload、P1 event 或平台壳入口。
- Go server 两客户端真实 HTTP 同步测试必须使用短生命周期 server、临时 metadata / blob 目录和真实 HTTP transport，覆盖设备授权、三类 P2 对象上传下载、客户端解密合并写回、stale conflict、v2 重新上传和日志脱敏；测试结束必须清理 server 进程与临时目录，不保留长期运行服务。
