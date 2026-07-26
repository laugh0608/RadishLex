# FFI 平台调用契约 Runbook

本文档说明平台绑定层调用 `radishlex-ime-ffi` 时必须遵守的生命周期、线程、错误和字符串规则，读者是后续实现 Swift / Kotlin / C++ / Flutter bridge 的开发者。本文不包含系统输入法注册、候选窗绘制、平台权限申请、`librime` 数据目录准备或 Flutter 页面设计。

## 当前适用范围

当前 runbook 适用于已落地的 C ABI host smoke：

- `radishlex_ffi_contract`
- 产品 install/upgrade startup gate、Manager/InputMethod candidate validation
- legacy/Rime/personalized Rime session 创建、版本化学习上下文和释放
- 版本化 key result、snapshot、commit、学习处置和个人化状态
- structured snapshot / candidate view 的 display/engine index 映射
- userdb learning status
- userdb sync preflight
- userdb add / explicit restore / delete / list
- rank explain
- dictionary inspect / export / import
- import batches 查询
- Manager status-only 产品摘要与隔离资格 run
- platform binding style view copy / release smoke
- error object 读取和释放

Rust host smoke 只能证明 ABI 契约；每个平台绑定层仍须按本 runbook 提供本语言的 wrapper 测试，真实产品还要另行证明平台生命周期、隐私信号来源和用户可见行为。

输入侧 C 声明以 `crates/ime-ffi/include/radishlex_input.h` 为准，并由 C11 / Objective-C 编译测试约束。当前仍没有恢复码、设备授权或设备撤销的 native command symbol；这些能力不属于本输入调用契约。

## 调用顺序

### 0. 产品业务初始化前依次执行两层门禁

macOS Manager 与 InputMethod 的正常产品启动必须先调用：

```text
radishlex_product_install_startup_gate(request_v1, result_v1, error_out)
radishlex_product_upgrade_startup_gate(request_v1, result_v1, error_out)
```

外层 install gate 必须先于数据 upgrade gate；前者检查程序事务和当前真实 bundle 身份，后者检查 Application Support 数据事务，两者不能合并或互相代替。平台层只负责从用户域解析固定 `Application Support/RadishLex` 和当前 effective uid，不接受命令行、环境变量、UI 或普通业务代码提供数据路径。外层 gate 不接受 bundle path、release、manifest 或 identity 字段；macOS FFI 从当前 executable 反向绑定固定用户域 bundle，并复验 Info.plist、完整 bundle tree、strict ad-hoc code identity 与 sealed requirement 集合。两个调用位置都必须早于 Flutter delegate、Manager settings/userdb、`IMKServer`、Rime runtime 和任何业务初始化。

外层 gate 只有下列 decision 可以继续到数据 gate：

```text
RADISHLEX_INSTALL_GATE_ALLOWED_FIRST_LAUNCH
RADISHLEX_INSTALL_GATE_ALLOWED_NO_INSTALL_STATE
RADISHLEX_INSTALL_GATE_ALLOWED_TERMINAL_RECEIPT
```

数据 gate 只有下列 decision 可以启动：

```text
RADISHLEX_STARTUP_GATE_ALLOWED_FIRST_LAUNCH
RADISHLEX_STARTUP_GATE_ALLOWED_NO_UPGRADE_STATE
RADISHLEX_STARTUP_GATE_ALLOWED_TERMINAL_RECEIPT
```

任一层出现 active guard、非终态 receipt、损坏 receipt、中断 artifact、未知对象、unsafe root/state 或 identity drift 都必须阻止启动；外层 `completed remove` 同样阻断。非 `Ok` status、result version 不是 v1、未知 decision/error/state 或 native symbol 缺失同样失败关闭。平台不能为了恢复启动而创建目录、chmod、删除 sidecar/receipt 或改用 fixture；现场处置只能交给对应协调器。

两层 gate 都完全只读。首次启动时 data root 不存在属于正常允许结果，平台只能在两层门禁通过后由既有产品 bootstrap 创建正常数据目录。

### 1. 进程启动后读取 ABI contract

平台绑定层初始化时先调用：

```text
radishlex_ffi_contract(contract_out, error_out)
```

当前必须识别：

```text
version = 9
session_thread_policy = owner_thread
panic_boundary = catch_unwind
```

处理规则：

