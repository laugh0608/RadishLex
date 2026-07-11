# ime-engine-rime Adapter 设计

本文档用于说明 RadishLex v1 接入 `librime` 的 adapter 边界、构建策略、验证方式和停止线，读者是实现 `crates/ime-engine-rime/`、CLI 真实 engine 模式和平台壳的开发者。本文不包含当前批次状态、`librime` 源码实现细节、Rime 词库内容、平台输入法协议或 ranker 学习策略；实时进度见 `docs/status/current.md`。

## 稳定定位

- 建立 `ime-engine-rime` crate 边界。
- 明确本机如何发现、链接和初始化 `librime`。
- 明确 Rime C API 到 RadishLex core model 的转换规则。
- 明确没有 `librime` 环境时的默认验证降级方式。
- 将 setup、initialize、notification 和 finalize 收口为进程级 runtime，单个 engine session 只管理对应的 Rime session。
- 为 CLI、FFI 和平台壳提供同一套可诊断 adapter 能力，不让任何一层依赖 Rime 私有对象。

非目标：

- 不复制 Rime 源码、私有函数结构、词库或 schema 数据。
- 不把 Rime candidate 内部对象 ID 暴露给 `ime-core`、ranker、userdb、Flutter 或平台壳。
- 不让 CI 默认依赖本机安装 `librime`。
- 不在 adapter 内实现用户词库、同步、加密、候选重排或平台输入法 UI。

## 外部事实基线

基于 2026-06-24 复核的官方资料：

- `rime/librime` 仓库说明其许可证为 BSD-3-Clause，README 中列出 C++17、CMake、Boost、LevelDB、marisa、OpenCC、yaml-cpp 等构建或运行依赖。
- `rime_api.h` 暴露 C API，其中包含 session management、input、output、schema selection 相关函数，例如 `create_session`、`destroy_session`、`process_key`、`get_commit`、`get_context`、`get_status`、`get_current_schema` 和 `select_schema`。
- 官方 README 同时列出 Linux、macOS、Windows 前端，说明 `librime` 本身不等同于平台输入法壳。

外部参考：

- <https://github.com/rime/librime>
- <https://github.com/rime/librime/blob/master/src/rime_api.h>

## Crate 边界

建议 crate：

```text
crates/ime-engine-rime/
  Cargo.toml
  build.rs
  src/
    lib.rs
    config.rs
    error.rs
    ffi.rs
    keymap.rs
    session.rs
    convert.rs
```

职责：

- `config.rs`：RadishLex 自有配置，例如 shared data dir、user data dir、schema id、是否部署。
- `ffi.rs`：最小 C API 绑定，只收纳 ABI 类型、函数表和 `unsafe` 调用边界。
- `keymap.rs`：把 `ime-core::KeyEvent` 映射为 Rime C API 需要的 keycode / mask。
- `convert.rs`：把 Rime context、composition、menu、candidate 转换为 RadishLex `Composition` / `Candidate`。
- `session.rs`：实现 `RimeEngine`，对外只暴露 `ime-core::Engine`。
- `error.rs`：把 Rime 初始化、会话、schema、候选、编码和 FFI 生命周期错误转换为可诊断错误。

`ime-engine-rime` 只能依赖 `ime-core` 的公开类型，不反向修改 `ime-core` 来适配 Rime 私有概念。若 `ime-core` trait 缺字段，先写清楚场景与失败用例，再判断是否扩展稳定模型。

## 构建策略

默认策略：

- `ime-engine-rime` 不 vendor `librime` 源码。
- 默认 workspace 检查不要求安装 `librime`。
- 真实 Rime 构建通过显式 feature 或环境变量启用。
- 本机 smoke 与 CI native job 分离；没有 native 依赖时仍能运行 `cargo test --workspace`。

建议 feature：

```text
default = []
native-rime = []
```

建议发现顺序：

1. 若设置 `RIME_INCLUDE_DIR` 与 `RIME_LIB_DIR`，优先使用显式路径。
2. 否则尝试通过 `pkg-config` 查找 `rime`。
3. 若目标平台后续采用 vcpkg / Homebrew / system package，应只作为安装说明，不写进默认构建路径。

