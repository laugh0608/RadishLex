# RadishLex 个人化学习设计

本文档用于定义本地个人化学习的职责边界、数据模型、隐私分级、排序接口、CLI/FFI 管理入口和验证标准，读者是实现 `ime-userdb`、`ime-ranker`、输入 runtime 和本地管理 UI 的开发者。本文不包含当前批次状态、SQLite migration 完整 SQL、ranker 调参历史、同步加密协议、Flutter 页面设计或平台输入法壳实现；实时进度见 `docs/status/current.md`。

## 稳定定位

本地个人化属于 M2，但其正确性基础在真实学习纵向链接入前完成。已有 CLI、FFI、userdb、ranker 和同步原型只能作为实现基础；只有真实平台选择在隐私策略约束下持久化，并可解释地影响后续候选，才构成产品证据。

个人化能力不改变底层 engine adapter 边界：

- `ime-engine-rime` 继续只负责真实候选生成和候选转换。
- `ime-core` 继续定义平台无关输入模型。
- `ime-userdb` 保存本地学习数据和用户词库。
- `ime-ranker` 根据 engine candidates 与 userdb summary 输出重排后的候选。
- `ime-cli` 提供可复验的学习、查询、删除、导入导出和 explain 命令。

平台壳只传递输入上下文和用户反馈，不实现学习语义。M2 可以交付本地 manager；远端同步、设备授权和密钥轮换属于 M3，自研拼音 engine 属于更后阶段。

## 产品 input runtime

`ime-runtime` 是真实输入 session 的个人化组合层，依赖 `ime-core`、`ime-ranker` 和 `ime-userdb`，但不依赖具体平台或具体 engine 实现。每个 runtime session 持有一个 engine session、一个指向固定 userdb 文件的独立 SQLite connection、ranker、隐私上下文和当次候选映射；多个输入 session 与 manager 通过 WAL 和短事务并发，不共享跨线程 `Connection`。

一次候选快照按以下顺序形成：

1. 从 engine 读取 composition、候选和稳定 `input_code`。
2. 根据平台传入的 secure input、敏感应用、隐私模式和上下文可信度决定个人化策略。
3. 对允许读取个人化数据的路径，在同一只读事务中一次取得当前候选身份匹配的 user term、ranker weight 与 tombstone，避免候选级 N+1 查询和跨查询状态漂移。
4. ranker 输出带 `original_index` 的确定性顺序；runtime 固化 display index 到 engine index 的映射，平台只使用 display index，真正选择时由 Rust 映射回 engine index。

隐私策略固定为：

| 上下文 | 候选读取 | 学习写入 |
| --- | --- | --- |
| secure input、敏感应用或无法安全判断 | 只用 engine 顺序，不读取 userdb 信号 | 禁止 |
| 用户隐私模式 | 可使用既有本地个人化摘要 | 禁止 |
| 已知普通上下文 | 允许本地重排 | 允许 P1 selection 与 P2 摘要事务写入 |

平台只传递枚举化 `context_kind` 和布尔策略信号，不传、不持久化原始 App ID、窗口标题或文档文本。当前稳定类别为 `general`、`browser`、`chat`、`code`、`editor`、`office`、`terminal`、`other`；未知类别必须拒绝，FFI view 另限制为不超过 32 个 UTF-8 bytes。上下文变化必须显式更新 runtime；切换到禁止学习的上下文时，清除尚未由 commit 确认的选择意图。

选择前，runtime 从当前快照捕获 input code、候选规范身份、display/engine index、候选数和受控 context kind。engine 立即返回匹配 commit 时记录一次 selection；分段选择没有立即 commit 时只保存待确认意图，后续仅在匹配 commit 到达时记录。reset、schema/client/context 变化、取消或不匹配 commit 必须丢弃待确认意图；无法关联候选的原始 engine commit 不推断学习。当前产品 runtime 不通过退格、改选或时间窗口自动推断负反馈。

