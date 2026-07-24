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
    ime-runtime/
    ime-ranker/
    ime-userdb/
    ime-product-upgrade/
      README.md
    ime-sync/
      README.md
    ime-sync-runtime/
      README.md
    ime-crypto/
    ime-ffi/
    ime-cli/
  server/
    sync-server/
      README.md
      cmd/
      internal/
      migrations/
      Dockerfile
  deploy/
    sync-server/
      README.md
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
      ValidationHost/
      build-bundle.sh
      cleanup-user-install.sh
      cleanup-r01b-test-userdb.sh
      cleanup-m2-manager-test-data.sh
      privacy-mode.sh
    macos-product/
      README.md
      UpgradePreflightHost/
      UpgradeValidationHosts/
        Sources/
    android-ime/
      keystore-bridge/
  packaging/
    macos/
      product.json
    rime/
      data/
      licenses/
      product-rime-data.json
  docs/
    status/
    remediation/
    adr/
    runbooks/
    devlogs/
  scripts/
    macos-product/
    rime-product/
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
| `packaging/` | 产品版本、兼容性和装配元数据 | 不保存签名凭据或构建产物 |
| `docs/` | 正式文档、临时整改、ADR、runbook 和周志 | 按文档职责分离当前状态与稳定边界 |
| `scripts/` | 仓库检查、构建和 smoke 入口 | 根目录只保留稳定高频入口 |
| `tests/` | 跨模块共享 fixture | 不存真实用户或敏感数据 |
| `.github/` | PR、Release 和仓库治理 workflow | 门禁应覆盖真实交付链 |

## 当前成熟度边界

| 范围 | 已有工程形态 | 尚未形成的产品能力 |
| --- | --- | --- |
| Rust input | core、进程级 Rime runtime、产品个人化 runtime、CLI、ABI v8、Manager 产品状态与隔离资格 run、管理查询和共库证据；M4-P02 已固定只读 userdb inspection、snapshot/candidate、receipt/guard、startup gate、双模式 validation、原子切换、最终复验、精确回滚与 guard-bound 驱动 | macOS host adapter 与发布复验 |
| 本地学习 | schema v9 userdb、事务化用户意图、本地导入批次关联、确定性 ranker、产品热路径、并发 migration/WAL、同步 cursor/journal/outbox、原子 apply、可信 public lifecycle、wrapped ciphertext 与 recovery lifecycle cache | 明文 master key/shared secret 只短暂进入 Rust snapshot，不进入 SQLite/settings |
| 同步 | P2 crypto/sync、Ed25519/P-256 profile、Go server、Rust HTTP/TLS transport、关闭态 orchestration、通用 processor、生产 provider、设备 lifecycle 验证、wrapped epoch v1、Apple signing/key-agreement 产品资格、双 userdb Go HTTP 收敛、本地 Caddy HTTPS、Manager 受控资格执行链 | 真实用户入口开放评审、首版后的发布级目标部署 |
| Flutter manager | 默认 product/显式 demo、Release FFI bundle、固定平台路径、隐私 method channel、deleted restore、导入批次审计、双端刷新、同步产品 status、本地 HTTPS 合成资格 run、widget/FFI/产品门禁 | M4 数据升级、安装载体与发布分发 |
| 平台 | macOS InputMethodKit 薄壳、contract/native bundle、生产 LearningContext 与 privacy/清理 contract；M4-P01 双 bundle 与 locked RimeData；M4-P02 只读 preflight、双端 startup gate 和各 bundle 独立双模式 upgrade validation host；Android Keystore 能力验证桥 | M4 完整协调入口、安装载体和普通用户安装包；其他系统输入法 |

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

### ime-runtime

产品输入 session 的本地组合层：

- 组合 `ime-core` engine session、`ime-ranker`、`ime-userdb` 与隐私策略
- 取得稳定 input code，并批量读取当前候选的个人化信号
- 保存 display index 到 engine index 的当次快照映射
- 决定 selection 的即时或分段学习时机，并返回显式学习结果
- 在 userdb/ranker 故障时保留 engine 输入与 commit，同时暴露退化状态

每个 runtime session 持有独立 SQLite connection；平台壳、manager 和 engine adapter 不复制学习、删除或排序语义。该 crate 不依赖具体平台框架、远端同步或具体 engine 实现。

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

产品升级使用独立的只读 inspection、SQLite backup snapshot 和候选 migration/validation 接口。snapshot 在一个只读事务中纳入 WAL 可见内容，输出 standalone `DELETE` journal 文件；协调器只能把隔离副本交给修改型入口，不能用运行时打开原地升级真实 Application Support 数据。

### ime-product-upgrade

产品数据升级协调 contract：

