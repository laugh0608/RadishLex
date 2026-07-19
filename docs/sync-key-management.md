# RadishLex 同步密钥与设备生命周期设计

本文档定义 RadishLex 进入真实同步前必须稳定的同步密钥、设备授权、恢复码、设备撤销、key epoch 和冲突边界。读者是后续实现 `ime-crypto`、`ime-sync`、Go sync server、管理 UI 同步页面和审阅隐私边界的开发者。本文不包含 Go server migration、Flutter 页面设计、平台输入法接入流程、生产恢复码代码或生产密钥存储实现；Go server API 与 storage 边界见 `docs/sync-server-api-storage.md`，生产恢复流程见 `docs/production-recovery-flow.md`，平台私钥存储 backend 边界见 `docs/adr/0004-platform-private-key-storage-backend.md`。

## 当前定位

当前已经完成：

- `ime-userdb` 可通过 Rust 内部只读迭代器生成 `dictionary.user_terms`、`ranker.weights` 和 `dictionary.deleted_terms` P2 plaintext payload。
- `ime-crypto` 已支持本地 object envelope、AAD、nonce、ciphertext hash、HKDF-SHA256 object key 派生和 XChaCha20Poly1305 加密 / 解密测试。
- `ime-sync` 已可从 `ime-crypto::EncryptedObjectEnvelope` 派生 `EncryptedSyncObjectDraft`。
- userdb P2 payload 已通过本地 integration test 进入 `ime-crypto` envelope，再派生 sync draft。
- `ime-crypto` 已补 device key descriptor、device wrapping key / record、recovery material 和撤销后新 `key_epoch` 加密边界测试。
- `ime-crypto` 已补恢复码 KDF 纯 Rust 模型与测试，覆盖 `RLX1` 格式、Argon2id profile、恢复 wrapping key、恢复记录 AAD 和错误恢复码失败。
- `ime-sync` 已补 `SyncDomain`、`SyncDevice`、`DeviceJoinRequest`、`DeviceAuthorizationPackage`、`DeviceRevocationRecord` 和 `SyncObjectVersion` 草案模型。
- `docs/adr/0002-recovery-code-kdf.md` 已固定恢复码 KDF 采用 Argon2id、`RLX1` 格式、恢复记录字段、失败限速和验证口径。
- `docs/adr/0003-device-signing-key-storage.md` 已固定设备签名、签名对象、私钥存储抽象、错误语义和验证口径。
- `docs/production-recovery-flow.md` 已固定生产恢复记录创建、轮换、撤销、新设备恢复加入、全部设备丢失、失败限速和停止线。
- `docs/adr/0004-platform-private-key-storage-backend.md` 已固定平台私钥存储 backend、capability metadata、FFI 边界、错误语义、迁移和停止线。
- `docs/runbooks/apple-keychain-signing-backend.md` 已固定 `apple-keychain-v1` 首个平台 backend 验证边界，`docs/adr/0005-apple-platform-signing-strategy.md` 已固定 Apple 平台签名策略；`docs/runbooks/android-keystore-signing-backend.md` 已固定 `android-keystore-v1` 验证边界。
- `docs/adr/0006-device-signature-algorithm-profiles.md` 已接受 `ecdsa-p256-sha256-v1` 作为与 `ed25519-v1` 共存的生产候选，固定编码、canonical bytes、错误、迁移和 `apple-keychain-p256-v1` 边界。
- `docs/adr/0007-apple-secure-enclave-p256-backend.md` 与独立 runbook 已固定 Secure Enclave token、access control、key identity、迁移、资格字段和产品取证停止线。
- `ime-crypto` 已补算法无关设备验签、Ed25519/P-256 profile、`test-memory-v1` signing key store、platform capability metadata、unavailable 明确失败、revoked key 阻断、两个 feature-gated macOS Keychain backend、Android bridge、signed sync object manifest 和 signed recovery record；`ime-sync` 已补 signed device authorization 与 signed device revocation。
- `ime-userdb` 已补已解密 P2 JSON 到 merge input 的解析入口，并能把合并模型接受的 user terms、deleted tombstones 和 ranker weights 写回真实 SQLite。
- Go server storage / API / runtime 验证模型已保存设备与 join request 的显式 `signing_algorithm`、公钥、authorization、wrapping、revocation、recovery、object、非敏感 audit 和密文 blob；历史 schema migration 只把旧设备/join 行回填为 `ed25519-v1`，新请求不做算法默认。Rust/Go 共同读取同一签名 profile fixture。
- `ime-sync` 已补 remote object client DTO / transport trait 和 std-only `http://` HTTP transport，上传入口只接收 `AssembledSyncObject` 和 `SignedSyncObjectManifest`，不接受 plaintext payload。
- Rust 侧两客户端 userdb harness 已覆盖设备 A 生成 P2 payload 并加密上传、设备 B 下载二进制密文后解密 / 解码 / 合并写回 SQLite、stale conflict latest metadata 映射，以及基于最新 base version 重新上传 v2。
- Rust userdb 两客户端真实 Go HTTP 测试已覆盖设备 B join / signed authorization、三类 P2 对象真实 HTTP 上传下载、客户端解密 / 解码 / SQLite 写回、stale conflict latest metadata、按最新 `base_version` 上传 v2 和 runtime 日志脱敏。
- `ime-sync` 已落地默认关闭的通用 crypto processor/provider port：cycle snapshot 分离当前写 epoch 与历史可读 epoch，固定远端 signer/revocation sequence 决策，并在生产构造器执行 backend gate；双 userdb HTTP service 已复用该处理链，合成 backend 只允许显式测试路径。
- `docs/runbooks/sync-server-production-deployment.md` 已固定生产部署边界，覆盖外部 TLS、认证 / 访问控制、冷备份、恢复、升级回滚和真实用户开放停止线。

