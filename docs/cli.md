# RadishLex CLI 说明

本文档用于说明 `radishlex-ime-cli` 当前可用命令、参数、输出字段、错误语义和安全边界，读者是需要在本地复验 Rust core 与 engine adapter 行为的开发者和协作者。本文不包含阶段路线、开发进度、Rime 数据准备细节、平台输入法安装流程、ranker 设计或同步协议。

## 定位

`radishlex-ime-cli` 是 Phase 1 的命令行复验入口，用于验证输入生命周期：

```text
input code -> push_key -> composition -> candidates -> commit_candidate
```

它不是系统输入法，也不注册平台输入法服务。CLI 只在当前进程内运行，用于观察 `ime-core` 与 adapter 的行为。

当前命令：

```text
radishlex-ime-cli demo <input-code> [candidate-index]
radishlex-ime-cli rime --schema <schema> --shared-data <path> --user-data <path> <input-code> [candidate-index]
```

## 输出字段

两个命令使用相同输出形态：

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

用途：

- 通过 `ime-engine-rime::RimeEngine` 调用真实 `librime`。
- 复验 Rime C API 到 RadishLex `Composition` / `Candidate` / `Commit` 的转换。
- 检查真实 engine 路径下的 schema、composition、候选和提交行为。

参数：

- `--schema <schema>`：Rime schema id，例如 `luna_pinyin`。
- `--shared-data <path>`：Rime shared data 目录，包含 schema 和公开词典数据。
- `--user-data <path>`：Rime user data 目录，保存本次 smoke 的用户配置和 build 产物。
- `<input-code>`：输入码，例如 `luobo`。
- `[candidate-index]`：可选候选索引；未传入时默认提交首候选。

构建要求：

- 必须显式启用 `--features native-rime`。
- 必须让 build script 找到 `rime_api.h` 和 `librime`。
- 可通过 `RIME_INCLUDE_DIR` 与 `RIME_LIB_DIR` 指定路径。
- 未启用 `native-rime` 时，命令会返回明确的构建提示，不会静默退回 `demo`。

Rime 数据准备步骤见 [Rime Native Smoke Runbook](runbooks/rime-native-smoke.md)。

## 输入限制

当前 CLI 的 `<input-code>` 只接受：

- ASCII 字母
- ASCII 数字
- apostrophe，即 `'`

其他字符会返回用法错误。该限制是 CLI 复验入口的输入约束，不代表后续平台壳只能接收这些按键。

## 退出码

- `0`：命令成功。
- `1`：core 或 engine 运行错误，例如底层 engine 初始化失败、候选提交失败。
- `2`：命令用法错误，例如缺少参数、未知选项、候选索引不是非负整数，或未启用 `native-rime` 运行 `rime`。

## 安全与隐私边界

- CLI 不注册系统输入法，不接管键盘输入。
- CLI 不上传输入内容，不连接 RadishLex 后端。
- `demo` 不读取本机输入法数据。
- `rime` 必须显式指定 `shared-data` 与 `user-data`，不应指向真实 Rime 用户目录。
- 本机 smoke 应使用 `/tmp` 下的隔离目录和合成输入码，不提交 schema 数据、用户目录、日志或输出中的敏感内容。

## 常见错误

### `rime command requires building ... --features native-rime`

原因：当前构建未启用真实 Rime feature。

处理：

```bash
cargo run -p radishlex-ime-cli --features native-rime -- rime ...
```

### `missing --schema`

原因：`rime` 命令缺少 schema id。

处理：

```bash
radishlex-ime-cli rime --schema luna_pinyin --shared-data <path> --user-data <path> luobo
```

### `candidate index must be a non-negative integer`

原因：候选索引不是非负整数。

处理：使用 `0`、`1`、`2` 这类索引值。

### `candidate index ... did not produce commit text`

原因：底层 engine 没有接受该候选索引或未产生提交文本。

处理：先用不带 `candidate-index` 的命令确认候选列表，再选择当前输出中存在的候选索引。真实 Rime 路径的非首候选、翻页候选和异常路径仍需要补充验证。

