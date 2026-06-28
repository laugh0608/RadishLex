# RadishLex FFI 边界

本文档定义后续 `ime-ffi` 的 ABI 职责、数据所有权、错误语义和平台壳停止线，读者是后续实现 C ABI、Flutter bridge、Swift/Kotlin/C++ 调用层和平台输入法薄壳的开发者。本文不包含具体平台输入法注册流程、TSF / InputMethodKit / Fcitx5 API 调用细节、Flutter 页面设计或移动端键盘 UI。

## 当前定位

当前已落地 `crates/ime-ffi/` 起步验证：C ABI 已覆盖 opaque session handle、ABI contract、session owner-thread policy、session options、Rime session options ABI、engine kind 门禁、错误对象、UTF-8 buffer、结构化 snapshot handle、candidate view、normalized key event、释放函数、schema 设置、按键输入、snapshot、候选提交、userdb learning status 只读摘要、userdb sync preflight 状态摘要、受控 userdb 词条管理入口、dictionary inspect / export / import 和 import batches 只读查询的 host smoke。当前 session 内部已使用 demo / Rime 可扩展 engine 封装；默认构建仍只启用 deterministic demo engine，`native-rime` feature 下 `radishlex_session_new_rime` 可通过显式 Rime 配置创建真实 `RimeEngine` session，并已通过隔离 Rime 数据目录 smoke。该状态仍不代表平台壳或系统输入法已经接入。

平台壳后续只能通过 FFI 调用 Rust core，不得直接访问 SQLite、Rime 私有对象或 ranker 内部状态。
平台绑定层的具体调用清单见 `docs/runbooks/ffi-platform-call-contract.md`。

## 职责边界

`ime-ffi` 负责：

- 暴露稳定 C ABI。
- 管理 Rust core session 句柄。
- 接收平台按键事件并返回 composition、candidate、commit 快照。
- 暴露用户词库管理、学习开关和同步状态的受控入口。
- 统一错误码、错误消息读取和内存释放规则。

`ime-ffi` 不负责：

- 注册系统输入法。
- 绘制平台候选窗。
- 直接连接 Go server。
- 保存平台私有窗口句柄或 UI 对象。
- 把 Rime session、Rime candidate 指针或 SQLite connection 暴露给平台端。

## ABI 基本模型

后续 C ABI 应优先采用 opaque handle：

```text
RadishLexSession*
RadishLexBuffer*
RadishLexSnapshot*
RadishLexUserTermList*
RadishLexError*
```

平台端只能持有 opaque pointer，不能解引用 Rust 内部结构。跨 ABI 文本优先使用带长度的 UTF-8 view；需要 Rust 分配的 buffer 或 snapshot handle 时，必须由 Rust 提供释放函数。

当前已落地函数按能力分组：

- ABI contract：`radishlex_ffi_contract`
- session 生命周期：`radishlex_session_new`、`radishlex_session_new_with_options`、`radishlex_session_new_rime`、`radishlex_session_free`、`radishlex_session_engine_kind`、`radishlex_session_reset`、`radishlex_session_set_schema`
- 输入与快照：`radishlex_session_push_key`、`radishlex_session_push_key_event`、`radishlex_session_snapshot`、`radishlex_session_snapshot_new`、`radishlex_snapshot_*`、`radishlex_session_commit_candidate`
- userdb 状态与词条管理：`radishlex_userdb_learning_status`、`radishlex_userdb_sync_preflight`、`radishlex_userdb_add_term`、`radishlex_userdb_delete_term`、`radishlex_userdb_terms_*`
- dictionary 文件与导入审计：`radishlex_userdb_dictionary_*`、`radishlex_userdb_import_batches_*`
- Rust 分配对象读取与释放：`radishlex_buffer_*`、`radishlex_error_*`

## 当前 ABI 数据结构

所有 `repr(C)` 结构只承载稳定 ABI 字段，不暴露 Rust 内部对象。数值常量是 ABI 的一部分，平台绑定层应使用常量名，不应直接依赖 Rust enum discriminant。

### FFI contract

