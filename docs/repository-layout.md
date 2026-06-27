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
    cli.md
    engine-boundary.md
    engine-rime-adapter.md
    personalization-learning.md
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
- `crates/ime-cli/`：基于 demo adapter、可选 Rime adapter、userdb 和 ranker 的命令行复验入口。
- `crates/ime-engine-rime/`：Rime adapter crate，默认不启用 native 绑定。
- `crates/ime-ffi/`：C ABI 起步 crate，覆盖 ABI contract、opaque handle、session owner-thread policy、session options、engine kind 门禁、错误对象、UTF-8 buffer、结构化 snapshot / candidate view、normalized key event、sync preflight 状态摘要、userdb 管理入口、dictionary 文件管理入口和 host smoke。
- `crates/ime-sync/`：同步 payload 来源分类、对象类型和加密对象外壳草案。
- `crates/ime-crypto/`：客户端加密本地模型 crate，当前覆盖 key role、object envelope、AAD、nonce 和 ciphertext hash 边界测试。
- `docs/cli.md`：`radishlex-ime-cli` 命令、输出、退出码和安全边界说明。
- `docs/engine-boundary.md`：Rust core 与底层输入引擎的稳定边界。
- `docs/engine-rime-adapter.md`：`ime-engine-rime` 的 adapter 边界、构建策略和验证分层。
- `docs/personalization-learning.md`：Phase 2 个人化学习、userdb、ranker、负反馈和 CLI 管理边界。
- `docs/sync-payload.md`：同步 payload 草案和 P1/P2 来源分类。
- `docs/crypto-boundary.md`：`ime-crypto` 进入实现前的客户端加密、密钥、envelope 和验证边界。
- `docs/sync-key-management.md`：真实同步前的同步密钥、设备授权、恢复码、设备撤销、key epoch 和冲突边界。
- `docs/ffi-boundary.md`：后续 C ABI、所有权、生命周期和错误语义边界。
- `docs/runbooks/ffi-platform-call-contract.md`：平台绑定层调用 C ABI 的错误、字符串、handle 释放和 owner-thread 调度规则。
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

当前已落地配置模型、错误类型、key 分类、候选转换、`native-rime` build 探测、FFI session 生命周期、输入处理、context / commit 读取路径；默认 workspace 检查不依赖本机安装 `librime`。macOS 本机 `librime` 1.17.0 与隔离 `luna_pinyin` 数据目录下的 native smoke 已覆盖首候选、非首候选、翻页后当前页候选和越界候选索引错误。

### ime-ranker

候选重排：

- 个人词权重。
- 最近使用。
- 应用上下文。
- 短语上下文。
- 负反馈。

当前已创建 `crates/ime-ranker/`，落地 `RankRequest`、`RankedCandidate`、结构化 explain 输出和频次、近期、上下文、负反馈、suppressed、deleted tombstone 排序测试；`ime-cli rank explain` 已接入基础解释链路。

### ime-userdb

本地用户词库：

- SQLite schema。
- 词条 CRUD。
- 选择事件。
- 学习记录。
- 导入导出。

当前已创建 `crates/ime-userdb/`，落地 SQLite schema migration、用户词条 CRUD、选择事件记录、负反馈记录、删除 tombstone、ranker weight 摘要、用户词库导入导出、同步前置计数，以及 `dictionary.user_terms` / `dictionary.deleted_terms` P2 plaintext payload 只读迭代器；基础 CLI 管理入口已由 `ime-cli` 承接。

### ime-sync

同步客户端：

- 增量同步。
- 冲突合并。
- 版本管理。
- 设备状态。

当前已创建 `crates/ime-sync/`，只落地 payload 来源分类、同步对象类型、P1/P2/本地审计分层和从 `ime-crypto` envelope 派生加密对象外壳元数据；不连接后端、不实现网络同步或设备授权。

### ime-crypto

加密：

- 主密钥。
- 设备密钥。
- blob 加密。
- 签名和校验。

当前已创建 `crates/ime-crypto/`，落地 XChaCha20Poly1305、HKDF-SHA256、SHA-256 ciphertext hash、key role、object envelope、AAD、nonce、删除同步和篡改失败测试；签名、设备授权和恢复码尚未落地。

### ime-ffi

跨语言边界：

- C ABI。
- Flutter bridge。
- Swift/Kotlin/C++ 调用边界。

当前已创建 `crates/ime-ffi/`，落地 C ABI 起步验证：ABI contract、opaque session handle、session owner-thread policy、session options、engine kind 门禁、错误对象、UTF-8 buffer、结构化 snapshot handle、candidate view、normalized key event、sync preflight 状态摘要、userdb 管理入口、dictionary 文件管理入口、释放函数 panic 边界、schema 设置、按键输入、snapshot 和候选提交。当前 host smoke 使用 deterministic demo engine，不代表真实平台壳已接入。

### ime-cli

调试工具：

- 输入 demo。
- 词库导入导出。
- 同步测试。
- ranker explain。

当前已落地基于合成 demo adapter 的 `demo <input-code> [candidate-index]` 命令，以及需要 `native-rime` feature 和本机 `librime` 依赖的 `rime --schema <schema> --shared-data <path> --user-data <path> [--key <name> ...] [--rank-db <path>] [--context <kind>] <input-code> [candidate-index]` 命令。`demo` 用于默认复验 `ime-core` 生命周期；它不代表真实中文输入引擎。Phase 2 起步已补 `dict list/add/delete`、`learn select/suppress`、`rank explain` 和 Rime rank smoke，通过显式 `--db` / `--rank-db` 的临时 SQLite 数据库复验用户词条、学习事件、负反馈、真实 engine candidates 重排和 explain 输出。

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
