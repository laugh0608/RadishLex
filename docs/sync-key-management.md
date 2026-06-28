# RadishLex 同步密钥与设备生命周期设计

本文档定义 RadishLex 进入真实同步前必须稳定的同步密钥、设备授权、恢复码、设备撤销、key epoch 和冲突边界。读者是后续实现 `ime-crypto`、`ime-sync`、Go sync server、管理 UI 同步页面和审阅隐私边界的开发者。本文不包含 HTTP API、Go server migration、Flutter 页面设计、平台输入法接入流程、完整恢复码 KDF 参数或生产密钥存储实现。

## 当前定位

当前已经完成：

- `ime-userdb` 可通过 Rust 内部只读迭代器生成 `dictionary.user_terms`、`ranker.weights` 和 `dictionary.deleted_terms` P2 plaintext payload。
- `ime-crypto` 已支持本地 object envelope、AAD、nonce、ciphertext hash、HKDF-SHA256 object key 派生和 XChaCha20Poly1305 加密 / 解密测试。
- `ime-sync` 已可从 `ime-crypto::EncryptedObjectEnvelope` 派生 `EncryptedSyncObjectDraft`。
- userdb P2 payload 已通过本地 integration test 进入 `ime-crypto` envelope，再派生 sync draft。
- `ime-crypto` 已补 device key descriptor、device wrapping key / record、recovery material 和撤销后新 `key_epoch` 加密边界测试。
- `ime-sync` 已补 `SyncDomain`、`SyncDevice`、`DeviceJoinRequest`、`DeviceAuthorizationPackage`、`DeviceRevocationRecord` 和 `SyncObjectVersion` 草案模型。

当前仍不做：

- 不连接 Go server，不定义远端 HTTP API。
- 不新增 CLI / FFI 明文同步 payload 入口。
- 不把 P1 原始选择事件、负反馈明细、上下文统计或本地审计批次纳入同步对象。
- 不推进平台壳、Flutter manager 或真实设备配对 UI。

下一步代码实现应继续补恢复码 KDF ADR、签名 / 设备密钥存储设计，以及客户端合并模型与真实 P2 payload / userdb 写回流程的接线，再进入后端 API。恢复码、签名 / 设备密钥存储和真实写回语义没有稳定前，不应启动真实远端同步主线。

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

- 设备加入同步域时生成的非对称密钥对。
- 公钥可以登记到服务端；私钥只在设备本地保存。
- 旧设备授权新设备时，使用新设备公钥包装同步密钥材料或设备包装密钥。

`DeviceWrappingKey`：

- 用于把同步域材料包装给某个授权设备。
- 每台授权设备应有独立包装记录，便于撤销。
- 包装记录只允许包含密文和必要元数据，不包含 plaintext sync key。

`RecoverySecret`：

- 从恢复码和恢复参数派生出的恢复材料。
- 用于在没有旧设备可用时恢复同步域材料。
- 恢复码属于低熵人工输入，具体 KDF 算法和参数进入实现前必须用 ADR 固化；本设计只固定职责和约束。

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
  public_key_id
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
- 恢复码 KDF 参数、格式、校验词和失败限速策略进入实现前必须写 ADR。

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
- 设备公钥和 key id
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
5. 已在 `ime-sync` 补客户端解密后合并模型和测试，覆盖 `dictionary.deleted_terms` tombstone 压过旧 user terms、旧 ranker weights、旧 epoch 上传和显式恢复语义；当前不解析真实 payload JSON、不写回 SQLite、不连接后端。
6. 已在 `ime-sync` 补 `SyncEnvelopeAssembler`，固定 Rust 内部 P2 payload 到 envelope 的组装边界，覆盖 sync master 派生 object key、nonce 复用阻断、draft 派生和 Debug 明文阻断。
7. 后续补恢复码 KDF ADR、签名 / 设备密钥存储设计，以及客户端合并模型与真实 userdb payload / 写回流程的接线。
8. 继续保持 userdb P2 payload 只作为 Rust 内部测试输入，不新增 CLI / FFI 明文 payload。
9. 恢复码、签名 / 设备密钥存储和真实 payload / userdb 写回接线稳定后，再设计 Go server API。

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
- 损坏的 envelope、非法 base version、未知设备状态和空 key id 必须返回明确错误。

## 停止线

- 恢复码 KDF 算法、参数和格式未通过 ADR 固化前，不实现生产恢复码。
- 生产恢复码 KDF、签名 / 设备密钥存储和历史重加密策略未固化前，不实现生产恢复流程。
- 冲突合并模型未接入真实 P2 payload 解析、userdb 写回和生产 envelope 组装边界前，不做远端上传下载。
- CLI / FFI 继续不得暴露 plaintext sync payload 或生产同步密钥材料。
