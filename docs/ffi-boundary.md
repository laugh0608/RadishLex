# RadishLex FFI 边界

本文档定义 `ime-ffi` 的 ABI 职责、数据所有权、错误语义和平台壳停止线，读者是实现 C ABI、Flutter bridge、Swift/Kotlin/C++ 调用层和平台输入法薄壳的开发者。本文不包含当前批次状态、具体平台输入法注册流程、TSF / InputMethodKit / Fcitx5 API 调用细节、Flutter 页面设计或移动端键盘 UI；实时进度见 `docs/status/current.md`。

## 稳定定位

`ime-ffi` 是 Rust 输入 runtime、平台输入法壳和 Flutter manager 的唯一稳定跨语言边界。平台按键入口必须无损返回 `KeyOutcome` 的 `consumed`、可选即时 commit 与同一事件后的 snapshot；只返回状态码再单独查询状态不能作为真实平台契约。

平台壳只能通过 FFI 调用 Rust runtime，不得直接访问 SQLite、Rime 私有对象或 ranker 内部状态。M3 真实领域模型、secret 生命周期和本地 HTTPS 已具备实现证据；下一批允许增加与普通用户同步入口明确隔离的 Manager 资格执行 ABI，但必须先固定 opaque run 所有权、单次运行、取消/超时、transient 参数和脱敏结果契约。approval、preview、migration review 和 no-symbol 证明不属于生产 ABI。Apple P-256 产品 validation ABI 是例外的受控自检面：普通 DPK 与 Secure Enclave 使用独立 symbol/environment gate，只返回固定 capability/lifecycle flags，不执行真实同步，也不直接进入 Dart binding。ABI v6 另增一条不带入参、不访问系统条目的 `radishlex_manager_sync_product_status`，只把 allowlist 产品状态送入现有 Manager snapshot；它不是同步命令，不能打开产品 gate。平台绑定层调用规则见 `docs/runbooks/ffi-platform-call-contract.md`。

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
RadishLexUserTermList* / RadishLexDeletedTermList*
RadishLexRankExplain*
RadishLexError*
```

平台端只能持有 opaque pointer，不能解引用 Rust 内部结构。跨 ABI 文本优先使用带长度的 UTF-8 view；需要 Rust 分配的 buffer 或 snapshot handle 时，必须由 Rust 提供释放函数。

当前已落地函数按能力分组：

- ABI contract 与 Manager 只读产品状态：`radishlex_ffi_contract`、`radishlex_manager_sync_product_status`
- session / Rime runtime 生命周期：`radishlex_session_new`、`radishlex_session_new_with_options`、`radishlex_session_new_rime`、`radishlex_session_new_personalized_rime`、`radishlex_session_free`、`radishlex_rime_runtime_shutdown`、`radishlex_session_engine_kind`、`radishlex_session_reset`、`radishlex_session_set_schema`、`radishlex_session_set_learning_context`
- 输入、候选选择与快照：`radishlex_session_handle_key_event`、`radishlex_session_select_candidate`、`radishlex_key_result_*`、兼容 `radishlex_session_push_key_event`、`radishlex_session_snapshot_new`、`radishlex_snapshot_*`
- userdb 状态与词条管理：`radishlex_userdb_learning_status`、`radishlex_userdb_sync_preflight`、`radishlex_userdb_rank_explain_*`、`radishlex_userdb_add_term`、`radishlex_userdb_delete_term`、`radishlex_userdb_restore_term`、`radishlex_userdb_terms_*`、`radishlex_userdb_deleted_terms_*`
- dictionary 文件与导入审计：`radishlex_userdb_dictionary_*`、`radishlex_userdb_import_batches_*`
- Rust 分配对象读取与释放：`radishlex_buffer_*`、`radishlex_error_*`、`radishlex_userdb_rank_explain_free`

## 当前 ABI 数据结构

所有 `repr(C)` 结构只承载稳定 ABI 字段，不暴露 Rust 内部对象。数值常量是 ABI 的一部分，平台绑定层应使用常量名，不应直接依赖 Rust enum discriminant。

### FFI contract

`radishlex_ffi_contract` 返回当前 ABI 契约版本、session 线程策略和 panic 边界策略。ABI contract v6 保留 v5 的 `import_batch_id` 与全部既有布局，并增加 Manager status-only 产品摘要 symbol。产品绑定必须同时校验 contract 与所需 symbol 集。当前 `session_thread_policy = owner_thread`，表示 `RadishLexSession*` 只能在创建线程使用；跨线程调用返回 `InvalidState`，无 `error_out` 的 session 读取入口返回空值。当前 `panic_boundary = catch_unwind`，表示带错误返回的入口和释放入口都不得让 panic 穿过 C ABI。

Apple 产品验证使用独立原生自检结构：普通 DPK 的 `radishlex_apple_p256_product_status/smoke` 分别使用 status schema v1 与 smoke schema v4；Secure Enclave signing 的 `radishlex_apple_secure_enclave_p256_product_status/smoke` 分别使用独立 status schema v1 与 smoke schema v1；Secure Enclave key-agreement 另用 `radishlex_apple_secure_enclave_key_agreement_product_status/smoke`，不得继承 signing 资格。三组 status 都是 metadata-only；smoke 只有 manager 产品进程显式场景与各自环境门同时满足才执行。key-agreement 使用独立固定摘要，只返回错误分类、数值 OSStatus、wrapped epoch 往返与 cleanup 布尔值，不返回 CFError 文本、private/public key、shared secret、wrapping key、master key、nonce 或 ciphertext。Dart dynamic binding、`ManagerBridge` 和 Flutter method channel 不得直接声明或调用这些 validation symbol；Dart 只绑定 ABI v6 的脱敏业务摘要。

### Manager sync product status

`radishlex_manager_sync_product_status` 是 ABI v6 的 status-only 入口，只返回固定 enum/boolean 产品摘要，不创建、读取、使用或删除平台 key item。signing 与 key-agreement 资格必须独立表达，组合 `product_qualified` 不能自动打开 `user_sync_enabled`。完整结构、常量、blocker 优先级、隐私 allowlist 和 Dart binding 检查见 [Manager 同步产品状态参考](manager-sync-product-status.md)。

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
- view 只在其所属 handle 存活期间有效，例如 snapshot view 依赖 `RadishLexSnapshot*`，term/deleted term view 分别依赖 `RadishLexUserTermList*` 与 `RadishLexDeletedTermList*`。

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
- 进程 runtime 初始化后，shared / user / log / deploy 配置保持不变；schema 仍属于各 session。配置冲突返回 `InvalidState`，不会重启已有 runtime。
- Rime session 创建和 `radishlex_session_set_schema` 都要求目标存在于 librime 已部署 schema list，并在选择后精确回读当前 schema；不存在或回读不一致返回 engine error，不能把 librime 的宽松成功返回当作有效 session。
- 首个成功初始化的 Rime session 固定进程 runtime owner thread；后续 Rime session 创建、调用、释放和 shutdown 必须使用同一线程，跨线程返回 `InvalidState` 或构成错误释放用法。
- `radishlex_session_free` 只释放对应 session，不触发全局 finalize。进程 teardown 必须在 runtime owner thread 先释放所有 Rime session，再调用可重复的 `radishlex_rime_runtime_shutdown`；仍有 session 时 shutdown 返回 `InvalidState`。

### 产品个人化 session 与学习上下文

`RadishLexPersonalizedRimeSessionOptions` 在 Rime 路径和 schema 之外增加非空 UTF-8 `userdb_path` 与 `session_id`。`session_id` trim 后必须非空且不超过 128 bytes，只用于本地 P1 选择事件关联，不是设备 ID、同步身份或跨进程稳定身份。`radishlex_session_new_personalized_rime` 只在 `native-rime` 构建中创建 `ime-runtime` 产品 session；数据库打开或 migration 失败不得删除、重命名或重建原文件，session 可以保留 engine 输入能力，但必须通过 snapshot 个人化状态暴露不可用原因类别。版本化 `RadishLexLearningContext` 携带 `secure_input`、`sensitive_application`、`privacy_mode`、`context_known` 与 `context_kind`。

布尔字段只允许 `0` 或 `1`。平台只传 `general`、`browser`、`chat`、`code`、`editor`、`office`、`terminal`、`other` 之一，`context_kind` view 必须非空且不超过 32 个 UTF-8 bytes；不得传 App ID、窗口标题或文档内容。secure input、敏感应用或 `context_known = 0` 时 runtime 只保留 engine 顺序且不读写 userdb；隐私模式允许使用既有本地个人化摘要，但禁止当前输入写入。上下文改变时清除未由 commit 确认的分段选择意图并失效旧候选映射；若已有 composition，平台必须刷新 snapshot 后再允许选择。

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

### Key result 与候选选择

真实平台按键与候选选择入口统一采用版本化、Rust-owned 的 `RadishLexKeyResult*`。当前 `RADISHLEX_KEY_RESULT_VERSION = 2`；v2 在 owned key result 上增加 `learning_disposition`，未知版本必须拒绝：

```text
radishlex_session_handle_key_event(
  session,
  event,
  result_out,
  error_out,
) -> RadishLexStatusCode

