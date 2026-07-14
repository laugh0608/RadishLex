# RadishLex 仓库结构

本文档说明 RadishLex 当前已提交的目录、模块职责和规划但尚未落地的边界，读者是维护者和实现者。本文不记录逐文件实现进度、测试流水、未来审批过程或平台安装步骤；当前批次见 `docs/status/current.md`，实现事实见源码和 devlog，平台操作见 runbook。

## 当前结构

```text
RadishLex/
  README.md
  LICENSE
  AGENTS.md
  CLAUDE.md
  Cargo.toml
  Cargo.lock
  .github/
    workflows/
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
      Dockerfile
  deploy/
    sync-server/
      docker-compose.local.yaml
      docker-compose.yaml
      nginx.prod.conf
  apps/
    radishlex-manager/
  platforms/
    macos-imk/
      Sources/
      Resources/
      Tests/
      Tools/
      ReferenceProbe/
      build-bundle.sh
      cleanup-user-install.sh
    android-ime/
      keystore-bridge/
  docs/
    status/
    remediation/
    adr/
    runbooks/
    devlogs/
  scripts/
  tests/
    fixtures/
```

这个树只列出已提交的主要入口。构建产物、IDE 配置、本地日志、临时目录、真实密钥和部署数据不属于仓库结构。

## 顶层职责

| 路径 | 职责 | 边界 |
| --- | --- | --- |
| `crates/` | Rust 输入核心、学习、加密、同步和 FFI | 跨平台业务真相源 |
| `server/` | Go 自部署同步服务 | 只处理密文对象和必要 metadata |
| `deploy/` | Compose、反向代理和部署示例 | 不保存真实 secret 或运行数据 |
| `apps/` | Flutter manager | 不进入输入热路径 |
| `platforms/` | 系统输入法与平台能力薄壳 | 不承载排序、同步或隐私真相源 |
| `docs/` | 正式文档、临时整改、ADR、runbook 和周志 | 按文档职责分离当前状态与稳定边界 |
| `scripts/` | 仓库检查、构建和 smoke 入口 | 根目录只保留稳定高频入口 |
| `tests/` | 跨模块共享 fixture | 不存真实用户或敏感数据 |
| `.github/` | PR、Release 和仓库治理 workflow | 门禁应覆盖真实交付链 |

## 当前成熟度边界

| 范围 | 已有工程形态 | 尚未形成的产品能力 |
| --- | --- | --- |
| Rust input | core、进程级 Rime runtime、CLI、ABI v3 selection/key result、受测输入 header 与 macOS 基础实机输入证据 | 完整 R01A 双应用/生命周期验收与可重复产品输入链 |
| 本地学习 | userdb、ranker、管理接口和测试 | 事务化用户意图、有效 recency 和固定评测基线 |
| 同步 | crypto/sync 模型、Go server、HTTP 集成测试 | 确定合并、完整设备生命周期、生产 HTTPS 编排 |
| Flutter manager | macOS 工程、真实开发期 FFI bridge、widget tests | 默认产品 FFI bundle、持久化和平台文件访问 |
| 平台 | macOS InputMethodKit 薄壳、contract/native bundle、基础实机输入与隔离 reference probe；Android Keystore 能力验证桥 | macOS R01A 完整退出、产品安装包；其他系统输入法 |

具体当前批次和停止线只在 `docs/status/current.md` 维护，本表只表达目录的产品边界。

## Rust crates

### ime-core

平台无关输入领域模型：

- input session
- key event 与 KeyOutcome
- composition、candidate、commit
- engine trait 与 schema/status
- 核心错误类型

不得依赖 SQLite、Rime 私有类型、网络或平台框架。

### ime-engine-rime

`librime` adapter：

- native binding 与构建探测
- 进程级 Rime runtime
- Rime session 生命周期
- key/composition/candidate/commit/status 转换
- adapter 错误和 native smoke

Rime 私有概念不得越过该 crate。

### ime-ranker

纯候选重排与 explain：

- engine 顺序因子
- user term、frequency、recency、context
- negative、suppressed、deleted
- deterministic ordering 与评测 helper

不得直接访问 SQLite、网络或平台 API。

### ime-userdb

SQLite 用户数据层：

- user terms 与 tombstone
- selection 和 negative feedback
- ranker summary
- import/export 与 batches
- schema migration、事务和同步 payload adapter

本地原始事件和 P2 同步摘要必须有明确转换边界。若 `ime-userdb` 依赖同步协议类型，应通过窄 adapter 或中立领域模型控制依赖方向。

### ime-crypto

客户端密码边界：

- key material 与 role
- AEAD envelope、AAD、nonce、hash
- device wrapping 与 recovery KDF
- signing、verification 和 platform key backend
- secret redaction/zeroization

不得包含 HTTP、Go server DTO 或 Flutter 状态。

### ime-sync

客户端同步协议和 orchestration：

- P2 object 与版本
- device/recovery/revocation lifecycle
- deterministic merge
- signed/encrypted remote DTO
- discovery、download、upload、retry 和 transport

不得提供 plaintext 远端上传入口。

### ime-ffi

C ABI 与 host contract：

