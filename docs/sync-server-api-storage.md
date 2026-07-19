# RadishLex 同步服务端 API 与存储边界

本文档定义 Go sync server 实现期间必须稳定的 API、存储、错误语义和验证口径。读者是后续实现 `server/sync-server`、`ime-sync` 远端客户端、同步 runbook 和审阅隐私边界的开发者。本文不展开客户端产品状态机、Docker Compose 逐步操作、Flutter 同步页面、生产部署操作流程或生产平台私钥存储 backend；客户端 transaction/cursor/outbox 见 `docs/sync-orchestration.md`，Docker Compose runbook 见 `docs/runbooks/sync-server-compose.md`，生产部署边界见 `docs/runbooks/sync-server-production-deployment.md`，生产恢复流程见 `docs/production-recovery-flow.md`，平台私钥存储 backend 边界见 `docs/adr/0004-platform-private-key-storage-backend.md`。

## 当前定位

当前 Rust 侧已经完成 P2 payload 本地加密、设备授权 / 撤销签名、signed epoch distribution、recovery-record-v2 签名轮换/可信解封/recovered-device activation/signed revocation、客户端解密合并和 `ime-sync` remote client。Go server metadata schema v9 已具备 API/storage/runtime、SQLite metadata、local blob、签名 profile 验证、对象/lifecycle/epoch distribution/recovery activation/revocation、审计、备份恢复和升级回滚受控证据。当前部署子阶段以本地 Compose / HTTPS 通过为退出条件；目标生产证据后移到首个正式版本发布后。合格生产私钥 backend 尚未闭环，真实用户同步保持关闭。

本阶段只固定服务端 API 和 storage 边界：

- 服务端默认不可信，只保存密文对象、设备公钥、签名记录、版本和必要同步元数据。
- 服务端可以验证对象 metadata、ciphertext hash、设备状态、签名、版本冲突和存储完整性。
- 服务端不能解密、不能解析 plaintext payload、不能合并用户词、不能读取 P1 原始事件。
- 客户端仍是真相源：解密、冲突合并、删除 tombstone 语义、显式恢复和 userdb 写回都在客户端完成。

Go 代码必须继续受本文件约束 migration、handler 和测试命名。ADR 0006 后，server 已按显式 `signing_algorithm` 分派 `ed25519-v1` / `ecdsa-p256-sha256-v1` verifier，历史设备与 join request 只由 schema migration 回填 Ed25519，新 API 请求不做默认或算法猜测。进入真实用户可用同步前，仍必须保持签名验证、HTTP API handler、Go runtime smoke、Rust HTTP transport 直连 Go server、Rust 侧两客户端 harness、生产部署、错误语义、审计日志和平台 backend 验证彼此一致。

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

`devices`：`domain_id`、`device_id`、`signing_algorithm`、`signing_public_key_id`、`signing_public_key`、`key_agreement_public_key_id`、`key_agreement_public_key`、`status`、`authorized_at_ms`、`revoked_at_ms`、`last_seen_at_ms`。设备显示名如果后续需要展示，应作为用户可编辑的非敏感标签处理，不得从系统用户名、联系人或输入内容自动采集。

`device_join_requests`：`domain_id`、`join_request_id`、`device_id`、`signing_algorithm`、`signing_public_key_id`、`signing_public_key`、`key_agreement_public_key_id`、`key_agreement_public_key`、`challenge`、`created_at_ms`、`expires_at_ms`、`status`。

服务端只转发待授权设备的公钥、challenge 和状态。短码应由客户端根据加入请求内容本地计算和展示；授权提交时需要携带 signed authorization 中的 `join_short_code`，用于验签绑定用户确认过的短码。

进入可信 lifecycle cache 的加入请求必须使用 `profile-sha256-v1:<lowercase hex>` challenge。摘要输入为 `device_join_profile` canonical fields：`domain_id`、`join_request_id`、设备 ID、签名算法、签名 key id 与完整公钥 bytes、密钥协商 key id 与完整公钥 bytes、创建时间和过期时间。signed authorization 继续签入该 challenge，从而把接收设备的两组公开密钥和加入请求时限绑定进授权签名。任意旧式自由文本 challenge 可以继续用于历史测试或迁移读取，但 Rust 产品 lifecycle verifier 必须失败关闭，不能据此信任服务端返回的 recipient public key。

`device_authorizations`：`domain_id`、`join_request_id`、`authorizer_device_id`、`recipient_device_id`、`recipient_signing_public_key_id`、`recipient_key_agreement_key_id`、`join_short_code`、`key_epoch`、`created_at_ms`、`signature_schema_version`、`signature_algorithm`、`signature_key_id`、`signature`。

授权记录只证明某个 active 设备接受了待加入设备的公钥和指定 key epoch，不包含同步主密钥明文或恢复码。当前 Go storage 会在同一事务中把 join request 置为 active、写入授权记录、写入 wrapping metadata，并激活接收设备。