失败语义：

- 未启用 `native-rime` 时不编译 FFI 绑定，不提供真实 `RimeEngine` 构造函数。
- 启用 `native-rime` 但找不到头文件或库时，build script 必须明确报错，指出需要配置 `RIME_INCLUDE_DIR` / `RIME_LIB_DIR` 或安装系统依赖。
- 不能在 build script 中联网下载源码或依赖。

## 生命周期映射

`ime-core::Engine` 到 `librime` 的初步映射：

```text
RimeEngine::new(config)
  -> setup / initialize
  -> create_session

Engine::reset
  -> clear_composition

Engine::push_key
  -> process_key
  -> get_commit if available

Engine::composition
  -> get_context
  -> convert context.composition

Engine::candidates
  -> get_context
  -> convert context.menu candidates

Engine::commit_candidate(index)
  -> select candidate by key or index strategy
  -> get_commit

Engine::set_schema(schema)
  -> select_schema

Drop
  -> destroy_session
  -> cleanup if owner policy allows
```

需要在实现前确认的开放点：

- Rime 候选选择应使用数字键模拟、page + select 组合，还是可用更直接的 API。实现前必须通过小型 smoke 记录确认。
- `get_context` 返回的 composition cursor 单位是否能直接映射到 UTF-8 byte cursor；不能确认时先保守转换并测试中文、ASCII、混合输入。
- schema 初始化、部署和用户目录隔离是否需要在 `RimeEngine::new` 显式执行，还是由外部安装流程负责。

## 数据目录策略

Rime 需要 shared data 和 user data。RadishLex adapter 不应把这些目录硬编码到用户本机环境。

建议配置：

```text
RimeEngineConfig
  shared_data_dir: PathBuf
  user_data_dir: PathBuf
  log_dir: Option<PathBuf>
  schema: SchemaId
  deploy_on_start: bool
```

规则：

- CLI smoke 必须通过参数或环境变量指定数据目录，不能隐式读取真实用户输入法目录。
- 测试 fixture 不包含真实用户词库、联系人、输入历史或有版权风险词库。
- 若需要最小 schema fixture，必须确认许可证与来源；不能从现有 Rime schema 复制数据后直接提交。
- 用户数据目录属于本地敏感数据，不进入同步、日志、截图或 golden 输出。

### FFI 配置策略

真实 Rime 进入 `ime-ffi` 时必须使用独立的 Rime session options，而不是复用通用 `engine_kind` 字段承载目录和 schema：

```text
RadishLexRimeSessionOptions
  version
  shared_data_dir
  user_data_dir
  schema
  log_dir
  deploy_on_start
```

规则：

- `radishlex_session_new_rime` 是后续真实 Rime FFI 的专用构造入口。
- `shared_data_dir`、`user_data_dir` 和 `schema` 必须显式传入，不能隐式读取真实用户输入法目录。
- `log_dir` 可选，传入时也必须是非空 UTF-8 路径。
- `deploy_on_start` 使用 `u8` 的 `0 / 1` 表示，避免跨语言 bool ABI 差异。
- 默认 workspace 构建下，该入口只做 ABI 参数校验并返回 `InvalidState`，不会静默退回 demo engine。
- `ime-ffi` 启用 `native-rime` feature 时，该入口会将 options 转为 `RimeEngineConfig` 并创建真实 `RimeEngine` session。
- `ime-ffi` 内部使用 demo / Rime 可扩展 session engine 封装，平台端仍只持有 opaque `RadishLexSession*`。
- 当前已通过 ignored native smoke 覆盖 `radishlex_session_new_rime -> push_key -> snapshot -> commit_candidate`；该 smoke 需要显式传入隔离 Rime shared / user data 目录。

## 候选转换规则

Rime candidate 转 RadishLex candidate 时只保留稳定字段：

- `text` -> `Candidate::text`
- `comment` 或等价展示注释 -> `Candidate::annotation`
- 可稳定获得的 reading -> `Candidate::reading`
- 来源统一标记为 `CandidateSource::Engine`

不保留：