- `contract_out` 必须是有效输出指针。
- 返回非 `Ok` 时读取并释放 `error_out`。
- 不认识的 `version` 或 `session_thread_policy` 不能静默继续；绑定层应拒绝启用输入热路径，并给出可诊断错误。
- `panic_boundary = catch_unwind` 只说明 Rust 侧不会让 panic 穿过 C ABI；平台绑定层仍必须按返回码处理失败。

### 2. 在固定线程创建 session

当前 `RadishLexSession*` 绑定创建线程。平台绑定层必须选择一个固定调用线程：

- 桌面平台可使用输入法主线程或专门的 IME worker thread。
- Android 可使用明确串行化的 IME 调用线程。
- Flutter bridge 不应从任意 isolate / background callback 直接操作同一个 session。

创建入口：

```text
radishlex_session_new(error_out)
radishlex_session_new_with_options(options, error_out)
radishlex_session_new_rime(options, error_out)
radishlex_session_new_personalized_rime(options, error_out)
```

规则：

- session 创建成功后，后续 `reset`、`set_schema`、`handle_key_event`、`snapshot_new`、`select_candidate` 和 `engine_kind` 都必须回到创建线程调用。
- 跨线程误用返回 `InvalidState`；无 `error_out` 的 session 读取入口返回空值，例如 `radishlex_session_engine_kind` 返回 `0`。
- 不要把 `RadishLexSession*` 放进全局并允许多个平台线程直接调用。
- 如果平台输入事件来自多个线程，先投递到 session owner thread，再调用 C ABI。
- `radishlex_session_free` 也必须投递到 session owner thread；非 owner thread 调用是 no-op，不能把它误判为已经释放。
- 首个 Rime session 同时固定进程 runtime owner thread；后续 Rime session 也必须在该固定线程创建和使用，不能只满足“各自回到自己的创建线程”。
- `radishlex_rime_runtime_shutdown` 不是 session 操作，只能在同一 runtime owner thread、进程 teardown、全部 Rime session 已释放且不再接收输入事件时调用。
- personalized Rime options 的 `userdb_path` 与 `session_id` 必须在创建调用期间保持有效；Rust 会复制必要值，不保存平台裸指针。`session_id` 只是本地事件关联键，不能复用设备 ID、账号 ID 或同步身份。
- personalized session 的数据库打开或 migration 失败会以 `storage_unavailable` 退化为 engine 输入，不会删除或重建数据库；平台必须读取 snapshot 状态，不能把 session 创建成功误判为个人化存储可用。

### 3. 每次调用都按 `error_out` 规范处理

带 `error_out` 的函数使用统一模式：

```text
RadishLexError* error = NULL;
status_or_handle = radishlex_xxx(..., &error);
```

规则：

- 调用前把 `error` 初始化为空指针。
- 返回 `Ok` 或非空 handle 时，`error` 应为空；若不为空也必须释放，避免绑定层泄漏。
- 返回非 `Ok` 或空 handle 时，先读取 `radishlex_error_code(error)` 和 `radishlex_error_message(error)`，再调用 `radishlex_error_free(error)`。
- 不能只按错误消息做分支；分支必须依赖 `RadishLexStatusCode`。
- 错误消息只用于诊断日志，不得展示或上传为包含用户输入上下文的遥测数据。

当前常见错误处理：

```text
InvalidArgument: 空指针、非法 UTF-8、非法 bool 标志、候选越界、格式不合法
InvalidState: 当前构建不支持 Rime、session 跨线程误用
EngineError: 底层 engine 或 native Rime 错误
UserDbError: SQLite、文件读写或 userdb 内部错误
InternalError: Rust panic 被 FFI 边界捕获，或内部不变量失败
```

### 4. 读取 string view 时立即复制

`RadishLexStringView` 是借用 view：

```text
data: *const u8
len: usize
```

规则：

- `data + len` 是 UTF-8 字节片段，不保证 NUL 结尾。
- 读取时按长度复制为平台字符串；不要调用需要 NUL 结尾的 C string API。
- `len = 0` 时 `data` 可以为空指针。
- view 只在所属 handle 存活期间有效。
- 读取 candidate、term 或 import batch view 后，如果需要跨调用、跨线程或异步展示，必须复制到平台自有内存。

所属关系：

