# RadishLex 产品同步编排边界

本文档定义 M3 产品同步周期在 Rust 客户端中的职责、状态机、持久化、事务、cursor、冲突重试和失败恢复边界。读者是 `ime-sync`、`ime-userdb`、`ime-crypto`、Go sync server、FFI 与 Manager 的实现者和审阅者。本文不重复 payload 字段、密码算法、设备授权协议、HTTP 部署步骤或 Flutter 页面设计；这些内容分别以 `docs/sync-payload.md`、`docs/crypto-boundary.md`、`docs/sync-key-management.md`、`docs/sync-server-api-storage.md` 和 `docs/manager-sync-entry-boundary.md` 为准。

## 当前决策

- 产品同步的状态与执行真相源固定在 Rust；Flutter、平台壳和 Go server 不复制编排、合并、cursor 或密钥策略。
- `apple-secure-enclave-p256-v1` 已按一个受支持 macOS 设备的真实 signing/key-agreement 主路径完成产品资格评审；无 Secure Enclave 环境的 unsupported 改为延期兼容性补测，不得在支持设备上模拟。
- backend `product_qualified` 只表达已评审的平台私钥能力，不自动开放用户同步。
- 在 Rust 编排、两个真实客户端、设备生命周期和产品入口边界全部闭环前，`user_sync_enabled` 保持关闭；发布级正式域名/证书/目标部署证据按产品决策后移到首版发布后。
- 关闭态 `sync_once`、稳定 discovery cursor、持久化 journal/outbox、默认关闭的通用 crypto processor/provider、可信 lifecycle/wrapped material 产品装载、signed epoch distribution remote 边界和严格 HTTPS transport 已经落地；恢复链、macOS backend 主路径、本地部署与双客户端证据也已闭合。剩余产品缺口是受控 Manager 资格执行链与真实用户入口退出评审。

## 职责与依赖方向

调用方向固定为：

```text
Manager（后续）
  -> narrow FFI command/status（后续）
     -> ime-sync orchestration service
        -> local repository port
        -> ime-crypto encryption/signing boundary
        -> SyncRemoteClient / SyncRemoteTransport

ime-userdb
  -> implements local repository port
  -> owns SQLite transaction, cursor, journal and outbox

Go sync server
  -> owns opaque metadata ordering and encrypted object storage
  -> never decrypts, merges or decides local cursor commit
```

约束：

- `ime-sync` 不反向依赖具体 SQLite schema。中立的 local repository port 和编排领域类型定义在 `ime-sync`；`ime-userdb` 提供实现。
- `ime-userdb` 不执行 HTTP，不持有平台私钥 backend，也不自行解释服务端错误。
- `ime-crypto` 不依赖 HTTP DTO、SQLite 或 Flutter 状态。
- 网络请求不得在 SQLite transaction 持锁期间执行。
- Manager 只消费稳定 phase、结果计数和脱敏错误；它不能读取或持久化 cursor、outbox、payload、signature 或 key material。

### Crypto processor 的可信输入边界

`SyncObjectProcessor` 是 orchestration 与密码/设备策略之间的 port，不是测试密钥容器。当前 `DefaultSyncObjectProcessor` 通过 `SyncCryptoProvider::freeze_cycle()` 在 cycle 开始时取得不可变可信快照，包含本机 signing handle/public key、当前写入 epoch、允许解密的 epoch material、远端 signing profile/public key，以及 provider 基于撤销时间或 change sequence 给出的对象接受阈值。

- 当前写入 epoch 只决定新 outbox 使用哪个 key descriptor，不等于只允许读取该 epoch。未重加密的合法历史对象可以继续属于允许解密集合。
- epoch 不在允许集合时返回稳定 `key_epoch_rejected`；不得尝试其他 key、降级到旧 master key 或改用测试 backend。
- 被策略判定为撤销后产生或不再可信的 signer 返回 `revoked_device`。设备当前为 revoked 不应脱离撤销时点而自动否定所有撤销前历史对象；该时间/sequence 判断由可信设备生命周期 provider 完成，processor 不自行猜测。
- signing algorithm、public key id、key epoch 和 device acceptance 必须显式匹配；任何 mismatch 都失败关闭，不做跨 profile fallback。
- 合成 provider 只能通过显式测试构造器进入，`test-memory-v1` 不得被产品构造器接受或作为 fallback。产品 provider 不得从 Manager settings、Flutter state 或普通文件读取私钥和 sync master key。

