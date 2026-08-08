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
    ime-product-install/
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
    linux-fcitx5/
      CMakeLists.txt
      include/
      src/
      config/
      tests/
      README.md
    linux-product/
      Cargo.toml
      README.md
      src/
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
      InstallAdapter/
      InstallCoordinatorAdapter/
      UpgradePreflightHost/
      UpgradeValidationHosts/
        Sources/
      UpgradeCoordinatorAdapter/
    android-ime/
      keystore-bridge/
  packaging/
    linux/
      assets/
      debian/
      product.json
      install-layout.json
    macos/
      product.json
      install-layout.json
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
    linux-product/
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
| Rust input | core、进程级 Rime runtime、产品个人化 runtime、CLI、ABI v9、Manager 产品状态与隔离资格 run、管理查询和共库证据；M4-P02 数据协调、M4-P03 外层 receipt/guard、双程序切换/恢复、两段终态、manifest/code-signature adapter、跨核心协调、Installer driver/executor/bridge 已形成独立边界 | 身份绑定的终态材料清理与真实跨发布兼容证据 |
| 本地学习 | schema v9 userdb、事务化用户意图、本地导入批次关联、确定性 ranker、产品热路径、并发 migration/WAL、同步 cursor/journal/outbox、原子 apply、可信 public lifecycle、wrapped ciphertext 与 recovery lifecycle cache | 明文 master key/shared secret 只短暂进入 Rust snapshot，不进入 SQLite/settings |
| 同步 | P2 crypto/sync、Ed25519/P-256 profile、Go server、Rust HTTP/TLS transport、关闭态 orchestration、通用 processor、生产 provider、设备 lifecycle 验证、wrapped epoch v1、Apple signing/key-agreement 产品资格、双 userdb Go HTTP 收敛、本地 Caddy HTTPS、Manager 受控资格执行链 | 真实用户入口开放评审、首版后的发布级目标部署 |
| Flutter manager | 默认 product/显式 demo、Release FFI bundle、固定平台路径、隐私 method channel、deleted restore、导入批次审计、双端刷新、同步产品 status、本地 HTTPS 合成资格 run、widget/FFI/产品门禁；M4 外层 install gate、数据 gate 与升级 validation helper；M5 Linux runner、固定 `.so`、共享 XDG/privacy source contract、ARM64 Release bundle、同库学习/删除/导入导出与重启实机证据，以及 Flutter 初始化前的 Linux 只读 startup gate | Linux system package 实机；真实用户同步入口与首版后的目标部署证据 |
| 平台 | macOS InputMethodKit 薄壳、contract/native bundle、生产 LearningContext 与 privacy/清理 contract；build 38 双 bundle、locked RimeData、数据/安装 gate、Installer、社区 ad-hoc identity、DMG evidence、首次安装/输入/修复/默认移除实机证据；Linux Fcitx5 C++/CMake addon、ABI/XDG/staged/system runtime-layout/Manager runtime/privacy/classifier contract、Debian 13 ARM64 Wayland/X11 输入及 Manager 同库个人化证据；Linux metadata/rootfs、真实 ARM64 product payload、确定性 `.deb`、actual package relationship、恢复型 fake transaction 与 Manager/Fcitx 共用只读 startup gate；Android Keystore 能力验证桥 | macOS 真实跨发布升级；Linux production fixed-path observer/executor、concrete mutable dpkg port、真实进程静止、privileged host/CLI、隔离 L6 与独立安装实机；完整 Android IME、Windows TSF 与 iOS Keyboard Extension |

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

### ime-product-install

产品程序安装事务 contract：

- `first_install`、`upgrade`、`repair`、`remove_programs` 的显式 source/target 关系；
- ProductManifest、bundle tree 与 canonical code identity evidence 组成的无路径产品身份；
- staged/backup/installed evidence 只追加状态机，以及程序提交后的显式 rollback；
- `.radishlex-install-v1` 私有 receipt、原子替换、root identity 与跨进程 Unix socket guard；
- 终态 operation chain 与当前运行 Manager/InputMethod 身份 startup decision。

该 crate 不依赖 `ime-product-upgrade`，不读取 bundle、验证签名、停止进程、复制/删除程序或接受安装路径。当前已形成外层事务状态、双目标 staging/rename/fsync、精确恢复、只读 current/binding 校验和 startup decision；M4-P02 结果映射由平台组合 crate 承担。API 与验证入口见 [ime-product-install 组件说明](../crates/ime-product-install/README.md)，完整边界见 [macOS 程序安装事务](macos-installation-transaction.md)。

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
- additive request/result v1 Linux product startup ABI；输入 session/key ABI contract 仍为 v9
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

