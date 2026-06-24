# RadishLex 仓库结构草案

```text
RadishLex/
  README.md
  LICENSE
  AGENTS.md
  Cargo.toml
  go.work
  melos.yaml
  crates/
    ime-core/
    ime-engine-rime/
    ime-ranker/
    ime-userdb/
    ime-sync/
    ime-crypto/
    ime-ffi/
    ime-cli/
  server/
    sync-server/
      cmd/
      internal/
      migrations/
      configs/
      Dockerfile
  apps/
    radishlex-manager/
      flutter/
    desktop-tools/
      egui-inspector/
  platforms/
    windows-tsf/
    macos-imk/
    linux-fcitx5/
    linux-ibus/
    android-ime/
    ios-keyboard/
  docs/
    technical-plan.md
    roadmap.md
    repository-layout.md
    privacy-sync.md
    engine-boundary.md
    engine-rime-adapter.md
    runbooks/
    platform-notes/
  scripts/
    check-repo.sh
    check-repo.ps1
    check-text-files.sh
    check-text-files.ps1
  examples/
    sample-schemas/
    sample-userdb/
  tests/
    fixtures/
    integration/
```

## 目录说明

- `crates/`：Rust 核心和跨端复用库。
- `server/`：Go 自部署同步服务。
- `apps/`：Flutter 管理器和可选工程工具。
- `platforms/`：各平台系统输入法薄壳。
- `docs/`：项目文档真相源。
- `scripts/`：仓库检查、格式和构建脚本。
- `examples/`：示例输入方案、词库和同步样本。
- `tests/`：跨语言、跨平台集成测试。

## 当前已落地

- `Cargo.toml`：Rust workspace 入口。
- `crates/ime-core/`：Rust 输入核心领域模型与 engine boundary 起步 crate。
- `crates/ime-cli/`：基于 demo adapter 与可选 Rime adapter 的命令行复验入口。
- `crates/ime-engine-rime/`：Rime adapter crate，默认不启用 native 绑定。
- `docs/engine-boundary.md`：Rust core 与底层输入引擎的稳定边界。
- `docs/engine-rime-adapter.md`：`ime-engine-rime` 的 adapter 边界、构建策略和验证分层。
- `docs/runbooks/rime-native-smoke.md`：真实 `librime` 本机 smoke 操作步骤。

## Rust crates 建议

### ime-core

输入法核心领域模型：

- `InputSession`
- `KeyEvent`
- `Composition`
- `Candidate`
- `Commit`
- `Engine`
- `Ranker`
- `LearningEvent`

当前已落地 `InputSession`、`KeyEvent`、`Composition`、`Candidate`、`Commit`、`Engine` 和基础生命周期测试。`Ranker` 与 `LearningEvent` 后续在个人化学习阶段补齐。

### ime-engine-rime

librime adapter：

- 封装 librime session。
- 转换 librime candidate 到 RadishLex candidate。
- 屏蔽 C++ 细节。

当前已落地配置模型、错误类型、key 分类、候选转换、`native-rime` build 探测、FFI session 生命周期、输入处理、context / commit 读取路径；默认 workspace 检查不依赖本机安装 `librime`。真实 native smoke 尚未完成。

### ime-ranker

候选重排：

- 个人词权重。
- 最近使用。
- 应用上下文。
- 短语上下文。
- 负反馈。

### ime-userdb

本地用户词库：

- SQLite schema。
- 词条 CRUD。
- 选择事件。
- 学习记录。
- 导入导出。

### ime-sync

同步客户端：

- 增量同步。
- 冲突合并。
- 版本管理。
- 设备状态。

### ime-crypto

加密：

- 主密钥。
- 设备密钥。
- blob 加密。
- 签名和校验。

### ime-ffi

跨语言边界：

- C ABI。
- Flutter bridge。
- Swift/Kotlin/C++ 调用边界。

### ime-cli

调试工具：

- 输入 demo。
- 词库导入导出。
- 同步测试。
- ranker explain。

当前已落地基于合成 demo adapter 的 `demo <input-code> [candidate-index]` 命令，以及需要 `native-rime` feature 和本机 `librime` 依赖的 `rime --schema <schema> --shared-data <path> --user-data <path> <input-code> [candidate-index]` 命令。`demo` 用于默认复验 `ime-core` 生命周期；它不代表真实中文输入引擎。

## Go server 建议

```text
server/sync-server/
  cmd/radishlex-server/
  internal/api/
  internal/auth/
  internal/devices/
  internal/sync/
  internal/storage/
  internal/packages/
  internal/config/
  migrations/
  configs/
```

优先支持：

- 单用户模式。
- SQLite。
- Docker Compose。
- 本地文件对象存储。

后续支持：

- 多用户。
- Postgres。
- S3-compatible storage。
- OIDC。

## Flutter app 建议

```text
apps/radishlex-manager/
  lib/
    app/
    features/
      dashboard/
      dictionary/
      learning/
      privacy/
      sync/
      devices/
      settings/
    bridge/
    design/
```

优先页面：

- Dashboard
- Dictionary
- Learning
- Privacy
- Sync
- Devices
- Settings

## 平台壳建议

平台壳只允许承担：

- 系统输入法生命周期。
- 按键事件接收。
- 候选窗展示。
- 提交文本。
- 调用 Rust core。

平台壳不应承担：

- 用户词库逻辑。
- 同步逻辑。
- 候选排序逻辑。
- 隐私策略。
- 业务配置真相源。
