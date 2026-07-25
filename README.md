# RadishLex

RadishLex（萝卜词核）是一款本地优先、可解释、可删除、支持自部署加密同步的源代码可见中文输入系统。它首先要成为可日常使用的本地输入法，再逐步扩展个人化学习与跨设备同步。

项目以 Rust 为输入核心、Go 为自部署同步后端、Flutter 为管理界面，并通过平台原生薄壳接入系统输入法。核心目标是让输入法逐步理解用户的词库、语气、场景和候选偏好，同时把数据控制权留给用户。

许可条款以仓库根 [LICENSE](LICENSE) 为准，当前采用 RadishLex Source-Available License。

## 设计原则

- **本地优先**：候选生成、候选重排、学习和常用输入路径必须离线可用。
- **服务端不可信**：后端只保存密文对象和必要 metadata，不进入每次按键热路径。
- **可解释学习**：用户能查看、删除、导出、暂停或限制输入法学到的内容。
- **引擎可替换**：v1 接入成熟底层引擎，Rust core 不依赖其私有实现。
- **平台薄壳**：系统输入法端只处理生命周期、按键、候选展示、文本提交和 FFI。
- **删除优先**：tombstone 防止旧事件、旧设备和旧备份复活用户已删除内容。

## 技术栈

- **Rust**：输入会话、engine adapter、候选重排、用户词库、学习、同步客户端、加密和 FFI。
- **Go**：单用户自部署同步服务、设备、密文对象、版本、备份恢复和审计。
- **Flutter**：本地词库、学习、隐私、同步、设备和诊断管理界面。
- **平台原生薄壳**：macOS InputMethodKit、Linux Fcitx5/IBus、Android IME、Windows TSF、iOS Keyboard Extension。

当前工程成熟度、停止线和下一步只在 [当前状态](docs/status/current.md) 维护。仓库已有 Rust、Go、Flutter 工程原型、macOS 离线输入 Alpha、本地个人化 MVP 与 M3 端到端加密同步 Beta 退出证据；当前进入 M4 产品发布候选，真实用户同步仍保持关闭。

## 稳定入口

- [当前状态](docs/status/current.md)：当前批次、验证基线、停止线和下一步。
- [技术方案](docs/technical-plan.md)：稳定架构、职责、输入链和平台策略。
- [产品交付路线图](docs/roadmap.md)：产品里程碑、交付物和退出标准。
- [macOS 产品包边界](docs/macos-product-package-boundary.md)：M4 组件、版本、数据与发布停止线。
- [macOS 安装载体 ADR](docs/adr/0008-macos-installation-carrier.md)：M4-P03 用户域 Installer、固定目标、程序事务与移除边界。
- [macOS 程序安装事务](docs/macos-installation-transaction.md)：外层 operation、产品身份、receipt/guard 与启动门禁。
- [macOS 数据升级协调器](docs/macos-data-upgrade-coordinator.md)：M4-P02 状态机、receipt、隔离 migration 与回滚边界。
- [macOS 产品装配 Runbook](docs/runbooks/macos-product-assembly.md)：锁定输入、双 bundle 构建、manifest 复验和失败处理。
- [RimeData 产品输入](packaging/rime/README.md)：固定 schema、Apache 词典、来源锁与逐资产许可证。
- [仓库结构](docs/repository-layout.md)：实际目录、模块职责和未落地边界。
- [隐私与同步](docs/privacy-sync.md)：数据分级、密钥、删除、恢复和威胁模型。
- [Engine Boundary](docs/engine-boundary.md)：核心 engine 契约。
- [Rime Adapter](docs/engine-rime-adapter.md)：librime adapter 与 native smoke。
- [个人化学习](docs/personalization-learning.md)：userdb、ranker、反馈和词库管理。
- [FFI Boundary](docs/ffi-boundary.md)：C ABI、所有权、线程和错误语义。
- [产品事务 FFI 参考](docs/ffi-product-upgrade-reference.md)：ABI v9 install/data startup 与 validation 结构、常量和证据转换。
- [Manager 同步产品状态](docs/manager-sync-product-status.md)：当前 ABI v9 保留的 status-only 字段、blocker 和隐私 allowlist。
- [macOS InputMethodKit](docs/macos-inputmethodkit-boundary.md)：第一平台的 runtime、按键链、目录和验收边界。
- [同步密钥管理](docs/sync-key-management.md)：设备、授权、恢复、撤销和 key epoch。
- [Sync Server API/Storage](docs/sync-server-api-storage.md)：Go API、metadata、blob 和错误语义。
- [Manager Boundary](docs/manager-ui-boundary.md)：Flutter manager 职责与数据可见性。

更细的 ADR、runbook 和协议专题从上述入口按任务进入。临时整改或发布专题只有被 `docs/status/current.md` 引用时才进入日常阅读链。

核心组件的就地开发说明：