userdb/ranker 读取失败时，runtime 必须保留 engine 原始顺序并返回明确的个人化退化状态：`ready` 表示个人化路径可用（当前页可以没有匹配信号），`policy_blocked` 表示策略强制 engine-only，`storage_unavailable` 表示数据库打开或迁移不可用，`read_failed` 与 `rank_failed` 分别表示本次摘要读取或排序失败。engine 已产生的 commit 不得因学习写入失败而丢失；key/select 结果同时携带 commit 和 `recorded`、`deferred`、`skipped_by_policy`、`failed` 等学习结果。错误诊断不得包含输入码、候选文本、数据库路径或原始应用信息。

## 目标

- 建立本地 SQLite userdb。
- 记录候选选择事件和必要的学习摘要。
- 支持用户词条 CRUD、导入、导出和删除语义。
- 支持 userdb P2 plaintext payload 只读迭代器，并通过本地加密组装测试证明可进入 `ime-crypto` envelope 与 `ime-sync` draft。
- 支持已解密 P2 payload 经客户端合并模型写回真实 userdb，保持 tombstone、显式恢复和 ranker weight 合并语义可复验。
- 支持负反馈，包括提交后撤销、改选候选和手动降权。
- 实现候选重排，且重排结果可解释。
- 明确 P0/P1/P2 数据边界，避免敏感输入进入日志、fixture 或同步对象。
- 通过 CLI 证明连续选择可提升排序，删除或降权不会被旧权重立即复活。

## 非目标

- 不从零实现完整拼音候选生成。
- 不把 Rime 内部对象 ID、内部评分或私有状态写入 userdb。
- 不在本地个人化里程碑实现远端同步、设备授权或密钥轮换。
- 不把原始选择事件默认纳入同步。
- 不把 P2 plaintext payload 暴露给 FFI、CLI 文件导出或平台壳。
- 不在 ranker 中读取平台私有生命周期、窗口句柄或 App 原始标题。
- 不用真实联系人、真实输入历史、真实 App 内容作为测试数据。

## 数据分级

个人化学习数据按 `docs/privacy-sync.md` 的分级处理：

| 数据 | 分级 | 默认策略 |
| --- | --- | --- |
| 密码框、支付、证件、secure text entry、隐私模式输入 | P0 | 不学习、不记录、不同步 |
| 原始选择事件、负反馈详细事件、应用上下文统计 | P1 | 仅本地学习，默认不同步 |
| 用户词库、候选权重摘要、自定义短语 | P2 | 后续阶段可端到端加密同步 |
| 官方词库包、输入方案模板 | P3 | 可公开下载 |

实现要求：

- P0 输入路径必须在事件进入 userdb 前被拦截。
- 隐私模式可以读取已经存在的本地排序摘要，但当前输入不得产生 selection、negative feedback、user term 或 ranker summary 写入。
- P1 事件日志可以压缩为 `ranker.weights` P2 权重摘要，但原始事件、负反馈明细和上下文统计默认不进入同步。
- P2 数据被删除时必须产生 tombstone 或等价语义，避免旧设备和旧备份复活词条。
- 日志、测试 fixture、golden 输出和截图不得包含真实明文输入历史或敏感上下文。

## 核心概念

### UserTerm

用户词条是用户可查看、可删除、可导入导出的学习结果。

稳定字段：

```text
UserTerm
  term_id
  text
  reading
  input_code
  source
  weight
  status
  created_at
  updated_at
  last_used_at
  restored_at
```

字段语义：

- `text`：候选文本。
- `reading`：可稳定获得的读音；未知时可为空。
- `input_code`：用户输入码，例如 `luobo`。
- `source`：`engine_selection`、`manual_import`、`manual_add`、`phrase_learning`。
- `weight`：本地学习权重，不直接等同于 engine score。
- `status`：`active`、`suppressed`、`deleted`。
- `restored_at`：只由显式恢复写入；普通新增、selection、导入和旧同步状态不得伪造该版本。

### SelectionEvent

选择事件记录一次候选提交行为，是学习权重的输入，不作为长期同步真相源。

```text
SelectionEvent
  event_id
  session_id
  input_code
  selected_text
  selected_reading
  candidate_index
  candidate_count
  context_kind
  created_at
```

规则：

