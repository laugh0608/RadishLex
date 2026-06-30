# RadishLex 同步服务端 API 与存储边界

本文档定义 Go sync server 实现期间必须稳定的 API、存储、错误语义和验证口径。读者是后续实现 `server/sync-server`、`ime-sync` 远端客户端、同步 runbook 和审阅隐私边界的开发者。本文不包含 Docker Compose 配置、Flutter 同步页面、Go server 两客户端真实联调或生产平台私钥存储 backend；生产恢复流程见 `docs/production-recovery-flow.md`，平台私钥存储 backend 边界见 `docs/adr/0004-platform-private-key-storage-backend.md`。

## 当前定位

当前 Rust 侧已经完成 P2 payload 本地加密、设备授权 / 撤销签名、恢复记录签名、客户端解密后合并模型、已解密 P2 payload 写回本地 SQLite 的执行器、`ime-sync` remote client DTO / transport trait、std-only `http://` `HttpSyncRemoteTransport`，以及使用内存 remote harness 的两客户端同步边界测试。Go server 已起步，当前 `server/sync-server` 已包含配置默认值、API request / response / error DTO、storage interface、SQLite metadata migration 文本、storage conformance tests、内存 metadata store、SQLite-backed metadata repository、local object storage staged transaction、metadata transaction 与 blob transaction 接线、Ed25519 签名验证抽象、签名篡改拒绝测试、recovery latest handler、domain / device / join request metadata handler、authorization handler、encrypted object version 上传 / metadata 读取 / payload 下载 handler、API 层 request id、panic recovery、非持久审计 hook、SQLite `audit_events` 写入测试、`cmd/radishlex-sync-server` 启动入口、runtime 配置装配、SQLite migration 嵌入、对象大小门禁、脱敏 audit logger、本机 smoke runbook 和短生命周期 HTTP smoke 测试；runtime smoke 已覆盖第二设备 join request / authorization、active 状态复验、跨设备同一 object 的 stale conflict 与 v2 payload 读取。Rust HTTP transport 直连 Go server 的短生命周期跨语言测试已覆盖 domain 初始化、signed encrypted object 上传、metadata / payload 读取和 stale conflict 映射。尚未实现 Docker Compose 或生产部署封装。SQLite driver 当前使用纯 Go `modernc.org/sqlite`，避免把 CGO 作为 server 单元测试前提。

本阶段只固定服务端 API 和 storage 边界：

- 服务端默认不可信，只保存密文对象、设备公钥、签名记录、版本和必要同步元数据。
- 服务端可以验证对象 metadata、ciphertext hash、设备状态、签名、版本冲突和存储完整性。
- 服务端不能解密、不能解析 plaintext payload、不能合并用户词、不能读取 P1 原始事件。
- 客户端仍是真相源：解密、冲突合并、删除 tombstone 语义、显式恢复和 userdb 写回都在客户端完成。

Go 代码必须继续受本文件约束 migration、handler 和测试命名；平台私钥存储 backend capability / unavailable backend 的 Rust 模型已经落地。进入 Docker Compose、生产部署封装或用户可用同步前，仍必须保持签名验证、HTTP API handler、Go runtime smoke、Rust HTTP transport 直连 Go server、Rust 侧两客户端 harness、错误语义、审计日志和平台 backend 验证彼此一致。

## 服务端职责

允许服务端承担：

- 同步域登记和单用户自部署初始化。
- 设备登记、设备公钥保存、加入请求转发、设备授权记录保存。
- 设备撤销记录保存，并拒绝被撤销设备继续写入新对象。
- 恢复记录保存，包括恢复 KDF 参数、salt、密文包装材料和签名 manifest。
- 加密对象 metadata 与 ciphertext blob 存储。
- 对象版本、`base_version`、`key_epoch` 和 `ciphertext_hash` 校验。
- 乐观冲突检测，返回最新远端 metadata，提示客户端拉取、解密、合并后再上传。
- 非敏感审计日志，例如请求类型、设备 ID、对象 ID、版本、字节数、结果码和服务端时间。
- 后续 P3 包分发，但包分发必须与个人 P2 同步对象分表、分路径、分权限。

禁止服务端承担：

- 每次按键、候选生成、候选排序、学习决策或输入热路径能力。
- 明文用户词、input code、reading、候选偏好、上下文统计、原始选择事件或负反馈明细的存储、日志或索引。
- plaintext payload hash、term identity hash，或任何可公开反查用户词身份的稳定标识。
- `record_count`、userdb merge summary、P1 事件计数之外的明文 payload 细节。
- 解析 `dictionary.user_terms`、`dictionary.deleted_terms`、`ranker.weights` 的 JSON 内容。
- 根据服务端账号密码、管理 token 或恢复码明文直接解密用户数据。
- 把对象 ID、文件路径、日志字段或错误信息设计成包含明文词条、拼音码、reading 或上下文。