`radishlex_ffi_contract` 返回当前 ABI 契约版本、session 线程策略和 panic 边界策略。当前 `session_thread_policy = owner_thread`，表示 `RadishLexSession*` 只能在创建线程使用；跨线程调用返回 `InvalidState`，无 `error_out` 的 session 读取入口返回空值。当前 `panic_boundary = catch_unwind`，表示带错误返回的入口和释放入口都不得让 panic 穿过 C ABI。

### Status 与文本 view

`RadishLexStatusCode`：

```text
Ok = 0
InvalidArgument = 1
InvalidState = 2
EngineError = 3
UserDbError = 4
RankerError = 5
SyncError = 6
InternalError = 255
```

`RadishLexStringView`：

```text
data: *const u8
len: usize
```

规则：

- `data + len` 表示 UTF-8 字节片段，不保证 NUL 结尾。
- `len = 0` 时 `data` 可以为空指针。
- view 只在其所属 handle 存活期间有效，例如 snapshot view 依赖 `RadishLexSnapshot*`，term view 依赖 `RadishLexUserTermList*`。

### Session options 与 engine kind

`RadishLexSessionOptions`：

```text
version: u32
engine_kind: u32
```

当前常量：

```text
RADISHLEX_SESSION_OPTIONS_VERSION = 1
RADISHLEX_ENGINE_KIND_DEMO = 1
RADISHLEX_ENGINE_KIND_RIME = 2
```

当前只允许 demo engine。Rime kind 作为后续稳定入口预留，当前返回 `InvalidState`，避免平台端误以为真实 Rime FFI 已可用。

### Rime session options

`RadishLexRimeSessionOptions`：

```text
version: u32
shared_data_dir: *const c_char
user_data_dir: *const c_char
schema: *const c_char
log_dir: *const c_char
deploy_on_start: u8
```

当前常量：

```text
RADISHLEX_RIME_SESSION_OPTIONS_VERSION = 1
```

规则：

- `radishlex_session_new_rime` 是后续真实 Rime FFI 的专用构造入口，不复用 `RadishLexSessionOptions.engine_kind = RIME` 承载路径和 schema。
- `shared_data_dir`、`user_data_dir` 和 `schema` 必须是非空 UTF-8 C string；`log_dir` 可以为空指针，传入时也必须非空 UTF-8。
- `deploy_on_start` 只接受 `0` 或 `1`，避免跨语言 bool 布局差异。
- 默认 workspace 构建下，参数通过 ABI 校验后返回 `InvalidState`，错误消息明确说明需要用 `native-rime` feature 构建 `ime-ffi`。
- 启用 `native-rime` feature 且本机 `librime` 可用时，该入口会把 options 转成 `RimeEngineConfig` 并创建真实 `RimeEngine` session。
- Rime session 必须使用隔离的 Rime shared / user data 目录；不得静默退回 demo engine，不得读取真实用户输入法目录。
- 平台端不能缓存这些路径指针；Rust 侧只在调用期间借用传入字符串，并在 `RimeEngineConfig` / native string 管理中复制必要配置。

### Key event

`RadishLexKeyEvent`：

```text
key_kind: u32
codepoint: u32
named_key: u32
modifiers: u32
phase: u32
```

当前 key kind：

```text
RADISHLEX_KEY_KIND_CHAR = 1
RADISHLEX_KEY_KIND_NAMED = 2
```

当前 named key：

```text
space = 1
enter = 2
backspace = 3
escape = 4
tab = 5
arrow_up = 6
arrow_down = 7
arrow_left = 8
arrow_right = 9
page_up = 10
page_down = 11
shift = 12
control = 13
alt = 14
meta = 15
unknown = 255
```

当前 modifiers bit：

```text
shift = 1 << 0
control = 1 << 1
alt = 1 << 2
meta = 1 << 3
```

当前 phase：

```text
press = 1
release = 2
```

字符键必须提供合法 Unicode scalar value。未知 key kind、未知 named key、未知 modifier bit 或未知 phase 均返回 `InvalidArgument`。

### Snapshot 与 candidate view

`RadishLexCandidateView`：

```text
index: usize
text: RadishLexStringView
reading: RadishLexStringView
reading_present: u8
annotation: RadishLexStringView
annotation_present: u8
source: u32
```

当前 candidate source：

```text
engine = 1
user_dictionary = 2
personalized = 3
system = 4
```