`device_wrapping_records`：`domain_id`、`recipient_device_id`、`recipient_key_agreement_key_id`、`authorizer_device_id`、`key_epoch`、`wrapping_key_id`、`algorithm`、`nonce`、`wrapped_key_len`、`ciphertext_hash`、`created_at_ms`、`signature_record_type`、`signature_schema_version`、`signature_algorithm`、`signature_key_id`、`signature`、`blob_ref`。

包装记录只保存给指定设备的包装密文元数据、签名和密文 blob ref，不保存 `SyncMasterKey`、`DeviceWrappingKey` 或恢复码明文。`recipient_key_agreement_key_id` 必须与已签名 authorization 和加入 profile 一致，使 HTTP 响应能够完整重建 wrapped epoch v1 AAD；不能在读取时从可变的当前设备 profile 猜测。当前 Go storage 已在授权事务中保存 / 读取 wrapped key bytes，并按 `wrapped_key_len` 与 `ciphertext_hash` 复验；该字段只能是密文 bytes。

`device_revocations`：`domain_id`、`revoked_device_id`、`revoker_device_id`、`previous_key_epoch`、`new_key_epoch`、`reason`、`created_at_ms`、`signature`。

撤销记录被接受后，服务端必须拒绝被撤销设备后续上传，并拒绝低于 `current_key_epoch` 的新对象版本写入。历史对象是否重加密由客户端和管理 UI 后续单独设计。

`domain_lifecycle_events`：`domain_id`、`lifecycle_sequence`、`event_type`、`record_id`、`reject_from_object_change_sequence`、`created_at_ms`。当前 event type 包含 `initial_device`、`device_authorized`、`device_revoked`、`recovery_record_rotated`、`device_recovered` 和 `recovery_record_revoked`。

`lifecycle_sequence` 是设备信任链的 domain 内严格递增序列，与加密对象的 `change_sequence` 属于不同命名空间。创建 domain、授权设备和撤销设备只在对应 metadata transaction 成功提交时追加生命周期事件；失败和回滚不得留下可发现事件。生命周期事件只引用第一设备 profile、signed authorization 或 signed revocation 的公开记录，不承载 wrapped key bytes、恢复材料或对象 payload。

撤销事务必须把当时 domain 内下一条对象序列记录为 `reject_from_object_change_sequence`。客户端把该值作为“从此对象序列开始拒绝被撤销设备签名”的不可回退顺序证据；signed revocation 仍负责证明撤销者、目标设备和 key epoch 变化。服务端不能仅凭 `devices.status` 建立客户端信任，客户端必须验证签名链，并把同一撤销记录对应的截点变化视为冲突。服务端恶意分叉或首次 bootstrap 欺骗不由单机 cursor 完全解决；当前边界通过本地已观察高水位禁止回退，后续跨设备透明度 / gossip 另行设计。

`recovery_records`：`record_schema_version`、`domain_id`、`recovery_record_id`、`previous_recovery_record_id`、`key_epoch`、KDF 参数、`salt`、envelope algorithm/nonce、wrapped length/hash、activation algorithm/key id/public key、`status`、`created_at_ms`、`updated_at_ms`、`revoked_at_ms`、signer/signature metadata 和 `blob_ref`。

恢复记录只保存加密后的同步域材料、KDF 参数和 activation 公钥。v2 新写入固定当前 epoch/profile，predecessor 必须指向当前 latest；旧 active 原子变为 `superseded`。v1 只保留迁移 metadata/blob，当前产品客户端拒绝直接解封或激活，必须由 active 设备轮换为 v2。服务端可以限速读取，但不能依赖限速替代恢复码强度。

`recovered_device_activations`：`domain_id`、`recovery_record_id`、`device_id`、完整 signing/key-agreement public profile、`key_epoch`、`created_at_ms`、activation signature schema/algorithm/key id/signature。

恢复激活只保存 possession proof 与新设备公开 profile，不保存恢复码、派生 activation private seed、同步主密钥或 ECDH shared secret。恢复记录消费、设备激活、activation 保存、完整 active cohort wrapped metadata 和 `device_recovered` lifecycle event 必须在一个 transaction 中提交。

`recovery_record_revocations`：`domain_id`、`recovery_record_id`、`revoker_device_id`、`key_epoch`、`reason`、`created_at_ms`、signature schema/algorithm/key id/signature。

恢复记录撤销只保存公开 signed decision。目标状态更新、revocation metadata 和 `recovery_record_revoked` lifecycle event 必须原子提交；不得删除 recovery wrapped blob，也不得让 unsigned bearer/header 请求改变恢复可用性。

`sync_objects`：`domain_id`、`object_id`、`object_type`、`latest_version`、`latest_ciphertext_hash`、`latest_key_epoch`、`latest_change_sequence`、`created_at_ms`、`updated_at_ms`。

`object_id` 必须是不含业务明文的 opaque ID。需要稳定 term identity 时，只能放在 encrypted payload 内，或使用客户端持有密钥派生的不可公开反查 ID。