通用 processor 已实现 manifest 验签、epoch material 选择、AEAD 解密、新 outbox 加密签名和 production gate，并接入双 userdb HTTP service 门禁。`ProductSyncCryptoProvider` 进一步固定三个产品端口：`SyncTrustedDeviceSource` 只提供已验证的 domain/device public lifecycle，`SyncEpochMaterialStore` 按本机 device 授权返回当前与历史 secret material，`SyncDeviceSigningBackend` 只通过不可导出 handle 签名。装载顺序必须先确认本机 active 和 backend 产品资格，再读取 epoch material；任一不匹配都在网络前失败关闭。

产品 adapter 已具备 verified lifecycle public cache、wrapped epoch ciphertext cache、独立 key-agreement 解封和 Apple signing adapter。远端取得链固定为“transaction 外按 verified lifecycle locator 精确下载 -> Rust 校验 signature source、公开 metadata/长度/hash与 distributor 签名 -> userdb transaction 幂等缓存密文 -> 后续 preflight 从本地 cache 解封”；网络 I/O 不得发生在 SQLite transaction 内。相同 locator 的不同响应必须作为 fork/tamper 失败，撤销设备必须在 server blob 读取和本地材料读取前分别阻断。具体 Secure Enclave key-agreement backend 仍只有编译与合成协议证据，不能继承 signing backend 的实机资格。

撤销或轮换产生新 epoch 后，distributor 必须先为同一 verified lifecycle 中的完整 active cohort 生成并签名 `epoch_distribution` batch。server 只有在 cohort、recipient key、签名、长度/hash 和全部 blob staging 均通过后才原子提交可读取 metadata；客户端取得 accepted response 后，后续 cycle 才能准备使用该 epoch 的新对象 outbox。坏签名、漏发、分叉或失败批次不得通过“先上传对象、稍后补材料”绕过；精确重放允许幂等恢复。

## 一次同步周期

稳定阶段按以下顺序执行：

```text
preflight
  -> discover
  -> download
  -> verify
  -> decrypt_and_decode
  -> apply_and_advance_cursor
  -> plan_upload
  -> prepare_signed_outbox
  -> upload
  -> acknowledge_outbox
  -> complete
```

### 1. Preflight

进入网络前必须同时确认：

- 当前不是 P0/隐私模式阻断状态；输入热路径不参与本调用。
- domain、device、endpoint、访问凭据状态、当前 key epoch 和 signing backend 状态可用。
- device 为 `active`，本地 schema 和 orchestration state version 受支持。
- 同一 domain 没有另一同步周期持有执行租约；崩溃遗留租约必须按明确过期规则恢复，不能永久锁死。
- 测试 backend 只能在测试构造器和合成数据环境使用；产品构造器必须执行 production signing gate。

Preflight 只能返回计数、状态和阻塞原因，不返回明文 P2、P1 明细、token、路径或平台错误正文。

### 2. Discover

客户端使用服务端单调递增、按 domain 隔离的 opaque change cursor 发现已提交对象版本。cursor 不使用客户端时间、`updated_after_ms` 或跨对象 `version` 拼接替代。

服务端 discovery 必须满足：

- 每个成功提交的新 object version 获得唯一、严格递增的 `change_sequence`；幂等重放不产生新 sequence，冲突和失败写入也不产生 sequence。
- 分页结果按 `change_sequence` 升序稳定返回 object metadata，不返回 payload bytes。
- 请求携带 `after_cursor` 和受限 `limit`；响应携带本页 entries、`next_cursor` 和 `has_more`。
- cursor 只表示服务端已提交 metadata 的读取位置，不表示客户端已经验签、解密或应用。
- 未识别、跨 domain、过期或格式非法的 cursor 必须返回结构化错误；客户端不得回退到本机时间猜测位置。