## 可见数据模型

服务端可持久化下列记录。字段名是 API / migration 设计约束，不要求后续实现逐字照搬，但不得扩大明文可见面。

`sync_domains`：`domain_id`、`current_key_epoch`、`active_key_id`、`created_at_ms`、`updated_at_ms`。

`current_key_epoch` 只用于拒绝撤销后的旧 epoch 新写入；客户端合并仍是最终冲突真相源。

`devices`：`domain_id`、`device_id`、`signing_public_key_id`、`signing_public_key`、`key_agreement_public_key_id`、`key_agreement_public_key`、`status`、`authorized_at_ms`、`revoked_at_ms`、`last_seen_at_ms`。设备显示名如果后续需要展示，应作为用户可编辑的非敏感标签处理，不得从系统用户名、联系人或输入内容自动采集。

`device_join_requests`：`domain_id`、`join_request_id`、`device_id`、`signing_public_key_id`、`signing_public_key`、`key_agreement_public_key_id`、`key_agreement_public_key`、`challenge`、`created_at_ms`、`expires_at_ms`、`status`。

服务端只转发待授权设备的公钥、challenge 和状态。短码应由客户端根据加入请求内容本地计算和展示；授权提交时需要携带 signed authorization 中的 `join_short_code`，用于验签绑定用户确认过的短码。

`device_authorizations`：`domain_id`、`join_request_id`、`authorizer_device_id`、`recipient_device_id`、`recipient_signing_public_key_id`、`recipient_key_agreement_key_id`、`join_short_code`、`key_epoch`、`created_at_ms`、`signature_schema_version`、`signature_algorithm`、`signature_key_id`、`signature`。

授权记录只证明某个 active 设备接受了待加入设备的公钥和指定 key epoch，不包含同步主密钥明文或恢复码。当前 Go storage 会在同一事务中把 join request 置为 active、写入授权记录、写入 wrapping metadata，并激活接收设备。

`device_wrapping_records`：`domain_id`、`recipient_device_id`、`authorizer_device_id`、`key_epoch`、`wrapping_key_id`、`algorithm`、`nonce`、`wrapped_key_len`、`ciphertext_hash`、`created_at_ms`、`signature`、`blob_ref`。

包装记录只保存给指定设备的包装密文元数据、签名和密文 blob ref，不保存 `SyncMasterKey`、`DeviceWrappingKey` 或恢复码明文。当前 Go storage 已在授权事务中保存 / 读取 wrapped key bytes，并按 `wrapped_key_len` 与 `ciphertext_hash` 复验；该字段只能是密文 bytes。

`device_revocations`：`domain_id`、`revoked_device_id`、`revoker_device_id`、`previous_key_epoch`、`new_key_epoch`、`reason`、`created_at_ms`、`signature`。

撤销记录被接受后，服务端必须拒绝被撤销设备后续上传，并拒绝低于 `current_key_epoch` 的新对象版本写入。历史对象是否重加密由客户端和管理 UI 后续单独设计。

`recovery_records`：`domain_id`、`recovery_record_id`、`key_epoch`、`kdf_profile`、`kdf_version`、`memory_kib`、`iterations`、`parallelism`、`output_len`、`salt`、`algorithm`、`nonce`、`wrapped_material_len`、`ciphertext_hash`、`status`、`created_at_ms`、`revoked_at_ms`、`signer_device_id`、`signature_schema_version`、`signature_algorithm`、`signature_key_id`、`signature`、`blob_ref`。

恢复记录只保存加密后的同步域材料和 KDF 参数。服务端可以对读取和替换恢复记录做限速，但不能依赖限速替代恢复码强度。

`sync_objects`：`domain_id`、`object_id`、`object_type`、`latest_version`、`latest_ciphertext_hash`、`latest_key_epoch`、`created_at_ms`、`updated_at_ms`。

`object_id` 必须是不含业务明文的 opaque ID。需要稳定 term identity 时，只能放在 encrypted payload 内，或使用客户端持有密钥派生的不可公开反查 ID。

`sync_object_versions`：`domain_id`、`object_id`、`version`、`base_version`、`owner_device_id`、`key_id`、`key_epoch`、`algorithm`、`nonce`、`encrypted_payload_len`、`ciphertext_hash`、`signature_schema_version`、`signature_algorithm`、`signature_key_id`、`signature`、`server_received_at_ms`、`client_created_at_ms`、`client_updated_at_ms`、`blob_ref`。