`sync_object_versions`：`domain_id`、`object_id`、`version`、`base_version`、`change_sequence`、`owner_device_id`、`key_id`、`key_epoch`、`algorithm`、`nonce`、`encrypted_payload_len`、`ciphertext_hash`、`signature_schema_version`、`signature_algorithm`、`signature_key_id`、`signature`、`server_received_at_ms`、`client_created_at_ms`、`client_updated_at_ms`、`blob_ref`。

`change_sequence` 在 domain 内严格递增，只在新 object version 与 metadata transaction 成功提交时分配。同一 `object_id + version + ciphertext_hash` 幂等重放复用原 sequence；冲突、blob 写入失败和 transaction 回滚不产生可发现 entry。服务端按 sequence 分页列出 metadata；产品同步默认发现全部受支持 P2 类型。若提供 `object_type` 诊断过滤，opaque cursor 必须绑定相同过滤条件，不得与全量 cursor 混用。客户端时间和跨对象 version 不能替代权威 cursor。payload bytes 必须通过 blob 存储读取，不放入日志或错误响应。

`audit_events`：`domain_id`、`event_type`、`device_id`、`object_id`、`version`、`result_code`、`bytes`、`server_time_ms`。

审计记录不得保存请求体、响应体、密文包装材料、恢复码、plaintext payload 或错误堆栈中的敏感字段。

## 当前 Go storage surface

当前 `server/sync-server/internal/storage.Store` 是 HTTP handler 前的内部边界，已经落地 `CreateDomain`、`Domain`、`Device`、`LifecycleSnapshot`、`LifecycleEventsAfter`、`SaveJoinRequest`、`PendingJoinRequests`、`AuthorizeJoinRequest`、`PutEpochDistribution`、`DeviceWrappedKey`、`RevokeDevice`、`PutRecoveryRecord`、`LatestRecoveryRecord`、`LatestRecoveryWrappedMaterial`、`RecoverDevice`、`RevokeRecoveryRecord`、`PutObjectVersion`、`ObjectVersion` 和 `ObjectPayload`。

这组方法当前用于验证 metadata、设备状态、版本冲突、blob 写入和错误语义，不等同于完整产品入口。对象 change cursor、对象 discovery 分页和设备 lifecycle cursor 已落地；审计日志查询和持久限速器仍未落地。产品编排不得用 `updated_after_ms`、客户端时间或逐个猜测 object/version 代替 discovery。

当前 storage conformance 已覆盖设备状态、授权/撤销、完整 active cohort epoch distribution、对象版本冲突与 blob 完整性；recovery v2 另覆盖首次创建、原子轮换、恢复激活、signed revocation、精确重放、陈旧 predecessor、同 id 分叉、activation/revocation 互斥、revoked head 后续轮换和密文篡改。signed object、authorization、device/recovery revocation、distribution、recovery 与 activation 字段篡改均被验签拒绝。

当前 storage 已在写入前使用设备登记的 `signing_algorithm + signing_public_key` 验证 object manifest、device authorization、device revocation、epoch distribution、recovery record 和 recovery revocation；签名 canonical bytes 对齐 Rust `radishlex-signature-v1` length-prefixed field list，不在失败时尝试另一 verifier。HTTP API 已覆盖 domain/device/join、authorization、epoch distribution、recovery create/latest/activation/revoke 与 object version 路径；签名错误对外保持顶层 `invalid_signature`，并以脱敏 `error_detail` 区分算法、编码、验签和 key lifetime。runtime 使用 idempotent schema migration，并要求 metadata/blob 共享专用非 symlink leaf、目录 `0700`、SQLite `0600`；审计与日志不包含 request body、public key、signature、真实存储路径或 canonical bytes。

metadata schema 的算法迁移必须区分“历史兼容”与“新写入契约”：全新 schema 的 `devices.signing_algorithm` 和 `device_join_requests.signing_algorithm` 是无默认值的 `NOT NULL` 字段；旧数据库只允许迁移事务在增加缺失列时用 `ed25519-v1` 回填历史行，并记录 metadata schema version 2。迁移后 application/storage 仍必须为每个新 device/join 显式写入算法，不能依赖 SQLite 列默认值把缺失请求解释成 Ed25519；API DTO 缺失、空值或未知算法必须在进入持久化前失败关闭。

## HTTP API 边界

首批 API 使用 `/api/v1` 前缀。metadata 使用 JSON；当前对象上传使用 JSON `payload` byte 字段承载 encrypted bytes，Go JSON 编码下表现为 base64 字符串；对象 payload 下载接口返回 `application/octet-stream` 二进制密文。后续可以调整传输细节，但不能改变“metadata 可验证、payload 仍为密文”的边界。

### Device wrapped epoch 读取

设备材料读取固定为 `GET /api/v1/domains/{domain_id}/devices/{recipient_device_id}/wrapped-epochs/{key_epoch}?wrapping_key_id=...`。响应只包含 wrapped epoch v1 的公开 metadata 与 base64 密文，不包含 authorization short code、平台 handle、shared secret 或明文 key。