服务端可以在内部用 sequence 编码 cursor，但对 Manager 和日志只暴露“已配置/版本/是否推进”等非敏感摘要。

### 3. Download、Verify、Decrypt 和 Decode

每个 discovery entry 按以下顺序处理：

1. 校验 metadata schema、对象类型、domain/device、版本关系、key epoch、长度和资源上限。
2. 根据设备登记的显式 `signing_algorithm + signing_public_key` 验证 manifest；失败时不得尝试另一算法。
3. 下载 encrypted payload，复验响应长度与 `ciphertext_hash`。
4. 校验 signer 为 active 且未撤销，key epoch 可被当前设备接受。
5. 使用 Rust 内部 object key 解密并验证 AAD。
6. 严格解码对应 P2 schema；拒绝未知字段、错误 object type、越界值和 P1/本地审计内容。

以上步骤在本地 transaction 外执行，但所有未提交 plaintext 只存在于一次 Rust 调用的受控生命周期中；不得进入 Debug、日志、FFI、临时文件、Manager state 或 crash diagnostics。

任一步失败都不得推进 cursor，也不得把同页后续 entry 视为已应用。实现可以重新下载整页或保存不含 plaintext 的待处理 metadata，但不能持久化解密后的 payload 作为恢复捷径。

### 4. Apply 与 Cursor 原子提交

下载页完成验证和解码后，`ime-userdb` 必须在同一个 `BEGIN IMMEDIATE` transaction 内：

1. 重新读取当前本地 tombstone、explicit restore、term 和 ranker state，避免网络期间的本地写入被旧 snapshot 覆盖。
2. 执行确定性 merge，只应用被 merge 模型接受且确实改变本地状态的记录。
3. 记录每个 remote object 的已观察 version、hash、owner device、key epoch 和 change cursor。
4. 对实际改变的 P2 本地对象增加 local revision/dirty state，使合并结果可在后续上传收敛；无变化的幂等重放不能制造新 dirty revision。
5. 将 domain cursor 更新为本页 `next_cursor`。
6. 提交 transaction。

任何写入、约束、磁盘或 busy 错误都必须回滚 payload apply、remote observation、dirty state 和 cursor。现有 `apply_decoded_sync_payload_batch()` 自身提交 transaction，不能在其外层再补一次 cursor 写入冒充原子性；实现时应提取 transaction-scoped 内部入口，再由现有 API 与 orchestration adapter 复用。

### 5. Outbound Plan 与持久化 Outbox

本地可同步状态使用每个 domain/object 的单调 local revision 表达。词条、tombstone、权重或已接受远端合并只有在真实改变 P2 当前状态时才增加 revision；P1 原始事件和本地 import batch 不得触发 P2 object。

准备上传时必须：

- 从同一只读 snapshot 取得 `local_revision + plaintext payload`，并读取已观察 remote latest version 作为 base version。
- 在 Rust 内派生 object key、生成新 nonce、加密、构建 manifest 并调用当前设备 signing backend。
- 在发送网络请求前持久化完整的 prepared outbox：目标 domain/object、local revision、version/base version、encrypted envelope、manifest 和 signature。
- outbox 不保存 plaintext payload、sync master/object key、设备私钥、canonical bytes、token 或恢复材料。

持久化 prepared outbox 是 crash-safe 幂等上传的必要条件。如果服务端已提交上传而客户端在本地 ack 前崩溃，重启必须重放完全相同的 version、nonce、ciphertext hash、manifest 和 signature；不得重新加密同一 version 生成不同 hash。

若网络准备期间本地 revision 再次变化，已准备对象仍代表一个合法历史 snapshot；上传成功后只确认该 revision，更新后的 revision 继续保持 dirty，不能被错误清除。

### 6. Upload、409 Conflict 与 Ack

上传结果分为：