服务端可以按 `object_type`、版本和时间分页列出 metadata；payload bytes 必须通过 blob 存储读取，不放入日志或错误响应。

`audit_events`：`domain_id`、`event_type`、`device_id`、`object_id`、`version`、`result_code`、`bytes`、`server_time_ms`。

审计记录不得保存请求体、响应体、密文包装材料、恢复码、plaintext payload 或错误堆栈中的敏感字段。

## 当前 Go storage surface

当前 `server/sync-server/internal/storage.Store` 是 HTTP handler 前的内部边界，已经落地 `CreateDomain`、`Domain`、`Device`、`SaveJoinRequest`、`PendingJoinRequests`、`AuthorizeJoinRequest`、`DeviceWrappedKey`、`RevokeDevice`、`PutRecoveryRecord`、`LatestRecoveryRecord`、`LatestRecoveryWrappedMaterial`、`PutObjectVersion`、`ObjectVersion` 和 `ObjectPayload`。

这组方法当前用于验证 metadata、设备状态、版本冲突、blob 写入和错误语义，不等同于完整 HTTP API。尚未暴露对象分页、审计日志查询或持久限速器。

当前 storage conformance 已覆盖：第一台设备必须为 `active`；join request 从 `pending` 授权到 `active`；wrapped device key bytes 随授权事务保存并可按 metadata 读取；revoked 设备和旧 `key_epoch` 写入被拒绝；object version 支持同 hash 幂等重试、同版本不同 hash 冲突和 stale `base_version` latest metadata；object payload 读取复验长度 / Rust envelope ciphertext hash；recovery record 写入校验 wrapped material 长度 / ciphertext hash 并分配 `blob_ref`；latest recovery metadata 与 wrapped material bytes 可一起读取并复验；signed object manifest、device authorization、device revocation 和 recovery record 字段篡改会被 Ed25519 验签拒绝。

当前 storage 已在写入前使用 `devices.signing_public_key` 验证 object manifest、device authorization、device revocation 和 recovery record；签名 canonical bytes 对齐 Rust `radishlex-signature-v1` length-prefixed field list。当前 API 层已补 `GET /api/v1/domains/{domain_id}/recovery-records/latest`，复用 `LatestRecoveryWrappedMaterial`，返回服务端可见 recovery metadata 与 encrypted wrapped material，并覆盖统一 JSON 错误响应、`recovery_rate_limited` 和不泄漏内部 `blob_ref`。API 层也已补 `POST /domains`、`GET /domains/{domain_id}/state`、`GET /domains/{domain_id}/devices/{device_id}`、`POST /domains/{domain_id}/join-requests`、`GET /domains/{domain_id}/join-requests` 和 `POST /domains/{domain_id}/join-requests/{join_request_id}/authorization`，覆盖创建 domain、读取 domain metadata、读取 active / pending device metadata、创建 / 列出 pending join request、authorization request 到 storage upload 的映射和非法 JSON 错误响应。对象版本 API 已补 `POST /api/v1/domains/{domain_id}/objects/{object_id}/versions`、`GET /api/v1/domains/{domain_id}/objects/{object_id}/versions/{version}` 和 `GET /api/v1/domains/{domain_id}/objects/{object_id}/versions/{version}/payload`，复用 `PutObjectVersion`、`ObjectVersion` 和 `ObjectPayload`，覆盖 encrypted payload 长度 / hash mismatch、stale base version latest metadata、同版本同 hash 幂等、同版本不同 hash 冲突、revoked / pending / unknown device 禁止上传、plaintext 字段拒绝和错误 / 审计不泄漏 payload。当前 handler 外层已补 `X-Request-ID` 透传 / 生成、panic recovery 结构化 `storage_unavailable` 响应和非持久 `AuditSink` hook；当底层 store 实现持久审计时会写入 SQLite `audit_events`。runtime 层已补 `cmd/radishlex-sync-server`、SQLite + local blob store 装配、idempotent migration、HTTP timeout、`RADISHLEX_SYNC_MAX_OBJECT_BYTES` 门禁和脱敏 audit logger。审计事件和 runtime 日志只包含 route name / event type、domain id、device id、object id、object type、version、result code、HTTP status、byte count、server time 和 latency，不包含请求体或响应体。

## HTTP API 边界

首批 API 使用 `/api/v1` 前缀。metadata 使用 JSON；当前对象上传使用 JSON `payload` byte 字段承载 encrypted bytes，Go JSON 编码下表现为 base64 字符串；对象 payload 下载接口返回 `application/octet-stream` 二进制密文。后续可以调整传输细节，但不能改变“metadata 可验证、payload 仍为密文”的边界。

### 同步域

`POST /api/v1/domains`