- `candidate_index` 是 RadishLex candidate 列表中的索引。
- 不保存 Rime 候选指针、Rime session id 或底层私有对象。
- `context_kind` 只保存上述受控场景类别，不保存 App ID、窗口标题或正文内容。

### NegativeFeedback

负反馈表达用户不希望某个候选继续靠前。

```text
NegativeFeedback
  feedback_id
  input_code
  text
  reading
  reason
  context_kind
  created_at
```

`reason` 初期支持：

- `immediate_backspace`：提交后立即退格。
- `reselect_same_code`：同一输入码下改选其他候选。
- `manual_suppress`：用户手动降权。
- `manual_delete`：用户删除词条。

### DeletedTerm

删除语义必须强于普通降权。删除后，旧权重摘要或旧备份不能让词条立即复活。

```text
DeletedTerm
  term_id
  input_code
  text
  reading
  deleted_at
  reason
```

规则：

- 客户端本地可以保存明文词条用于管理 UI 展示。
- 同步对象中后续应使用加密 payload；服务端只看到密文。
- ranker 遇到 deleted tombstone 时不得用旧权重提升该词。

## SQLite schema

本地个人化 schema 保留以下职责和稳定字段；实现可调整字段名，但必须保留语义并通过 migration 测试证明升级路径。

| 表 | 稳定字段 |
| --- | --- |
| `user_terms` | `id`、`text`、`reading`、`input_code`、`source`、`weight`、`status`、`created_at`、`updated_at`、`last_used_at`、`restored_at`、`import_batch_id` |
| `selection_events` | `id`、`session_id`、`input_code`、`selected_text`、`selected_reading`、`candidate_index`、`candidate_count`、`context_kind`、`created_at` |
| `negative_feedback` | `id`、`input_code`、`text`、`reading`、`reason`、`context_kind`、`created_at` |
| `deleted_terms` | `id`、`term_id`、`input_code`、`text`、`reading`、`deleted_at`、`reason` |
| `ranker_weights` | `id`、`input_code`、`text`、`reading`、`frequency`、`last_used_at_ms`、`negative_score`、`context_kind`、`updated_at` |
| `import_batches` | `id`、`source_name`、`term_count`、`total_count`、`inserted_count`、`updated_count`、`skipped_deleted_count`、`skipped_duplicate_count`、`created_at`、`notes` |
| `sync_domain_state`、`sync_remote_objects`、`sync_local_objects` | object cursor、远端 observation、本地 revision 与 dirty 状态 |
| `sync_prepared_outbox`、`sync_cycle_journal` | crash-safe 密文 outbox、cycle phase、lease 与取消状态 |
| `sync_trusted_domains`、`sync_trusted_devices`、`sync_trusted_lifecycle_events` | 已验签的公开 domain/device/recovery lifecycle cache |
| `sync_wrapped_epoch_materials` | 版本化 wrapped epoch ciphertext 与公开 AAD metadata |

### SQLite 稳定决策