radishlex_session_select_candidate(
  session,
  display_index,
  result_out,
  error_out,
) -> RadishLexStatusCode
```

成功时 `result_out` 非空，结果至少表达：

```text
version: u32
consumed: u8
commit: RadishLexStringView
commit_present: u8
learning_disposition: u32
snapshot: *const RadishLexSnapshot
```

稳定语义：

- `consumed = 0` 时平台必须把按键交还宿主应用；不能根据 composition 是否为空猜测。
- `commit_present = 1` 时 commit 必须在本次事件结果中返回；平台不能依赖下一次 snapshot 推断提交文本。
- 候选选择索引来自当前 snapshot 的候选页；分段候选可能返回 `consumed = 1`、`commit_present = 0` 和更新后的 composition/candidates，平台不得直接提交展示文本。
- `learning_disposition` 明确区分 `not_applicable`、`recorded`、`deferred`、`skipped_by_policy` 和 `failed`。engine 已产生的 commit 即使学习失败也必须返回；平台不得因 `failed` 丢弃提交文本。
- snapshot 与 `consumed`、commit 必须来自同一次按键处理后的状态，不允许跨事件拼装。
- key result 拥有 commit storage 与 snapshot；其 string/candidate view 只在 result 存活期间有效。
- `radishlex_key_result_free` 负责释放整个结果；平台不得单独释放借用的 snapshot，也不得在释放后缓存任何 view。
- `version` 未知时平台必须明确拒绝，布尔字段只允许 `0` 或 `1`，保留字段必须初始化为零。
- 失败时 `result_out` 保持空，错误通过 status 与 `RadishLexError*` 返回；不得同时返回部分可用结果。

现有只返回 `RadishLexStatusCode` 的按键函数只作为兼容或测试入口，不能作为 InputMethodKit 等真实平台壳的主契约。输入侧声明由 `crates/ime-ffi/include/radishlex_input.h` 提供，并通过 C11、Objective-C 编译和 Rust function pointer 测试约束；Swift / Objective-C 不手抄 Rust `repr(C)` 布局。

### Snapshot 与 candidate view

`RadishLexCandidateView`：

```text
index: usize
engine_index: usize
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