当前仍不做：

- 不新增 CLI / FFI 明文同步 payload 入口。
- 不把 P1 原始选择事件、负反馈明细、上下文统计或本地审计批次纳入同步对象。
- 不推进真实设备配对成功路径；M1/M2 平台输入与本地 manager 可独立推进，但不得调用真实同步或把平台签名 backend 标记为生产可用。

进入用户可用同步前，应按生产部署 runbook 补发布级目标部署运行证据。普通 DPK P-256 当前 `compiled/available/can_create/can_sign=true`，但 `exportable=true` 使产品资格保持关闭。独立 Secure Enclave backend 已取得 qualification lifecycle、denied 与设备锁屏 locked 证据，产品资格仍待真实 unsupported 环境；当前缺少该环境，因此只允许按 `docs/sync-orchestration.md` 推进关闭产品入口、合成 P2、测试 backend 的 Rust 编排，不开放真实产品签名路径。既有 Apple/Android Ed25519 失败结论继续有效。

## 设计目标

- 服务端默认不可信，不能通过账号密码、对象 ID、hash 或同步元数据解密或猜测用户词库。
- 新设备加入必须由已有设备授权，或由用户持有的恢复码解锁同步密钥。
- 设备撤销后，后续对象必须使用新 `key_epoch` 或新同步密钥材料，旧设备不能解密撤销后的新对象。
- 删除 tombstone 必须能随加密同步传播，旧设备、旧备份和旧权重摘要不能复活用户已删除词条。
- 真实上传前，`EncryptedSyncObjectDraft` 只能携带 envelope 元数据，不携带 plaintext bytes 或 encrypted payload bytes。
- 所有密钥、设备、对象版本和冲突语义都必须能用合成 fixture 测试，不依赖真实输入历史。

## 密钥角色

`ProfileRootKey`：

- 本设备本地保护的根材料。
- 用于解锁本机保存的同步材料或恢复流程。
- 不上传服务端，不通过 FFI 暴露。

`SyncMasterKey`：

- 用户同步域的主密钥材料。
- 用于派生对象加密密钥和设备包装密钥。
- 服务端永远不可见明文。
- 撤销设备后，后续对象应切换到新的 `SyncMasterKey` 或新的 key epoch。

`ObjectKey`：

- 按 `object_type + object_id + version + key_epoch` 从同步域材料派生。
- 只用于加密一个对象版本的 payload。
- 当前 `ime-crypto` 已有 object key descriptor 和 HKDF-SHA256 派生测试；后续要把 key epoch 与设备撤销流程绑定。