- 一次用户意图是最小事务边界。`add`、`explicit restore`、selection、negative feedback、delete 及导入分别在单个 `BEGIN IMMEDIATE` 事务内完成相关事件、词条、摘要和 tombstone 写入；任一语句失败时整项回滚，不能留下半条意图。
- 文件型 userdb 固定使用 WAL、`busy_timeout = 5000 ms`、`foreign_keys = ON` 和 `synchronous = NORMAL`。IME 与 manager 各持有独立 SQLite 连接，不跨线程共享同一个 `Connection`；IME 可持有长期热路径连接，manager 使用独立短事务连接，写事务不得跨 UI 或平台回调等待。
- 文件连接必须在任何 schema/version/integrity SQL 之前安装 busy timeout；首次并发打开争用 WAL journal mode 时，只对 SQLite busy/locked 或尚未切换到 WAL 的结果在同一 5 秒预算内重试，其他错误立即返回。不能把初始化竞争暴露成偶发启动失败，也不能无界重试或吞掉非锁错误。
- Unix 上数据库主文件及已生成的 `-wal`、`-shm` sidecar 权限固定收紧为 `0600`。userdb 不依赖 shell 环境变量或真实用户 Rime 目录。
- 当前 schema 为 v9。migration 在一个 `BEGIN IMMEDIATE` 事务内完成；打开数据库时先读取并检查 `PRAGMA user_version`，取得写事务后必须重读版本，高于当前实现的未来版本必须在任何 schema 写入前拒绝。全新空库直接建立当前学习、导入和同步表；旧库只按缺失能力补齐 legacy tombstone identity / ranker recency、恢复版本、导入批次、同步 orchestration、可信公开 lifecycle、key-agreement 公钥、wrapped epoch ciphertext 和 recovery lifecycle。迁移不得重放已经完成的 identity/recency 变换，也不得为历史词条伪造导入来源；缺少已签名 key-agreement profile、存在 lifecycle 分叉或 recovery event 无法无歧义迁移时必须整体失败并保留旧库。
- 同步扩展表不改变本地学习、排序和删除的真相源。`sync_prepared_outbox` 只能保存密文 envelope、签名 manifest 和幂等重放 metadata；可信 lifecycle cache 只能保存服务端同样可见的公开签名链；wrapped epoch 表只能保存密文。明文 master key、ECDH shared secret、派生 wrapping key、恢复码、bearer token 和 plaintext payload 不得写入 SQLite。
- 文件损坏、身份迁移歧义或 migration 失败时，原数据库文件必须原位保留并返回带路径/SQLite 原因的显式错误；不得静默删除、重命名后新建、降级为空库或用 fixture 代替。
- tombstone 的唯一身份使用 trim 归一化后的 `(input_code, text, reading)` 复合键。旧 64 位 FNV 字段只允许在 v1/v2 migration 中帮助关联既有本地行，不再作为当前 schema 的查询、唯一性或同步判断依据；无法无歧义恢复身份时 migration 整体失败并保留旧库。

### 学习状态与版本优先级

同一规范化复合身份按以下规则决策：

1. `deleted` tombstone 高于 selection、导入、ranker weight、普通 `manual_add` 和没有显式恢复标记的旧本地/备份/同步状态。
2. 没有 tombstone 时，`suppressed` 高于普通 active、selection 和导入状态；selection 可以继续记录 P1 事件，但不能隐式解除 suppress。
3. 只有单独的 `explicit restore` 入口可以清除 tombstone 或 suppress。恢复版本必须严格晚于当前删除版本，事务同时清除 tombstone、恢复 active 状态并写入新的 `restored_at`/`updated_at`；普通 `add` 不承担恢复语义。
4. 当前同步 plaintext schema v1 没有显式恢复字段，因此解密后的 `manual_add` 一律按普通 synced term 处理，不能推断为恢复。M3 若要跨设备传播恢复，必须先版本化增加显式 restore intent；在此之前旧备份或旧同步对象只能被 tombstone 阻断，不能复活词条。

删除写入同样必须晚于本地已知状态；旧 tombstone 不能覆盖版本更新的显式恢复。ranker 只消费上述状态决策后的摘要，不能自行反转 userdb 真相源。

## Ranker 输入输出

`ime-ranker` 只接收 RadishLex 稳定模型和 userdb summary，不直接访问 Rime。

输入：

```text
RankRequest
  input_code
  candidates
  context_kind
  user_terms
  ranker_weights
  deleted_terms
```

输出：

```text
RankedCandidate
  candidate
  original_index
  final_score
  explanation
```

解释字段至少包含：

- engine order factor
- user term boost
- frequency boost
- recency boost
- context boost
- negative feedback penalty
- deleted/suppressed reason

排序组合保持稳定、可测试；frequency、recency 等单项先按各自有界公式变换，再线性合成。权重参数必须集中配置，不散落在 CLI 或 adapter 中。后续优化排序质量时必须保留 explain 输出。

### Ranker 稳定公式