- 用于单用户自部署初始化。
- 请求包含客户端生成的 `domain_id`、第一台设备公钥、初始 `key_epoch`、可选恢复记录 metadata 和签名 manifest。
- 服务端创建 domain 与第一台 `active` 设备。
- 不接收同步主密钥、恢复码明文或任何 userdb payload。

`GET /api/v1/domains/{domain_id}/state`

- 返回 domain metadata、设备列表、恢复记录状态和对象 latest version 摘要。
- 不返回对象 payload；客户端需要按对象版本显式下载密文 bytes。

### 设备登记与授权

`POST /api/v1/domains/{domain_id}/join-requests`

- 待加入设备提交设备 ID、公钥、challenge、创建时间和过期时间。
- 服务端保存为 `pending`，不把设备标记为可同步。

`GET /api/v1/domains/{domain_id}/join-requests`

- 已授权设备读取待处理加入请求。
- 响应只包含设备公钥、challenge、过期时间和状态。

`POST /api/v1/domains/{domain_id}/join-requests/{join_request_id}/authorization`

- 已授权设备提交 signed authorization、接收设备包装记录和必要 metadata。
- 服务端验证 authorizer 是 `active`，join request 未过期，recipient public key 与请求一致，签名有效。
- 通过后将接收设备置为 `active`，并保存包装记录。

`POST /api/v1/domains/{domain_id}/devices/{device_id}/revocations`

- `active` 设备提交 signed revocation、`previous_key_epoch`、`new_key_epoch` 和可选新 epoch 包装记录集合。
- 服务端验证 revoker 是 `active`，`new_key_epoch` 大于当前 epoch，签名有效。
- 通过后标记目标设备 `revoked` / `lost`，推进 domain `current_key_epoch`。

`POST /api/v1/domains/{domain_id}/devices/{device_id}/heartbeat`

- 更新 `last_seen_at_ms` 和服务端可见健康状态。
- 不上传输入状态、候选状态、学习事件或本地数据库摘要。

### 加密对象

`POST /api/v1/domains/{domain_id}/objects/{object_id}/versions`

- 上传一个新加密对象版本。
- 请求 metadata 必须包含 `object_type`、`version`、`base_version`、`owner_device_id`、`key_id`、`key_epoch`、`algorithm`、`nonce`、`encrypted_payload_len`、`ciphertext_hash`、客户端时间和 signed object manifest。
- 当前请求体为 JSON，`payload` 字段只能是 encrypted bytes；不接受 plaintext user term、input code、reading、P1 event 或 ranker 明细字段。HTTP handler 会在进入 storage 前按 `RADISHLEX_SYNC_MAX_OBJECT_BYTES` 拒绝过大的 `encrypted_payload_len` 或实际 `payload` bytes。
- 服务端验证设备 active、签名有效、metadata 合法、payload 长度和 ciphertext hash 匹配。
- 对象版本的 `ciphertext_hash` 必须对齐 Rust `ime-crypto` envelope hash：使用 `radishlex-ciphertext-hash-v1` domain separator，依次绑定公开 object AAD、AAD 长度、encrypted payload 长度和 encrypted payload bytes。Go server 使用请求 metadata 重建公开 AAD 后复验，不使用 `sha256(payload)` 作为 object hash；device wrapping 和 recovery wrapped material 仍使用裸密文 bytes 的 hash / length 校验。
- 新对象要求 `version = 1` 且 `base_version = 0`。
- 已存在对象要求 `base_version` 等于服务端 latest version，且 `version = latest_version + 1`。
- 若 `base_version` 落后，返回 `409 conflict_stale_base_version` 和 latest metadata；客户端拉取密文、解密合并后再上传新版本。
- 同一 `object_id + version + ciphertext_hash` 的重试可以幂等成功；同版本不同 hash 必须拒绝。

`GET /api/v1/domains/{domain_id}/objects`

- 按 `object_type`、`since_version`、`updated_after_ms` 和分页参数列出对象 metadata。
- 不返回 payload bytes。

`GET /api/v1/domains/{domain_id}/objects/{object_id}/versions/{version}`

- 返回指定版本 metadata。

`GET /api/v1/domains/{domain_id}/objects/{object_id}/versions/{version}/payload`

- 返回指定版本 encrypted bytes。
- 服务端不解密、不转码、不压缩 plaintext。

业务删除不通过 HTTP 删除明文词条表达。用户词删除必须进入 `dictionary.deleted_terms` 加密对象；服务端级删除只用于用户明确清空同步域密文数据或管理员清理整域数据。

### Rust 客户端 DTO 映射

Rust `ime-sync` remote client 与上述对象版本 API 的稳定映射如下：