- 请求必须携带全局 bearer token 和 `X-RadishLex-Device-ID`；header device 必须与 route recipient 完全一致。该 header 是当前单用户自部署访问边界中的设备声明，不替代未来按设备认证，但可以阻止客户端误读其他 recipient 的记录。
- storage 必须在同一锁/事务观察点确认 recipient 当前为 `active`，再读取 wrapping metadata；revoked/lost/pending/missing device 返回 `forbidden_device`，且不得读取 wrapped blob。撤销前已经取得或缓存的历史材料无法追回。
- query 只接受单个非空 `wrapping_key_id`；epoch 必须为正整数。未知 query、重复参数、跨 recipient、错误 epoch/key id 与不存在记录失败关闭，不做“最新记录”猜测。
- wrapped bytes 固定上限为 64 KiB；授权写入、storage validation、HTTP 读取和 Rust response validation 使用同一上限。长度、裸密文 SHA-256、nonce、算法、recipient key id 或其他 AAD 字段不一致时不得缓存或解封。
- 审计只记录 route、domain、recipient、epoch、结果码和密文字节数，不记录 query value、nonce、ciphertext hash、wrapped bytes、signature 或平台错误文本。
- 精确读取同时承载 join authorization 产生的初始 record 与独立 `epoch_distribution` record。前者的信任来自已验证 lifecycle authorization；后者必须返回 `signature_record_type` 与完整 signature metadata，由客户端按 trusted distributor profile 复验后才能缓存。

### Signed epoch distribution 上传

轮换分发固定为 `POST /api/v1/domains/{domain_id}/epoch-distributions`。请求携带 `distributor_device_id`、目标 `key_epoch` 和最多 64 条独立 signed record；每条包含 recipient/key-agreement key id、wrapped epoch v1 metadata、wrapped bytes 与 `epoch_distribution` signature metadata。请求必须携带与 distributor 相同的 `X-RadishLex-Device-ID`，但 header 只作为访问声明，不能替代记录签名。

- `key_epoch` 必须等于事务中 domain `current_key_epoch`；distributor 和全部 recipients 必须在同一观察点为 active。recipient 集合必须无重复并精确等于当前 active 设备全集，因此 revoked/pending/lost 设备不能取得新 epoch，漏发某台 active 设备也不能形成已接受批次。
- 每条 `epoch_distribution` canonical bytes 完整签入 distributor/domain/recipient/recipient key-agreement key id/epoch/wrapping key id/algorithm/nonce/wrapped length/ciphertext hash/created time。server 使用 distributor 当前登记的 signing profile 验签，并复核 recipient 当前 key-agreement key id。
- 每条 wrapped bytes 上限 64 KiB，整批合计上限 4 MiB。所有 metadata、hash、签名、active cohort 与 blob staging 都通过后，metadata 在一个 transaction 中提交；任一记录失败不得产生可读取的部分批次。
- 精确整批重放返回成功并报告 `inserted_records=0`；既有 locator 的 metadata/hash/bytes 不同返回 `conflict_epoch_distribution`。混合“已精确接受 + 尚缺记录”的重试只补齐缺记录，但仍需请求覆盖完整 active cohort。
- runtime/audit 只记录 route、distributor、epoch、结果码、record count 和总密文字节数，不记录 recipient key id、wrapping key id、nonce、hash、signature 或 wrapped bytes。

### 单用户访问 token

首个生产访问控制方案固定为单用户自部署 bearer access token。它只证明请求进入了部署者控制的 sync server，不代表服务端账号体系，不替代设备签名、join request、object manifest 验签、恢复记录签名或客户端加密。

OIDC / Radish 产品账号体系接入已后置为未来专题，见 `docs/sync-server-oidc-roadmap.md`。当前 token 是单用户自部署的临时门禁，不是最终登录和授权体系；进入 OIDC 实现前，应先补认证策略 ADR，并把 Go handler 中的访问校验收敛为 `AccessAuthorizer` 之类的可插拔边界。

- Go server 通过 `RADISHLEX_SYNC_ACCESS_TOKEN` 配置访问 token；为空时认证门禁关闭，仅允许本地单元测试、短生命周期 smoke 或受控网络内的开发验证使用。
- 生产部署必须设置非空 token，长度至少 32 bytes，且不包含空白字符。token 必须由部署者随机生成，不能使用 domain id、device id、object id、恢复码、同步主密钥或平台私钥派生。
- 配置 token 后，所有 `/api/v1` 请求必须携带 `Authorization: Bearer <token>`。缺失、重复、格式错误或不匹配时返回 `401 unauthenticated`，不得进入业务 storage 读写。
- `HttpSyncRemoteTransport` 继续禁止 URL credentials、query token 和 fragment token；客户端如需访问启用 token 的 server，只能通过 transport 配置 bearer token 并发送 `Authorization` header。
- token 不是加密材料，不参与 envelope、object hash、设备授权或恢复记录；更换 token 不改变同步域密钥、设备状态、object version 或 tombstone 语义。
- API 响应、audit event、runtime log、Nginx access log、Docker log 和测试 fixture 不得打印 token、Authorization header、请求体、payload、signature、wrapped material 或恢复材料。

