# RadishLex 个人化学习设计

本文档用于定义 Phase 2 个人化学习的职责边界、数据模型、隐私分级、排序接口、CLI 管理入口和验证标准，读者是后续实现 `ime-userdb`、`ime-ranker`、`ime-cli` 学习命令和管理 UI 的开发者。本文不包含 SQLite migration 完整 SQL、ranker 权重公式最终参数、同步加密协议、Flutter 页面设计或平台输入法壳实现。

## 阶段定位

当前处于 Phase 2 起步。`ime-core`、`ime-cli demo` 与真实 Rime adapter 已能复验 `compose -> candidates -> commit`，`ime-userdb` 已开始在 RadishLex candidate 层保存本地用户词库、选择事件、负反馈和删除 tombstone，`ime-ranker` 已提供可解释候选重排模型，`ime-cli` 已具备基础 `dict`、`learn`、`rank explain`、`rime --rank-db`、用户词库导入导出、导入格式检查和同步前置检查命令。`ime-sync` 已补 payload 来源分类和加密对象外壳草案，`ime-ffi` 已补结构化 snapshot / candidate ABI、normalized key event、engine kind 门禁、sync preflight 状态入口和 userdb add / delete / list host smoke。下一阶段目标是继续补齐 Rime adapter FFI 配置策略和更完整的受控 userdb 管理入口。

Phase 2 不改变底层 engine adapter 边界：

- `ime-engine-rime` 继续只负责真实候选生成和候选转换。
- `ime-core` 继续定义平台无关输入模型。
- `ime-userdb` 保存本地学习数据和用户词库。
- `ime-ranker` 根据 engine candidates 与 userdb summary 输出重排后的候选。
- `ime-cli` 提供可复验的学习、查询、删除、导入导出和 explain 命令。

Phase 2 仍不推进平台壳、同步后端、Flutter manager 或自研拼音 engine。真实平台输入法应等待真实 engine candidates 与学习层复验稳定后再进入。

## 目标

- 建立本地 SQLite userdb。
- 记录候选选择事件和必要的学习摘要。
- 支持用户词条 CRUD、导入、导出和删除语义。
- 支持负反馈，包括提交后撤销、改选候选和手动降权。
- 实现候选重排，且重排结果可解释。
- 明确 P0/P1/P2 数据边界，避免敏感输入进入日志、fixture 或同步对象。
- 通过 CLI 证明连续选择可提升排序，删除或降权不会被旧权重立即复活。

## 非目标

- 不从零实现完整拼音候选生成。
- 不把 Rime 内部对象 ID、内部评分或私有状态写入 userdb。
- 不在 Phase 2 实现远端同步、设备授权或密钥轮换。
- 不把原始选择事件默认纳入同步。
- 不在 ranker 中读取平台私有生命周期、窗口句柄或 App 原始标题。
- 不用真实联系人、真实输入历史、真实 App 内容作为测试数据。

## 数据分级

Phase 2 的学习数据按 `docs/privacy-sync.md` 的分级处理：

| 数据 | 分级 | 默认策略 |
| --- | --- | --- |
| 密码框、支付、证件、secure text entry、隐私模式输入 | P0 | 不学习、不记录、不同步 |
| 原始选择事件、负反馈详细事件、应用上下文统计 | P1 | 仅本地学习，默认不同步 |
| 用户词库、候选权重摘要、自定义短语 | P2 | 后续阶段可端到端加密同步 |
| 官方词库包、输入方案模板 | P3 | 可公开下载 |

实现要求：

- P0 输入路径必须在事件进入 userdb 前被拦截。
- P1 事件日志可以压缩为 P2 权重摘要，但原始事件默认不进入同步。
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
```

字段语义：

- `text`：候选文本。
- `reading`：可稳定获得的读音；未知时可为空。
- `input_code`：用户输入码，例如 `luobo`。
- `source`：`engine_selection`、`manual_import`、`manual_add`、`phrase_learning`。
- `weight`：本地学习权重，不直接等同于 engine score。
- `status`：`active`、`suppressed`、`deleted`。

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
- `context_kind` 只保存归类后的场景，例如 `general`、`chat`、`code`、`search`，不保存窗口标题或正文内容。

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
  text_hash
  reading_hash
  input_code_hash
  deleted_at
  reason
```