- version/capability
- session、runtime 和 manager handles
- Rust-owned key result、同事件 snapshot 与受编译测试约束的输入 header
- string/buffer/view ownership
- structured errors 与 panic boundary
- thread policy 与 release functions

生产源码只保存真实 ABI 和必要兼容层。审批流程、未来 symbol 列表和“尚未导出”证明应放文档或历史材料，不应长期存在于 `src/`。

### ime-cli

开发和领域验证入口：

- real/demo engine 输入
- dictionary 与 learning
- rank explain
- sync preflight 和受控 smoke

CLI 不作为平台壳或 manager 的运行时依赖。

## Go sync server

```text
server/sync-server/
  cmd/radishlex-sync-server/   executable assembly
  internal/api/                HTTP routing and DTO mapping
  internal/config/             environment configuration
  internal/runtime/            server lifecycle and wiring
  internal/storage/            metadata and blob repositories
  migrations/                  embedded SQLite migrations
```

边界要求：

- `api` 不绕过 storage interface 直接访问 SQLite 或 blob path。
- `storage` 负责 metadata/blob 一致性、签名验证前置和持久化错误语义。
- `runtime` 负责 timeout、shutdown、audit 和依赖装配。
- `cmd` 保持薄，只处理启动、配置和退出码。
- 默认单用户 SQLite，自部署优先，不提前拆微服务或多租户。

## Flutter manager

```text
apps/radishlex-manager/
  lib/src/bridge/       Dart FFI and ManagerBridge
  lib/src/models/       UI-facing immutable models
  lib/src/screens/      manager, dictionary, learning, sync, settings
  lib/src/data/         explicit fixture/demo data
  test/                 unit, mapper and widget tests
  macos/                Flutter macOS host
```

边界要求：

- `bridge` 复制 native borrowed data 后再释放 handle。
- `models` 不重新实现 Rust 排序、合并或隐私规则。
- `screens` 只编排用户交互和展示结构化状态。
- fixture 必须通过显式开发模式启用，不可成为产品默认成功路径。
- 产品构建需要打包匹配版本的 Rust native library。

后续若支持更多 Flutter host，应复用 manager bridge 和模型，但不把 Flutter 引入系统输入候选窗。

## 平台目录

当前 `platforms/macos-imk/` 已包含 Objective-C InputMethodKit 薄壳、bundle build、不安装系统输入法的 wrapper contract smoke、公开 TIS 只读状态/监视工具、授权清理入口，以及不接 Rime/FFI 的合成 reference probe。正式薄壳已完成 Apple Development 短时安装，并在精确 source 归属下通过候选跟随光标、方向视觉与 Space 提交一致、鼠标选择和宿主焦点；边缘定位、多屏/全屏、输入菜单、client 切换、进程重启、断网与双应用交叉矩阵尚未闭合，不能据此宣称 R01A 或 M1 退出。`platforms/android-ime/keystore-bridge/` 只是 Android Keystore 算法与 JNI 能力验证工程，不是完整 Android IME。

后续平台目录按进入顺序创建：

1. `platforms/linux-fcitx5/`：macOS 达到退出标准后的桌面候选。
2. `platforms/android-ime/`：在现有 keystore bridge 之外补完整 IME。
3. `platforms/windows-tsf/`。
4. `platforms/ios-keyboard/`。

只有对应平台设计边界和退出标准明确后才创建目录。空目录、占位文件或安装脚本不构成平台进度证据。

## 文档目录

- `docs/status/current.md`：唯一当前阶段短入口。
- `docs/remediation/`：当前状态明确引用的临时执行专题，完成后归档。
- `docs/adr/`：已决策且需要长期追溯的架构选择。
- `docs/runbooks/`：可重复操作步骤、环境前提和停止线。
- `docs/devlogs/`：周内事实、命令、提交和历史流水。
- `docs/*.md`：稳定架构、边界、协议、guide 或 reference。

新增或大改文档必须在开头说明用途、读者和不包含内容。状态事实不得复制到多个稳定专题；详细实现流水不得进入 roadmap 或协作入口。

## 规划但尚未落地

以下内容不是当前仓库事实，只有出现真实需要时才创建：

- `go.work`：只有出现第二个 Go module 时再评估。
- `melos.yaml`：只有出现多个需要统一编排的 Dart/Flutter package 时再评估。
- `examples/`：只有形成可维护的公开 schema、词库或配置样例后创建。
- 顶层 `tests/integration/`：只有跨语言测试无法合理归属现有 crate/server 时创建。
- `apps/desktop-tools/`：不为一次性调试工具提前建目录。
- 未进入当前顺位的平台壳目录。

规划名称不构成承诺。新增目录前应先确认职责不能由现有模块清晰承担。

## 结构治理规则

- 单个源码文件接近 1000 行时优先拆分职责，原则上不超过 1500 行。
- `src/` 使用浅层职责分组，避免全部平铺或过深目录树。
- `scripts/` 根目录保留稳定入口，较长实现放浅层分类目录。
- committed 相对路径默认不超过 180 个字符。
- 生成物、SDK、native build cache、IDE 私有状态、真实部署数据和 secret 不提交。
- 测试 fixture 只能使用合成或公开数据，不包含真实输入历史和联系人。
- 目录变化影响职责或平台策略时，同步更新本文；逐文件进度变化不更新本文。
