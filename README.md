# RadishLex

RadishLex 是一个以 Rust 为输入核心、Go 为自部署同步后端、Flutter 为管理界面的源代码可见中文输入系统。

项目目标不是再做一个单平台输入法外壳，而是构建一个本地优先、隐私可信、可跨设备同步、能够长期学习个人输入习惯的中文输入基础设施。

## 当前定位

- **项目名**：`RadishLex`
- **中文定位**：萝卜词核
- **核心目标**：让输入法逐步理解用户的词库、语气、场景和候选偏好，达到可解释、可删除、可自部署的个人化输入体验。
- **技术主轴**：Rust + Go + Flutter
- **复核日期**：2026-07-06

## 设计原则

- **本地优先**：候选生成、候选重排、用户学习和常用输入路径必须离线可用。
- **自部署优先**：后端只做同步、备份、设备管理和模型/词库分发，不进入每次按键热路径。
- **隐私优先**：服务端默认不持有明文输入习惯、用户词库和上下文数据。
- **可解释学习**：用户应该能看到输入法学会了什么，并能删除、暂停或限制学习范围。
- **引擎可替换**：第一阶段可接入成熟底层引擎，长期保留 Rust 自研输入引擎的替换空间。
- **平台薄壳**：Windows、macOS、Linux、Android、iOS 只承担系统输入法接入，业务逻辑沉入 Rust core。

## 技术栈

- **Rust**：输入会话、候选重排、用户词库、个人化学习、同步客户端、加密、FFI。
- **Go**：自部署后端、设备管理、加密 blob 同步、版本历史、备份恢复、模型与词库包分发。
- **Flutter**：移动端设置页、桌面管理器、词库可视化、同步状态、隐私控制台。
- **平台原生薄壳**：TSF、InputMethodKit、Fcitx5/IBus、Android IME、iOS Keyboard Extension。

## 稳定入口

- [当前状态短入口](docs/status/current.md)
- [详细技术方案](docs/technical-plan.md)
- [阶段路线图](docs/roadmap.md)
- [仓库结构草案](docs/repository-layout.md)
- [CLI 说明](docs/cli.md)
- [Engine Boundary](docs/engine-boundary.md)
- [ime-engine-rime Adapter 设计](docs/engine-rime-adapter.md)
- [个人化学习设计](docs/personalization-learning.md)
- [隐私与同步设计](docs/privacy-sync.md)
- [同步 Payload 草案](docs/sync-payload.md)
- [ime-crypto 边界设计](docs/crypto-boundary.md)
- [同步密钥与设备生命周期设计](docs/sync-key-management.md)
- [同步服务端 API 与存储边界](docs/sync-server-api-storage.md)
- [Sync Server Compose Runbook](docs/runbooks/sync-server-compose.md)
- [Sync Server Production Deployment Runbook](docs/runbooks/sync-server-production-deployment.md)
- [Sync Server OIDC 未来接入规划](docs/sync-server-oidc-roadmap.md)
- [Sync Server Admin Console 远期专题](docs/sync-server-admin-console.md)
- [生产恢复流程设计](docs/production-recovery-flow.md)
- [管理端边界](docs/manager-ui-boundary.md)
- [Flutter manager 本地验收口径](docs/manager-local-acceptance.md)
- [Flutter manager 真实同步入口前置边界](docs/manager-sync-entry-boundary.md)
- [Manager 恢复码与设备授权交互进入条件](docs/manager-recovery-device-auth-flow.md)
- [Manager Readiness 场景目录](docs/manager-readiness-scenarios.md)
- [Manager 同步 Action 协议预演边界](docs/manager-sync-action-protocol-preview.md)
- [Manager 同步 Action 验收矩阵](docs/manager-sync-action-acceptance-matrix.md)
- [Manager 同步 Bridge 命令 Contract 检查清单](docs/manager-sync-bridge-command-contract-checklist.md)
- [Manager 同步 Bridge 命令 Contract 草案](docs/manager-sync-bridge-command-contract.md)
- [Manager Settings 与诊断报告字段参考](docs/manager-settings-diagnostics.md)
- [ADR 0002: 恢复码 KDF 与同步域恢复边界](docs/adr/0002-recovery-code-kdf.md)
- [ADR 0003: 设备签名与私钥存储边界](docs/adr/0003-device-signing-key-storage.md)
- [ADR 0004: 平台私钥存储 Backend 边界](docs/adr/0004-platform-private-key-storage-backend.md)
- [ADR 0005: Apple 平台签名策略](docs/adr/0005-apple-platform-signing-strategy.md)
- [平台私钥 Backend 策略](docs/platform-private-key-backend-strategy.md)
- [Apple Keychain Signing Backend Runbook](docs/runbooks/apple-keychain-signing-backend.md)
- [Android Keystore Signing Backend Runbook](docs/runbooks/android-keystore-signing-backend.md)
- [FFI 边界](docs/ffi-boundary.md)

