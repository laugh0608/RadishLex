# RadishLex FFI 边界

本文档定义后续 `ime-ffi` 的 ABI 职责、数据所有权、错误语义和平台壳停止线，读者是后续实现 C ABI、Flutter bridge、Swift/Kotlin/C++ 调用层和平台输入法薄壳的开发者。本文不包含具体平台输入法注册流程、TSF / InputMethodKit / Fcitx5 API 调用细节、Flutter 页面设计或移动端键盘 UI。

## 当前定位

当前已落地 `crates/ime-ffi/` 起步验证：C ABI 已覆盖 opaque session handle、错误对象、UTF-8 buffer、结构化 snapshot handle、candidate view、normalized key event、释放函数、schema 设置、按键输入、snapshot 和候选提交的 host smoke。当前 session 内部使用 deterministic demo engine 证明 ABI 生命周期，不代表真实 Rime adapter、平台壳或系统输入法已经接入。

平台壳后续只能通过 FFI 调用 Rust core，不得直接访问 SQLite、Rime 私有对象或 ranker 内部状态。

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
RadishLexError*
```

平台端只能持有 opaque pointer，不能解引用 Rust 内部结构。跨 ABI 文本优先使用带长度的 UTF-8 view；需要 Rust 分配的 buffer 或 snapshot handle 时，必须由 Rust 提供释放函数。

建议基本函数族：

```text
radishlex_session_new
radishlex_session_free
radishlex_session_reset
radishlex_session_set_schema
radishlex_session_push_key
radishlex_session_push_key_event
radishlex_session_snapshot
radishlex_session_snapshot_new
radishlex_session_commit_candidate
radishlex_buffer_free
radishlex_error_code
radishlex_error_message
radishlex_error_free
```

当前已落地函数：

```text
radishlex_session_new
radishlex_session_free
radishlex_session_reset
radishlex_session_set_schema
radishlex_session_push_key
radishlex_session_push_key_event
radishlex_session_snapshot
radishlex_session_snapshot_new
radishlex_snapshot_schema
radishlex_snapshot_preedit
radishlex_snapshot_cursor
radishlex_snapshot_candidate_count
radishlex_snapshot_candidate
radishlex_snapshot_free
radishlex_session_commit_candidate
radishlex_buffer_data
radishlex_buffer_len
radishlex_buffer_free
radishlex_error_code
radishlex_error_message
radishlex_error_free
```

`radishlex_session_push_key` 保留为字符输入便利函数；真实平台壳后续应优先使用 `radishlex_session_push_key_event`。当前 normalized key event 使用数值常量承载字符键、命名键、修饰键、按下 / 释放阶段和平台不可识别键，避免让无效 enum discriminant 在 FFI 边界形成未定义行为。

结构化 snapshot 规则：

- `radishlex_session_snapshot_new` 返回 `RadishLexSnapshot*`，由 `radishlex_snapshot_free` 释放。
- `radishlex_snapshot_schema`、`radishlex_snapshot_preedit` 和 candidate view 中的文本均为借用自 snapshot 的 UTF-8 `data + len` view。
- 平台端只能在 snapshot 释放前读取这些 view；不得缓存 view 指针。
- `radishlex_snapshot_candidate` 通过输出参数返回单个 `RadishLexCandidateView`，候选越界或输出指针为空时返回 `InvalidArgument`。
- `radishlex_session_snapshot` 保留为调试用文本 buffer，不作为后续平台壳读取 composition / candidate 的主入口。

## 所有权与生命周期

规则：

- 创建函数返回的 handle 必须由对应 `*_free` 释放。
- Rust 分配的字符串、数组和 snapshot buffer 必须由 Rust 释放。
- 平台端传入的字符串只在调用期间借用，Rust 不保存裸指针。
- FFI 不跨线程共享裸指针；如需跨线程，必须在 Rust 侧显式建模。
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
- `ime-ffi` 至少有 C ABI 单元测试或 host smoke，证明字符串、数组、snapshot、candidate view、normalized key event 和错误释放路径可复验。当前已完成结构化 snapshot / candidate ABI 与 normalized key event 起步 host smoke；真实平台壳前仍需补 engine adapter 选择策略和受控 userdb / sync 状态入口。

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