- success 或相同 `object_id + version + ciphertext_hash` 的幂等成功：在本地 transaction 中确认 outbox、更新 remote observation；仅当当前 local revision 等于已上传 revision 时清除 dirty。
- `conflict_stale_base_version`：不得修改请求后盲目重试。丢弃或标记 superseded 的 prepared outbox，回到 discover，下载 latest metadata/payload，完成验签、解密、merge 和 cursor transaction，再基于新 base version 准备新的 signed outbox。
- `conflict_object_version`：同 version 不同 hash 是安全/一致性错误，不视为幂等成功，停止当前对象并要求诊断。
- revoked、旧 epoch、签名或认证错误：失败关闭，不自动切换 backend、device 或 epoch。
- retryable transport/storage 错误：只重放同一个 prepared outbox，使用有界指数退避、抖动和最大次数；取消或重启后仍由 journal 恢复。

409 重试必须有上限。达到上限后保留可恢复 dirty/outbox 状态并返回结构化 `conflict_retry_exhausted`，不能覆盖远端或丢弃本地意图。

## 持久化状态

具体 SQLite 表名由实现批次固定，但语义至少包括：

- domain sync state：schema version、opaque cursor、当前已知 key epoch、最近成功时间。
- remote object observation：object identity、latest version/hash、owner device、key epoch、change cursor。
- local object revision：当前 revision、最后成功上传 revision、dirty 状态。
- prepared outbox：不可变请求 identity、version/base version、encrypted bytes、manifest/signature、尝试次数和最后脱敏错误类别。
- cycle journal/lease：当前 phase、开始时间、取消请求和 crash recovery 所需的非敏感状态。

这些状态属于 Rust/userdb 真相源，不写入 manager settings。数据库已经包含用户 P2 明文，仍必须保持现有父目录与文件权限、WAL、busy timeout、迁移、备份和损坏恢复边界；outbox/journal 不得扩大日志或导出面。

## 取消、并发与重启

- 同一 domain 同一时刻只允许一个 writer cycle；不同只读 UI 查询不能持有同步写租约。
- 取消是请求，不是任意点强杀。transaction 开始后必须完成 commit 或 rollback；上传返回后必须先持久化 ack/可恢复状态，再报告取消。
- 进程在下载或解密阶段退出：cursor 未前移，重启重新发现。
- 进程在本地 transaction 中退出：SQLite 原子回滚或提交，cursor 与数据保持一致。
- 进程在 outbox 持久化后、发送前退出：重启重放同一 outbox。
- 进程在服务端提交后、本地 ack 前退出：重放同一 outbox并取得幂等成功，再完成本地 ack。
- lease 过期只能恢复编排所有权，不能删除 prepared outbox、清除 dirty 或跳过未处理 cursor。

## 状态与错误

对 FFI/Manager 可见的稳定状态只包含：

- phase；
- started/completed/cancelled/blocked；
- discovered/downloaded/applied/uploaded/conflicted 数量；
- retry 次数和是否仍可重试；
- 最后成功时间；
- 脱敏错误类别和 blocker code。

错误类别至少区分：policy blocked、backend unavailable、unauthenticated、revoked device、key epoch rejected、unsupported schema/algorithm、invalid metadata、signature/hash/AAD failure、decrypt/decode failure、local transaction failure、cursor failure、conflict exhausted、transport timeout、server unavailable 和 cancellation。

不得向 FFI、Dart、日志或 diagnostics 返回：token、recovery code、key bytes、canonical bytes、signature bytes、nonce、payload、HTTP body、SQLite 路径、用户词、input code、reading、context 或平台错误正文。

## 实施状态与后续批次