规则：

- 客户端本地可以保存明文词条用于管理 UI 展示。
- 同步对象中后续应使用加密 payload；服务端只看到密文。
- ranker 遇到 deleted tombstone 时不得用旧权重提升该词。

## SQLite 草案

Phase 2 初始 schema 建议包含：

```text
user_terms
selection_events
negative_feedback
deleted_terms
ranker_weights
import_batches
```

`user_terms`：

- `id`
- `text`
- `reading`
- `input_code`
- `source`
- `weight`
- `status`
- `created_at`
- `updated_at`
- `last_used_at`

`selection_events`：

- `id`
- `session_id`
- `input_code`
- `selected_text`
- `selected_reading`
- `candidate_index`
- `candidate_count`
- `context_kind`
- `created_at`

`negative_feedback`：

- `id`
- `input_code`
- `text`
- `reading`
- `reason`
- `context_kind`
- `created_at`

`deleted_terms`：

- `id`
- `term_id`
- `text_hash`
- `reading_hash`
- `input_code_hash`
- `deleted_at`
- `reason`

`ranker_weights`：

- `id`
- `input_code`
- `text`
- `reading`
- `frequency`
- `recency_score`
- `negative_score`
- `context_kind`
- `updated_at`

`import_batches`：

- `id`
- `source_name`
- `term_count`
- `created_at`
- `notes`

实现阶段可以调整字段名，但必须保留这些语义，并通过 migration 测试证明升级路径。

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

Phase 2 初始排序可以使用稳定、可测试的线性加权；权重参数必须集中配置，不散落在 CLI 或 adapter 中。后续优化排序质量时必须保留 explain 输出。

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
radishlex-ime-cli dict list --db <path>
radishlex-ime-cli dict add --db <path> --input <code> --text <text> [--reading <reading>]
radishlex-ime-cli dict delete --db <path> --input <code> --text <text> [--reading <reading>]
radishlex-ime-cli dict export --db <path> --file <path>
radishlex-ime-cli dict inspect --file <path>
radishlex-ime-cli dict import --db <path> --file <path> [--source <name>] [--dry-run]
radishlex-ime-cli dict import-batches --db <path>
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
- `dict import` 普通导入不得复活本地 deleted tombstone 命中的词条；恢复删除词条必须通过 `dict add` 这类明确人工动作。
- `dict import --dry-run` 必须复用实际导入的分类逻辑，报告 `inserted`、`updated`、`skipped_deleted` 和 `skipped_duplicate`，但不得写入词条或导入批次。
- `dict import-batches` 用于查看导入批次来源、导入数量、插入数量、更新数量、删除跳过数量、重复跳过数量和创建时间。
- `dict inspect` 用于在不打开 userdb 的情况下检查导入文件格式版本、记录数和 CLI 输入码兼容性。
- `sync preflight` 只输出 P2 可同步对象计数、P1 本地事件计数和本地审计计数，不生成明文同步 payload。

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

导入会记录 `import_batches`，其中 `source_name` 来自 `dict import --source <name>`，未传时为 `cli`。该批次记录只表达导入来源，不改变每条词条的 `source` 字段。
`source_name` 只允许 ASCII 字母、数字、dot、underscore 和 dash，最长 64 bytes。导入文件内重复的 `input_code`、`text`、`reading` 身份会跳过后续重复项；同 `input_code`、`text` 但不同 `reading` 视为不同词条。

### 同步前置检查

`sync preflight` 只做本地分类计数：

- P2 后续可加密同步：`dictionary.user_terms`、`ranker.weights`、`dictionary.deleted_terms`。
- P1 默认本地保留：`selection_events`、`negative_feedback`。
- 本地审计记录：`import_batches`。

该命令必须输出 `plaintext_payload: false`，表示当前阶段没有生成明文同步对象，也没有连接后端。

