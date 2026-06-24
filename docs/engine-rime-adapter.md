# ime-engine-rime Adapter 设计

本文档用于说明 RadishLex v1 接入 `librime` 的 adapter 边界、构建策略、验证方式和停止线，读者是实现 `crates/ime-engine-rime/`、CLI 真实 engine 模式和后续平台壳的开发者。本文不包含 `librime` 源码实现细节、Rime 词库内容、平台输入法协议或 ranker 学习策略。

## 阶段定位

当前处于 Phase 1：Rust Core 原型。`ime-core` 已有平台无关 engine trait，`ime-cli` 已有合成 demo adapter。下一步目标不是直接做平台壳，而是让真实底层 engine 能通过 `ime-core::Engine` 进入 CLI 可复验链路。

阶段目标：

- 建立 `ime-engine-rime` crate 边界。
- 明确本机如何发现、链接和初始化 `librime`。
- 明确 Rime C API 到 RadishLex core model 的转换规则。
- 明确没有 `librime` 环境时的默认验证降级方式。

非目标：

- 不复制 Rime 源码、私有函数结构、词库或 schema 数据。
- 不把 Rime candidate 内部对象 ID 暴露给 `ime-core`、ranker、userdb、Flutter 或平台壳。
- 不让 CI 默认依赖本机安装 `librime`。
- 不在本阶段实现用户词库、同步、加密或平台输入法 UI。

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
- 所有 Rime 分配的 context、commit、status、schema list 必须按 C API 对应 free 函数释放。
- C string 转 Rust string 时必须处理 null、非 UTF-8 和空字符串。
- Rime session id 只能存于 `RimeEngine` 内部，不进入 `ime-core` 模型。
- `Drop` 必须尽力释放 session，但释放失败不能 panic。
- adapter 错误必须包含阶段信息，例如 `initialize`、`create_session`、`process_key`、`get_context`、`select_schema`。

## CLI 集成策略

`ime-cli` 当前通过独立子命令接入真实 engine：

```text
radishlex-ime-cli demo <input-code> [candidate-index]
radishlex-ime-cli rime --schema luna_pinyin --shared-data <path> --user-data <path> <input-code> [candidate-index]
```

规则：

- `demo` 保持无 native 依赖，继续作为默认 smoke。
- `rime` 只在启用 `native-rime` 且本机依赖可用时编译或运行。
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
```

本机准备步骤见 `docs/runbooks/rime-native-smoke.md`。

退出标准：

- 未安装 `librime` 时，默认仓库基线仍通过。
- 安装并配置 `librime` 时，CLI 能完成真实 `compose -> candidates -> commit`。
- Rime adapter 错误可诊断，不静默退回 demo adapter。
- `ime-core` 不出现 Rime 私有类型、路径或 session id。

## 实施顺序

1. 新增本设计文档和入口索引。
2. 新增 `crates/ime-engine-rime/` skeleton，但不默认启用 native 绑定。
3. 增加 build script 的本地探测和明确错误信息。
4. 增加最小 FFI 绑定与 session 管理。
5. 增加 conversion 单元测试，优先测试 Rust 侧转换和错误语义。
6. 增加本机 native smoke 文档与可选 CI job。
7. 将 `ime-cli rime` 接入真实 adapter。
8. 在安装 `librime` 和合法 schema 数据的开发机上执行真实 native smoke。

当前进度：

- 第 1-3 步已落地。
- 已补配置模型、错误类型、key 分类和候选转换测试。
- 第 4 步已覆盖 `setup`、`initialize`、`create_session`、`select_schema` 和 `destroy_session` 的 FFI session 管理。
- 已补 `process_key`、`get_context`、`get_commit`、`free_context` 和 `free_commit` 的 Rust 侧调用路径。
- 已实现 `ime-cli rime` 子命令；默认 feature 下会给出明确 `native-rime` 构建提示，启用 feature 后可构造 `RimeEngine` 并进入 `InputSession`。
- 已在 macOS 本机 `librime` 1.17.0、`luna_pinyin` 隔离数据目录下完成真实 native smoke，`luobo` 可输出 composition、候选和默认 commit。
- 候选提交当前通过当前页 `select_keys` 模拟选择；native smoke 已验证默认首候选可提交，非首候选和翻页选择后续仍需补充覆盖。

阶段停止线：在 `ime-cli rime` 的非首候选选择、翻页选择和 native 异常路径没有补充验证前，不推进平台壳、ranker 或 userdb。