- [产品数据升级核心](crates/ime-product-upgrade/README.md)：receipt、guard、snapshot/candidate、startup gate 和 validation evidence。
- [Rust 同步客户端](crates/ime-sync/README.md)：cycle 编排、产品密码装载、transport 和验证入口。
- [Manager 同步组合层](crates/ime-sync-runtime/README.md)：合成资格 request、状态机、错误、取消和清理契约。
- [Go 同步服务](server/sync-server/README.md)：服务边界、配置、存储和开发验证。
- [同步服务部署](deploy/sync-server/README.md)：本地 HTTPS 与自部署反向代理拓扑。
- [Flutter Manager](apps/radishlex-manager/README.md)：页面能力、FFI bridge、产品路径和本地验证。
- [macOS InputMethodKit](platforms/macos-imk/README.md)：平台薄壳、构建、安装与实机验收边界。
- [macOS 产品升级宿主](platforms/macos-product/README.md)：只读 preflight、双端 validation、manifest 绑定 adapter 和产品构建接线。

## 开发验证入口

仓库常态基线：

```bash
./scripts/check-repo.sh
```

Rust CLI demo：

```bash
cargo run -p radishlex-ime-cli -- demo luobo
```

本地词库和学习：

```bash
cargo run -p radishlex-ime-cli -- dict add \
  --db /tmp/radishlex-userdb.sqlite --input luobo --text 萝卜
cargo run -p radishlex-ime-cli -- learn select \
  --db /tmp/radishlex-userdb.sqlite --input luobo --text 萝卜
cargo run -p radishlex-ime-cli -- rank explain \
  --db /tmp/radishlex-userdb.sqlite --input luobo --candidate 萝卜
```

真实 Rime adapter 需要本机 `librime` 和隔离 schema 数据：

```bash
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime --schema luna_pinyin --shared-data <path> --user-data <path> luobo
```

详细命令见 [CLI 说明](docs/cli.md)，精确学习状态与非选择候选快照见 [学习取证 CLI 参考](docs/cli-learning-evidence.md)，本机 Rime 环境见 [Rime Native Smoke Runbook](docs/runbooks/rime-native-smoke.md)。

macOS InputMethodKit 不安装验证：

```bash
./scripts/check-macos-imk.sh
```

该入口编译正式 Objective-C 条件分支，并运行 wrapper、AppKit candidate panel、controller 与只读 TIS 工具契约，但不会安装或启用输入法。native-rime bundle、授权后签名安装、实时来源监视和完整移除见 [macOS 平台说明](platforms/macos-imk/README.md) 与 [开发 runbook](docs/runbooks/macos-inputmethodkit-development.md)。

Flutter manager：

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
```

`check-manager-ffi-smoke.sh` 使用临时 SQLite、settings 和合成词库验证开发期真实 Dart FFI bridge；它不证明正常产品包已经携带 native library，也不连接真实同步服务。

Go sync server：

```bash
(cd server/sync-server && go test ./...)
cargo test -p radishlex-ime-sync
cargo test -p radishlex-ime-userdb --test two_client_go_http_sync
docker compose -f deploy/sync-server/docker-compose.local.yaml config
```

部署、备份恢复、连接健康和证据校验见：

- [Compose Runbook](docs/runbooks/sync-server-compose.md)
- [Local Smoke Runbook](docs/runbooks/sync-server-local-smoke.md)
- [Production Deployment Runbook](docs/runbooks/sync-server-production-deployment.md)

真实 Keychain、Android Keystore、平台输入法安装、Docker 长流程和发布部署可能修改外部环境，必须按对应 runbook 和人工授权执行。

## 交付梯度

RadishLex 按用户可见纵向链分阶段交付，不要求同步、完整 manager 和最终安装包同时完成：

1. **M1 macOS 离线输入 Alpha**：真实应用中完成 composition、候选、选择、commit 和未消费按键回传；输入热路径完全离线。
2. **M2 本地个人化 MVP**：真实选择安全写入 userdb 并影响后续候选；用户可在 manager 中管理词库、学习和隐私设置。
3. **M3 加密同步 Beta**：两个真实客户端完成端到端加密同步、冲突收敛、删除传播、设备授权、恢复与撤销。
4. **M4 产品发布候选**：输入法、manager、Rust native library、`librime`、schema、签名、升级和发布门禁形成可重复产品包。

v1 不重写完整中文输入引擎。拼音切分、基础候选和长句转换可由成熟底层引擎提供，RadishLex 聚焦稳定 Rust 输入核心与 engine adapter、用户词库、候选重排、个人化学习、端到端加密同步和至少一个可日常使用的真实平台输入法。

第一真实平台固定为 macOS InputMethodKit。同步不阻塞 M1/M2 的本地输入与个人化交付；第二平台只有在第一平台达到退出标准后再选择和启动。

## 非目标

- 不做云端实时输入法 API。
- 不默认上传原始输入流、P1 原始事件或明文用户词库。
- 不用 Flutter 或 egui 强行统一系统候选窗。
- 不在 v1 阶段重写完整拼音引擎。
- 不复制外部项目源码结构、实现细节或受限词库。
- 不同时展开全部桌面和移动平台。

## 一句话

RadishLex 是一个把个人输入习惯留在自己手里的源代码可见中文输入系统。