- `SyncRemoteClient::upload_object_version()` 调用 `POST /api/v1/domains/{domain_id}/objects/{object_id}/versions`。
- `SyncRemoteClient::object_version()` 调用 `GET /api/v1/domains/{domain_id}/objects/{object_id}/versions/{version}`。
- `SyncRemoteClient::object_payload()` 先调用 metadata GET，再调用 `GET /api/v1/domains/{domain_id}/objects/{object_id}/versions/{version}/payload`。

上传请求来源：

- Rust 上传入口必须接收 `AssembledSyncObject` 和 `SignedSyncObjectManifest`。
- `AssembledSyncObject.draft` 提供 object metadata，`AssembledSyncObject.envelope.encrypted_payload` 提供 encrypted bytes。
- `SignedSyncObjectManifest.signature` 提供 `signature_schema_version`、`signature_algorithm`、`signature_key_id` 和 signature bytes。
- 客户端在发送前必须验证 manifest 与 encrypted object metadata 完全一致，包括 domain、object、version、base version、key、algorithm、nonce、payload length、ciphertext hash 和时间戳。
- Rust DTO 必须发送 `AssembledSyncObject.draft.ciphertext_hash`，该值来自 `ime-crypto` envelope 的 AAD + encrypted payload hash。Go server 按同一公开 AAD 规则复验，不能把对象版本 hash 降级为裸 payload hash。

JSON byte 字段：

- Go `encoding/json` 会把 `[]byte` 编码为 base64 字符串。
- Rust DTO 必须把 `nonce`、`signature` 和上传 `payload` 编码为 base64 字符串；响应中的 `nonce` 和 `signature` 也必须按 base64 解码。
- 不得把这些字段编码为 JSON 数字数组，也不得把 payload 改成 UTF-8 字符串。
- `/payload` 下载响应不是 JSON，必须按 `application/octet-stream` 二进制密文处理。

版本与冲突：

- Rust `base_version = None` 映射为 HTTP JSON 的 `base_version = 0`；响应中的 `base_version = 0` 映射回 `None`。
- `409 conflict_stale_base_version` 必须映射为包含 latest version 和 latest ciphertext hash 的客户端错误；该错误不包含 payload bytes。
- `409 conflict_object_version` 表示同一 object version 已存在但 ciphertext hash 不一致，客户端不得把它当作幂等成功。

客户端脱敏：

- Rust request / response / payload wrapper 的 `Debug` 不打印请求体、nonce、signature 或 payload bytes。
- 客户端错误对象不得保存原始 request body、response body、payload bytes、wrapped material 或 plaintext payload。
- 错误 message 只能用于开发诊断，不得拼接用户词、input code、reading、P1 event 或 ranker 明细。

### 恢复记录

`PUT /api/v1/domains/{domain_id}/recovery-records/{recovery_record_id}`

- 上传或替换 signed recovery record。
- 请求包含 KDF profile、salt、nonce、wrapped material 长度、ciphertext hash、状态和签名。
- 服务端验证签名设备 active，metadata 合法，payload hash 匹配。

`GET /api/v1/domains/{domain_id}/recovery-records/latest`

- 返回当前 active recovery record metadata 和 encrypted wrapped material。
- 服务端应对该接口做基于 domain、IP、设备和时间窗的限速；限速失败返回结构化错误。

`POST /api/v1/domains/{domain_id}/recovery-records/{recovery_record_id}/revoke`

- 保存 signed recovery record revocation。
- 不删除历史审计 metadata，但后续 `latest` 不再返回 revoked 记录作为 active。

## 错误语义

错误响应使用稳定结构：

```text
error_code
message
retryable
server_time_ms
latest_version
latest_ciphertext_hash
```

`message` 只能包含非敏感说明；不得回显请求体、payload bytes、恢复码、签名材料或明文业务字段。

首批错误码：

- `invalid_request`：字段缺失、格式错误、非法对象类型、非法 nonce / hash / 版本关系。
- `unauthenticated`：缺少自部署访问凭证或传输层认证失败。
- `forbidden_device`：设备不是 `active`、已撤销、join request 未授权或签名公钥不匹配。
- `not_found`：domain、device、object、version 或 recovery record 不存在。
- `conflict_stale_base_version`：上传基于旧版本，客户端必须拉取并合并。
- `conflict_object_version`：同一对象版本存在但 ciphertext hash 不一致。
- `invalid_signature`：对象 manifest、授权、撤销或恢复记录验签失败。
- `invalid_ciphertext_metadata`：payload 长度、ciphertext hash 或 algorithm metadata 与请求不一致。
- `payload_too_large`：超过服务端配置的对象大小上限。
- `recovery_rate_limited`：恢复记录读取或恢复尝试触发限速。
- `storage_unavailable`：SQLite 或对象存储不可写 / 不一致。