```text
snapshot schema / preedit / candidate view -> RadishLexSnapshot*
key result commit / borrowed snapshot -> RadishLexKeyResult*
user term view -> RadishLexUserTermList*
deleted term view -> RadishLexDeletedTermList*
import batch view -> RadishLexImportBatchList*
error message -> RadishLexError*
buffer data -> RadishLexBuffer*
```

### 5. 按所有权释放 handle

每个 Rust 分配的 handle 都必须用对应释放函数释放：

```text
RadishLexSession*          -> radishlex_session_free
RadishLexKeyResult*        -> radishlex_key_result_free
RadishLexSnapshot*         -> radishlex_snapshot_free
RadishLexBuffer*           -> radishlex_buffer_free
RadishLexUserTermList*     -> radishlex_userdb_terms_free
RadishLexDeletedTermList*  -> radishlex_userdb_deleted_terms_free
RadishLexImportBatchList*  -> radishlex_userdb_import_batches_free
RadishLexRankExplain*      -> radishlex_userdb_rank_explain_free
RadishLexError*            -> radishlex_error_free
```

规则：

- `*_free(NULL)` 是允许的。
- 不要用平台 allocator 释放 Rust handle。
- 不要重复释放同一个 handle。
- 释放 handle 后，所有从该 handle 借出的 pointer / view 立即失效。
- 释放入口会捕获 panic，但这不是重复释放或 use-after-free 的兜底许可。

## 功能调用清单

### 输入热路径

推荐调用形态：

```text
radishlex_session_new_personalized_rime(options, error_out)  // product input
radishlex_session_new_rime(options, error_out)               // non-personalized smoke
radishlex_session_set_schema(session, schema, error_out)
radishlex_session_set_learning_context(session, context, error_out)
radishlex_session_handle_key_event(session, event, result_out, error_out)
radishlex_key_result_consumed(result)
radishlex_key_result_commit_present(result)
radishlex_key_result_commit(result)
radishlex_key_result_learning_disposition(result)
radishlex_key_result_snapshot(result)
radishlex_snapshot_personalization_status(borrowed_snapshot)
radishlex_snapshot_candidate(borrowed_snapshot, index, candidate_out, error_out)
radishlex_session_select_candidate(session, index, result_out, error_out)
radishlex_key_result_commit_present(result)
radishlex_key_result_commit(result)
radishlex_key_result_snapshot(result)
radishlex_key_result_free(result)
radishlex_session_free(session)
radishlex_rime_runtime_shutdown(error_out)  // process teardown only
```

规则：

- 真实平台壳必须使用 `radishlex_session_handle_key_event`；`push_key` 与 `push_key_event` 会丢弃 KeyOutcome，只保留为兼容和测试入口。
- 产品输入必须创建 personalized Rime session；普通 Rime session 的个人化状态为 `not_enabled`，对其调用 `set_learning_context` 返回 `InvalidState`。
- 平台在按键、候选选择或 composition commit 前，根据 secure input、敏感应用、隐私模式和上下文可信度更新版本化 learning context。context kind 只能使用受控枚举，不能传 App ID、窗口标题或正文；上下文变化后若已有 composition，先使用刷新后的 snapshot 再允许选择。
- secure input、敏感应用或未知上下文必须得到 `policy_blocked` 的 engine-only snapshot；隐私模式可以读既有本地摘要，但 learning disposition 不能为 `recorded`。
- `consumed = 0` 时平台把按键交还宿主；`commit_present = 1` 时立即复制并提交 commit。
- key result 中的 snapshot 与 consumed / commit 来自同一次按键处理，不能用下一次独立 snapshot 调用拼接。
- `radishlex_key_result_snapshot` 返回借用指针，不得调用 `radishlex_snapshot_free`；释放 key result 后该 snapshot 与全部 view 一并失效。
- 候选选择返回 owned key result；分段候选可能 `commit_present = 0`，此时平台只应用同一结果中的 snapshot，不得把候选展示文本直接提交。
- 独立 `snapshot_new` 只保留为兼容和调试入口；snapshot 不会跟随 session 后续输入自动更新。
- `RadishLexCandidateView.index` 是 display index，`engine_index` 只供诊断和映射复验；平台始终把 display index 原样传给 `radishlex_session_select_candidate`，不得自行换算或假定两者相等。提交前如 session 状态已变化，平台层应重新取 snapshot。
- `learning_disposition = failed` 不撤销 engine commit；平台仍须提交文本，只记录不含输入内容、候选和路径的受控状态码。
- `session_free` 只销毁该 session；不要在应用切换或 client 切换时 shutdown 进程 runtime。最终 shutdown 可重复调用，但活动 session 存在时必须按 `InvalidState` 处理为生命周期错误。

