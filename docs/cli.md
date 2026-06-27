# RadishLex CLI 说明

本文档用于说明 `radishlex-ime-cli` 当前可用命令、参数、输出字段、错误语义和安全边界，读者是需要在本地复验 Rust core 与 engine adapter 行为的开发者和协作者。本文不包含阶段路线、开发进度、Rime 数据准备细节、平台输入法安装流程、ranker 设计或同步协议。

## 定位

`radishlex-ime-cli` 是当前 Rust 侧命令行复验入口，用于验证两类链路：

```text
input code -> push_key -> composition -> candidates -> commit_candidate
userdb -> learning event -> ranker summary -> rank explain
```

它不是系统输入法，也不注册平台输入法服务。CLI 只在当前进程内运行，用于观察 `ime-core`、engine adapter、`ime-userdb` 与 `ime-ranker` 的行为。

当前命令：

```text
radishlex-ime-cli demo <input-code> [candidate-index]
radishlex-ime-cli rime --schema <schema> --shared-data <path> --user-data <path> [--key <name> ...] [--rank-db <path>] [--context <kind>] <input-code> [candidate-index]
radishlex-ime-cli dict list --db <path>
radishlex-ime-cli dict add --db <path> --input <code> --text <text> [--reading <reading>]
radishlex-ime-cli dict delete --db <path> --input <code> --text <text> [--reading <reading>]
radishlex-ime-cli dict export --db <path> --file <path>
radishlex-ime-cli dict inspect --file <path>
radishlex-ime-cli dict import --db <path> --file <path> [--source <name>] [--dry-run]
radishlex-ime-cli dict import-batches --db <path>
radishlex-ime-cli learn status --db <path>
radishlex-ime-cli learn select --db <path> --input <code> --text <text> [--reading <reading>] [--index <n>] [--count <n>] [--session <id>] [--context <kind>]
radishlex-ime-cli learn suppress --db <path> --input <code> --text <text> [--reading <reading>] [--reason <reason>] [--context <kind>]
radishlex-ime-cli rank explain --db <path> --input <code> --candidate <text> [--reading <reading>] [--context <kind>]
radishlex-ime-cli sync preflight --db <path>
```

## 输入会话输出

`demo` 与 `rime` 使用相同输出形态：

```text
schema: <schema-id>
input: <input-code>
composition: <preedit>
candidates:
  0. <candidate>
  1. <candidate>
commit: <text>
```

字段含义：

- `schema`：当前输入方案标识。
- `input`：本次传入的输入码。
- `composition`：当前预编辑文本。
- `candidates`：当前候选列表，索引从 `0` 开始。
- `commit`：提交文本；没有候选或未产生提交时显示 `<none>`。

`candidate-index` 也是 `0` 基索引。未传入时，CLI 默认尝试提交首候选。

## demo 命令

用法：

```bash
cargo run -p radishlex-ime-cli -- demo luobo
cargo run -p radishlex-ime-cli -- demo luobo 1
```

用途：

- 复验 `ime-core::InputSession` 的基础生命周期。
- 在不安装 `librime` 的环境中保持默认 smoke 可运行。
- 验证输出渲染、候选索引和错误提示。

边界：

- `demo` 使用合成 adapter，不代表真实中文输入质量。
- `demo` 不读取 Rime schema、用户词库或系统输入法目录。
- `demo` 不应用个人化学习、ranker、userdb 或同步逻辑。

## rime 命令

用法：

```bash
RIME_INCLUDE_DIR=/opt/homebrew/opt/librime/include \
RIME_LIB_DIR=/opt/homebrew/opt/librime/lib \
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime \
  --schema luna_pinyin \
  --shared-data /tmp/radishlex-rime-smoke.<id>/shared \
  --user-data /tmp/radishlex-rime-smoke.<id>/user \
  luobo
```

翻页或导航 smoke 可在输入码后追加命名键：