`DeviceKeyPair`：

- 设备加入同步域时生成的非对称密钥集合。
- 后续应拆分为 `DeviceSigningKey` 与 `DeviceKeyAgreementKey`，签名 key 只签对象和设备操作，key agreement / wrapping key 只用于包装同步域材料。
- 公钥可以登记到服务端；私钥只在设备本地保存。
- 旧设备授权新设备时，使用新设备 key agreement 公钥包装同步密钥材料或设备包装密钥；签名 key 不参与加密。

`DeviceWrappingKey`：

- 用于把同步域材料包装给某个授权设备。
- 每台授权设备应有独立包装记录，便于撤销。
- 包装记录只允许包含密文和必要元数据，不包含 plaintext sync key。

### macOS wrapped epoch material v1

M3 的首个产品材料 profile 固定为 `p256-ecdh-hkdf-sha256-xchacha20poly1305-v1`。设备签名 key 与 key-agreement key 必须使用不同的 key id、application tag 和平台 handle；签名 backend 不能被复用为 ECDH 私钥入口。`profile-sha256-v1` 已签入接收设备完整 key-agreement 公钥和 key id，因此本 profile 暂不增加可由服务端替换的算法字段，而是把 P-256、65-byte SEC1 uncompressed 公钥作为 v1 的唯一解释；后续增加其他曲线时必须升级 join profile 和 wrapped record schema，不能按长度猜测或尝试多算法回退。

wrapped record 的公开字段固定为：

```text
schema_version = 1
algorithm = p256-ecdh-hkdf-sha256-xchacha20poly1305-v1
domain_id
recipient_device_id
recipient_key_agreement_key_id
wrapping_key_id
key_epoch
nonce (24 bytes)
wrapped_key bytes
created_at_ms
```

`wrapped_key bytes` 是版本化 binary envelope：magic、envelope version、65-byte ephemeral P-256 public key 和 XChaCha20-Poly1305 ciphertext。明文固定为当前 epoch 的 `active_key_id` 与 32-byte `SyncMasterKey`，解析时拒绝未知 version、重复/空 key id、尾随 bytes 和非法长度。ECDH 只产生临时 shared secret；HKDF-SHA256 以独立 domain separator 和完整 record AAD 派生 32-byte AEAD key。AAD 至少绑定 schema/algorithm、domain、recipient device、recipient key-agreement key id、wrapping key id、epoch、nonce 和创建时间；任一字段变化必须解封失败。

平台边界固定如下：

- authorizer 使用一次性 ephemeral P-256 private key 为 recipient 公钥包装材料；ephemeral private key 只存在于本次调用并在析构时清零。
- recipient 的长期 key-agreement private key 由独立平台 backend 持有；macOS 产品候选使用独立 Secure Enclave P-256 key identity，并通过 `SecKeyCopyKeyExchangeResult` 返回 shared secret，Rust 随即派生/解封并清零临时 buffer。
- `SyncEpochMaterialStore` 只能读取已验证 lifecycle 中与本机 device/key id/public key 完全一致的 wrapped record；错误 domain、device、key id、公钥、epoch、算法、AAD、ciphertext 或 backend 状态一律失败关闭。
- SQLite 可以保存签名链验证后的公开 lifecycle 字段；若后续保存 wrapped record，也只能保存服务端同样可见的密文和公开 metadata。SQLite/settings/Manager state 永远不保存明文 master key、ECDH shared secret 或派生 wrapping key。
- 当前与历史 epoch 都按独立 record 解封；当前 epoch 必须存在且 `active_key_id` 匹配 domain。撤销设备的 trusted profile 在材料读取前阻断，因此不得请求或解封新 epoch；已持有的历史材料不承诺技术追回。
- locked、user-presence、denied、unavailable、unsupported、missing、corrupted 和 authentication failure 保持可区分的内部错误；编排层只映射为稳定脱敏分类，不拼接 key bytes、wrapped bytes、shared secret、nonce 或平台错误文本。