- 单向升级、失败保留和 rollback 状态机
- 严格、版本化且无敏感内容的 receipt
- 固定逻辑槽位的文件身份与阶段证据
- 私有状态目录内的原子 receipt 存储、严格加载与逐状态替换
- 绑定固定数据根身份的跨进程 Unix socket guard
- 固定 userdb 源/目标、保守空间预算、SQLite snapshot 编排与阶段故障注入
- 固定 settings 保留副本与隔离 migration candidate 编排
- 完全只读的 startup gate 与终态/非终态恢复判断
- 双端 validation evidence v1 与 `candidate_verified` / `aborted_preserved` 持久化
- 固定旧库 backup、同文件系统双 rename、目录 `fsync` 与 `switch_prepared` / `switched` 幂等恢复
- 最终路径双端 evidence、`post_switch_verified` / `completed` 与完成前重复复验
- 失败新库回迁、旧库原 inode 恢复、source-release evidence 与 `rolled_back`
- settings/snapshot/candidate evidence-only 崩溃恢复与 guard-bound checkpoint 驱动
- 稳定失败分类和中断恢复判断

该 crate 当前已闭合固定布局内从 `preflighted` 到终态的核心调度与数据恢复，但仍不停止进程、不定位或启动产品 host，也不提供安装载体；API、副作用与验证入口见 [ime-product-upgrade 组件说明](../crates/ime-product-upgrade/README.md)，macOS 完整状态机见 [数据升级协调器边界](macos-data-upgrade-coordinator.md)。

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

模块、产品密码端口、cycle 数据流和开发验证入口见 [ime-sync 组件说明](../crates/ime-sync/README.md)。

### ime-sync-runtime

Manager 同步产品组合层：

- 组合 `ime-sync` orchestration/HTTPS、`ime-userdb` repository 与 `ime-crypto` provider
- 管理 Rust-owned qualification run、worker、取消、超时与临时目录
- 生成隔离合成双客户端身份和固定 P2 数据
- 输出固定脱敏 phase/result/error/count/cleanup 摘要

不得承载 C ABI、Flutter 状态、真实用户入口 gate 或输入热路径。资格运行不得触碰真实 userdb、Keychain/Secure Enclave，也不得把 test backend 冒充生产 backend。

请求约束、状态机、错误、取消和清理契约见 [ime-sync-runtime 组件说明](../crates/ime-sync-runtime/README.md)。

### ime-ffi

C ABI 与 host contract：

- version/capability
- session、runtime 和 manager handles
- Rust-owned key result、同事件 snapshot 与受编译测试约束的输入 header
- string/buffer/view ownership
- structured errors 与 panic boundary
- thread policy 与 release functions
- Apple 普通 DPK / Secure Enclave 独立产品 validation status/smoke（仅原生 gated host，不进入 Dart）

生产源码只保存真实 ABI 和必要兼容层。审批流程、未来 symbol 列表和“尚未导出”证明应放文档或历史材料，不应长期存在于 `src/`。

### ime-cli

开发和领域验证入口：

- real/demo engine 输入
- dictionary 与 learning
- R01B `case-status` 精确状态检查与 fresh isolated Rime user-data 非选择 snapshot
- rank explain
- sync preflight 和受控 smoke

CLI 不作为平台壳或 manager 的运行时依赖。

## Go sync server