```bash
RIME_INCLUDE_DIR=/opt/homebrew/opt/librime/include \
RIME_LIB_DIR=/opt/homebrew/opt/librime/lib \
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime \
  --schema luna_pinyin \
  --shared-data /tmp/radishlex-rime-smoke.<id>/shared \
  --user-data /tmp/radishlex-rime-smoke.<id>/user \
  luobo --key page-down 0
```

用途：

- 通过 `ime-engine-rime::RimeEngine` 调用真实 `librime`。
- 复验 Rime C API 到 RadishLex `Composition` / `Candidate` / `Commit` 的转换。
- 检查真实 engine 路径下的 schema、composition、候选和提交行为。
- 可选接入 `--rank-db`，把真实 engine candidates 送入 `ime-ranker` 并输出 explain。

参数：

- `--schema <schema>`：Rime schema id，例如 `luna_pinyin`。
- `--shared-data <path>`：Rime shared data 目录，包含 schema 和公开词典数据。
- `--user-data <path>`：Rime user data 目录，保存本次 smoke 的用户配置和 build 产物。
- `--key <name>`：可重复，用于在输入码之后追加命名键事件，只作为 CLI smoke 调试入口。
- `--rank-db <path>`：可选 SQLite userdb 路径；传入后会对当前 Rime candidates 执行重排和 explain。
- `--context <kind>`：可选上下文分类，只在传入 `--rank-db` 时有效，默认 `general`。
- `<input-code>`：输入码，例如 `luobo`。
- `[candidate-index]`：可选候选索引；未传入时默认提交首候选。启用 `--rank-db` 后，该索引表示重排后的候选索引，CLI 提交时会映射回原始 engine candidate index。

当前支持的 `--key` 值：`space`、`enter`、`backspace`、`escape`、`tab`、`arrow-up`、`arrow-down`、`arrow-left`、`arrow-right`、`page-up`、`page-down`。

构建要求：

- 必须显式启用 `--features native-rime`。
- 必须让 build script 找到 `rime_api.h` 和 `librime`。
- 可通过 `RIME_INCLUDE_DIR` 与 `RIME_LIB_DIR` 指定路径。
- 未启用 `native-rime` 时，命令会返回明确的构建提示，不会静默退回 `demo`。

Rime 数据准备步骤见 [Rime Native Smoke Runbook](runbooks/rime-native-smoke.md)。

### Rime rank smoke 输出

传入 `--rank-db` 后，`rime` 命令会在 `composition` 后增加 rank 上下文，并将候选输出为重排后的顺序：

```bash
RIME_INCLUDE_DIR=/opt/homebrew/opt/librime/include \
RIME_LIB_DIR=/opt/homebrew/opt/librime/lib \
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime \
  --schema luna_pinyin \
  --shared-data /tmp/radishlex-rime-smoke.<id>/shared \
  --user-data /tmp/radishlex-rime-smoke.<id>/user \
  --rank-db /tmp/radishlex-userdb.sqlite \
  --context chat \
  luobo
```

输出形态：

```text
schema: luna_pinyin
input: luobo
composition: luo bo
rank_context: chat
candidates:
  0. <candidate> (engine_index=<n> score=<score>)
     explain: engine_order=<score> user_term=<score> frequency=<score> recency=<score> context=<score> negative=<score> suppressed=<score> deleted=<score>
commit: <text>
commit_engine_index: <n>
```

字段含义：

- `engine_index`：该候选在底层 engine 当前候选列表中的原始索引。
- `score`：ranker 最终分数。
- `explain`：本地 userdb 与 ranker summary 对该候选的排序贡献。
- `commit_engine_index`：最终提交给 `commit_candidate` 的原始 engine index。

该模式只读取显式传入的 `--rank-db`，不把 Rime 内部对象 ID 写入 userdb。

## dict 命令

`dict` 命令用于管理本地 SQLite userdb 中的用户词条。所有子命令都必须显式传入 `--db <path>`，CLI 不会隐式读取系统输入法目录或真实 Rime 用户目录。

添加词条：

```bash
cargo run -p radishlex-ime-cli -- \
  dict add \
  --db /tmp/radishlex-userdb.sqlite \
  --input luobo \
  --text 萝卜 \
  --reading "luo bo"
```