服务端不返回“词条冲突”“候选偏好冲突”或“权重合并失败”这类业务错误；这些属于客户端解密后的合并语义。

## SQLite 与对象存储边界

默认自部署形态是 Go server + SQLite metadata + local object storage：

- SQLite 保存 domain、device、join request、authorization、revocation、recovery record、object metadata、blob ref 和审计事件。
- local object storage 保存 encrypted payload bytes 和 encrypted wrapped material bytes。
- `blob_ref` 使用服务端生成路径或 key，不得使用明文词条、input code、reading、上下文或用户可反查内容。
- 对象存储路径可以包含 `domain_id`、opaque `object_id`、version 和 ciphertext hash；这些字段本身必须已经满足不含明文业务语义。
- 后续支持 S3-compatible storage 时，S3 object key 遵循同样约束。

当前配置默认值：`RADISHLEX_SYNC_LISTEN=127.0.0.1:7319`、`RADISHLEX_SYNC_METADATA_PATH=data/sync-server.sqlite`、`RADISHLEX_SYNC_BLOB_DIR=data/objects`、`RADISHLEX_SYNC_MAX_OBJECT_BYTES=16 MiB`、`RADISHLEX_SYNC_RECOVERY_READS_PER_HOUR=12`。

当前 local object storage 的 `blob_ref` 校验规则：

- 必须是安全相对路径，不允许绝对路径、反斜杠、冒号、`..`、非 canonical path 或 `.tmp` 保留命名空间。
- 只允许 ASCII 字母、数字、`/`、`.`、`_`、`-`。
- 当前 object / recovery blob ref 由服务端生成，并把 `domain_id`、`object_id`、`recovery_record_id`、`ciphertext_hash` 等 opaque 字段做 URL-safe base64 path component，避免路径分隔和 shell 特殊字符污染。

当前对象上传写入顺序：

1. 服务端先校验 metadata 与 encrypted bytes 的长度 / ciphertext hash；对象版本 hash 使用 Rust envelope AAD + encrypted bytes 规则，wrapped key / recovery material 使用裸密文 bytes 规则。
2. 在 SQLite transaction 中验证 domain、设备状态、key epoch、版本关系和对象类型。
3. 把 encrypted bytes 写入 local object storage 临时 blob。
4. 在 SQLite transaction 中插入或更新 object metadata 与 version metadata。
5. 将临时 blob 提升为正式 blob；若同 ref 已存在且 bytes 相同，视为幂等成功；若 bytes 不同，返回 `conflict_object_version`。
6. 提交 SQLite transaction；若提交失败，删除刚提升的正式 blob。
7. 任何 metadata 失败或 hash / length mismatch 都必须清理 staged blob，不留下可达 metadata。

读取顺序：

1. 先读取 SQLite metadata 并检查访问权限。
2. 再按 `blob_ref` 读取 encrypted bytes。
3. 返回前可重新校验长度和 ciphertext hash；对象版本读取时按 metadata 重建 AAD 后复验。
4. 校验失败返回 `storage_unavailable`，并写入非敏感审计事件。

对象版本保留策略：

- 初期保留所有版本，优先保证离线设备能拉取历史冲突上下文。
- 后续版本 GC 必须有单独策略：至少保留 latest、最近 N 个版本和未被所有 active 设备确认的版本。
- GC 不能删除 `dictionary.deleted_terms` 的最新 tombstone 对象，也不能用服务端侧删除替代客户端加密 tombstone。

## 版本冲突与客户端合并

服务端只做乐观并发控制：

- `base_version == latest_version`：允许写入下一版本。
- `base_version < latest_version`：返回 409 和 latest metadata。
- `base_version > latest_version`：返回 `invalid_request`，说明客户端本地状态与服务端不一致。
- `version` 必须严格等于 `base_version + 1`。

冲突后的流程：

1. 客户端根据 409 响应拉取 latest encrypted bytes。
2. 客户端用本地 key 解密。
3. 客户端按 `ClientSyncMergeInput` 语义合并 user terms、deleted tombstones 和 ranker weights。
4. 客户端写回本地 userdb。
5. 客户端重新组装 encrypted object version 并上传。

服务端不得根据 `updated_at_ms`、`key_epoch` 或对象类型自行合并业务内容。`key_epoch` 只用于拒绝撤销后的旧 epoch 新写入和辅助客户端判断。

## 恢复与撤销边界

恢复记录：

- 服务端保存的是 signed recovery record 和 encrypted wrapped material。
- 恢复码输入、KDF、解包同步域材料和新设备激活都在客户端完成。
- 服务端可以限制 recovery record 读取频率，但攻击者一旦获得记录仍可能离线尝试恢复码；恢复码强度和 Argon2id 参数不能被服务端限速替代。
- 恢复记录创建、轮换、撤销和新设备恢复加入流程见 `docs/production-recovery-flow.md`。