```text
server/sync-server/
  README.md                     component boundary and development entry
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

服务配置、schema、日志脱敏和测试入口见 [Go 同步服务说明](../server/sync-server/README.md)；API 与 storage 的规范性字段参考仍以 [Sync Server API/Storage](sync-server-api-storage.md) 为准。部署目录的两种拓扑和 secret 边界见 [同步服务部署说明](../deploy/sync-server/README.md)。

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
- macOS method channel 只解析固定产品路径、收紧本地文件权限和读写 InputMethodKit 隐私偏好；不承载 userdb、排序、同步或密钥真相源。
- `models` 不重新实现 Rust 排序、合并或隐私规则。
- `screens` 只编排用户交互和展示结构化状态。
- fixture 必须通过显式开发模式启用，不可成为产品默认成功路径。
- 产品构建需要打包匹配版本的 Rust native library。
- macOS 产品 host 使用非 App Sandbox profile 与 InputMethodKit 共享用户 Application Support userdb；M4 首个候选继续使用 `application-support-v1`，未来 App Group 变化必须由独立 ADR 与迁移复验驱动。

后续若支持更多 Flutter host，应复用 manager bridge 和模型，但不把 Flutter 引入系统输入候选窗。

## 产品打包目录

`packaging/macos/product.json` 是 macOS 产品版本、build、最低系统、bundle ID、FFI ABI、userdb schema 和 manifest 格式的单一元数据真相源。`packaging/rime/product-rime-data.json` 绑定产品 schema、Apache 词典来源 commit/hash、运行时路径和逐资产许可证；`scripts/rime-product/product_data.py` 负责离线校验与装配。

`scripts/macos-product/product_manifest.py` 校验源码声明、生成/复验无绝对路径的 `ProductManifest.json`，`scripts/build-macos-product.sh` 从 committed RimeData 输入装配双 bundle 产品目录。该目录不承担安装、签名凭据、公证上传或用户数据迁移；具体构建和复验步骤见 [macOS 产品装配 Runbook](runbooks/macos-product-assembly.md)。

## 平台目录

当前 `platforms/macos-imk/` 已包含 Objective-C InputMethodKit 薄壳、bundle build、不安装系统输入法的 wrapper contract、公开 TIS 只读状态/监视工具、生产 `LearningContext`、privacy CFPreferences receipt、精确进程 stop、R01B userdb 与 M2 manager 固定测试数据 receipt 清理入口，以及合成 reference probe 和 unknown/P0 `ValidationHost`。两个数据 profile 复用同一 hardened helper：R01B 只允许四个 SQLite 名称，M2 另允许 manager settings 与原子写临时文件；二者都不接受调用方路径。分类 contract 直接编译 controller 使用的生产源码，并以两个 host 的固定 Bundle ID 覆盖 unknown/P0；host 本体只构建不启动，也不读取或保存输入框内容。正式薄壳已在 Apple Development build 32 完成 R01A；R01B 以同一 Apple Development build 34 完成真实重排、重启、删除/恢复、隐私/unknown/P0/secure 系统路由与零残留退出。曾冻结的 build 33 只保留历史意义。

`platforms/macos-product/UpgradePreflightHost/` 是 M4-P02 双 bundle 之外的只读升级平台端口。生产 executable 不接受路径或其他参数，只解析用户域固定 `Application Support/RadishLex`；Manager/InputMethod bundle ID 从 `packaging/macos/product.json` 编译进入二进制。host 验证私有数据根和受控普通文件，查询卷级可用容量，并以 `NSRunningApplication` 与固定 `lsof` 检查双端进程和 SQLite/settings 打开句柄；只输出固定 JSON 状态，不创建目录、不停止进程、不修改文件。contract 使用合成目录验证容量、打开/关闭句柄、symlink 拒绝和无参数边界，生产 host 不在仓库门禁中对真实数据根执行。

`platforms/macos-product/UpgradeValidationHosts/` 保存双端共用的数据库字节/sidecar 复验与两个受控 main。Manager helper 链接 Manager bundle 自带 native library；InputMethod helper 链接 native Rime 产品库并从自身 bundle 固定读取 `RimeData`。生产入口只接受无参数 candidate 模式或 `--post-switch` 最终路径模式，路径均由 Application Support v1 布局推导，不接受调用方路径或其他 validation input；共享参数与路径 contract 由独立 Objective-C 门禁覆盖。

`platforms/macos-product/UpgradeCoordinatorAdapter/` 是实现 `UpgradeCoordinatorPort` 的 Rust 平台组合层。它只从 source/target 产品根的严格 `ProductManifest.json` 解析固定双 bundle 与 helper，逐次复验 helper 长度和 SHA-256；所有 checkpoint 使用 target Manager preflight，candidate/final 使用 target 双端 validation，回滚恢复使用 source 双端 validation。该 crate 不解释 SQLite 内容、不接收数据路径、不进入输入热路径，也不替代 M4-P03 的 code signature 与安装来源验证。

R01B 实机与回滚遵循 [专用 runbook](runbooks/macos-r01b-personalization-acceptance.md) 的授权 A/B：授权 A 才允许签名、安装、系统设置、人工交互和保留 userdb 的普通清理；授权 B 只在 receipt 归属、设置恢复和数据库关闭条件满足后删除本轮四个固定 SQLite 文件并把预存空父目录恢复为 `0755`，不得删除父目录。该 runbook 现在作为关闭证据与回归边界保留。副屏和 VoiceOver 仍按平台边界文档的已知限制处理，自动 contract 不能替代对应实机证据。`platforms/android-ime/keystore-bridge/` 只是 Android Keystore 算法与 JNI 能力验证工程，不是完整 Android IME。

后续平台目录按进入顺序创建：

1. `platforms/linux-fcitx5/`：macOS 达到退出标准后的桌面候选。
2. `platforms/android-ime/`：在现有 keystore bridge 之外补完整 IME。
3. `platforms/windows-tsf/`。
4. `platforms/ios-keyboard/`。

只有对应平台设计边界和退出标准明确后才创建目录。空目录、占位文件或安装脚本不构成平台进度证据。

## 文档目录

- `docs/status/current.md`：唯一当前阶段短入口。
- `docs/remediation/`：仅在当前状态明确引用活动临时专题时使用；当前无活动专题。
- `docs/archive/`：已关闭且退出默认阅读链的历史专题与 review-only 材料。
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