- 底层候选对象指针。
- 底层内部索引以外的私有 ID。
- 会话私有状态。
- Rime 内部评分作为 core 必需字段。

后续 ranker 如果需要 engine score，只能通过 `ime-core` 中明确增加的稳定字段传入，不能读取 Rime 私有结构。

## 错误和安全边界

- 所有 FFI 调用集中在 `ffi.rs` / `session.rs` 的极小边界内，并配套 `unsafe` 注释说明所有权、空指针、释放责任和线程假设。
- `native-rime` feature 测试覆盖 startup / runtime 必需 API 缺失；缺失函数必须返回带函数名的 `MissingApiFunction`。
- 所有 Rime 分配的 context、commit、status、schema list 必须按 C API 对应 free 函数释放。
- C string 转 Rust string 时必须处理 null、非 UTF-8 和空字符串。
- Rime session id 只能存于 `RimeEngine` 内部，不进入 `ime-core` 模型。
- `Drop` 必须尽力释放 session，但释放失败不能 panic。
- adapter 错误必须包含阶段信息，例如 `initialize`、`create_session`、`process_key`、`get_context`、`select_schema`。

## CLI 集成策略

`ime-cli` 当前通过独立子命令接入真实 engine：

```text
radishlex-ime-cli demo <input-code> [candidate-index]
radishlex-ime-cli rime --schema luna_pinyin --shared-data <path> --user-data <path> [--key <name> ...] <input-code> [candidate-index]
```

规则：

- `demo` 保持无 native 依赖，继续作为默认 smoke。
- `rime` 只在启用 `native-rime` 且本机依赖可用时编译或运行。
- `rime --key <name>` 仅作为 CLI smoke 调试入口，用于在输入码后追加 `page-down`、`page-up`、方向键等命名键事件。
- CLI 输出继续包含 schema、composition、candidates、commit。
- 没有真实 Rime 环境时，测试只验证参数解析和错误提示，不伪造真实 Rime 输出。

命令参数、输出字段和退出码说明见 `docs/cli.md`。

## 验证分层

默认验证：

```text
cargo fmt --check
cargo test --workspace
cargo run -p radishlex-ime-cli -- demo luobo
./scripts/check-repo.sh
```

可选 native Rime 验证：

```text
cargo test -p radishlex-ime-engine-rime --features native-rime
cargo check -p radishlex-ime-cli --features native-rime
cargo run -p radishlex-ime-cli --features native-rime -- rime --schema <schema> --shared-data <path> --user-data <path> <input-code>
RADISHLEX_RIME_SHARED_DATA=<path> RADISHLEX_RIME_USER_DATA=<path> cargo test -p radishlex-ime-ffi --features native-rime rime_session_native_smoke_uses_ffi_entrypoint -- --ignored
```

本机准备步骤见 `docs/runbooks/rime-native-smoke.md`。

退出标准：

- 未安装 `librime` 时，默认仓库基线仍通过。
- 安装并配置 `librime` 时，CLI 能完成真实 `compose -> candidates -> commit`。
- Rime adapter 错误可诊断，不静默退回 demo adapter。
- `ime-core` 不出现 Rime 私有类型、路径或 session id。

## 已验证能力与未闭合边界

已有实现与历史 smoke 已证明真实 Rime adapter 能完成 composition、候选、翻页、选择、commit、错误映射和 ranker 接入，`ime-ffi` 也可在显式 `native-rime` feature 下创建真实 Rime session。详细完成记录留在 devlog，不在本文持续追加。

平台接入前仍必须闭合：

- 进程级 setup/initialize/finalize 与多 session 生命周期；
- 从 `KeyOutcome` 到版本化 FFI 结果的 `consumed`、即时 commit 和 snapshot 无损传递；
- 受测试约束的 C header 或等价平台模块边界；
- native library、`librime` 与 schema 的开发版和发布版分发策略；
- macOS InputMethodKit 真实应用 smoke。

这些未闭合项属于 M1 macOS 离线输入 Alpha，不应再被同步后端工作延后。最终发布包中的 `librime` 与 schema 分发属于 M4。