### 同步域

`POST /api/v1/domains`

- 用于单用户自部署初始化。
- 请求包含客户端生成的 `domain_id`、第一台设备公钥、初始 `key_epoch`、可选恢复记录 metadata 和签名 manifest。
- 服务端创建 domain 与第一台 `active` 设备。
- 不接收同步主密钥、恢复码明文或任何 userdb payload。

`GET /api/v1/domains/{domain_id}/state`

- 当前返回 domain metadata；设备公开信任链通过独立 lifecycle API 获取，不能从该响应中的服务端状态推导客户端信任。
- 不返回对象 payload；客户端需要按对象版本显式下载密文 bytes。

`GET /api/v1/domains/{domain_id}/lifecycle`

- 返回一致性 lifecycle snapshot：domain metadata、第一设备与当前设备公开 profile、完整 signed authorization / revocation 记录、对应事件序列和 snapshot cursor。
- 第一设备 profile 只能与客户端本地创建或恢复所得信任锚比对；服务端返回第一设备不能自行成为信任锚。
- 客户端必须按事件序列从信任锚归约状态，不能直接采用服务端 `devices.status`。
- snapshot 不返回 wrapped key、恢复材料、同步密钥、对象 payload 或 access token。

`GET /api/v1/domains/{domain_id}/lifecycle/events`

- 使用 domain-bound opaque `after_cursor` 与受限 `limit` 返回追加式生命周期事件。
- 响应按 `lifecycle_sequence` 严格升序，包含 `entries`、`next_cursor` 和 `has_more`。
- cursor 与对象 discovery cursor 类型隔离，跨 domain、跨 endpoint 或非法 cursor 必须失败关闭。
- 客户端在整页签名链验证和本地事务写入都成功后才能推进 cursor；任一记录失败时不得部分采用服务端状态。

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

- `active` 设备只提交 signed revocation、`previous_key_epoch` 与 `new_key_epoch`；新 epoch 包装记录必须通过独立 distribution endpoint 提交，不能混入撤销 canonical record。
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

- 使用 opaque `after_cursor` 和受限 `limit`，按 domain 内 `change_sequence` 升序列出已提交 object version metadata；产品同步默认不加类型过滤。
- 当前稳定接口不接受 `object_type` 或其他过滤参数；未知 query key 失败关闭。后续若增加过滤，cursor 必须绑定过滤条件并版本化。
- 响应包含 `entries`、`next_cursor` 和 `has_more`；不返回内部 SQL row id，也不要求客户端解析 cursor。
- 首次请求省略 `after_cursor`；非法、跨 domain 或不受支持的 cursor 返回结构化错误，不按客户端时间回退。
- `since_version` 只在单一 object 内有意义，`updated_after_ms` 受时钟和同时间戳分页影响，二者均不得作为产品增量同步 cursor。
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
- `SyncRemoteClient::discover_object_versions()` 调用 `GET /api/v1/domains/{domain_id}/objects`，只接受 opaque cursor 和有界 page limit。
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
- SQLite metadata schema v3 为历史 object version 按 domain、`server_received_at_ms`、`object_id`、`version` 确定性回填 `change_sequence`；迁移先补列和校验正序列，再创建唯一索引，避免旧表在补列前因索引引用新字段而失败。

客户端脱敏：

- Rust request / response / payload wrapper 的 `Debug` 不打印请求体、nonce、signature 或 payload bytes。
- 客户端错误对象不得保存原始 request body、response body、payload bytes、wrapped material 或 plaintext payload。
- 错误 message 只能用于开发诊断，不得拼接用户词、input code、reading、P1 event 或 ranker 明细。

### 恢复记录

`POST /api/v1/domains/{domain_id}/recovery-records`

- 创建或乐观轮换 signed recovery-record-v2；请求 body 的 signer 必须等于 transport device identity。
- 请求包含 recovery id/predecessor、固定 KDF/envelope profile、salt/nonce、wrapped material 长度/hash、activation public profile、时间和签名。
- 服务端验证当前 epoch、active signer、完整 v2 canonical signature 和密文；同记录精确重放幂等，旧 latest 与新记录在同一 transaction 切换。

`GET /api/v1/domains/{domain_id}/recovery-records/latest`

- 返回当前 active recovery record metadata 和 encrypted wrapped material。
- 服务端应对该接口做基于 domain、IP、设备和时间窗的限速；限速失败返回结构化错误。

`POST /api/v1/domains/{domain_id}/recovery-records/{recovery_record_id}/activation`

- 请求包含由恢复记录绑定的 activation key 签名的新设备完整 signing/key-agreement profile，以及由新设备 signing key 签名、精确覆盖事务后全部 active 设备的当前 epoch distribution。
- 服务端先验证 recovery record 是当前 active v2、activation possession proof、未登记的新 device/profile、当前 epoch、完整 cohort 与每条 distribution，再原子激活设备、消费记录、保存公开 activation/wrapped metadata 并追加 lifecycle；任一失败不得留下部分状态。
- 已消费、superseded、revoked 或未知 recovery record 不能再次激活设备；bearer token、device header 或 recovery record id 不能替代 activation signature。

