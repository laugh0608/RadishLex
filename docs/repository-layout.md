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
  deploy/
    sync-server/
      caddy/
      docker-compose.local.yaml
      docker-compose.yaml
      nginx.prod.conf
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
    sync-payload.md
    crypto-boundary.md
    sync-key-management.md
    sync-server-api-storage.md
    production-recovery-flow.md
    ffi-boundary.md
    adr/
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
- `deploy/`：容器部署、反向代理和环境切换配置。
- `apps/`：Flutter 管理器和可选工程工具。
- `platforms/`：各平台系统输入法薄壳。
- `docs/`：项目文档真相源。
- `scripts/`：仓库检查、格式和构建脚本。
- `examples/`：示例输入方案、词库和同步样本。
- `tests/`：跨语言、跨平台集成测试。

## 当前已落地

- `Cargo.toml`：Rust workspace 入口。
- `deploy/sync-server/docker-compose.local.yaml`：Go sync server 本地容器验证入口，使用 Caddy internal TLS 暴露 `https://localhost:7319`。
- `deploy/sync-server/docker-compose.yaml`：Go sync server 部署态入口，只暴露 HTTP 上游 `http://127.0.0.1:7319`，外部反代负责 TLS。
- `deploy/sync-server/.env.example`：唯一 env 示例，真实部署复制为 `.env` 后修改。
- `deploy/sync-server/nginx.prod.conf`：生产外部 Nginx TLS 终止示例。
- `crates/ime-core/`：Rust 输入核心领域模型与 engine boundary 起步 crate。
- `crates/ime-cli/`：基于 demo adapter、可选 Rime adapter、userdb 和 ranker 的命令行复验入口。
- `crates/ime-engine-rime/`：Rime adapter crate，默认不启用 native 绑定。
- `crates/ime-ffi/`：C ABI 起步 crate，覆盖 ABI contract、opaque handle、session owner-thread policy、session options、engine kind 门禁、错误对象、UTF-8 buffer、结构化 snapshot / candidate view、normalized key event、sync preflight 状态摘要、userdb 管理入口、dictionary 文件管理入口和 host smoke。
- `crates/ime-sync/`：同步 payload 来源分类、对象类型、P2 envelope 组装、加密对象外壳草案、设备生命周期、对象版本冲突、客户端合并模型、signed device authorization、signed device revocation、remote client DTO / transport trait 和 std-only `http://` HTTP transport。
- `crates/ime-crypto/`：客户端加密本地模型 crate，当前覆盖 key role、object envelope、AAD、nonce、ciphertext hash、device wrapping、recovery material、Argon2id recovery KDF、Ed25519 signing、test-memory signing key store、platform backend capability / unavailable 模型、signed object manifest 和 signed recovery record。
- `server/sync-server/`：Go sync server 起步 module，当前覆盖配置默认值、API request / response / error DTO、storage interface、SQLite metadata migration 文本、storage conformance tests、内存 metadata store、SQLite-backed metadata repository、local object storage staged transaction、metadata transaction 与 blob transaction 接线、Ed25519 签名验证抽象、签名篡改拒绝测试、device wrapping encrypted key bytes 承载、recovery wrapped material 读取接口、recovery latest handler、domain / device / join request metadata handler、authorization handler、encrypted object version 上传 / metadata 读取 / payload 下载 handler、单用户 bearer access token 门禁、request id、panic recovery、非持久审计 hook、SQLite audit_events 写入、`cmd/radishlex-sync-server`、runtime 配置装配、HTTP timeout、对象大小门禁、脱敏 audit logger、本机 smoke runbook、短生命周期双设备 HTTP smoke、Dockerfile / `.dockerignore`、Docker Compose 本地 / 部署态入口，以及 Rust HTTP transport 直连 Go server 的短生命周期跨语言测试；不包含完整真实用户生产封装。
- `docs/cli.md`：`radishlex-ime-cli` 命令、输出、退出码和安全边界说明。
- `docs/engine-boundary.md`：Rust core 与底层输入引擎的稳定边界。
- `docs/engine-rime-adapter.md`：`ime-engine-rime` 的 adapter 边界、构建策略和验证分层。
- `docs/personalization-learning.md`：Phase 2 个人化学习、userdb、ranker、负反馈和 CLI 管理边界。
- `docs/sync-payload.md`：同步 payload 草案和 P1/P2 来源分类。
- `docs/crypto-boundary.md`：`ime-crypto` 进入实现前的客户端加密、密钥、envelope 和验证边界。
- `docs/sync-key-management.md`：真实同步前的同步密钥、设备授权、恢复码、设备撤销、key epoch 和冲突边界。
- `docs/sync-server-api-storage.md`：Go sync server API、SQLite metadata、对象存储、版本冲突、恢复 / 撤销记录、错误语义和停止线。
- `docs/production-recovery-flow.md`：生产恢复记录创建、轮换、撤销、新设备恢复加入、失败限速和停止线。
- `docs/adr/0002-recovery-code-kdf.md`：恢复码 Argon2id KDF、格式、恢复记录字段和生产实现验证口径。
- `docs/adr/0003-device-signing-key-storage.md`：设备签名、签名对象、私钥存储抽象、错误语义和验证口径。
- `docs/adr/0004-platform-private-key-storage-backend.md`：平台私钥存储 backend、capability metadata、FFI 边界、错误语义和停止线。
- `docs/ffi-boundary.md`：后续 C ABI、所有权、生命周期和错误语义边界。
- `docs/runbooks/ffi-platform-call-contract.md`：平台绑定层调用 C ABI 的错误、字符串、handle 释放和 owner-thread 调度规则。
- `docs/runbooks/rime-native-smoke.md`：真实 `librime` 本机 smoke 操作步骤。
- `docs/runbooks/sync-server-local-smoke.md`：Go sync server 本机启动边界、自动化 smoke 和日志脱敏检查。
- `docs/runbooks/sync-server-compose.md`：Go sync server Docker Compose 本地 HTTPS、部署态 HTTP 上游、持久化目录、外部反代示例、清理和停止线 runbook。

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