设备撤销：

- 接受撤销记录后，服务端必须立即拒绝被撤销设备上传新对象、授权新设备或替换恢复记录。
- `current_key_epoch` 推进后，服务端拒绝低于当前 epoch 的新对象写入。
- 历史对象仍可存在；撤销前旧设备已取得的历史密钥无法被服务端追回。
- 后续如支持历史重加密，应作为独立客户端能力设计，不在服务端悄悄改写 ciphertext。

## 日志与运维

日志允许包含：

- request id
- route name
- domain id
- device id
- object id
- object type
- version
- encrypted byte length
- result code
- latency

日志禁止包含：

- 请求体或响应体。
- encrypted payload bytes、wrapped material bytes 或 signature bytes。
- 明文用户词、input code、reading、候选偏好、上下文、P1 事件、恢复码或本地文件路径中的敏感片段。
- 由 panic / stack trace 泄漏的请求 JSON。

当前 runtime audit logger 只消费 handler 产生的非敏感 `AuditEvent`，不会读取请求体、响应体、payload、signature、wrapped material 或 recovery material。发生 `storage_unavailable` 时，日志只记录 route、对象 metadata、长度和错误分类，不打印 payload；后续生产配置仍可增加日志级别和输出目标开关。

## 验证口径

后续 Go server 实现至少需要覆盖：

- API 层没有任何接收 plaintext user term、input code、reading、P1 event 或 ranker 明细的字段。
- 对象上传拒绝缺失 `ciphertext_hash`、空 `object_id`、非法 `object_type`、非法 `nonce`、0 payload、错误长度和 Rust envelope hash mismatch。
- 新对象必须使用 `version = 1` / `base_version = 0`；已有对象必须顺序递增。
- stale `base_version` 返回 409，且响应不包含 payload bytes。
- 同一 `object_id + version + ciphertext_hash` 重试幂等；同版本不同 hash 拒绝。
- revoked / pending / unknown device 不能上传对象、授权设备或替换恢复记录。
- 撤销后 `current_key_epoch` 推进，低于当前 epoch 的新对象写入被拒绝。
- signed object manifest、device authorization、device revocation 和 recovery record 验签失败时拒绝写入；Go storage conformance 已覆盖字段篡改失败路径。
- recovery record 读取和替换遵守限速与签名校验，不接受恢复码明文。
- SQLite transaction 失败时不留下可达 metadata；blob 写入失败时不提交 metadata。
- 对象读取时按 metadata 重建 AAD，校验 blob 长度和 Rust envelope ciphertext hash；不一致返回 `storage_unavailable`。
- device wrapping record 已覆盖 wrapped key bytes 的存储和读取测试；真实授权 handler 仍必须只返回密文 bytes 和服务端可见 metadata，不返回同步主密钥明文。
- recovery record 已覆盖 latest metadata 与 wrapped material bytes 一起读取；真实恢复 handler 的响应仍不得包含恢复码、KDF 输出或同步主密钥明文。
- 审计日志和错误响应不含请求体、payload bytes、wrapped material、恢复码或明文业务字段。
- `./scripts/check-repo.sh`、`go test ./...` 和后续 server smoke 均通过。

## 实施顺序建议