`POST /api/v1/domains/{domain_id}/recovery-records/{recovery_record_id}/revoke`

- 请求包含 revoker device id、当前 key epoch、稳定 reason、created time 和完整 signature metadata；`X-RadishLex-Device-ID` 必须与 signed revoker 一致，但不能替代签名。
- canonical record type 固定为 `recovery_record_revocation`；服务端以 revoker 当前 active profile 验签，并要求 target 是当前 active chain head且绑定当前 epoch。
- 同一 transaction 保存 revocation、把 target 标为 `revoked` 并追加 `recovery_record_revoked` lifecycle；精确重放幂等，同 target 分叉或已经被 activation/rotation 消费返回 `conflict_recovery_record`。
- 不删除历史 metadata/blob，但后续 `latest` 不再返回 revoked 记录作为 active；rotation 可以严格承接 revoked chain head创建全新恢复记录，不能改变旧记录的 revoked 状态。

## 错误语义

错误响应使用稳定结构：

```text
error_code
error_detail
message
retryable
server_time_ms
latest_version
latest_ciphertext_hash
```

`error_detail` 是可选固定 allowlist，当前用于 `invalid_signature` 的算法、编码、验签与 key lifetime 分类；`message` 只能包含非敏感说明。两者都不得回显请求体、public key、signature、canonical bytes、payload、恢复码或明文业务字段。

首批错误码：