成功后输出新增词条、输入码、状态和初始权重。

查看词条：

```bash
cargo run -p radishlex-ime-cli -- dict list --db /tmp/radishlex-userdb.sqlite
```

输出按输入码列出词条文本、reading、source、status 和 weight。

删除词条：

```bash
cargo run -p radishlex-ime-cli -- \
  dict delete \
  --db /tmp/radishlex-userdb.sqlite \
  --input luobo \
  --text 萝卜 \
  --reading "luo bo"
```

删除会写入 tombstone，并清除对应 ranker 权重摘要，避免旧选择事件或旧导入立即复活该词。后续如需恢复同一词条，必须通过 `dict add` 这类明确的人工添加动作。

导出用户词库：

```bash
cargo run -p radishlex-ime-cli -- \
  dict export \
  --db /tmp/radishlex-userdb.sqlite \
  --file /tmp/radishlex-terms.tsv
```

成功后输出导出数量、文件路径和格式版本。

检查导入文件兼容性：

```bash
cargo run -p radishlex-ime-cli -- \
  dict inspect \
  --file /tmp/radishlex-terms.tsv
```

成功后输出格式版本、记录数、同步分级和兼容状态。当前只支持 `radishlex-user-terms-v1`，未来版本文件会返回明确的不兼容错误，不会被当成 v1 继续导入。

导入用户词库：

```bash
cargo run -p radishlex-ime-cli -- \
  dict import \
  --db /tmp/radishlex-userdb.sqlite \
  --file /tmp/radishlex-terms.tsv \
  --dry-run
```

预览输出不会写入词条或 `import_batches`，会返回 `dry_run: true`、`would_import`、`total`、`inserted`、`updated`、`skipped_deleted`、`skipped_duplicate`、`source` 和 `file`。

实际导入用户词库：

```bash
cargo run -p radishlex-ime-cli -- \
  dict import \
  --db /tmp/radishlex-userdb.sqlite \
  --file /tmp/radishlex-terms.tsv \
  --source smoke
```

成功后输出 `import_batch`、`imported`、`total`、`inserted`、`updated`、`skipped_deleted`、`skipped_duplicate`、`source` 和 `file`。

查看导入批次：

```bash
cargo run -p radishlex-ime-cli -- \
  dict import-batches \
  --db /tmp/radishlex-userdb.sqlite
```

输出每个批次的来源、导入数量、插入数量、更新数量、跳过数量、总记录数和创建时间；无批次时显示 `<none>`。

导入导出格式为 UTF-8 TSV：

```text
# radishlex-user-terms-v1
input_code	text	reading	source	weight	status
luobo	萝卜	luo bo	manual_add	2	active
```

字段固定为 `input_code`、`text`、`reading`、`source`、`weight`、`status`。字段内 tab、换行、回车和反斜杠分别写作 `\t`、`\n`、`\r` 和 `\\`。

边界：

- `dict export` 只导出 P2 用户词条字段，不导出 P1 原始选择事件、负反馈详细事件、上下文统计或 ranker 权重摘要。
- `dict import` 不接受 `deleted` 状态的词条。
- `dict import` 遇到本地 deleted tombstone 命中的词条会跳过，并计入 `skipped_deleted`，不会把普通导入当成恢复删除词条。
- `dict import` 遇到同一导入文件内重复的 `input_code`、`text`、`reading` 身份会跳过后续重复项，并计入 `skipped_duplicate`。
- `dict import` 对已存在且未删除的同身份词条执行更新，并计入 `updated`；不同 `reading` 视为不同词条。
- `--source <name>` 只用于记录导入批次来源，允许 ASCII 字母、数字、dot、underscore 和 dash，最长 64 bytes；未传时为 `cli`。
- `dict export --output <path>` 与 `dict import --input <path>` 是路径别名；正式文档优先使用 `--file <path>`。
- `dict inspect --input <path>` 是 `dict inspect --file <path>` 的路径别名。

## learn 命令