`reading_present` 和 `annotation_present` 用于区分“字段不存在”和“存在但为空字符串”。candidate view 中所有 string view 都借用自 `RadishLexSnapshot*`。

### Learning status summary

`RadishLexLearningStatusSummary`：

```text
schema_version: i64
plaintext_payload: u8
p1_raw_details: u8
context_stats: u8
active_user_terms: usize
suppressed_user_terms: usize
ranker_weights: usize
deleted_term_tombstones: usize
selection_events: usize
negative_feedback: usize
import_batches: usize
latest_user_term_updated_at_ms: i64
latest_user_term_updated_at_present: u8
latest_selection_event_at_ms: i64
latest_selection_event_at_present: u8
latest_negative_feedback_at_ms: i64
latest_negative_feedback_at_present: u8
latest_deleted_term_at_ms: i64
latest_deleted_term_at_present: u8
latest_import_batch_at_ms: i64
latest_import_batch_at_present: u8
latest_activity_at_ms: i64
latest_activity_at_present: u8
```

`plaintext_payload`、`p1_raw_details` 和 `context_stats` 当前固定为 `0`，表示该入口不生成明文同步 payload、不导出 P1 原始事件明细、不返回上下文分布统计。`*_present` 字段用于区分对应 latest timestamp 不存在和存在但值为 `0`。

### Sync preflight summary

`RadishLexSyncPreflightSummary`：

```text
schema_version: i64
plaintext_payload: u8
syncable_user_terms: usize
syncable_ranker_weights: usize
syncable_deleted_terms: usize
local_selection_events: usize
local_negative_feedback: usize
local_import_batches: usize
```

`plaintext_payload` 当前固定为 `0`，表示没有生成明文同步 payload，也没有连接远端服务。`syncable_*` 是后续可进入加密对象的 P2 计数，`local_*` 是不得直接同步的 P1 或本地审计计数。

Rust 内部的 `UserDb::p2_plaintext_payloads()`、`ime-sync::SyncEnvelopeAssembler`、`ime-crypto::EncryptedObjectEnvelope` 和 `ime-sync::EncryptedSyncObjectDraft` 当前只用于 crate 内测试与 Rust 内部组装边界。FFI 不导出这些对象，也不导出 payload bytes、密文、hash、签名、key id 或上传草案。

### User term view

`RadishLexUserTermView`：

```text
id: i64
input_code: RadishLexStringView
text: RadishLexStringView
reading: RadishLexStringView
reading_present: u8
source: u32
status: u32
weight: f64
created_at_ms: i64
updated_at_ms: i64
last_used_at_ms: i64
last_used_at_present: u8
```

当前 term source：

```text
engine_selection = 1
manual_import = 2
manual_add = 3
phrase_learning = 4
```

当前 term status：

```text
active = 1
suppressed = 2
deleted = 3
```

term list 当前只返回 active / suppressed 词条。删除 tombstone 不通过 list 暴露；需要通过删除语义和 sync preflight 的 `syncable_deleted_terms` 观察。

### Dictionary file summaries

当前 dictionary file FFI 只处理用户明确管理的 P2 用户词条 TSV，不导出 P1 选择事件、负反馈明细、上下文统计或 ranker 权重摘要。

当前常量：

```text
RADISHLEX_DICTIONARY_FORMAT_USER_TERMS_V1 = 1
RADISHLEX_SYNC_CLASS_P2_ENCRYPTED_SYNC = 2
```

`RadishLexDictionaryInspectSummary`：

```text
format_version: u32
record_count: usize
sync_class: u32
```

`RadishLexDictionaryExportSummary`：

```text
format_version: u32
exported_terms: usize
sync_class: u32
```

`RadishLexDictionaryImportSummary`：

```text
import_batch_id: i64
import_batch_id_present: u8
total_records: usize
imported_terms: usize
inserted_terms: usize
updated_terms: usize
skipped_deleted_terms: usize
skipped_duplicate_terms: usize
dry_run: u8
```

`RadishLexImportBatchView`：