`index` 是本次 snapshot 的 display index，`engine_index` 是 runtime 固化的 engine 原始索引；平台选择时只回传 display index，不自行换算或假定两者相等。`reading_present` 和 `annotation_present` 用于区分“字段不存在”和“存在但为空字符串”。candidate view 中所有 string view 都借用自 `RadishLexSnapshot*`。snapshot 个人化状态固定区分 `not_enabled`、`ready`、`policy_blocked`、`storage_unavailable`、`read_failed` 和 `rank_failed`：legacy/demo/Rime session 使用 `not_enabled`，personalized session 使用其余状态；平台只显示受控状态，不记录候选或数据库路径。

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

### Rank explain view

`RadishLexRankExplainView` 字段：`input_code`、`candidate_text`、`reading`、`reading_present`、`context_kind`、`original_index`、`final_score`、`engine_order_factor`、`user_term_boost`、`frequency_boost`、`recency_boost`、`context_boost`、`negative_feedback_penalty`、`suppressed_penalty`、`deleted_penalty`。
规则：

- `radishlex_userdb_rank_explain_new` 必须显式传入 UTF-8 SQLite 路径、输入码、候选文本、可选 reading 和可选 context kind。
- 输入码和候选文本不能为空；reading 为空或空白时按无 reading 处理，context kind 为空或空白时按 `general`。
- 该入口只对单个候选返回稳定贡献项摘要，用 userdb 中匹配的用户词条、ranker weight 和删除 tombstone 调用 `ime-ranker`；不返回选择事件明细、负反馈 reason 明细、上下文统计分布、SQLite connection、statement、row 指针或同步 payload。
- 返回的 `RadishLexRankExplain*` 由 `radishlex_userdb_rank_explain_free` 释放；`radishlex_userdb_rank_explain_view` 返回的 string view 借用自该 handle，平台端必须在释放前复制。
- `final_score` 是 `ime-ranker` 对该候选的当前解释分数；平台 UI 可以展示摘要，但不得把这些字段作为输入热路径之外的业务真相源。