仓库根 `version.json` 是产品版本与 Flutter build number 的唯一人工真相源。`packaging/macos/product.json` format v3 是其 macOS 镜像，并固定最低系统、bundle ID、FFI ABI、userdb schema、manifest 版本、数据布局与 `community-adhoc-v1`；`packaging/macos/install-layout.json` 固定 DMG + 独立用户域 Installer、两个 component-to-target 映射、Application Support、安装事务状态和移除语义。`packaging/rime/product-rime-data.json` 绑定产品 schema、Apache 词典来源 commit/hash、运行时路径和逐资产许可证；`scripts/rime-product/product_data.py` 负责离线校验与装配。

`scripts/macos-product/product_manifest.py` 校验版本镜像与源码声明，生成/复验无绝对路径的 ProductManifest v3；`install_layout.py` 以 committed layout、target 与显式历史 source assembly 生成 InstallPayloadManifest v2；`release_identity.py` 固定双 component strict ad-hoc requirement 集合；`community_release.py` 生成/复验 DMG SHA-256 evidence。四层证据都不保存签名凭据、公证上传或用户数据；具体产品构建见 [macOS 产品装配 Runbook](runbooks/macos-product-assembly.md)，用户安装与发布载体见 [macOS 社区 ad-hoc DMG Runbook](runbooks/macos-release-carrier.md)。

`packaging/linux/` 保存 format v1 Linux product mirror、`debian-system-v1` component layout、Debian control/artifact contract、desktop/icon source 和 `debian-local-deb-v1` identity；不保存生成的 `.deb`、rootfs 构建物、apt repository metadata、签名凭据或用户数据。`scripts/linux-product/product_metadata.py` 负责格式与派生值，`source_contract.py` 交叉验证 version/build、ABI/schema、RimeData lock、dependency/font profile 与源码身份，`rootfs.py` 从显式 Manager/addon 输入离线装配临时 `DESTDIR` 并生成 canonical product manifest；`deb_artifact.py` 生成和重验 canonical ar/USTAR、control、md5sums 与 SHA-256 evidence，`shlibdeps_diagnostics.py` 精确核验私有未版本库和 Debian 13 ARM64 libc6 usrmerge 诊断。Rust/Cargo 与 C++ 产品构建路径映射为稳定 identity，强门禁扫描全部 ELF 的 repo/home/staging 泄漏；真实 Debian 13.6 ARM64 载荷与重复 `.deb` 构建已通过。以上入口不写真实 `/usr`、`/var`、dpkg database 或 XDG，也不把载体生成称为安装完成。完整边界见 [Linux 安装维护边界](linux-installation-maintenance-boundary.md)。

## 平台目录

M5-P02 已以真实实现和 contract 建立 `platforms/linux-fcitx5/`：

```text
platforms/linux-fcitx5/
  CMakeLists.txt
  src/                 Fcitx5 addon、input context/session、FFI、privacy 与 XDG 实现
  include/             Linux addon/Manager 共用 XDG 与投影 header；公共 ABI 仍来自 ime-ffi
  config/              addon/inputmethod metadata 的 committed source
  dev/                 固定 digest 的 Debian 13 ARM64 开发镜像
  evidence/            默认不加载的离线桌面取证夹具
  tests/               key、candidate、lifecycle、privacy、classifier 与 XDG contract
  tools/               staged addon runtime layout 与 native loader 诊断
  README.md            开发构建、非安装 smoke 和边界索引
```

该目录不包含 userdb schema、ranker、Rime 私有候选逻辑、同步状态机或自绘候选 UI。`include/radishlex/linux/xdg_paths.h` 与 `src/xdg_paths.cpp` 是 addon、Flutter Linux host 和诊断的单一 XDG resolver；test injection 只在测试编译态可见。`ffi_projection` 复制 ABI v9 owned result 并守住 owner thread，`fcitx_addon` 只使用 framework input panel、commit 和 lifecycle。应用身份 evidence 仅用于默认关闭的固定候选桌面评审，不能进入默认产品日志或替代生产 allowlist 复核。CMake 默认 `staged` profile 继续服务 P02/P04，`system` profile 则把 Rime shared data 固定到 `/usr/share/radishlex/rime` 并使用产品版本渲染 metadata；两类 profile 均不构成安装成功。

`platforms/linux-product/` 是 M5-P05B 的 Debian package 事务、关系校验与只读 startup decision 层。Rust crate 固定 canonical receipt、operation chain、source/target 私有 staging、root/state identity、regular-file advisory guard、五类 operation 与恢复状态机；每次 mutation/retry 重验 staged relationship，并以 move-only permit 绑定静止证明。state store 处理 receipt/stage 临时文件崩溃窗口、current required-slot 精确集合、原子 mode 与直接父目录 `fsync`；v1 历史 operation 只验证 structure/pair metadata，不作为当前恢复材料。