```text
id: i64
source_name: RadishLexStringView
total_records: usize
imported_terms: usize
inserted_terms: usize
updated_terms: usize
skipped_deleted_terms: usize
skipped_duplicate_terms: usize
created_at_ms: i64
notes: RadishLexStringView
notes_present: u8
```

规则：

- `radishlex_userdb_dictionary_inspect` 只读取导入文件并返回格式版本、记录数和同步分类，不打开 userdb。
- `radishlex_userdb_dictionary_export` 必须显式传入 SQLite 路径和输出文件路径，只导出 active / suppressed 用户词条字段。
- `radishlex_userdb_dictionary_import` 必须显式传入 SQLite 路径、输入文件路径、可选 source name 和 `dry_run` 的 `0 / 1` 值。
- `dry_run = 1` 时复用实际导入分类逻辑，但不写入词条或 import batch。
- `dry_run = 0` 时写入词条并记录 import batch；导入仍遵守 deleted tombstone，不复活用户已删除词条。
- `radishlex_userdb_import_batches_new` 返回只读 `RadishLexImportBatchList*`，由 `radishlex_userdb_import_batches_free` 释放。
- import batch view 中的 string view 借用自 batch list handle，平台端只能在 list 释放前读取，不得缓存裸指针。

`radishlex_session_push_key` 保留为字符输入便利函数；真实平台壳后续应优先使用 `radishlex_session_push_key_event`。当前 normalized key event 使用数值常量承载字符键、命名键、修饰键、按下 / 释放阶段和平台不可识别键，避免让无效 enum discriminant 在 FFI 边界形成未定义行为。

Engine adapter 选择规则：

- `radishlex_session_new` 等价于创建 demo engine session。
- `radishlex_session_new_with_options` 接收带 `version` 的 `RadishLexSessionOptions`，当前只允许 `RADISHLEX_ENGINE_KIND_DEMO`。
- `RADISHLEX_ENGINE_KIND_RIME` 已保留为稳定 kind，但当前返回 `InvalidState`；真实 Rime adapter 不通过该通用 options 入口传路径。
- `radishlex_session_new_rime` 是 Rime 专用构造入口，负责校验 `RadishLexRimeSessionOptions`；默认构建下返回 `InvalidState`，`native-rime` feature 下创建真实 Rime session。
- 未知 options version 或未知 engine kind 返回 `InvalidArgument`。
- 平台端不能直接创建或持有 Rime session、Rime candidate 指针或底层 native handle。

结构化 snapshot 规则：

- `radishlex_session_snapshot_new` 返回 `RadishLexSnapshot*`，由 `radishlex_snapshot_free` 释放。
- `radishlex_snapshot_schema`、`radishlex_snapshot_preedit` 和 candidate view 中的文本均为借用自 snapshot 的 UTF-8 `data + len` view。
- 平台端只能在 snapshot 释放前读取这些 view；不得缓存 view 指针。
- `radishlex_snapshot_candidate` 通过输出参数返回单个 `RadishLexCandidateView`，候选越界或输出指针为空时返回 `InvalidArgument`。
- `radishlex_session_snapshot` 保留为调试用文本 buffer，不作为后续平台壳读取 composition / candidate 的主入口。

Userdb 状态入口规则：

- `radishlex_userdb_learning_status` 必须显式传入 UTF-8 SQLite 路径，函数只在调用期间打开数据库并运行 migration / learning status 聚合查询。
- learning status 返回结构只包含 schema version、用户词条状态计数、ranker weight 计数、deleted tombstone 计数、P1 本地事件计数、本地审计计数、latest timestamp 和 `plaintext_payload / p1_raw_details / context_stats = false`。
- learning status 不返回用户词明文、选择事件明细、负反馈 reason 明细、上下文统计、导入批次内容、同步 payload、SQLite connection、statement 或 row 指针。
- `radishlex_userdb_sync_preflight` 必须显式传入 UTF-8 SQLite 路径，函数只在调用期间打开数据库并运行 migration / preflight 计数。
- 返回结构只包含 schema version、P2 可同步对象计数、P1 本地事件计数、本地审计计数和 `plaintext_payload = false`。
- 函数不返回用户词明文、选择事件明细、负反馈明细、导入批次内容、同步 payload、SQLite connection、statement 或 row 指针。
- 该入口不连接 Go server，不执行加密、hash、签名、上传下载或冲突合并，也不暴露 Rust 内部 P2 plaintext payload 迭代器或 envelope / draft 类型。