### Userdb dictionary views

user term、deleted tombstone、dictionary inspect/export/import 与 import batch 的字段级结构、数值常量和本地审计语义统一见 [FFI Dictionary Reference](ffi-dictionary-reference.md)。本文件只保留所有权、隐私和跨平台通用边界。

ABI v5 的 `import_batch_id` 只关联最近一次真实写入词条的本地导入批次，不进入 P2 payload；所有 term/deleted/batch string view 都借用自对应 owned list handle，平台绑定必须复制字段后再释放 handle。

`radishlex_session_push_key` 与 `radishlex_session_push_key_event` 保留为兼容和测试入口；真实平台壳必须使用 `radishlex_session_handle_key_event`。当前 normalized key event 使用数值常量承载字符键、命名键、修饰键、按下 / 释放阶段和平台不可识别键，避免让无效 enum discriminant 在 FFI 边界形成未定义行为。

Engine adapter 选择规则：

- `radishlex_session_new` 等价于创建 demo engine session。
- `radishlex_session_new_with_options` 接收带 `version` 的 `RadishLexSessionOptions`，当前只允许 `RADISHLEX_ENGINE_KIND_DEMO`。
- `RADISHLEX_ENGINE_KIND_RIME` 已保留为稳定 kind，但当前返回 `InvalidState`；真实 Rime adapter 不通过该通用 options 入口传路径。
- `radishlex_session_new_rime` 是 Rime 专用构造入口，负责校验 `RadishLexRimeSessionOptions`；默认构建下返回 `InvalidState`，`native-rime` feature 下创建真实 Rime session。
- `radishlex_rime_runtime_shutdown` 是进程 teardown 入口；默认构建下返回 `InvalidState`，native 构建下只在零活动 Rime session 时 finalize，重复 shutdown 返回成功。
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
- `radishlex_userdb_rank_explain_new` / `radishlex_userdb_rank_explain_view` 只返回单候选 ranker explain 摘要；view 字符串借用自 explain handle，平台绑定层必须复制后再释放 handle。
- rank explain 摘要允许包含候选文本和 reading，因为它面向用户正在管理的本地词条；不得扩展为 P1 原始事件明细、上下文分布、同步 payload 或 ranker 内部可变对象导出。

Userdb 词条管理入口规则：

- `radishlex_userdb_add_term`、`radishlex_userdb_delete_term` 和 `radishlex_userdb_restore_term` 必须显式传入 UTF-8 SQLite 路径、输入码、词条文本和可选 reading。
- `radishlex_userdb_add_term` 只新增或更新未删除词条，不能清除 tombstone 或隐式解除 suppress；`radishlex_userdb_restore_term` 是唯一显式恢复动作，必须沿用 userdb 的严格新版本与原子清除 tombstone 语义。
- 这些入口只表达用户明确管理的 P2 词条操作，不记录 P1 selection event、negative feedback 或上下文统计。
- `radishlex_userdb_delete_term` 必须沿用 userdb tombstone 语义，后续旧权重或普通导入不得立即复活该词条。
- `radishlex_userdb_terms_new` 返回只读 `RadishLexUserTermList*`，由 `radishlex_userdb_terms_free` 释放。
- `radishlex_userdb_terms_get` 返回的 string view 借用自 term list handle，平台端只能在 list 释放前读取，不得缓存指针。
- term list 只列出当前 active / suppressed 用户词条。
- `radishlex_userdb_deleted_terms_new` 返回只读 `RadishLexDeletedTermList*`，由 `radishlex_userdb_deleted_terms_free` 释放；`count/get` 的 view 借用自该 handle，平台端必须复制 input code、text、reading、deleted timestamp 和 reason 后再释放。
- deleted list 只提供 explicit restore 所需的 P2 identity 与删除摘要，不提供 P1 原始 selection/negative event、上下文、SQL row ID 或内部 tombstone version；普通导入、学习和新增不得据此隐式调用 restore。
- dictionary file 入口同样只处理用户明确管理的 P2 词条，不记录 P1 selection event、negative feedback 或上下文统计。