`learn` 命令用于查看本地学习状态或向 userdb 写入本地学习事件。当前只支持显式数据库路径、合成数据和人工指定参数，适合验证排序变化，不代表平台壳已经接入真实输入事件。

查看学习状态只读摘要：

```bash
cargo run -p radishlex-ime-cli -- \
  learn status \
  --db /tmp/radishlex-userdb.sqlite
```

输出：

```text
learning_status: ready
schema_version: 2
plaintext_payload: false
p1_raw_details: false
context_stats: false
p2_learning:
  active_user_terms: 1
  suppressed_user_terms: 1
  ranker_weights: 1
  deleted_tombstones: 0
p1_local_only:
  selection_events: 1
  negative_feedback: 1
local_audit:
  import_batches: 0
latest_activity:
  user_terms_updated_at_ms: <timestamp-or-none>
  selection_event_at_ms: <timestamp-or-none>
  negative_feedback_at_ms: <timestamp-or-none>
  deleted_term_at_ms: <timestamp-or-none>
  import_batch_at_ms: <timestamp-or-none>
  overall_at_ms: <timestamp-or-none>
```

`learn status` 面向后续管理 UI 的学习状态概览，只输出聚合计数、最新活动时间和隐私边界标记。它不输出 P1 原始选择事件、负反馈 reason 明细、上下文分布、用户词明文或同步明文 payload。

记录一次候选选择：

```bash
cargo run -p radishlex-ime-cli -- \
  learn select \
  --db /tmp/radishlex-userdb.sqlite \
  --input luobo \
  --text 萝卜 \
  --index 1 \
  --count 2 \
  --context chat
```

参数：

- `--index <n>`：候选在 RadishLex candidate 列表中的 `0` 基索引，默认 `0`。
- `--count <n>`：本次候选总数，默认 `index + 1`。
- `--session <id>`：本次 CLI 学习事件的会话标识，默认 `cli`。
- `--context <kind>`：归类后的上下文，例如 `general`、`chat`、`code`、`search`，默认 `general`。

成功后输出 selection event id、输入码和候选文本。

记录一次负反馈：

```bash
cargo run -p radishlex-ime-cli -- \
  learn suppress \
  --db /tmp/radishlex-userdb.sqlite \
  --input luobo \
  --text 萝卜 \
  --reason manual_suppress
```

当前支持的 `--reason`：

```text
immediate_backspace
reselect_same_code
manual_suppress
manual_delete
```

未传 `--reason` 时默认使用 `manual_suppress`。负反馈会记录 P1 本地事件，并更新对应 ranker 权重摘要；如果词条存在且未删除，会把词条状态降为 `suppressed`。

## rank explain 命令

`rank explain` 用于从 userdb 读取用户词条和权重摘要，调用 `ime-ranker` 输出结构化排序解释。

```bash
cargo run -p radishlex-ime-cli -- \
  rank explain \
  --db /tmp/radishlex-userdb.sqlite \
  --input luobo \
  --candidate 萝卜 \
  --context chat
```

输出：

```text
input: luobo
candidate: 萝卜
context: chat
original_index: 0
final_score: 2.650
explain:
  engine_order_factor: 1.000
  user_term_boost: 1.000
  frequency_boost: 0.350
  recency_boost: 0.000
  context_boost: 0.300
  negative_feedback_penalty: 0.000
  suppressed_penalty: 0.000
  deleted_penalty: 0.000
```

解释字段含义：

- `engine_order_factor`：原始 engine 候选顺序折算因子；CLI 当前只解释单个候选，所以原始索引为 `0`。
- `user_term_boost`：用户词条带来的提升。
- `frequency_boost`：选择频次带来的提升。
- `recency_boost`：近期使用带来的提升。
- `context_boost`：同一 `context_kind` 下的场景提升。
- `negative_feedback_penalty`：负反馈权重惩罚。
- `suppressed_penalty`：词条处于 suppressed 状态时的额外惩罚。
- `deleted_penalty`：删除 tombstone 命中时的惩罚。

## sync preflight 命令