远端取得边界固定为精确 locator：`domain_id + recipient_device_id + key_epoch + wrapping_key_id`。Go metadata 必须持久化 record 创建时已签名绑定的 `recipient_key_agreement_key_id`，不能用读取时的设备目录值补写 AAD。客户端只能在已验证 lifecycle 给出 locator 后请求；HTTP transport 的设备声明必须等于 recipient，server storage 在读取 blob 前原子确认 recipient 仍为 active。单条 wrapped bytes 上限为 64 KiB；服务端响应通过长度/hash 检查后，Rust 仍要重新执行 schema、algorithm、AAD、hash 和 AEAD 验证。

远端响应先进入 userdb 的密文 cache transaction，再由产品材料 store 在后续 Rust cycle snapshot 中解封。重复取得相同 locator + 相同 bytes 幂等；相同 locator 出现不同 metadata/hash/bytes 视为 fork/tamper 并回滚，不能覆盖旧缓存。网络 I/O 不得发生在 SQLite transaction 内。

可信公开缓存从 schema v6 演进时必须把 `key_agreement_public_key_id` 与 `key_agreement_public_key` 作为结构化列原子保存，不能只依赖历史 `record_json`。这是重启后把平台 handle 绑定回已签名公钥的必要条件，不改变“公开缓存不保存 secret”的边界。

`RecoverySecret`：

- 从恢复码和恢复参数派生出的恢复材料。
- 用于在没有旧设备可用时恢复同步域材料。
- 恢复码 KDF 算法、参数、格式和恢复记录字段见 `docs/adr/0002-recovery-code-kdf.md`；本设计只固定同步域职责和设备生命周期约束。

## 同步域与设备状态

同步域至少需要这些概念：

```text
SyncDomain
  domain_id
  current_key_epoch
  active_key_id
  created_at_ms
  updated_at_ms
```

设备记录至少需要：

```text
SyncDevice
  device_id
  signing_algorithm
  signing_public_key_id
  signing_public_key
  key_agreement_public_key_id
  key_agreement_public_key
  status
  authorized_at_ms
  revoked_at_ms
  last_seen_at_ms
```

`status` 初期建议：

- `pending`：新设备已生成加入请求，但尚未被授权。
- `active`：可解密当前 epoch 后续对象。
- `revoked`：不得再获得新 epoch 材料。
- `lost`：用户声明设备丢失，行为上等同撤销，但 UI 可单独展示原因。

服务端可以保存设备 ID、公钥、状态和时间戳，但不能保存可解密用户数据的明文密钥。

当前 Rust 类型对应关系：

- `SyncDomain::advance_key_epoch` 只允许向前推进 `current_key_epoch`，并要求 `active_key_id` 非空、更新时间不早于创建时间。
- `SyncDevice::pending`、`activate` 和 `revoke` 固定 pending -> active -> revoked / lost 的状态转移；只有 `active` 设备可以接收后续 key epoch。
- `DeviceJoinRequest` 固定 `device_id`、`public_key_id`、一次性 `challenge`、用户核对 `short_code`、创建时间和过期时间，且过期时间必须晚于创建时间。
- `DeviceAuthorizationPackage` 要求授权设备和接收设备都为 `active`，并校验 `DeviceWrappingRecord.recipient_device_id` 与接收设备一致；该结构只保存 recipient / authorizer id、key epoch、wrapping key id、包装密文长度和创建时间，不复制包装密文本体。
- `DeviceRevocationRecord` 要求 `new_key_epoch` 大于 `previous_key_epoch`，用于表达撤销后续对象必须进入新 epoch。
- `SyncObjectVersion::needs_client_merge_against` 只判断 base version 是否落后于远端版本，用于触发客户端合并；它不是合并执行器。

Rust 与 Go 的包装记录边界：

