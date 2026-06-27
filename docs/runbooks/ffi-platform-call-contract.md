# FFI 平台调用契约 Runbook

本文档说明平台绑定层调用 `radishlex-ime-ffi` 时必须遵守的生命周期、线程、错误和字符串规则，读者是后续实现 Swift / Kotlin / C++ / Flutter bridge 的开发者。本文不包含系统输入法注册、候选窗绘制、平台权限申请、`librime` 数据目录准备或 Flutter 页面设计。

## 当前适用范围

当前 runbook 适用于已落地的 C ABI host smoke：

- `radishlex_ffi_contract`
- session 创建、按键、snapshot、commit 和释放
- structured snapshot / candidate view
- userdb sync preflight
- userdb add / delete / list
- dictionary inspect / export / import
- import batches 查询
- error object 读取和释放

当前不表示真实平台壳已经接入。平台壳进入前，绑定层仍需要按本 runbook 写出本平台自己的 smoke 或 wrapper 测试。

## 调用顺序

### 1. 进程启动后读取 ABI contract

平台绑定层初始化时先调用：

```text
radishlex_ffi_contract(contract_out, error_out)
```

当前必须识别：

```text
version = 1
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
```

规则：

- session 创建成功后，后续 `reset`、`set_schema`、`push_key_event`、`snapshot_new`、`commit_candidate` 和 `engine_kind` 都必须回到创建线程调用。
- 跨线程误用返回 `InvalidState`；无 `error_out` 的 session 读取入口返回空值，例如 `radishlex_session_engine_kind` 返回 `0`。
- 不要把 `RadishLexSession*` 放进全局并允许多个平台线程直接调用。
- 如果平台输入事件来自多个线程，先投递到 session owner thread，再调用 C ABI。

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
user term view -> RadishLexUserTermList*
import batch view -> RadishLexImportBatchList*
error message -> RadishLexError*
buffer data -> RadishLexBuffer*
```

### 5. 按所有权释放 handle

每个 Rust 分配的 handle 都必须用对应释放函数释放：

```text
RadishLexSession*          -> radishlex_session_free
RadishLexSnapshot*         -> radishlex_snapshot_free
RadishLexBuffer*           -> radishlex_buffer_free
RadishLexUserTermList*     -> radishlex_userdb_terms_free
RadishLexImportBatchList*  -> radishlex_userdb_import_batches_free
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
radishlex_session_new_rime(options, error_out)
radishlex_session_set_schema(session, schema, error_out)
radishlex_session_push_key_event(session, event, error_out)
radishlex_session_snapshot_new(session, error_out)
radishlex_snapshot_candidate(snapshot, index, candidate_out, error_out)
radishlex_session_commit_candidate(session, index, error_out)
radishlex_snapshot_free(snapshot)
radishlex_buffer_free(commit)
radishlex_session_free(session)
```

规则：

- 真实平台壳应优先使用 `radishlex_session_push_key_event`，不要长期依赖字符便利函数。
- 候选提交返回 `RadishLexBuffer*`，读取后必须释放。
- snapshot 是一次性状态快照，不会跟随 session 后续输入自动更新。
- 候选索引来自 snapshot 的当前候选列表；提交前如 session 状态已变化，平台层应重新取 snapshot。

### Userdb 和 dictionary 管理

管理入口必须显式传入 SQLite 路径或文件路径：

```text
radishlex_userdb_sync_preflight(db_path, summary_out, error_out)
radishlex_userdb_add_term(db_path, input_code, text, reading, error_out)
radishlex_userdb_delete_term(db_path, input_code, text, reading, error_out)
radishlex_userdb_terms_new(db_path, error_out)
radishlex_userdb_dictionary_inspect(file_path, summary_out, error_out)
radishlex_userdb_dictionary_export(db_path, file_path, summary_out, error_out)
radishlex_userdb_dictionary_import(db_path, file_path, source_name, dry_run, summary_out, error_out)
radishlex_userdb_import_batches_new(db_path, error_out)
```

规则：

- 这些入口不暴露 SQLite connection、statement 或 row 指针。
- dictionary import 的 `dry_run` 使用 `0 / 1`，其他值返回 `InvalidArgument`。
- dictionary export 只导出用户明确管理的 P2 词条，不导出 P1 原始选择事件、负反馈明细、上下文统计或 ranker 权重摘要。
- import batches 是本地审计信息，不作为云端同步 payload。

## 平台绑定注意事项

### Swift / Objective-C

- 用 `Data(bytes:count:)` 或等价方式按 `RadishLexStringView.len` 复制 UTF-8，不要把 view 当作 NUL 结尾字符串。
- 用一个 owner queue 串行访问 `RadishLexSession*`。
- 用 `defer` 或 wrapper `deinit` 调用对应 free 函数。
- 错误对象读取后立即释放；不要把 `RadishLexError*` 存进异步闭包。

### Kotlin / JNI

- JNI 层保存 native handle 时必须绑定到一个串行调度线程。
- 从 native 读取 string view 后立即复制成 `jstring` 或 byte array。
- `close()` / `finalize` 不能作为唯一释放路径；上层需要显式生命周期。
- JNI 异常只包装 FFI status，不应吞掉 `RadishLexStatusCode`。

### C / C++

- 用 RAII wrapper 管理 `RadishLexSession*`、`RadishLexSnapshot*`、`RadishLexBuffer*` 和 `RadishLexError*`。
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

- contract 查询成功，能识别 `version = 1` 和 owner-thread policy。
- 创建 session 后在 owner thread 上 push key、读取 snapshot、提交候选并释放所有 handle。
- 从非 owner thread 调用 session mutation 返回 `InvalidState`。
- 非 UTF-8、空指针、非法 bool、候选越界能返回稳定错误码并释放 error。
- snapshot / term list / import batch list 的 string view 能按长度复制。
- `*_free(NULL)` 不崩溃。
- userdb 管理入口使用显式临时 SQLite 路径，不读取真实用户输入法目录。

推荐本仓库先用以下命令复验 Rust 侧基线：

```text
cargo test -p radishlex-ime-ffi
RIME_INCLUDE_DIR=/opt/homebrew/opt/librime/include RIME_LIB_DIR=/opt/homebrew/opt/librime/lib cargo test -p radishlex-ime-ffi --features native-rime
./scripts/check-repo.sh
```