`sync preflight` 是同步实现前的本地检查入口，只统计 userdb 当前可进入后续加密同步对象的数据类别，不生成明文 payload，不连接后端。

```bash
cargo run -p radishlex-ime-cli -- \
  sync preflight \
  --db /tmp/radishlex-userdb.sqlite
```

输出包含 `dictionary.user_terms`、`ranker.weights`、`dictionary.deleted_terms` 这类 P2 可同步计数，以及 `selection_events`、`negative_feedback` 这类 P1 本地计数。`plaintext_payload: false` 表示该命令没有输出明文同步对象。

## 输入限制

当前 CLI 的 `<input-code>` 与 `--input <code>` 只接受：

- ASCII 字母
- ASCII 数字
- apostrophe，即 `'`

其他字符会返回用法错误。该限制是 CLI 复验入口的输入约束，不代表后续平台壳只能接收这些按键。

`--key` 仅用于 `rime` 命令的 smoke 调试，不改变 `<input-code>` 的字符限制，也不代表后续平台壳的完整按键协议。

## 退出码

- `0`：命令成功。
- `1`：core、engine、userdb 或 ranker 运行错误，例如底层 engine 初始化失败、候选提交失败、SQLite 读写失败或学习事件被隐私策略跳过。
- `2`：命令用法错误，例如缺少参数、未知选项、候选索引不是非负整数，或未启用 `native-rime` 运行 `rime`。

## 安全与隐私边界

- CLI 不注册系统输入法，不接管键盘输入。
- CLI 不上传输入内容，不连接 RadishLex 后端。
- `demo` 不读取本机输入法数据。
- `rime` 必须显式指定 `shared-data` 与 `user-data`，不应指向真实 Rime 用户目录。
- `rime --rank-db` 必须显式指定隔离 userdb，建议使用 `/tmp` 下临时 SQLite 文件。
- `dict`、`learn` 和 `rank explain` 必须显式指定 `--db`，不应指向真实用户生产库；本阶段建议使用 `/tmp` 下临时 SQLite 文件。
- `dict import/export` 的文件也建议放在 `/tmp` 下，测试内容使用合成词，不应导入真实个人词库或真实输入历史。
- `learn status` 只输出聚合学习状态，不输出用户词明文、P1 事件明细、负反馈 reason 明细或上下文统计。
- `sync preflight` 只输出分类计数，不输出用户词明文、事件明文或加密 payload。
- `learn` 当前没有平台 secure text entry 信号输入，CLI smoke 只应使用合成词、虚构上下文和临时数据库。
- 本机 smoke 应使用 `/tmp` 下的隔离目录和合成输入码，不提交 schema 数据、用户目录、日志或输出中的敏感内容。

## 常见错误

- `rime command requires building ... --features native-rime`：当前构建未启用真实 Rime feature；用 `cargo run -p radishlex-ime-cli --features native-rime -- rime ...` 复验。
- `missing --schema`：`rime` 命令缺少 schema id；同时提供 `--schema`、`--shared-data` 和 `--user-data`。
- `candidate index must be a non-negative integer`：候选索引必须是 `0`、`1`、`2` 这类非负整数。
- `candidate index ... did not produce commit text`：先确认当前候选列表，再选择存在的候选索引；真实 Rime 候选文本会受数据版本和隔离 user data 学习状态影响。
- `unknown key name: ...`：`--key` 只接受文档列出的命名键，例如 `page-down`、`page-up`、`arrow-down`、`arrow-up`。
- `--context requires --rank-db for rime`：`rime --context` 只在 rank smoke 中有效，必须同时传入 `--rank-db`。
- `missing --db`：`dict`、`learn` 或 `rank explain` 必须显式指定 SQLite 路径。
- `invalid import_file`：导入文件需要符合 `radishlex-user-terms-v1` TSV，且词条状态只能是 `active` 或 `suppressed`。
- `unknown negative feedback reason ...`：`learn suppress --reason` 只接受 `immediate_backspace`、`reselect_same_code`、`manual_suppress` 或 `manual_delete`。