- `ime-crypto::DeviceWrappingRecord` 持有 `encrypted_key` bytes，并在 Debug 中脱敏。
- `ime-sync::DeviceAuthorizationPackage` 与 `SignedDeviceAuthorization` 签 recipient、authorizer、join challenge / short code、key epoch、wrapping key id 和 `encrypted_key_len`，不复制密文本体。产品生命周期链要求 join challenge 使用 `profile-sha256-v1`，其摘要覆盖两组完整设备公钥、算法、key id、domain / join request id 和有效期；因此授权签名不能只依赖服务端可替换的 opaque key id。
- Go server 当前把 wrapped key bytes 作为密文 blob 保存，并在读取时复验长度和 ciphertext hash；authorization handler 只能提交 signed authorization、wrapping metadata 和 encrypted wrapped key bytes。
- 服务端永远不能从 wrapping record 推导 `SyncMasterKey`，也不能把 wrapped key bytes 写进日志、错误响应或审计 payload。

## 新设备加入

已有设备授权流程：

1. 新设备本地生成 `DeviceKeyPair`。
2. 新设备创建加入请求，包含 `device_id`、公钥、一次性挑战和用户可核对的短码。
3. 旧设备读取加入请求，并显示待授权设备信息。
4. 用户在旧设备确认授权。
5. 旧设备校验挑战后，为新设备公钥包装同步域材料或设备包装密钥。
6. 新设备取得包装密文后，在本地解开同步材料。
7. 新设备开始拉取密文对象，并只在本地解密。

规则：

- 服务端只能转发加入请求、公钥、包装密文和状态。
- 旧设备必须是 `active` 状态，且持有当前 epoch 材料，才能授权新设备。
- 加入请求应有过期时间和一次性挑战，避免旧请求被重复使用。
- 授权过程不得要求服务端账号密码直接解密用户数据。

恢复码流程：

1. 第一台设备初始化同步域时生成恢复码。
2. 用户离线保存恢复码。
3. 新设备输入恢复码，并从服务端拉取恢复参数和密文包装记录。
4. 新设备用恢复码派生 `RecoverySecret`。
5. 新设备解开同步域材料后，登记为新 `active` 设备。

规则：

- 恢复码只能用于恢复同步域材料，不能作为服务端登录密码。
- 恢复参数可以公开保存，但不得降低离线攻击成本到不可接受水平。
- 恢复码 KDF 参数、格式、校验段和失败限速策略已由 `docs/adr/0002-recovery-code-kdf.md` 固定；生产恢复记录创建、轮换、撤销和新设备恢复加入见 `docs/production-recovery-flow.md`。当前 Rust model / test 已覆盖格式解析、KDF 派生和恢复记录解密，管理 UI 和服务端 handler 仍未实现。

## 设备撤销与 key epoch

撤销设备时必须产生新的后续解密边界：

1. 用户在仍可信设备上选择撤销目标设备。
2. 客户端把目标设备标记为 `revoked` 或 `lost`。
3. 客户端创建新 `key_epoch`，并生成新的同步域材料或派生新 epoch 材料。
4. 后续上传对象使用新 `key_id` / `key_epoch`。
5. 只给仍 `active` 的设备写入新 epoch 包装记录。
6. 被撤销设备不能获得新 epoch 材料，因此不能解密撤销后的新对象。

历史对象策略：

- 默认可以先不重加密历史对象。
- 如果不重加密，管理 UI 必须说明：旧设备在撤销前已经取得的历史密钥无法被技术上追回。
- 后续可提供显式“重加密历史对象”操作，但这属于独立能力，需要单独设计验证和成本边界。
- 客户端必须区分“当前写入 epoch”和“允许解密的历史 epoch 集合”；前者只用于新对象，后者由本地可信 key lifecycle 状态显式提供，不能用 `remote.key_epoch != current_key_epoch` 一刀切历史对象。
- 不在允许集合中的对象返回 `key_epoch_rejected` 且不推进 cursor。远端 signer 是否因撤销而拒绝，必须结合撤销时点或 change sequence 判断；不得只看当前 `revoked` 标记就抹掉撤销前历史签名的可验证性。

删除对象策略：

- `dictionary.deleted_terms` 必须跟随当前 epoch 加密同步。
- 旧 epoch 上传的 `dictionary.user_terms` 或 `ranker.weights` 不得覆盖新 epoch 的删除 tombstone。
- 合并时删除意图优先于旧选择事件、旧导入和旧权重摘要。

## 对象版本与冲突

每个加密对象至少有：

```text
object_id
object_type
version
base_version
key_id
key_epoch
updated_at_ms
```