1. 已补 Go module、配置默认值、API request / error DTO、SQLite migration 文本、storage interface、storage conformance tests、内存 metadata store 和 local object storage staged transaction，字段按本文档命名。
2. 已补 SQLite-backed metadata repository，并把 metadata transaction 与 local object storage transaction 接起来，覆盖 blob 写入、metadata 提交、失败清理、读取 hash 复验和 conformance tests。
3. 已补 Go storage 签名验证抽象与测试，覆盖 object manifest、device authorization、device revocation 和 recovery record 的验签失败路径。
4. 已补 device wrapping wrapped key bytes 的承载方式和读取接口，继续走密文 bytes + hash / length 校验。
5. 已补 recovery wrapped material 的读取接口，继续走密文 bytes + hash / length 校验。
6. 已补 recovery latest metadata API handler，覆盖统一错误响应、恢复读取限速和内部 `blob_ref` 不外泄。
7. 已补 domain / device / join request metadata API，覆盖 domain 创建 / 读取、device 读取、pending join request 创建 / 列表和非法 JSON 错误响应。
8. 已补 API 层 request id、panic recovery 和非持久审计 hook，覆盖 request id header、结构化 panic error、审计事件不包含请求体字段。
9. 已补 authorization handler，把 signed authorization、wrapping metadata 和 encrypted wrapped key bytes 映射到 storage upload；storage conformance 覆盖授权后 pending join request 不再列出、设备激活和 wrapped key bytes 读取。
10. 已补 SQLite `audit_events` 写入，handler 会把非敏感审计事件映射到 storage audit model；测试覆盖 SQLite 行写入和 handler 自动调用持久审计 recorder。
11. 已补 encrypted object 上传下载和版本冲突 HTTP 语义，覆盖 metadata / payload 读取、Rust envelope hash / length mismatch、stale latest metadata、幂等重试、同版本不同 hash 冲突、设备状态门禁、plaintext 字段拒绝和 audit / error 脱敏。
12. 已补 runtime 装配和启动入口，覆盖 config env override、SQLite migration 嵌入与重复启动、local blob store 装配、HTTP timeout、对象大小门禁和脱敏 audit logger 测试；当前没有启动长期运行服务做真实联调。
13. 已补 `docs/runbooks/sync-server-local-smoke.md` 和短生命周期 HTTP smoke 测试，覆盖 domain 创建、第二设备 join / authorization、active 状态复验、跨设备 encrypted object 上传、metadata 读取、payload 下载、stale base version 冲突、v2 payload 读取和 runtime 日志脱敏。
14. 已补 Rust `ime-sync` remote client DTO / transport trait，客户端上传入口以 `AssembledSyncObject` 和 `SignedSyncObjectManifest` 为输入，生成 JSON metadata + base64 encrypted payload 请求，不接受 plaintext payload；测试覆盖 metadata / binary payload 读取、stale conflict latest metadata、server error code 映射、payload length mismatch 和请求 / 错误 debug 脱敏。
15. 已补 Rust `ime-sync` std-only `http://` `HttpSyncRemoteTransport`，复用 `SyncRemoteRequest` / `SyncRemoteResponse` 边界传递 JSON request 与 binary payload response；短生命周期 TCP 测试覆盖 upload request、metadata 读取、payload 下载、chunked response、stale conflict 错误映射、base path 拼接和 transport 错误脱敏。
16. 已补 Rust 侧两客户端 userdb 同步边界测试，覆盖设备 A 生成 P2 payload 并加密上传、设备 B 下载密文后解密 / 解码 / 合并写回 SQLite、本机 tombstone 阻断旧远端词条、stale base version 409 latest metadata 映射，以及 B 基于最新 base version 重新组装并上传 v2。
17. 已补 Rust `HttpSyncRemoteTransport` 直连 Go sync server 的短生命周期跨语言测试，覆盖 domain 初始化、Rust signed encrypted object 通过 Go HTTP API 上传、metadata / binary payload 读取、Go 服务端按 Rust envelope hash 复验，以及 stale conflict latest metadata 映射。

任何阶段都不应把 Flutter manager、平台壳、真实系统输入法服务或输入热路径接入 Go server。

## 停止线

- Rust 侧两客户端 harness 已覆盖 encrypted userdb payload 的上传、下载、解密、合并写回和 stale conflict 重新上传；Go runtime smoke 已覆盖第二设备授权和跨设备 object 版本链；Rust HTTP transport 直连 Go server 的短生命周期测试已覆盖跨语言 DTO、handler、storage、错误语义和日志脱敏边界。进入部署封装或用户可用同步前，仍必须扩展真实两客户端 HTTP 同步和平台私钥 backend 复验。
- 平台私钥存储 backend 能力模型已落地；真实平台 backend 验证未完成前，不提供用户可用同步 UI。
- device authorization handler 对外开放前必须继续复用 wrapped key bytes 的存储 / 读取语义，且不得返回明文同步域材料。
- recovery latest handler 已复用 wrapped material bytes 读取语义，并补齐限速与内部 `blob_ref` 不外泄测试；object version handler 已复用 encrypted object blob 读写语义，并补齐冲突、设备状态和脱敏测试；API handler 已补 panic recovery、request id、非持久审计 hook 和 SQLite `audit_events` 写入；runtime 已补配置装配、脱敏 audit logger、本机 smoke runbook 和双设备 HTTP smoke。Rust remote client 已补 DTO、transport trait、HTTP transport、错误映射、两客户端 userdb harness 和直连 Go server 的短生命周期测试；进入 Docker Compose、部署封装或真实用户同步前仍需单独确认运行边界。
- 服务端能保存、打印或索引明文用户词、input code、reading、P1 原始事件或候选偏好时，必须停止并回退该设计。
- 服务端版本冲突检测未稳定前，不允许客户端把本地合并结果自动上传到真实远端。
- 包分发、P3 资源下载和个人 P2 同步对象必须保持独立 API 与存储边界。