- 每个 `RankRequest` 必须显式携带 `evaluated_at_ms`。recency 只由 `last_used_at_ms` 与该评估时间计算：未来时间按 age 0 处理，缺少最后使用时间贡献 0；默认使用 7 天半衰期的确定性指数衰减。同一输入与同一评估时间必须得到逐因子一致的结果。
- selection 只把 engine 学习词条的存在权重初始化为稳定基值，不随每次选择重复线性累加；重复选择只增长有上限的 frequency 计数并更新 `last_used_at_ms`。
- frequency 贡献使用 `ln(1 + frequency)` 后再按配置封顶；negative feedback 贡献同样封顶。用户词条权重、非法负值和所有外部浮点输入都必须校验或限制到明确范围，最终分数及 explain 每个因子必须为有限数。
- `deleted` 候选只生效 engine order 与 delete penalty，旧正负摘要和 suppress 不重复计分；`suppressed` 候选不生效 user term、frequency、recency 或 context 正向因子。active 候选才消费正向摘要。
- context boost 只在同场景存在正向 selection 摘要时生效。最终排序按有限 `final_score` 降序；完全相同时保留 `original_index`，不引入不稳定哈希或数据库行顺序 tie-break。

当前默认参数与实现保持同源：engine order 每后一位减 `0.01`；user term 先封顶 `4.0` 再乘 `1.0`；frequency 为 `min(ln(1 + frequency) × 0.35, 2.0)`；recency 为 `2^(-age/7天) × 0.25`；同场景正向摘要贡献 `0.3`；negative feedback 为 `min(ln(1 + negative_score) × 1.2, 4.0)`；suppressed 与 deleted penalty 分别为 `2.0`、`10.0`。`final_score` 必须与 explain 的加减项逐项重构一致；修改这些参数必须同步固定合成评测预期。

## 学习流程

一次普通选择提交：

```text
InputSession state
  -> engine candidates
  -> ranker rerank
  -> user selects candidate
  -> commit text
  -> userdb records SelectionEvent
  -> userdb updates UserTerm and ranker summary
```

一次负反馈：

```text
commit text
  -> immediate backspace / reselect / manual suppress / manual delete
  -> userdb records NegativeFeedback
  -> userdb updates ranker summary
  -> ranker lowers or removes candidate
```

删除词条：

```text
manual delete
  -> mark user_terms.status = deleted
  -> insert deleted_terms tombstone
  -> remove active ranker boost
  -> future imports and old summaries must respect tombstone
```

## CLI 管理入口

当前已落地的 `radishlex-ime-cli` 学习管理入口：

```text
radishlex-ime-cli rime --schema <schema> --shared-data <path> --user-data <path> [--key <name> ...] --rank-db <path> [--context <kind>] <input-code> [candidate-index]
radishlex-ime-cli rime snapshot --schema <schema> --shared-data <path> --user-data <fresh-empty-path> --deploy-on-start <0|1> [--rank-db <path>] [--context <kind>] <input-code>
radishlex-ime-cli dict list --db <path>
radishlex-ime-cli dict add --db <path> --input <code> --text <text> [--reading <reading>]
radishlex-ime-cli dict restore --db <path> --input <code> --text <text> [--reading <reading>]
radishlex-ime-cli dict delete --db <path> --input <code> --text <text> [--reading <reading>]
radishlex-ime-cli dict export --db <path> --file <path>
radishlex-ime-cli dict inspect --file <path>
radishlex-ime-cli dict import --db <path> --file <path> [--source <name>] [--dry-run]
radishlex-ime-cli dict import-batches --db <path>
radishlex-ime-cli learn status --db <path>
radishlex-ime-cli learn case-status --db <path> --input <code> --text <text> [--reading <reading>] [--context <kind>]
radishlex-ime-cli learn select --db <path> --input <code> --text <text> [--reading <reading>] [--index <n>] [--count <n>] [--session <id>] [--context <kind>]
radishlex-ime-cli learn suppress --db <path> --input <code> --text <text> [--reading <reading>] [--reason <reason>] [--context <kind>]
radishlex-ime-cli rank explain --db <path> --input <code> --candidate <text> [--reading <reading>] [--context <kind>]
radishlex-ime-cli sync preflight --db <path>
```

规则：