production-only `VerifiedArtifactRelationship::verify_package` 从 actual `.deb` 同一有界流计算 size/SHA-256，严格解析三成员 ar、canonical uncompressed USTAR、仅 `control`/`md5sums` 的 control 和 actual payload inventory，并把唯一 product manifest 与 evidence、control、canonical md5 inventory 和 actual `Installed-Size` 逐项交叉；同域 pure relationship 另行校验依赖、Debian version 与 dpkg status。typed command 层固定 `/usr/bin/dpkg`、私有 staged argv、清空环境、null stdin、有界诊断、配置与 lifecycle projection，但不执行命令；v1 package 明确无 RadishLex 自有 maintainer scripts，外部 scripts/triggers 不能代替 transaction proof。

startup observer 继续只读 `/var/lib/dpkg/status`、guard/tmp/receipt、terminal staging 与 component identity；Manager/Fcitx 在业务初始化前通过 additive request/result v1 取得 decision，C++ binding 以 `dladdr`/canonical path 拒绝错误 sibling symbol。它尚未连接上述完整 dependency relationship。当前没有 production fixed-path system observer/executor、concrete mutable `DpkgTransactionPort`、真实进程静止、privileged host/CLI 或 L6；普通门禁只用合成 artifact、fake port 与临时目录，不写 `/usr`、`/var`、XDG 或既有 guest。

当前 macOS 机器不直接安装 Linux 工具链；`./scripts/build-linux-fcitx5-container.sh` 在 Docker Desktop 的 Linux VM 中以 Debian 13 ARM64、Fcitx5 Core 5.1.12 和 librime 1.13.1 编译 native-rime FFI 与 `radishlex.so`，仓库只读挂载，Cargo cache/target 使用独立 named volume。该入口还执行 staged addon-relative 装配、ELF `$ORIGIN`/依赖/构建路径门禁和 headless native loader probe；这些结果不是 Wayland/X11、Fcitx daemon 或真实应用输入证据。开发入口与停止线见 [Fcitx5 addon README](../platforms/linux-fcitx5/README.md)。

当前 `platforms/macos-imk/` 已包含 Objective-C InputMethodKit 薄壳、bundle build、不安装系统输入法的 wrapper contract、公开 TIS 只读状态/监视工具、生产 `LearningContext`、privacy CFPreferences receipt、精确进程 stop、R01B userdb 与 M2 manager 固定测试数据 receipt 清理入口，以及合成 reference probe 和 unknown/P0 `ValidationHost`。两个数据 profile 复用同一 hardened helper：R01B 只允许四个 SQLite 名称，M2 另允许 manager settings 与原子写临时文件；二者都不接受调用方路径。分类 contract 直接编译 controller 使用的生产源码，并以两个 host 的固定 Bundle ID 覆盖 unknown/P0；host 本体只构建不启动，也不读取或保存输入框内容。正式薄壳已在 Apple Development build 32 完成 R01A；R01B 以同一 Apple Development build 34 完成真实重排、重启、删除/恢复、隐私/unknown/P0/secure 系统路由与零残留退出。曾冻结的 build 33 只保留历史意义。

`platforms/macos-product/UpgradePreflightHost/` 是 M4-P02 双 bundle 之外的只读升级平台端口。生产 executable 不接受路径或其他参数，只解析用户域固定 `Application Support/RadishLex`；Manager/InputMethod bundle ID 从 `packaging/macos/product.json` 编译进入二进制。host 验证私有数据根和受控普通文件，查询卷级可用容量，并以 `NSRunningApplication` 与固定 `lsof` 检查双端进程和 SQLite/settings 打开句柄；只输出固定 JSON 状态，不创建目录、不停止进程、不修改文件。contract 使用合成目录验证容量、打开/关闭句柄、symlink 拒绝和无参数边界，生产 host 不在仓库门禁中对真实数据根执行。

`platforms/macos-product/UpgradeValidationHosts/` 保存双端共用的数据库字节/sidecar 复验与两个受控 main。Manager helper 链接 Manager bundle 自带 native library；InputMethod helper 链接 native Rime 产品库并从自身 bundle 固定读取 `RimeData`。生产入口只接受无参数 candidate 模式或 `--post-switch` 最终路径模式，路径均由 Application Support v1 布局推导，不接受调用方路径或其他 validation input；共享参数与路径 contract 由独立 Objective-C 门禁覆盖。

`platforms/macos-product/UpgradeCoordinatorAdapter/` 是实现 `UpgradeCoordinatorPort` 的 Rust 平台组合层。它只从 source/target 产品根的严格 `ProductManifest.json` 解析固定双 bundle 与 helper，逐次复验 helper 长度和 SHA-256；所有 checkpoint 使用 target Manager preflight，candidate/final 使用 target 双端 validation，回滚恢复使用 source 双端 validation。仅测试 feature 可把这些真实 helper 重定向到 canonical temp 根下的私有合成 user home，并覆盖成功、端点失败、静止丢失、回滚和重启恢复；生产 runner 清除该覆盖。该 crate 不解释 SQLite 内容、不接收数据路径、不进入输入热路径，也不替代 M4-P03 的 code signature 与安装来源验证。