通用规则：

- `version` 从 1 开始递增。
- `base_version` 必须小于 `version`。
- 客户端离线写入时，可以基于最后已知 `base_version` 产生新版本。
- 服务端只做版本冲突检测，不解析 payload。
- 冲突合并必须在客户端解密后按对象类型执行。

对象合并方向：

- `dictionary.user_terms`：按 `input_code + text + reading` 合并；删除 tombstone、`key_epoch` 和更新时间参与判断，普通同步词条不能清除 tombstone。
- `dictionary.deleted_terms`：删除意图优先；较新的 tombstone 应阻止旧词条、旧导入和旧权重摘要复活。当前 `ime-sync` 已用纯 Rust 合成记录模型覆盖 tombstone 压过旧 user terms / ranker weights、旧 epoch 上传不能复活删除词和显式恢复清理 tombstone。
- `ranker.weights`：按 `input_code + text + reading + context_kind` 合并；active tombstone 阻断同一 term identity 下的旧摘要。显式恢复词条后，只有晚于恢复意图的权重摘要才可继续保留，不能回放 P1 明细。
- `settings.profile` / `settings.schema`：可以先采用 last-write-wins，后续管理 UI 再提供显式冲突提示。
- `backup.snapshot`：作为完整快照，不参与细粒度合并。

## 服务端可见元数据

服务端最多可见：

- `domain_id`
- `device_id`
- 设备 signing / key agreement 公钥和 key id
- `object_id`
- `object_type`
- `version`
- `base_version`
- `key_id`
- `key_epoch`
- `algorithm`
- `nonce`
- `encrypted_payload_len`
- `ciphertext_hash`
- `created_at_ms`
- `updated_at_ms`
- device authorization / revocation / recovery / object manifest signature
- wrapped key / recovery material / object payload 的密文长度、ciphertext hash 和 blob ref

服务端不得可见：

- `SyncMasterKey`、`ObjectKey`、`DeviceWrappingKey` 或恢复码明文。
- 明文用户词、input code、reading、候选偏好、上下文统计、原始选择事件、负反馈明细。
- plaintext payload hash。
- 可从公开 hash 反查用户词身份的 term identifier。

`object_id` 不得包含明文用户词、input code、reading 或上下文。需要稳定身份时，应放在 encrypted payload 内，或使用同步域密钥派生的 keyed identifier。

当前 Rust 草案和测试中的对象 ID、设备 ID、短码和密文长度均使用合成值。正式协议不得把这些 fixture 命名规则写入 Go server API 或平台壳绑定。

## Rust 实施顺序