- CLI 必须显式传入 `--db`，不隐式读取真实用户输入法数据。
- `rime --rank-db` 必须显式传入隔离 userdb，重排后的候选索引需要映射回原始 engine index 再提交。
- 测试使用临时 SQLite 数据库和合成词。
- `rank explain` 输出排序因子，不能只输出最终分数。
- `dict export` 只导出 P2 用户词条数据，不导出 P1 原始选择事件、负反馈详细事件、上下文统计或 ranker 权重摘要。
- `dict add` 只新增或更新未删除词条，不清除 tombstone，也不隐式解除 suppress。
- `dict import` 普通导入不得复活本地 deleted tombstone 命中的词条；恢复删除或 suppressed 词条必须通过独立的 `dict restore` 明确人工动作。
- `dict import --dry-run` 必须复用实际导入的分类逻辑，报告 `inserted`、`updated`、`skipped_deleted` 和 `skipped_duplicate`，但不得写入词条或导入批次。
- `dict import-batches` 用于查看导入批次来源、导入数量、插入数量、更新数量、删除跳过数量、重复跳过数量和创建时间。
- `dict inspect` 用于在不打开 userdb 的情况下检查导入文件格式版本、记录数和 CLI 输入码兼容性。
- `learn status` 用于查看管理 UI 需要的只读学习状态摘要，只输出词条、ranker weight、deleted tombstone、P1 本地事件和本地审计批次的总量与最新活动时间，不输出 P1 选择事件明细、负反馈明细、上下文分布、用户词文本或同步明文 payload。
- `learn case-status` 是版本化的合成用例取证读模型：在一个 SQLite 读事务中返回规范化身份、全库计数/时间、精确 term、指定 context 的 ranker weight 和 tombstone，并明确省略 P1 原始行；它不替代 manager 的受限聚合接口。
- `rime snapshot` 只编排 fresh Rime user data 上的非选择候选证据。带 `--rank-db` 时必须走产品 `PersonalizedInputSession`，输出 display/engine index 和 explain；它不新增 engine trait，也不能用单次 UI 顺序代替操作前后数据库增量。
- `sync preflight` 只输出 P2 可同步对象计数、P1 本地事件计数和本地审计计数，不生成明文同步 payload。

精确字段、路径安全边界与组合取证顺序见 [学习取证 CLI 参考](cli-learning-evidence.md)。

### FFI 管理入口

`ime-ffi` 当前暴露的 userdb 管理入口只覆盖用户明确管理的 P2 词条与同步状态摘要：

```text
radishlex_userdb_add_term(db_path, input_code, text, reading)
radishlex_userdb_restore_term(db_path, input_code, text, reading)
radishlex_userdb_delete_term(db_path, input_code, text, reading)
radishlex_userdb_terms_new(db_path)
radishlex_userdb_terms_count(terms)
radishlex_userdb_terms_get(terms, index, term_out)
radishlex_userdb_terms_free(terms)
radishlex_userdb_dictionary_inspect(file_path, summary_out)
radishlex_userdb_dictionary_export(db_path, file_path, summary_out)
radishlex_userdb_dictionary_import(db_path, file_path, source_name, dry_run, summary_out)
radishlex_userdb_import_batches_new(db_path)
radishlex_userdb_import_batches_count(batches)
radishlex_userdb_import_batches_get(batches, index, batch_out)
radishlex_userdb_import_batches_free(batches)
radishlex_userdb_learning_status(db_path, summary_out)
radishlex_userdb_sync_preflight(db_path, summary_out)
radishlex_ffi_contract(contract_out)
```

规则：