`platforms/macos-product/InstallAdapter/` 是 M4-P03 的 manifest-bound 程序平台组合层。它内嵌 committed install layout，严格解析 payload v2、target 与全部历史 source ProductManifest，按完整文件与内部 symlink 形成 bundle tree 身份，并把 strict ad-hoc `TeamIdentifier=not set`、CodeDirectory/CDHash、designated requirement 与 sealed requirement 集合转换为脱敏 code identity SHA-256。adapter 只从 authoritative current-user home 形成固定双目标，按外层 receipt release 暴露只读历史 source lookup，使用 metadata-preserving `ditto` 填充核心 staging，递归同步后记录 evidence，并在 source、installed 和 restored 阶段重新复验 tree/code identity。普通门禁只使用合成 verifier/copy port，不访问真实用户目录或签名凭据。

`platforms/macos-product/InstallCoordinatorAdapter/` 是 M4-P03 的跨核心组合层。它不接管两个 receipt 的字段或文件操作，只在同时持有 install/upgrade guard 时验证同一 operation ID、data-root identity、source/target release 和双 `ProgramSwitchStore` binding。M4-P02 每个 quiescence checkpoint 同时消费 `InstallAdapter` 提供的 target 双 bundle 身份结果；数据失败只有在 data terminal、source 双程序精确恢复并重复复验后才把外层推进到 `rolled_back`。9 项合成测试覆盖成功、两类数据失败、静止/身份暂不可得、重启续跑、未持久化双 receipt 状态和绑定拒绝。

`platforms/macos-product/InstallerDriver/` 把外层 receipt/guard 和平台已验证的 installed-product situation 投影为 snapshot v1，不创建状态目录、不解释路径或执行 mutation。它固定 operation、phase、action、稳定 error、receipt progress、manual prompt 与保留数据策略，并在形成 authorized intent 前拒绝 stale action 和缺失确认。`InstallerExecutor/` 在 guard 内重取证并执行可续跑事务，upgrade 还从外层 receipt 与固定 data root bootstrap/rebind 数据 receipt。`InstallerBridge/` 固定 AppKit 使用的整数 enum、POD snapshot 和版本化 native ABI，并从系统用户数据库、当前 executable、sealed community ad-hoc identity 和内嵌 InstallPayload 形成 user-domain bootstrap；四类 operation 都已接真实 platform mutation port，upgrade 在 dispatch 前按 receipt 精确选择并构造历史 source/target coordinator，缺失或漂移 source 在写入前阻断。`InstallerApp/` 只展示 snapshot、固定目标与脱敏摘要，不在 UI 中实现复制、rename、删除、receipt 或 TIS 操作。普通开发构建因缺失 sealed identity 返回 `product_identity_unavailable`。

R01B 实机与回滚遵循 [专用 runbook](runbooks/macos-r01b-personalization-acceptance.md) 的授权 A/B：授权 A 才允许签名、安装、系统设置、人工交互和保留 userdb 的普通清理；授权 B 只在 receipt 归属、设置恢复和数据库关闭条件满足后删除本轮四个固定 SQLite 文件并把预存空父目录恢复为 `0755`，不得删除父目录。该 runbook 现在作为关闭证据与回归边界保留。副屏和 VoiceOver 仍按平台边界文档的已知限制处理，自动 contract 不能替代对应实机证据。`platforms/android-ime/keystore-bridge/` 只是 Android Keystore 算法与 JNI 能力验证工程，不是完整 Android IME。

平台目录按主线顺序创建：

1. `platforms/linux-fcitx5/` 与 `platforms/linux-product/`：前者承载 M5-P02-P04 输入/Manager 主线、system runtime profile 与共用 C++ startup binding；后者承载 actual `.deb` relationship、恢复型 receipt/advisory guard、五类 fake transaction 与只读 startup decision。确定性载体生成仍归属 `packaging/linux/` 与 `scripts/linux-product/`；当前继续 production fixed-path observer/executor、concrete mutable port、进程静止、privileged host/CLI 与 fake crash-command matrix。
2. `platforms/android-ime/`：在现有 keystore bridge 之外补完整 IME。
3. `platforms/windows-tsf/`。
4. `platforms/ios-keyboard/`。

只有对应平台设计边界和退出标准明确后才创建目录；M5-P01 的文档完成不自动创建 Linux 目录。空目录、占位文件或安装脚本不构成平台进度证据。第二平台顺序与发布关系见 [ADR 0009](adr/0009-second-platform-linux-fcitx5.md)，Linux 运行职责见 [Linux Fcitx5 平台边界](linux-fcitx5-boundary.md)。

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