1. 已在 `ime-crypto` 补 key epoch、device key descriptor、device wrapping key / record、recovery material 的纯模型和验证。
2. 已在 `ime-crypto` 测试撤销后新对象使用新 `key_epoch`，旧 epoch key 不能解密新对象。
3. 已在 `ime-sync` 补同步域、设备状态、加入请求、授权包、撤销记录和对象版本冲突草案模型。
4. 已在 `ime-sync` 测试 active / pending / revoked 设备状态转移、授权设备和接收设备都必须 active、撤销必须推进 key epoch、版本关系和 stale base version 检测边界。
5. 已在 `ime-sync` 补客户端解密后合并模型和测试，覆盖 `dictionary.deleted_terms` tombstone 压过旧 user terms、旧 ranker weights、旧 epoch 上传和显式恢复语义；该合并模型本身不解析 payload JSON、不连接后端。
6. 已在 `ime-sync` 补 `SyncEnvelopeAssembler`，固定 Rust 内部 P2 payload 到 envelope 的组装边界，覆盖 sync master 派生 object key、nonce 复用阻断、draft 派生和 Debug 明文阻断。
7. 已补 `docs/adr/0002-recovery-code-kdf.md`，固定恢复码 Argon2id KDF、格式、恢复记录字段、失败限速和验证口径。
8. 已按 ADR 落地恢复码 KDF 纯 Rust 模型与测试，覆盖 `RecoveryCode`、`RecoveryKdfProfile`、恢复 wrapping key 和 `RecoveryMaterial` 恢复记录加解密。
9. 已补 `docs/adr/0003-device-signing-key-storage.md`，固定设备签名、签名对象、canonical bytes、私钥存储抽象、错误语义和验证口径。
10. 已按 ADR 落地签名 / 设备密钥存储 Rust 模型，当前使用合成 `test-memory-v1` key store，并补 platform backend capability metadata、unavailable backend 明确失败和 revoked key 阻断测试。
11. 已补 `apple-keychain-v1` 平台 runbook 和 Apple 签名策略 ADR，固定 Apple Keychain 创建、加载、签名、删除、锁屏 / 权限、备份迁移、日志脱敏和策略停止线；macOS backend 已在 `apple-keychain` feature 下接线，默认测试不访问系统 Keychain，真实 smoke 已运行但阻塞于 `ed25519-v1` 创建，backend status 已阻断生产签名。
12. 已补 `android-keystore-v1` 平台 runbook、`android-keystore` feature、不可用状态门禁、Rust bridge wrapper、bridge contract、raw JNI glue、合成 bridge 单测、ignored smoke 入口、仓库内 Kotlin bridge source、Gradle harness、`@JvmStatic` facade、gated instrumented smoke、provider diagnostics、smoke 记录模板和设备矩阵记录，固定 Android Keystore Ed25519 创建 / 加载 / 签名 / 删除、锁屏 / 权限、备份迁移、IME 生命周期和日志脱敏验证边界；Android target build 已通过 `./scripts/check-android-target.sh` 复验 `radishlex-ime-crypto --features android-keystore --target aarch64-linux-android`；Android Gradle harness 已在 Pixel 9 Pro API 35 AVD 上执行真实 smoke 和 provider diagnostics，并在 Pixel 10 Pro API 37 AVD 上执行 provider diagnostics，结果均为 `unsupported_signature_algorithm`，不解除生产签名门禁。
13. 已补 ADR 0006、Rust/Go 算法分派、显式 `signing_algorithm` metadata/migration、共享跨语言 vectors 和独立 `apple-keychain-p256-v1`；普通测试不访问 Keychain，manager Release native 接线、严格 DPK 选择、固定错误/OSStatus、五场景 gated smoke、ad-hoc denied 与合格产品生命周期均有证据。评审结论为软件运行时可用、`exportable=true`、产品资格拒绝。
14. 已补 ADR 0007、独立 `apple-secure-enclave-p256-v1`、token/private-key-usage access control、不可导出 probe、Rust/FFI/manager native、六场景 gated smoke 和默认无外部状态产品构建门禁；qualification lifecycle、不可导出、hardware-backed、ad-hoc denied 与真实设备锁屏 locked 已有产品证据，当前等待真实 unsupported 环境和产品资格评审。
15. 已补真实 userdb P2 payload 解析到 merge input 的接线。
16. 已补客户端合并结果写回真实 userdb 的执行器。
17. 继续保持 userdb P2 payload 只作为 Rust 内部测试输入，不新增 CLI / FFI 明文 payload。
18. 已补 Go server API / storage 边界设计。
19. 已补生产恢复流程设计和平台私钥存储 backend ADR。
20. 已起步 Go server metadata / storage / API / runtime 验证模型，当前覆盖配置默认值、API request / error DTO、SQLite migration、storage interface、storage conformance tests、内存 storage、SQLite-backed metadata repository、local object storage staged transaction、签名验证、wrapped key bytes、recovery wrapped material、object version 上传下载、版本冲突、撤销设备阻断、非敏感 audit events 和隐私字段检查。
21. 已补 Rust remote object client DTO / transport trait 和 std-only `http://` HTTP transport，固定 encrypted object upload request、metadata 读取、binary payload 下载、stale conflict latest metadata、server error code 映射、真实 HTTP request / response 传递和 Debug 脱敏。
22. 已补 Rust 侧两客户端 userdb harness，覆盖 P2 payload 加密上传、另一客户端下载密文、解密、解码、合并写回、本机 tombstone 阻断旧远端词条、stale conflict latest metadata 映射和 v2 重新上传。
23. 已补 Rust userdb 两客户端真实 Go HTTP 测试，覆盖设备授权、三类 P2 对象上传下载、客户端解密写回、stale conflict、v2 重新上传和 runtime 日志脱敏。
24. 已按 `docs/sync-orchestration.md` 落地 change cursor、local repository port、transaction-scoped apply + cursor、持久化 journal/outbox、取消/重启、409 重新发现和默认关闭的通用 crypto processor/provider；合成双 userdb Go HTTP service 已覆盖历史 epoch、撤销 sequence、cycle snapshot 与新 outbox 轮换边界。
25. 已落地 `ProductSyncCryptoProvider` 与可信 device lifecycle、epoch material、platform signing 三端口；装载顺序先验证本机 active、registered public key 与 production backend，再读取 material。合成授权/撤销/轮换测试覆盖旧设备不读取新 epoch、历史对象、撤销 sequence、新 epoch outbox 和重启 snapshot 重建。
26. 下一批补完整设备目录/lifecycle sequence、signed authorization/revocation 的 Rust 验证与本地 public cache，再实现 wrapped epoch material 解封装和平台 adapter；明文 sync master key 不进入 SQLite/settings，不接 Manager 产品命令。