### Userdb 和 dictionary 管理

管理入口必须显式传入 SQLite 路径或文件路径：

```text
radishlex_userdb_learning_status(db_path, summary_out, error_out)
radishlex_userdb_sync_preflight(db_path, summary_out, error_out)
radishlex_userdb_add_term(db_path, input_code, text, reading, error_out)
radishlex_userdb_restore_term(db_path, input_code, text, reading, error_out)
radishlex_userdb_delete_term(db_path, input_code, text, reading, error_out)
radishlex_userdb_terms_new(db_path, error_out)
radishlex_userdb_deleted_terms_new(db_path, error_out)
radishlex_userdb_dictionary_inspect(file_path, summary_out, error_out)
radishlex_userdb_dictionary_export(db_path, file_path, summary_out, error_out)
radishlex_userdb_dictionary_import(db_path, file_path, source_name, dry_run, summary_out, error_out)
radishlex_userdb_import_batches_new(db_path, error_out)
```

规则：

- 这些入口不暴露 SQLite connection、statement 或 row 指针。
- `add_term` 只处理未删除词条，不能清除 tombstone 或 suppressed；只有用户明确触发的 `restore_term` 可以执行版本更新的显式恢复。绑定层不得在普通新增、导入或输入选择后自动调用 restore。
- `delete_term` 写入 tombstone 并阻断旧 selection、weight、导入和旧状态复活；调用成功不表示远端同步已经开放。
- active/suppressed term list 与 deleted term list 是两个独立 owned handle；两类 `get` 返回的字符串都只借用到对应 handle 释放前。绑定层必须先复制 deleted identity、删除时间和非敏感 reason，再释放 handle；不得把 view 指针缓存进 widget/model。
- deleted list 只用于展示 tombstone 和承接用户明确确认的 `restore_term`，不能读取 P1 原始事件，也不能在刷新、导入或普通选择后自动恢复。
- learning status 只返回聚合计数、latest timestamp 和 `plaintext_payload / p1_raw_details / context_stats = false` 标记；不得把 P1 原始选择事件、负反馈 reason 明细、上下文统计或用户词明文导出给管理 UI。
- dictionary import 的 `dry_run` 使用 `0 / 1`，其他值返回 `InvalidArgument`。
- dictionary export 只导出用户明确管理的 P2 词条，不导出 P1 原始选择事件、负反馈明细、上下文统计或 ranker 权重摘要。
- import batches 是本地审计信息，不作为云端同步 payload。

### 产品升级候选验证

产品 candidate validation 只能由各自 bundle 内无参数 `Contents/Helpers/RadishLexUpgradeValidationHost` 调用，普通 Dart bridge、InputMethod controller、安装参数或通用 CLI 不得直接暴露：

```text
radishlex_manager_upgrade_validate_candidate(request_v1, summary_v1, error_out)
radishlex_input_method_upgrade_validate_candidate(request_v1, summary_v1, error_out)
```

规则：

- Manager host 固定 candidate/settings 路径，验证 current-schema 管理查询与 settings format v1 兼容；InputMethod host 固定 candidate、本 bundle 只读 RimeData/schema 与隔离临时 Rime user data。锁定 YAML 只允许部署到该临时目录，不能写入 bundle、Application Support 或 candidate。
- 两个 FFI request 的路径字段只用于原生 host 到 Rust 的窄调用边界，不授权上层接受任意路径；host 必须自行解析固定产品位置并拒绝参数。
- validation summary version 固定为 v1。Manager 只能设置 management/settings check，InputMethod 只能设置 personalized runtime/candidate signal check；schema 必须与 receipt 目标一致。
- candidate 必须保持全字节不变，调用前后都不得存在 WAL/SHM/journal；不得产生选择、负反馈、导入、同步或 settings 写入。
- FFI summary 不是持久化授权。只有持有 upgrade guard 的协调核心在复验 receipt、candidate identity 与 sidecar 后，才能记录 validation evidence 并推进状态。

## 平台绑定注意事项