1. 已落地：`ime-sync` phase/result/error、opaque cursor、discovery page、local repository port 与 prepared outbox；Go/Rust change cursor discovery 已覆盖分页、幂等 sequence、非法/跨域/越界 cursor。
2. 已落地：userdb schema v7 持久化 domain state、remote observation、local revision、cycle journal/outbox、可信 public lifecycle cache 与 wrapped epoch ciphertext cache；transaction-scoped apply 使 payload、observation、revision 与对象 cursor 同提交或回滚，已验证 lifecycle state 与独立 lifecycle cursor 同事务替换且拒绝回退 / 分叉。明文 master key/shared secret 不进入 SQLite。
3. 已落地：关闭态 `sync_once` 组合 remote/local/crypto processor port，测试使用真实 test-memory signing 与密文解密，覆盖 `409` 重新发现、重新合并和新版本签名；文件重开可恢复同一 outbox。
4. 已落地关闭态风险矩阵：取消、retry exhaustion、local revision race、lease recovery、签名/密文/AAD-bound metadata、epoch/revocation 拒绝、decode/transaction cursor rollback、v4→v5 与 v5→v6 migration rollback、outbox prepare/ack crash point，以及两个隔离 userdb 通过短生命周期 Go HTTP 服务第二轮零上传收敛。
5. 已落地：默认关闭的 `DefaultSyncObjectProcessor`、cycle-frozen `SyncCryptoCycleSnapshot` 与 `SyncCryptoProvider` port；产品/合成构造路径分离，默认无 provider 时网络前阻断。测试覆盖历史 epoch、撤销 sequence、snapshot 漂移和轮换 outbox，双 userdb HTTP fixture 已复用该通用实现。
6. 已落地：`ProductSyncCryptoProvider`、可信 lifecycle/material/signing 三端口和严格装载顺序；合成授权、撤销、轮换与重启测试证明 revoked/test backend 不读取 material、历史 epoch 可读、撤销后 sequence 拒绝和新 epoch outbox。
7. 已落地：独立 lifecycle sequence、Rust trust-anchor signed record 验证、userdb public cache、wrapped epoch material store、Apple signing/key-agreement adapters 与精确远端读取；真实 Go HTTP 组合测试覆盖授权、同步、撤销、缓存和重启恢复。
8. 已落地：独立 signed epoch distribution batch、完整 active cohort、原子 metadata、幂等/分叉冲突和 Rust distributor/recipient 验证边界；A/B/C Go HTTP 证据覆盖坏签名无部分可读、漏发拒绝、A/C 轮换重启和 revoked B 无新 epoch。下一批推进 recovery record 轮换与恢复加入；production backend 资格通过前不接真实签名路径，产品链稳定后才设计窄 FFI command/status。

## 验证矩阵

必须覆盖：

- 任意 discovery 分页与重复投递得到相同合并结果，满足交换律、结合律和幂等性。
- metadata、algorithm、signer、signature、ciphertext hash、AAD、payload length、schema、key epoch 和 revoked device 的负向路径。
- 下载/解密/解码/merge/SQLite 任一步失败时 cursor 不前移且无半提交。
- 本地 transaction 中发生并发词条删除或显式恢复时，旧 remote snapshot 不能覆盖新意图。
- outbox 准备后各 crash point 重启，服务端最多得到一个相同 object version，本地 dirty 不丢失。
- 409 必须重新发现、复验、解密和合并；禁止只替换 `base_version` 重签后盲目覆盖。
- tombstone 防旧设备、旧 epoch、旧 backup 和旧 weight 复活；schema v1 `manual_add` 不伪装远端显式恢复。
- P1 原始事件、负反馈明细、上下文统计和 import batch 不进入 payload、outbox、FFI、日志或 fixture。
- 两个隔离合成客户端通过短生命周期 Go server 完成首轮同步、并发冲突、第二轮收敛和重启恢复。

## 停止线

- 本文允许的是关闭产品入口的 Rust/Go 本地实现与合成集成验证，不是用户可用同步授权。
- unsupported 环境证据只允许在真实不支持目标上补测；缺失不会降级已由支持设备主路径评审的 `product_qualified`，也不允许在当前设备伪造。本地 orchestration 测试通过仍不允许改写 `user_sync_enabled`。
- 只允许新增 localhost、合成 P2、单次调用内存参数的窄资格命令；普通用户成功入口、真实设备配对和后台自动同步继续缺席。
- 不实现 plaintext HTTP/CLI/FFI 上传，不持久化解密 payload，不把 token、附加 CA 或 key material 放入 manager settings。
- 不在本批实现恢复码 UI、设备加入/授权/撤销 UI、真实 key epoch 轮换入口、M4 发布包或第二平台。