当前已创建 `crates/ime-userdb/`，落地 SQLite schema migration、用户词条 CRUD、选择事件记录、负反馈记录、删除 tombstone、ranker weight 摘要、用户词库导入导出、同步前置计数、`dictionary.user_terms` / `ranker.weights` / `dictionary.deleted_terms` P2 plaintext payload 只读迭代器、已解密 P2 JSON 到 `ime-sync` merge input 的解析入口、合并结果写回真实 userdb 的事务执行器，以及两客户端 userdb 同步边界 integration test；基础 CLI 管理入口已由 `ime-cli` 承接。

### ime-sync

同步客户端：

- 增量同步。
- 冲突合并。
- 版本管理。
- 设备状态。

当前已创建 `crates/ime-sync/`，落地 payload 来源分类、同步对象类型、P1/P2/本地审计分层、P2 plaintext payload 到 `ime-crypto` envelope 的 Rust 内部组装边界、从 crypto envelope 派生加密对象外壳元数据、同步域、设备状态、加入请求、授权包、撤销记录、对象版本冲突草案模型、客户端解密后合并模型、remote client DTO / transport trait 和 std-only `http://` HTTP transport；不启动长期运行后端，不开放用户可用同步。

### ime-crypto

加密：

- 主密钥。
- 设备密钥。
- blob 加密。
- 签名和校验。

当前已创建 `crates/ime-crypto/`，落地 XChaCha20Poly1305、HKDF-SHA256、SHA-256 ciphertext hash、Argon2id recovery KDF、key role、object envelope、AAD、nonce、device key descriptor、device wrapping key / record、recovery material、Ed25519 设备签名、test-memory signing key store、platform backend capability metadata、unavailable backend 明确失败、revoked key 阻断、signed sync object manifest、signed recovery record、删除同步和篡改失败测试；生产恢复流程和平台私钥存储 backend 边界已由文档固定，真实平台 backend 尚未实现。

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

Go server 当前实现继续以 `docs/sync-server-api-storage.md` 为 API、storage、错误语义和验证边界。服务端只保存密文对象、设备公钥、签名记录、版本和必要同步元数据，不解析 userdb payload，不接触 P1 原始事件，也不进入输入热路径。

```text
server/sync-server/
  cmd/radishlex-sync-server/
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

当前已起步 `server/sync-server/`，但只实现 metadata / storage / API / runtime 验证模型、SQLite-backed metadata repository、local object storage staged transaction、对象 Rust envelope hash / length 复验、Ed25519 签名验签门禁、device wrapping encrypted key bytes 承载、recovery wrapped material 读取、recovery latest handler、domain / device / join request metadata handler、authorization handler、encrypted object version 上传 / metadata 读取 / payload 下载 handler、单用户 bearer access token 门禁、request id、panic recovery、非持久审计 hook、SQLite audit_events 写入、启动入口、runtime 配置装配、脱敏日志、本机 runbook、Docker Compose 本地 / 部署态入口和短生命周期双设备 HTTP smoke。Rust `ime-sync` 已起步 remote client DTO / transport trait、std-only `http://` HTTP transport 和可选 bearer token header，`ime-userdb` 已补两客户端 userdb harness，Rust HTTP transport 直连 Go server 的短生命周期跨语言测试已覆盖对象上传、下载和 stale conflict；完整真实用户生产封装、Flutter manager 和平台壳继续后置。

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