- FFI 入口必须显式传入 UTF-8 SQLite 路径，不隐式读取真实用户输入法目录。
- `add_term` 使用 `manual_add` 来源，只表示用户明确新增或更新未删除词条，不承担恢复语义。
- `delete_term` 沿用 userdb tombstone 语义，删除后普通导入和旧权重不得立即复活该词。
- `restore_term` 是独立且可审计的 explicit restore；`add_term`、selection 和导入不能替代它清除 tombstone 或 suppress。
- `terms_new` 返回只读 list handle，平台端只能通过 `terms_get` 读取 view，并必须调用 `terms_free` 释放。
- list view 中的字符串只在 list handle 释放前有效，平台端不得缓存裸指针。
- dictionary inspect 只读取导入文件格式、记录数和 P2 同步分类，不打开 userdb。
- dictionary export 只导出 active / suppressed 用户词条字段，不导出 P1 原始选择事件、负反馈详细事件、上下文统计或 ranker 权重摘要。
- dictionary import 支持 `dry_run`，dry-run 不写词条、不写 import batch；实际导入必须记录 import batch，并继续遵守 deleted tombstone。
- import batches 通过只读 list handle 暴露来源和统计，不暴露 SQLite handle、statement 或 row 指针。
- learning status 通过单个 `repr(C)` summary 暴露聚合计数、latest timestamp 和 `plaintext_payload / p1_raw_details / context_stats = false` 标记，不返回 string view、用户词明文、P1 事件行、负反馈 reason 列表或上下文统计。
- userdb 管理 FFI 不直接接收 selection event、negative feedback 或上下文统计；产品输入 FFI 则通过 personalized session 在 Rust runtime 内部记录允许的 selection，平台不能自行拼写学习事件。
- `restore_term` 已作为独立 FFI 管理入口开放；调用方必须把它呈现为明确恢复动作，不能在普通新增、导入或输入选择中隐式调用。
- 当前 FFI contract 明确 session 绑定创建线程，平台端不得跨线程直接操作同一 `RadishLexSession*`。

### 用户词库导入导出格式

当前导入导出格式为 UTF-8 TSV，版本头和字段表头固定：

```text
# radishlex-user-terms-v1
input_code	text	reading	source	weight	status
luobo	萝卜	luo bo	manual_add	2	active
```

字段：

- `input_code`：输入码，CLI 导入时仍按当前输入码规则校验，只允许 ASCII 字母、数字和 apostrophe。
- `text`：用户词条文本。
- `reading`：读音，可为空。
- `source`：`engine_selection`、`manual_import`、`manual_add`、`phrase_learning`。
- `weight`：非负有限数值，用于 ranker 的用户词提升。
- `status`：当前只接受 `active` 或 `suppressed`；导入文件不得携带 `deleted`。

转义规则：

- 字段内 tab 写作 `\t`。
- 字段内换行写作 `\n`。
- 字段内回车写作 `\r`。
- 字段内反斜杠写作 `\\`。

导入解析先识别格式版本，再按对应 header 解析字段。当前只支持 `radishlex-user-terms-v1`；未来未知版本必须返回明确的不兼容错误，不能按 v1 静默导入。

实际导入会在同一事务内先记录 `import_batches`，再把本批次真实插入或更新的词条写入对应 `import_batch_id`；dry run、deleted skip 和 duplicate skip 不创建或改写本批次关联，既有词条的历史关联保持原样。`source_name` 来自 `dict import --source <name>`，CLI 未传时为 `cli`。该批次记录只表达本地导入审计来源，不改变每条词条的 `source` 字段，也不进入 P2 payload。
`source_name` 只允许 ASCII 字母、数字、dot、underscore 和 dash，最长 64 bytes。导入文件内重复的 `input_code`、`text`、`reading` 身份会跳过后续重复项；同 `input_code`、`text` 但不同 `reading` 视为不同词条。

### 同步前置检查

`sync preflight` 只做本地分类计数：

- P2 后续可加密同步：`dictionary.user_terms`、`ranker.weights`、`dictionary.deleted_terms`。
- P1 默认本地保留：`selection_events`、`negative_feedback`。
- 本地审计记录：`import_batches`。

该命令必须输出 `plaintext_payload: false`，表示当前阶段没有生成明文同步对象，也没有连接后端。

## 验证标准

本地个人化必须覆盖：