- `invalid_request`：字段缺失、格式错误、非法对象类型、非法 nonce / hash / 版本关系。
- `unauthenticated`：缺少自部署访问凭证或传输层认证失败。
- `forbidden_device`：设备不是 `active`、已撤销、join request 未授权或签名公钥不匹配。
- `not_found`：domain、device、object、version 或 recovery record 不存在。
- `conflict_stale_base_version`：上传基于旧版本，客户端必须拉取并合并。
- `conflict_object_version`：同一对象版本存在但 ciphertext hash 不一致。
- `conflict_epoch_distribution`：同一 recipient/epoch/wrapping key locator 已存在不同 metadata、hash 或 bytes。
- `conflict_recovery_record`：recovery predecessor 已过期，或同 recovery id 已存在不同 metadata、签名或 bytes。
- `invalid_signature`：对象 manifest、授权、撤销、epoch distribution 或恢复记录验签失败。
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
- discovery change sequence 只在成功新版本提交时递增；幂等重放复用原 sequence，失败/冲突不产生可见 gap 语义；分页必须稳定、无重复遗漏，并拒绝非法或跨 domain cursor。
- 同一 `object_id + version + ciphertext_hash` 重试幂等；同版本不同 hash 拒绝。
- revoked / pending / unknown device 不能上传对象、授权设备或替换恢复记录。
- 撤销后 `current_key_epoch` 推进，低于当前 epoch 的新对象写入被拒绝。
- epoch distribution 必须覆盖完整 active cohort；坏签名、错误 recipient key 或漏发时无可读取部分 metadata，精确批次重放幂等，locator 分叉返回专用冲突。
- signed object manifest、device authorization、device revocation 和 recovery record 验签失败时拒绝写入；Go storage conformance 已覆盖字段篡改失败路径。
- recovery record 读取和替换遵守限速与签名校验，不接受恢复码明文。
- SQLite transaction 失败时不留下可达 metadata；blob 写入失败时不提交 metadata。
- 对象读取时按 metadata 重建 AAD，校验 blob 长度和 Rust envelope ciphertext hash；不一致返回 `storage_unavailable`。
- device wrapping record 已覆盖 wrapped key bytes 的存储和读取测试；真实授权 handler 仍必须只返回密文 bytes 和服务端可见 metadata，不返回同步主密钥明文。
- recovery record 已覆盖 latest metadata 与 wrapped material bytes 一起读取；真实恢复 handler 的响应仍不得包含恢复码、KDF 输出或同步主密钥明文。
- 配置 `RADISHLEX_SYNC_ACCESS_TOKEN` 后，所有 API 请求缺失或错误 bearer token 时返回 `401 unauthenticated`，不得进入业务 storage 读写；响应、audit event 和 runtime log 不得泄漏 token。
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
12. 已补 runtime 装配和启动入口，覆盖 config env override、SQLite migration 嵌入与重复启动、专用非 symlink metadata/blob leaf、`0700/0600` 私有权限、local blob store、HTTP timeout、对象大小门禁和脱敏 audit logger；部署预演使用短生命周期容器，不保留长期运行服务。
13. 已补 `docs/runbooks/sync-server-local-smoke.md` 和短生命周期 HTTP smoke 测试，覆盖 domain 创建、第二设备 join / authorization、active 状态复验、跨设备 encrypted object 上传、metadata 读取、payload 下载、stale base version 冲突、v2 payload 读取和 runtime 日志脱敏。
14. 已补 Rust `ime-sync` remote client DTO / transport trait，客户端上传入口以 `AssembledSyncObject` 和 `SignedSyncObjectManifest` 为输入，生成 JSON metadata + base64 encrypted payload 请求，不接受 plaintext payload；测试覆盖 metadata / binary payload 读取、stale conflict latest metadata、server error code 映射、payload length mismatch 和请求 / 错误 debug 脱敏。
15. 已补 Rust `ime-sync` std-only `http://` `HttpSyncRemoteTransport`，复用 `SyncRemoteRequest` / `SyncRemoteResponse` 边界传递 JSON request 与 binary payload response；短生命周期 TCP 测试覆盖 upload request、metadata 读取、payload 下载、chunked response、stale conflict / unauthenticated 错误映射、base path 拼接、可选 bearer access token header 和 transport 错误脱敏。
16. 已补 Rust 侧两客户端 userdb 同步边界测试，覆盖设备 A 生成 P2 payload 并加密上传、设备 B 下载密文后解密 / 解码 / 合并写回 SQLite、本机 tombstone 阻断旧远端词条、stale base version 409 latest metadata 映射，以及 B 基于最新 base version 重新组装并上传 v2。
17. 已补 Rust `HttpSyncRemoteTransport` 直连 Go sync server 的短生命周期跨语言测试，覆盖 domain 初始化、Rust signed encrypted object 通过 Go HTTP API 上传、metadata / binary payload 读取、Go 服务端按 Rust envelope hash 复验，以及 stale conflict latest metadata 映射。
18. 已补 Docker Compose 本地 / 部署态入口、sync server Dockerfile、Docker build context ignore、本地 Caddy HTTPS 入口、部署态 HTTP 上游、Nginx 生产反代示例和 `docs/runbooks/sync-server-compose.md`；本地默认 `https://localhost:7319`，部署态默认同机 HTTP 上游 `http://127.0.0.1:7319`，两者使用同一个对外端口，并明确生产认证 / 备份 / 平台私钥 backend 未补齐前不得开放给真实用户。
19. 已补 Docker Compose 容器实际启动 smoke 证据；本地模式现由独立自动化门禁通过 Caddy internal TLS 到达 sync-server，验证 bearer `401` / authorized backend response、loopback-only、容器 hardening、日志脱敏和唯一 project 的 container/volume 清理。部署态 HTTP upstream 预演另覆盖私有权限、冷备份与隔离恢复；两种模式均不保留运行容器或真实数据。
20. 已补 Rust userdb 两客户端真实 Go HTTP 同步测试，覆盖设备 B join / signed authorization、`dictionary.user_terms` / `ranker.weights` / `dictionary.deleted_terms` 三类 P2 对象真实 HTTP 上传下载、客户端解密 / 解码 / SQLite 写回、stale conflict latest metadata、按 `base_version = 1` 上传 v2 和 runtime 日志脱敏。
21. 已补 `docs/runbooks/sync-server-production-deployment.md`，固定部署拓扑、外部 TLS、认证 / 访问控制、数据目录权限、冷备份、恢复、升级回滚、验证证据和真实用户开放停止线。
22. 已补单用户自部署 bearer access token 门禁，覆盖 Go handler 认证失败不进入业务 storage、runtime 配置透传、日志脱敏、Rust HTTP transport 可选 `Authorization: Bearer` header 和 `unauthenticated` 错误映射；发布级目标部署运行证据和可用平台私钥 backend 仍是用户可用同步停止线，但不阻塞当前本地联调和非上传 UI gate 开发。
23. 已补 runtime 备份恢复 smoke，覆盖短生命周期 Go server 写入 domain、第二设备授权、三类 P2 encrypted object、recovery record 和审计事件，停止后复制 SQLite metadata 与 encrypted blob dir 到备份目录，再恢复到隔离目录并重启验证 domain / device / recovery latest / object payload / stale conflict 与日志脱敏。
24. 已补 runtime 外部 TLS 反代 smoke，覆盖 HTTPS client、TLS 1.2+、TLS reverse proxy 到 HTTP upstream、`Authorization` header 透传、`X-Forwarded-Proto=https`、Go bearer token 门禁、encrypted object 上传下载、Go 对象大小门禁和日志脱敏。
25. 已补 runtime 升级回滚 smoke，覆盖升级前数据写入、关闭后冷备份、同一数据目录重启触发 idempotent migration、升级后 v2 写入、恢复升级前备份到隔离目录、确认 v2 不可见、v1 payload / stale conflict 仍按 latest metadata 返回，以及日志不泄漏 payload、signature、wrapped material 或恢复敏感字段。
26. 已补 ADR 0006 对应的算法分派、设备/join `signing_algorithm` API/SQLite metadata、历史 Ed25519 migration、稳定 `error_detail` 和 Rust/Go 共享正负向 vectors；新请求缺少算法或传入未知算法时失败关闭。
27. 已按 `docs/sync-orchestration.md` 增加 domain 内 change sequence、opaque cursor discovery storage/API、Rust remote DTO 和分页/幂等/非法 cursor 测试；对象增量同步不能用时间戳过滤替代。
28. 已补独立 lifecycle sequence、snapshot / events API、设备 revocation API、Rust trust-anchor signed chain verifier、`profile-sha256-v1` 公钥绑定 challenge 和 userdb schema v6 public cache；两个文件 userdb 已在短生命周期 Go HTTP 中完成授权、同步、撤销、缓存和重启恢复。
29. Rust userdb schema 已升至 v7，结构化缓存签名链绑定的 key-agreement key id/public key，并只保存版本化 wrapped epoch ciphertext；`ProductWrappedEpochMaterialStore` 与 Apple signing/key-agreement adapter 已落地。
30. Go metadata schema v5 已结构化保存 wrapping record 的 recipient key-agreement key id；精确 wrapped epoch GET handler、transport device identity、active-before-blob storage 门禁、64 KiB 上限、Rust remote source与 userdb 幂等/fork cache 已接入双文件 Go HTTP 授权/轮换/撤销/重启证据。
31. Go metadata schema v6 已追加 wrapping signature source/profile，独立 signed epoch distribution endpoint 与 storage transaction 已覆盖完整 active cohort、64 条/64 KiB/4 MiB 上限、坏签名无部分可读、精确重放幂等和 locator 分叉冲突；Rust remote client 在上传前验证 trusted lifecycle，在下载后复验 distributor signature。A/B/C Go HTTP 证据覆盖 B 撤销后 A/C 取得 epoch 2、历史 epoch 与 userdb/provider 重启恢复。
32. Go metadata schema v8 与 userdb schema v8 已落地 recovered-device activation transaction、`recovery_record_rotated` / `device_recovered` 公开 lifecycle、完整 active cohort 当前 epoch 分发和重启恢复。
33. Go metadata schema v9 与 userdb schema v9 已落地 signed recovery record revocation、`recovery_record_revoked` 公开 lifecycle、exact replay/分叉、activation/rotation 线性化、revoked 历史状态保持和 Rust/Go HTTP/userdb 重启证据。