## 当前可运行入口

当前仓库已提供 `radishlex-ime-cli` 作为 Rust 侧复验入口：

```bash
cargo run -p radishlex-ime-cli -- demo luobo
```

真实 Rime adapter 需要本机 `librime` 和隔离 schema 数据：

```bash
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime --schema luna_pinyin --shared-data <path> --user-data <path> luobo
```

Phase 2 起步的本地学习链路可通过显式 SQLite 路径复验：

```bash
cargo run -p radishlex-ime-cli -- dict add --db /tmp/radishlex-userdb.sqlite --input luobo --text 萝卜
cargo run -p radishlex-ime-cli -- learn select --db /tmp/radishlex-userdb.sqlite --input luobo --text 萝卜
cargo run -p radishlex-ime-cli -- rank explain --db /tmp/radishlex-userdb.sqlite --input luobo --candidate 萝卜
cargo run -p radishlex-ime-cli -- sync preflight --db /tmp/radishlex-userdb.sqlite
```

启用 `native-rime` 时，可进一步用 `rime --rank-db <path>` 验证真实 Rime candidates 进入本地 ranker。

完整命令说明见 [CLI 说明](docs/cli.md)，本机 Rime 数据准备步骤见 [Rime Native Smoke Runbook](docs/runbooks/rime-native-smoke.md)。

Go sync server 当前已有短生命周期测试、Docker Compose 本地 / 部署态入口和生产部署 runbook。常用复验入口：

```bash
(cd server/sync-server && go test ./...)
cargo test -p radishlex-ime-sync
cargo test -p radishlex-ime-userdb --test two_client_go_http_sync
docker compose -f deploy/sync-server/docker-compose.local.yaml config
docker compose -f deploy/sync-server/docker-compose.yaml --env-file deploy/sync-server/.env.example config
./scripts/check-sync-server-deployment-rehearsal.sh --config-only
./scripts/check-sync-server-connection-health.sh --self-test
./scripts/check-sync-deployment-evidence.sh tests/fixtures/sync-deployment-evidence-valid.txt
./scripts/check-sync-deployment-evidence.sh --summary-json tests/fixtures/sync-deployment-evidence-valid.txt
```

本地 Compose 测试态使用 Caddy internal TLS 暴露 `https://localhost:7319`；部署态只提供同机 HTTP upstream `http://127.0.0.1:7319`，外部 TLS 和访问控制由部署者配置。生产访问控制当前先使用 `RADISHLEX_SYNC_ACCESS_TOKEN` 单用户 bearer token；OIDC / Radish 产品账号体系已作为后续专题记录，不是当前必须部署的账号系统。

完整部署预演可执行 `./scripts/check-sync-server-deployment-rehearsal.sh`，它会用临时 env、随机 bearer token 和仓库外数据目录短生命周期启动部署态 Compose，验证 token 门禁、日志脱敏和冷备份恢复；该命令需要 Docker daemon 可用，默认仓库检查不运行。

本地同步服务连接健康摘要可用 `./scripts/check-sync-server-connection-health.sh` 采集，脚本只输出 `sync_connection_health.v1` 非敏感状态码、来源标签、HTTP 状态分类和错误分类，不输出 token、请求 / 响应体、证书内容、真实路径或 payload bytes。目标部署证据包格式和脱敏规则可用 `./scripts/check-sync-deployment-evidence.sh <evidence-file>` 复验；通过后可用 `--summary-json` 或 `--summary-text` 导出非敏感交接摘要。该命令只验证证据文件结构、敏感内容黑名单和摘要字段，不证明目标环境已经部署成功。