### Swift / Objective-C

- 用 `Data(bytes:count:)` 或等价方式按 `RadishLexStringView.len` 复制 UTF-8，不要把 view 当作 NUL 结尾字符串。
- 用一个 owner queue 串行访问 `RadishLexSession*`。
- 用 `defer` 或 wrapper `deinit` 释放 key result 和其他 owned handle；从 key result 借用的 snapshot 不单独释放。
- 错误对象读取后立即释放；不要把 `RadishLexError*` 存进异步闭包。

### Kotlin / JNI

- JNI 层保存 native handle 时必须绑定到一个串行调度线程。
- 从 native 读取 string view 后立即复制成 `jstring` 或 byte array。
- `close()` / `finalize` 不能作为唯一释放路径；上层需要显式生命周期。
- JNI 异常只包装 FFI status，不应吞掉 `RadishLexStatusCode`。

### C / C++

- 用 RAII wrapper 管理 `RadishLexSession*`、`RadishLexKeyResult*`、独立 `RadishLexSnapshot*`、`RadishLexBuffer*` 和 `RadishLexError*`。
- wrapper 类型应禁止复制，允许 move。
- 所有 C ABI 调用都应封装在检查返回码的薄函数里，不要在业务代码里散落裸调用。
- string view 转 `std::string` 时必须使用 `(ptr, len)` 构造。

### Flutter bridge

- Dart FFI 层不要跨 isolate 直接复用同一个 session pointer。
- 如果 manager UI 只是管理 userdb，可不创建输入 session，只调用显式路径的 userdb / dictionary 入口。
- Bridge 返回给 Dart 的字符串必须已经复制到 Dart owned memory 或通过 Rust buffer 明确释放。
- 输入热路径不应经过 Flutter UI isolate；真实平台壳应直接调用 Rust FFI。

## Smoke 要求

每个平台绑定层进入真实平台壳前，至少补以下 smoke：

- contract 查询成功，能识别 `version = 9`、key result v2、按键/候选选择共用的 owned result 和 owner-thread policy。
- 外层 install gate 与数据 upgrade gate 分别覆盖 data root absent、状态目录 absent、终态允许、非终态阻止、active guard、损坏 receipt、中断 artifact 和 identity drift；外层另覆盖 `completed remove`，并检查两层门禁前后目录树与字节保持不变。
- Manager/InputMethod validation host 拒绝参数，只读固定 candidate；覆盖候选损坏、双端打开失败、字节保持不变和 sidecar 零残留。
- 创建 session 后在 owner thread 上 handle key，核对 consumed / commit / 同事件 snapshot，提交候选并释放所有 owned handle。
- personalized session smoke 覆盖受控 learning context、display/engine index 不同仍提交正确候选、selection 的 `recorded/deferred`、学习失败不丢 commit，以及 storage/read/rank 退化状态。
- secure、敏感或 unknown context 断言不读取不写入个人化数据；隐私模式断言可读既有排序但不新增 selection、term 或 weight。
- 从非 owner thread 调用 session mutation 返回 `InvalidState`。
- 非 UTF-8、空指针、非法 bool、候选越界能返回稳定错误码并释放 error。
- key result / snapshot / term list / import batch list 的 string view 能按长度复制，并在释放所属 handle 后继续使用已复制值。
- `*_free(NULL)` 不崩溃。
- 两个 Rime session 共享进程 runtime；释放其中一个后另一个仍可输入，全部释放后显式 runtime shutdown 成功，活动 session 存在时 shutdown 返回 `InvalidState`。
- userdb 管理入口使用显式临时 SQLite 路径，不读取真实用户输入法目录；learning status smoke 需要断言 P1 明细和上下文统计标记为 false，并覆盖 add 不能复活 tombstone、restore 只能由独立入口执行。
- 本仓库 Rust host smoke 已覆盖 key result / snapshot / user term / import batch / error 的复制后释放；平台 wrapper 仍需在本语言层复验同一规则。

推荐本仓库先用以下命令复验 Rust 侧基线：

```text
cargo test -p radishlex-ime-ffi
RIME_INCLUDE_DIR=/opt/homebrew/opt/librime/include RIME_LIB_DIR=/opt/homebrew/opt/librime/lib cargo test -p radishlex-ime-ffi --features native-rime
./scripts/check-repo.sh
```