任何阶段都不应把 Flutter manager、平台壳、真实系统输入法服务或输入热路径接入 Go server。

## 停止线

- Rust 侧两客户端 harness 已覆盖 encrypted userdb payload 的上传、下载、解密、合并写回和 stale conflict 重新上传；Go runtime smoke 已覆盖第二设备授权、跨设备 object 版本链、备份恢复链路、外部 TLS 反代链路和升级回滚链路；Rust HTTP transport 直连 Go server 的短生命周期测试已覆盖跨语言 DTO、handler、storage、错误语义和日志脱敏边界；Rust userdb 两客户端真实 Go HTTP 测试已覆盖客户端解密合并写回和 v2 重新上传。当前部署子阶段以真实本地 Compose / HTTPS 和部署预演通过退出；进入用户可用生产同步前仍必须补可用平台私钥 backend，目标部署证据按产品决策在首个正式版本发布后补齐。
- 平台私钥存储 backend 能力模型已落地；普通 DPK 软件 key 因 `exportable=true` 不具备产品资格，独立 Secure Enclave backend 已取得 qualification lifecycle、denied 与设备锁屏 locked 证据但仍待 unsupported 和产品资格评审。生产 backend 评审通过前不提供用户可用同步 UI。
- device authorization handler 对外开放前必须继续复用 wrapped key bytes 的存储 / 读取语义，且不得返回明文同步域材料。
- recovery latest handler 已复用 wrapped material bytes 读取语义，并补齐限速与内部 `blob_ref` 不外泄测试；object version handler 已复用 encrypted object blob 读写语义，并补齐冲突、设备状态和脱敏测试；API handler 已补 panic recovery、request id、非持久审计 hook、SQLite `audit_events` 写入和 bearer access token 门禁；runtime 已补配置装配、脱敏 audit logger、本机 smoke runbook、双设备 HTTP smoke、备份恢复 smoke、外部 TLS 反代 smoke、升级回滚 smoke、Docker Compose 本地 / 部署态入口、容器实际启动 smoke 证据和生产部署边界 runbook。Rust remote client 已补 DTO、transport trait、HTTP transport、错误映射、可选 bearer token header、两客户端 userdb harness、直连 Go server 的短生命周期测试和 userdb 两客户端真实 Go HTTP 测试；进入真实用户部署前仍需补可用平台私钥 backend 和发布级目标部署运行证据，进入 manager 同步入口非上传开发可先依赖本地联调证据。
- 服务端能保存、打印或索引明文用户词、input code、reading、P1 原始事件或候选偏好时，必须停止并回退该设计。
- 服务端版本冲突检测未稳定前，不允许客户端把本地合并结果自动上传到真实远端。
- change cursor discovery、客户端原子 apply + cursor 和 crash-safe outbox 未稳定前，不允许把现有逐对象测试 harness 包装成产品 `sync_once`。
- 包分发、P3 资源下载和个人 P2 同步对象必须保持独立 API 与存储边界。