Userdb 词条管理入口规则：

- `radishlex_userdb_add_term` 和 `radishlex_userdb_delete_term` 必须显式传入 UTF-8 SQLite 路径、输入码、词条文本和可选 reading。
- 这些入口只表达用户明确管理的 P2 词条操作，不记录 P1 selection event、negative feedback 或上下文统计。
- `radishlex_userdb_delete_term` 必须沿用 userdb tombstone 语义，后续旧权重或普通导入不得立即复活该词条。
- `radishlex_userdb_terms_new` 返回只读 `RadishLexUserTermList*`，由 `radishlex_userdb_terms_free` 释放。
- `radishlex_userdb_terms_get` 返回的 string view 借用自 term list handle，平台端只能在 list 释放前读取，不得缓存指针。
- term list 只列出当前 active / suppressed 用户词条；deleted tombstone 不通过词条列表导出，只通过删除语义和 sync preflight 计数体现。
- dictionary file 入口同样只处理用户明确管理的 P2 词条，不记录 P1 selection event、negative feedback 或上下文统计。

## 所有权与生命周期

规则：

- 创建函数返回的 handle 必须由对应 `*_free` 释放。
- Rust 分配的字符串、数组和 snapshot buffer 必须由 Rust 释放。
- 平台端传入的字符串只在调用期间借用，Rust 不保存裸指针。
- `RadishLexSession*` 绑定创建线程；平台端如需跨线程调度输入，必须在平台侧投递回创建线程，或后续在 Rust 侧显式建模线程安全队列。
- FFI 不跨线程共享 snapshot、buffer、term list、import batch list 或 error 裸指针。
- session drop 必须释放 engine adapter、userdb handle 和临时 buffer。
- panic 不能跨 FFI 边界，必须转换为错误码。

## 错误语义

错误返回必须同时支持机器可判定和人工可诊断：

```text
Ok
InvalidArgument
InvalidState
EngineError
UserDbError
RankerError
SyncError
InternalError
```

平台端不能只依赖错误消息做分支；错误消息用于日志和诊断。错误消息不得包含明文输入历史、真实联系人、密码、证件号、支付信息或窗口正文。

## 数据边界

允许跨 FFI：

- 输入码和归一化按键事件。
- composition preedit 和 cursor。
- candidate 文本、reading、annotation、rank explain 摘要。
- commit 文本。
- 用户明确管理的词条。
- 同步状态摘要和对象计数。

禁止跨 FFI：

- Rime 内部指针和私有 ID。
- SQLite connection、statement 或 row 指针。
- 原始 P1 事件明细的批量导出。
- P0 输入内容。
- 平台窗口标题、正文内容、控件句柄和系统私有对象。

## 平台壳停止线

进入平台壳前必须满足：

- `ime-core` 输入会话、candidate、commit 和 engine trait 已稳定。
- Rime adapter 与 rank smoke 可复验。
- userdb 删除 tombstone、导入导出和 ranker explain 已通过测试。
- 同步 payload 草案已区分 P1 本地和 P2 加密同步。
- FFI 文档明确所有权、生命周期、错误语义、字符串编码和释放责任。
- `ime-ffi` 至少有 C ABI 单元测试或 host smoke，证明字符串、数组、snapshot、candidate view、normalized key event、session options、Rime session options、learning status 只读摘要、sync preflight 状态摘要、userdb 管理入口、ABI contract、session owner-thread policy、平台绑定式 view copy / release 和错误释放路径可复验。当前已完成上述 host smoke；真实平台壳前仍需由具体平台 wrapper 复验线程调度、字符串复制和释放规则。

## 验证口径

后续落地 `ime-ffi` 时至少需要：

```text
cargo test -p radishlex-ime-ffi
cargo test -p radishlex-ime-core
cargo test -p radishlex-ime-userdb
cargo test -p radishlex-ime-ranker
./scripts/check-repo.sh
```

涉及平台原生 shell 后，还必须补对应平台 build 或 smoke 记录；真实系统输入法安装、启用、权限和系统目录写入需要人工明确授权。