- 新建空 userdb，schema migration 成功。
- 添加、查询、更新、删除用户词条。
- 记录选择事件后，相关词条频次或权重更新。
- 连续选择某个候选后，该候选排序提升。
- 手动降权后，该候选排序下降。
- 删除词条后，旧权重摘要或重新导入不能立即复活该词。
- P0 输入事件不会写入 userdb。
- P1 原始事件不会出现在导出文件或同步 payload 草案中。
- `ranker.weights` payload 只包含 P2 权重摘要字段，不包含原始 selection event、负反馈 reason、上下文统计或本地审计批次。
- 用户词库导出只包含 P2 词条字段，导入 malformed 文件返回明确错误。
- 导入 dry-run 不写数据库，实际导入记录 `import_batches`，并区分 insert、update、deleted skip 和 duplicate skip。
- 导入格式检查能识别当前 v1 文件，并对未知未来版本返回明确不兼容错误。
- 同步前置检查只输出分类计数，不输出明文用户词、原始事件或负反馈明细。
- 学习状态只读摘要只输出聚合计数、latest timestamp 和隐私边界标记，不输出明文用户词、原始选择事件、负反馈 reason 明细或上下文统计。
- `rank explain` 能说明候选排序变化原因。
- trigger 或等价故障注入分别证明 selection、negative feedback、delete 和 explicit restore 的多表写入完整回滚。
- 两个文件型独立连接在受控写竞争下依靠 WAL 与 busy timeout 完成短事务，不把常态 manager/IME 并发暴露为频繁 `SQLITE_BUSY`。
- 受支持旧 schema 到当前 schema 的迁移保留数据与已验证 lifecycle，并分别覆盖 legacy learning/import、sync orchestration、可信 lifecycle、wrapped epoch 和 recovery event 迁移；未来 schema 在写入前拒绝；损坏文件和迁移失败文件原位保留。
- 固定合成排序评测至少记录 Top-1、Top-3、MRR 和 case 数；样例只使用公开合成词，不使用真实输入历史。基线变差必须由权重/语义变更说明解释，不能只凭主观体验接受。
- 候选重排延迟使用固定候选数、固定迭代次数和 warm-up 记录可复验统计；CI 只校验结果、样本规模与统计值有限，不使用易受共享机器波动影响的严苛墙钟上限。

R02L 按上述口径建立了 schema v3 学习语义与固定测试基线：5 个公开合成 case 的 Top-1 为 `0.8`、Top-3 为 `1.0`、MRR 为 `0.9`；延迟样本固定为 50 个候选、100 次 warm-up 和 1000 次计时迭代。schema v4 增加不进入同步 payload 的本地导入批次关联，v5 至 v9 增加同步 orchestration、可信公开 lifecycle、wrapped epoch ciphertext 与 recovery lifecycle 持久化；这些扩展不改变既有学习、排序和删除语义。该基线只证明本地正确性与可复验性；产品 runtime 自动化不能替代真实平台纵向证据，具体机器观测与全仓门禁记录在对应验收材料。

默认验证入口：

```text
cargo fmt --check
cargo test -p radishlex-ime-userdb
cargo test -p radishlex-ime-ranker
cargo test --workspace
./scripts/check-repo.sh
```

已有 CLI、FFI、userdb、ranker、导入导出、P2 摘要和合成同步验证的完成记录保留在 devlog。本文后续只在数据模型、稳定边界或验收口径改变时更新。

## 稳定停止线

- selection、negative feedback、delete 和 explicit restore 未具备事务性前，不接入真实平台学习热路径。
- SQLite WAL、busy timeout、连接并发和数据库文件权限未明确前，不让输入法与 manager 共享生产 userdb。
- recency、frequency、suppress、delete 与 restore 语义未通过固定评测前，不凭主观体验调整权重。
- P0/P1/P2 分级未在测试中体现前，不进入 manager 产品数据视图或远端同步。
- P1 原始事件不得进入 FFI 管理接口、日志、导出文件或同步 payload。

## 产品退出证据

M2 本地个人化退出时必须证明：

- 真实平台选择在隐私策略允许时原子写入 userdb；
- 重启输入法后学习结果仍存在，并能解释地改变后续候选；
- P0、隐私模式与禁止学习场景不产生学习记录；
- 删除与显式恢复不会被旧事件或旧备份静默反转；
- manager 能通过真实 FFI 管理本地词库、学习和隐私设置，且不依赖远端同步。