## 验证标准

Phase 2 起步必须覆盖：

- 新建空 userdb，schema migration 成功。
- 添加、查询、更新、删除用户词条。
- 记录选择事件后，相关词条频次或权重更新。
- 连续选择某个候选后，该候选排序提升。
- 手动降权后，该候选排序下降。
- 删除词条后，旧权重摘要或重新导入不能立即复活该词。
- P0 输入事件不会写入 userdb。
- P1 原始事件不会出现在导出文件或同步 payload 草案中。
- 用户词库导出只包含 P2 词条字段，导入 malformed 文件返回明确错误。
- 导入 dry-run 不写数据库，实际导入记录 `import_batches`，并区分 insert、update、deleted skip 和 duplicate skip。
- 导入格式检查能识别当前 v1 文件，并对未知未来版本返回明确不兼容错误。
- 同步前置检查只输出分类计数，不输出明文用户词、原始事件或负反馈明细。
- `rank explain` 能说明候选排序变化原因。

默认验证入口：

```text
cargo fmt --check
cargo test -p radishlex-ime-userdb
cargo test -p radishlex-ime-ranker
cargo test --workspace
./scripts/check-repo.sh
```

当前 `ime-userdb` 与 `ime-ranker` 均已创建。起步验证以 `cargo test -p radishlex-ime-userdb`、`cargo test -p radishlex-ime-ranker` 和仓库级检查为准。
基础 CLI 学习命令已接入 `ime-userdb` 与 `ime-ranker`，应额外覆盖 `cargo test -p radishlex-ime-cli`，确认 `dict add/list/delete/import/export`、`learn select/suppress` 和 `rank explain` 的命令参数、输出与隐私边界。

## 实施顺序

1. `crates/ime-userdb/` 已创建，已包含 SQLite schema、migration、词条 CRUD、选择事件、负反馈记录和删除 tombstone 起步测试。
2. `crates/ime-ranker/` 已创建，已包含 `RankRequest`、`RankedCandidate`、explain 模型和频次、近期、负反馈、删除 tombstone 排序测试。
3. `ime-cli` 已扩展 `dict`、`learn` 和 `rank explain` 命令，基础学习链路可通过临时 SQLite 数据库复验。
4. `ime-cli rime --rank-db` 已把 Rime adapter candidates 接入 ranker smoke，单元测试覆盖候选重排、explain 输出和原始 engine index 提交映射；本机 native rank smoke 命令已写入 runbook。
5. 用户词库导入导出已补入 `ime-userdb` 与 `ime-cli`，格式为带版本头和字段表头的 UTF-8 TSV，并通过测试覆盖 P1 不导出、deleted tombstone 不复活和 malformed 文件错误。
6. 导入 dry-run、批次查询、insert/update/duplicate/deleted 统计和 `import_batches` v2 migration 已补入。
7. 导入格式版本解析、`dict inspect` 和 `sync preflight` 已补入。
8. `ime-sync` 已补同步 payload 来源分类、同步对象类型和加密对象外壳草案。
9. `ime-ffi` 已补 C ABI 起步验证，覆盖 opaque session handle、错误对象、UTF-8 buffer 和释放函数。
10. `ime-ffi` 已补结构化 snapshot / candidate ABI 和 normalized key event。
11. `ime-ffi` 已补 session options、engine kind 门禁和 sync preflight 状态摘要入口，当前只允许 demo engine，Rime kind 明确返回未可用。
12. `ime-ffi` 已补受控 userdb add / delete / list 管理入口，继续使用显式 SQLite 路径，不暴露 SQLite handle，不记录 P1 学习事件。
13. 下一步继续补 Rime adapter FFI 配置策略和更完整的受控 userdb 管理入口。

阶段停止线：

- userdb schema 与删除语义未验证前，不接同步。
- ranker explain 未落地前，不做主观权重调参。
- 本机 Rime rank smoke 与用户词库导入导出未形成可复验证据前，不推进平台壳。
- P0/P1/P2 分级未在测试中体现前，不进入管理 UI 或远端同步设计。