Flutter manager 当前已在 `apps/radishlex-manager/` 起步，通过受控 `ManagerBridge` contract 接入合成 fixture，并可在显式配置本地 SQLite userdb 与 `ime-ffi` 动态库后切到真实 Dart FFI bridge。当前管理端展示本地词库、import batches、学习摘要、`rank explain`、`sync preflight`、settings draft、sync gate 草案、服务连接健康、恢复码 / 设备授权只读准备态、readiness 聚合摘要、`manager_sync_readiness.v1` 非敏感摘要导入、开发期 evidence bundle 同源回归、只读交互进入计划、`manager_sync_action_command_preview.v1` 非执行命令预演、request / result preview、future bridge command contract 检查清单 / 草案和脱敏诊断摘要预览 / 导出；本地验收口径见 [Flutter manager 本地验收口径](docs/manager-local-acceptance.md)，真实同步入口进入 UI / bridge 前的交互边界见 [Flutter manager 真实同步入口前置边界](docs/manager-sync-entry-boundary.md)，恢复码与设备授权只读流程见 [Manager 恢复码与设备授权交互进入条件](docs/manager-recovery-device-auth-flow.md)，action 预演边界见 [Manager 同步 Action 协议预演边界](docs/manager-sync-action-protocol-preview.md)，真实 bridge 命令前检查清单见 [Manager 同步 Bridge 命令 Contract 检查清单](docs/manager-sync-bridge-command-contract-checklist.md)，future contract 草案见 [Manager 同步 Bridge 命令 Contract 草案](docs/manager-sync-bridge-command-contract.md)。真实远端同步、恢复码生成 / 输入、join request 创建、设备授权成功和设备撤销 UI 仍按管理端边界保持关闭：

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
```

`check-manager-ffi-smoke.sh` 会构建 `radishlex-ime-ffi` 动态库，并用临时 SQLite userdb、合成词库文件和临时 settings JSON 复验 Dart FFI bridge 的本地 list / delete / import / export、import batches、learning status、rank explain、sync preflight 摘要、设置草案持久化和脱敏诊断报告导出；该命令不连接真实同步后端，不读取真实输入法目录。

Apple Keychain backend 已在 `apple-keychain` feature 下接线，但真实 smoke 阻塞于 `ed25519-v1` 创建，`apple-keychain-v1` 在该 blocker 解除前会阻断生产签名。默认测试不会触碰本机 Keychain。

Android Keystore backend 已在 `android-keystore` feature 下接入 Rust bridge wrapper、raw JNI glue、仓库内 Kotlin bridge、Gradle harness、gated smoke 和 provider diagnostics。默认仓库验证不会触碰 Android Keystore；Android target build 可用仓库根命令复验：

```bash
./scripts/check-android-target.sh
```

Android Kotlin harness 位于 `platforms/android-ime/keystore-bridge/`，普通构建不创建 Keystore item：

```bash
cd platforms/android-ime/keystore-bridge
JAVA_HOME=<Android Studio bundled JBR> ./gradlew assembleDebug
```

真实设备 / AVD 诊断或 smoke 必须显式传入 gated 参数，并在执行前确认允许触碰测试设备 Android Keystore：

```bash
JAVA_HOME=<Android Studio bundled JBR> ./gradlew connectedAndroidTest -Pradishlex.runAndroidKeystoreDiagnostics=true
JAVA_HOME=<Android Studio bundled JBR> ./gradlew connectedAndroidTest -Pradishlex.runAndroidKeystoreSmoke=true
```

Pixel 9 Pro API 35 AVD 和 Pixel 10 Pro API 37 AVD 当前诊断结果均为 `unsupported_signature_algorithm`：JCA factory 表面可用，但 `AndroidKeyStore` 实际生成 `EC` key，不能满足 `ed25519-v1` 设备签名协议。`android-keystore-v1` production gate 继续关闭，不切换 P-256，也不回退到 seed / app storage / `test-memory-v1`。

无新增 Android 真机或不同系统镜像时，平台私钥 backend 推进以 [平台私钥 Backend 策略](docs/platform-private-key-backend-strategy.md) 为准：保留 `ed25519-v1`，不在现有 backend 内降级；当前产品开发优先推进 manager 同步入口的非上传状态派生和本地 Docker / 本地 HTTPS 联调，发布级目标部署运行证据留到正式发布或真实用户开放前补齐。

## MVP 边界

第一阶段不重写完整中文输入引擎。拼音切分、候选生成、长句转换和基础词库能力可由底层引擎提供，RadishLex 的重点放在：

- 统一 Rust 输入核心抽象
- 用户词库与候选重排
- 个人化学习与负反馈
- 自部署加密同步
- 桌面/移动管理界面
- 至少一个真实平台输入法端落地

## 非目标

- 不做云端实时输入法 API。
- 不默认上传原始输入流。
- 不用 Flutter 或 egui 强行统一系统候选窗。
- 不在 v1 阶段重写完整拼音引擎。
- 不复制开源项目源码结构或实现细节。

## 一句话

RadishLex 是一个把个人输入习惯留在自己手里的源代码可见中文输入系统。