## 所有权与生命周期

规则：

- 创建函数返回的 handle 必须由对应 `*_free` 释放。
- Rust 调用侧凡可能解引用输入/输出裸指针的公开入口均标记为 `unsafe` 并提供 `# Safety` 契约；C header 的调用约定不变，调用方仍须保证非空指针可读/可写且 handle 存活。
- Rust 分配的字符串、数组和 snapshot buffer 必须由 Rust 释放。
- 平台端传入的字符串只在调用期间借用，Rust 不保存裸指针。
- `RadishLexSession*` 绑定创建线程；平台端如需跨线程调度输入，必须在平台侧投递回创建线程，或后续在 Rust 侧显式建模线程安全队列。
- `radishlex_session_free` 也必须回到 session owner thread；非 owner thread 的释放调用不消费 handle，调用方仍须在 owner thread 完成释放。
- FFI 不跨线程共享 snapshot、buffer、term list、import batch list 或 error 裸指针。
- session drop 必须释放 engine adapter、userdb handle 和临时 buffer。
- 释放最后一个 Rime session 不等同于进程 teardown；平台仍须显式调用 runtime shutdown，且不得在任何 session 活动时调用。
- panic 不能跨 FFI 边界，必须转换为错误码；真实 `ffi_status`、`ffi_ptr` 与 `ffi_release` 测试同时固定 panic payload 不进入错误消息。

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

- 输入热路径在调用期间需要的归一化按键事件、学习上下文布尔值和受控 context kind。
- composition preedit 和 cursor。
- candidate 文本、reading、annotation、rank explain 摘要。
- commit 文本。
- 用户明确管理的词条。
- 同步状态摘要和对象计数。

禁止跨 FFI：

- Rime 内部指针和私有 ID。
- SQLite connection、statement 或 row 指针。
- 原始 P1 事件明细的批量导出。
- 通过 userdb 管理、诊断、导出或同步 FFI 暴露 P0 输入内容；P0 场景在输入 ABI 中只允许完成当前 engine 处理和宿主 commit 所需的短生命周期数据流，不得进入学习、日志或持久化返回对象。
- 平台窗口标题、正文内容、控件句柄和系统私有对象。

## 平台壳停止线

进入平台壳前必须满足：

- `ime-core` 输入会话、candidate、commit 和 engine trait 已稳定。
- Rime adapter 与 rank smoke 可复验。
- 按键 ABI 无损返回版本化 `consumed`、可选即时 commit 和同事件 snapshot。
- librime setup/initialize/finalize 已收口为进程级 runtime，多 session 生命周期可复验。
- FFI 文档明确所有权、生命周期、错误语义、字符串编码和释放责任。
- 仓库提供受编译测试约束的 C header 或等价平台模块边界，并由具体 wrapper 复验线程调度、字符串复制和释放规则。
- `ime-ffi` 有 C ABI 单元测试或 host smoke，覆盖 key result、snapshot、candidate view、normalized key event、session options、ABI contract、owner-thread、copy/release 和错误路径。

userdb/ranker 正确性已由 R02L 收口，R01B 通过产品个人化 session 把它们接入 macOS 输入链，并由 build 34 完成真实应用学习、重启持久化、删除/恢复、隐私/unknown/P0 零写入和 secure 系统路由证据。M2 manager 正常产品运行态与同库实机已关闭；当前 M3 的同步 payload、设备授权和平台私钥仍不进入输入热路径。

## 验证口径

修改 `ime-ffi` 时至少需要：

```text
cargo test -p radishlex-ime-ffi
cargo test -p radishlex-ime-core
cargo test -p radishlex-ime-userdb
cargo test -p radishlex-ime-ranker
./scripts/check-repo.sh
```

涉及平台原生 shell 后，还必须补对应平台 build 或 smoke 记录；真实系统输入法安装、启用、权限和系统目录写入需要人工明确授权。