## 验证口径

进入真实同步前必须覆盖：

- P1 明细和本地审计来源不能进入 crypto payload。
- `object_id`、`ciphertext_hash` 和日志不能包含明文词条或 input code。
- 同一 key epoch 下 nonce 不重复。
- P2 envelope 组装必须从 sync master 派生 object key，并拒绝非 object key descriptor。
- AAD 任一字段变化会导致解密失败。
- 撤销设备后，新对象使用新 `key_epoch`，旧设备材料不能解密新对象。
- 只给 `active` 设备生成新 epoch 包装记录。
- 恢复码只能恢复同步域材料，不能绕过设备状态或直接解密服务端对象。
- `dictionary.deleted_terms` tombstone 能压过旧 user terms 和旧 ranker weights；旧 epoch 上传不能靠更晚本机时间复活删除词，显式恢复必须晚于 tombstone，且恢复前旧权重不随词条恢复一起复活。
- 已解密 userdb P2 payload 写回必须只应用被合并模型接受的记录，并在事务内覆盖 user terms、deleted tombstones、ranker weights、显式恢复清理和旧权重阻断。
- 损坏的 envelope、非法 base version、未知设备状态和空 key id 必须返回明确错误。
- Rust/Go 必须读取同一设备签名 profile fixture，并保持未知/不一致算法、非法公钥/签名编码、篡改 canonical bytes、签名不匹配和 revoked key 的固定负向语义；新设备 metadata 缺少 `signing_algorithm` 时不得默认 Ed25519。
- 平台私钥存储 backend unavailable 时，签名、授权、撤销和恢复记录轮换必须明确失败，不能回退到 `test-memory-v1`。
- 真实新设备加入入口必须持续覆盖 wrapped key bytes 密文存储 / 读取、hash / length 复验、authorization 签名验证和日志脱敏。

## 停止线

- 恢复码 KDF 算法、参数、格式、Rust model 和生产恢复流程设计已落地；服务端恢复记录 API 与管理 UI 未实现前，不提供用户可用恢复入口。
- 设备签名模型、两个签名 profile、跨语言 verifier/vectors、私钥存储抽象、平台 capability、Apple/Android runbook 与 feature-gated backend 已落地；普通 DPK 软件运行时已验证但可导出，Secure Enclave lifecycle、denied 与真实设备锁屏 locked 已验证，unsupported 和最终产品资格仍待真实环境证据，既有 Android/Apple Ed25519 阻塞也未解除。关闭态 Rust orchestration 可以独立推进，但任何局部 capability 或合成编排证据都不得开放用户可用远端对象上传下载。
- 服务端若回退到只保存 wrapping metadata 而不能保存 / 返回 wrapped key bytes，则不得开放真实设备授权 handler。
- Go server 与 Rust HTTP transport 继续推进时，必须先满足 `docs/sync-server-api-storage.md` 的签名、metadata API、版本冲突、错误语义和脱敏验证。
- CLI / FFI 继续不得暴露 plaintext sync payload 或生产同步密钥材料。
