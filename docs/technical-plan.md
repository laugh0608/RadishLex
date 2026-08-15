# RadishLex 技术方案

本文档是 RadishLex 稳定技术方向的入口，读者是需要判断架构边界、模块职责、平台策略和验证分层的维护者与实现者。本文不记录当前批次状态、完整 trait/DTO 字段、SQLite migration、平台安装步骤、命令流水或历史推进过程；这些内容分别进入 `docs/status/current.md`、对应专题、runbook 和 devlog。

当前阶段与验证基线见 [当前状态短入口](status/current.md)，长期阶段顺序见 [路线图](roadmap.md)。临时整改专题只有在当前状态明确引用时才承担执行跟踪，不替代本文。

## 设计原则

- 本地优先：输入热路径、候选生成、候选重排和学习必须离线可用。
- 隐私优先：服务端默认不可信，不保存明文输入历史、明文用户词库、明文候选偏好或明文上下文。
- 引擎可替换：v1 使用成熟底层引擎，但 Rust core 不依赖其私有对象、内部 ID 或生命周期细节。
- 平台薄壳：平台只处理系统输入法生命周期、按键、候选展示、文本提交和 FFI 调用。
- 可解释学习：用户能查看、删除、导出、暂停或限制输入法学习结果。
- 可删除同步：删除意图通过 tombstone 或等价语义传播，旧设备和旧备份不能静默复活数据。
- 自部署优先：后端只做设备管理、密文对象存储、版本、备份恢复和包分发。
- 失败可诊断：FFI、数据库、同步、加密和平台错误必须明确返回，不使用静默 fallback 掩盖问题。

## 总体架构

```text
Platform IME Shell
  macOS InputMethodKit / Linux Fcitx5 / Android IME / Windows TSF / iOS Extension
        |
        v
ime-ffi
        |
        v
Input Runtime
  ime-runtime + ime-core + engine adapter + ime-ranker + ime-userdb + privacy policy
        |
        +-----------------------------+
        |                             |
        v                             v
ime-engine-rime                 ime-sync + ime-crypto
                                      |
                                      v
                              Go Sync Server
                              encrypted objects only

Flutter Manager
  -> controlled manager bridge / ime-ffi
  -> ime-sync-runtime product composition
  -> local settings, dictionary, diagnostics, device and sync management
```

客户端本地数据库和 Rust core 是用户数据真相源。Flutter、平台壳和 Go server 都不能各自复制一套候选排序、删除语义、同步状态或隐私策略。

## 产品交付顺序

架构模块可以提前形成原型和受控测试，但产品里程碑按用户可见纵向链退出：

1. M1 先打通 macOS 离线输入，闭合按键消费、候选、commit、FFI、进程级 engine runtime 和平台壳。
2. M2 再闭合本地个人化，让真实选择、删除和反馈以正确事务语义影响后续候选，并提供本地 manager 管理界面。
3. M3 在本地数据语义稳定后开放端到端加密同步、设备授权、恢复、撤销和 manager 同步界面。
4. M4 闭合 macOS native library、`librime`、schema、manager、签名、升级和供应链门禁，并冻结可回归参考产品。
5. M5 以 Linux Fcitx5 验证第二平台的原生薄壳、共享 FFI、XDG 数据、个人化和安装维护能力。
6. M5 退出后依次推进 Android、Windows 和 iOS；每次只推进一条真实平台主线。

同步原型、loopback、短生命周期服务和跨语言测试可以继续演进，但不能进入真实用户产品入口，也不能替代 M3 的退出证据。公开发布与各平台验收分离：当前统一发布评审延期到计划内平台分别退出之后，已冻结平台仍必须持续通过共享代码回归。

## Rust 模块职责

### ime-core

`ime-core` 定义平台无关的输入领域模型和稳定 engine 边界：

- input session、composition、candidate、commit 和 schema；
- normalized key event；
- `KeyOutcome`，至少表达按键是否消费和可选即时 commit；
- engine reset、push key、composition、candidates、commit、schema 和状态查询；
- 平台无关错误语义。

`ime-core` 不负责系统输入法注册、平台候选窗、SQLite、网络、云端转换或完整拼音引擎实现。

### ime-engine-rime

`ime-engine-rime` 把 `librime` 的输入、候选、composition、commit 和状态转换为 RadishLex 稳定模型。

- Rime API、session ID、C/C++ 生命周期和数据目录停留在 adapter 内部。
- librime setup、initialize、notification 和 finalize 由进程级 runtime 管理；session drop 不触发 finalize，进程 teardown 在零活动 session 后显式 shutdown。
- 单个输入 session 只管理对应的 librime session。
- engine 原始分数可以作为 ranker 因子，但不是核心真相源。
- bindings 必须有 ABI 版本与布局验证，不能只靠手写结构长期假定兼容。

长期可以增加 Rust 自研 engine，但不得抢占真实平台、个人化学习和安全同步的近期优先级。

### ime-runtime

`ime-runtime` 组合单个 engine session、`ime-ranker`、文件型 `ime-userdb` 和隐私策略，是产品输入热路径的 Rust 真相源。

- 每个输入 session 持有独立 engine session 与独立 SQLite connection；多个输入 session、manager 与诊断入口通过同一个 WAL 数据库文件并发，不共享跨线程 `Connection`。
- runtime 从 engine 取得稳定 `input_code`，一次读取当前候选所需的 user term、ranker weight 和 tombstone，再输出 display index 到 engine index 的显式映射。
- 平台壳只提交 display index 和受控的隐私上下文；候选选择、分段提交、学习判定与 userdb 写入由 runtime 统一决策。
- secure input、敏感应用或无法安全判断的上下文只使用 engine 顺序且不读取、不写入个人化数据；隐私模式允许使用既有本地摘要，但不产生新学习写入。
- userdb/ranker 读取失败时保留 engine 候选顺序并暴露退化状态；选择已经产生的 engine commit 不因学习写入失败而丢失，失败必须作为可诊断学习结果返回。

`ime-runtime` 不负责平台生命周期、SQLite 管理 UI、远端同步、Rime 进程初始化或候选窗绘制。

### ime-userdb

`ime-userdb` 保存本地用户数据和学习摘要：

- `user_terms`
- `selection_events`
- `negative_feedback`
- `deleted_terms`
- `ranker_weights`
- `import_batches`

选择、负反馈、删除和显式恢复是用户意图，跨表写入必须具备事务性。数据库需要明确 WAL、busy timeout、并发访问、文件权限、备份恢复和 schema migration 策略。schema migration 必须按实际起始版本顺序执行且每次取得写事务后重读版本；已完成的旧结构迁移不得在后续版本升级时重放。

`user_terms.import_batch_id` 只记录最近一次实际写入该词条的本地导入批次，和 `import_batches.id` 在同一事务内建立；它是 manager 审计关联，不进入 P2 词条 payload。词条 `source` 仍是稳定的来源枚举，不能用来源显示标签猜测导入批次。

P1 原始事件只在本地用于学习，不得通过 FFI 管理接口或同步 payload 暴露。P2 导出只允许从明确的压缩摘要与用户可管理数据生成。

M4 产品升级把运行时打开与产品迁移分开：`ime-userdb` 提供不配置 WAL、不改权限且不执行 migration 的只读文件 inspection、基于 SQLite backup API 的一致 snapshot，以及只供隔离候选使用的显式 migration/validation summary。snapshot 在只读事务中纳入 WAL 可见内容，输出通过 `quick_check(1)` 的 standalone `DELETE` journal 文件；产品协调层不得用运行时 `UserDb::open` 对原库做升级 preflight。

### ime-product-upgrade

`ime-product-upgrade` 保存产品数据升级的状态机、版本化 receipt、固定对象身份与稳定失败分类：

- 状态只能按 preflight、静止、快照、候选 migration、双端验证、切换和最终验证的证据顺序推进；
- 切换前失败保留原数据，切换后失败只能进入显式 rollback；
- receipt 严格解析且不保存真实路径、数据正文、内容 hash、输入历史或 secret；
- data root 与状态目录在每次加载/持久化前重验对象身份，receipt 以私有临时文件、文件/目录 `fsync` 和原子 rename 落盘；
- 跨进程 Unix socket guard 绑定用户与固定 data root 身份，只恢复精确失活 socket，非 socket 或身份漂移失败关闭；
- snapshot 只读固定 `userdb.sqlite3`，空间预算覆盖三份逻辑工作副本和 64 MiB reserve，并按临时文件、backup、rename、receipt evidence、状态推进顺序提供故障注入；
- 完全只读的 startup gate 必须在 Manager/InputMethod 的 userdb、settings、Rime runtime 和业务初始化之前执行，首次启动与终态允许，其余不确定升级现场失败关闭；
- 双端产品 validation host 无参数只读取固定 migration candidate，`--post-switch` 只读取最终 `userdb.sqlite3`；协调器只在 guard、receipt、目标 identity 和 sidecar 仍一致时消费 validation evidence v1；
- 原子切换固定旧库 backup、candidate 与最终 userdb 路径，先持久化 `switch_prepared`，再按同文件系统双 rename 和目标/源目录 `fsync` 推进 `switched`；中断恢复只接受 receipt 与精确 inode 证明的四类现场；
- 最终双端验证先推进 `post_switch_verified`，独立完成动作再次复验现场后推进 `completed`；验证或完成复验失败进入显式 rollback；
- rollback 先把失败新库移回 candidate，再把旧库原 inode 恢复到最终路径；只有 source-release evidence 与核心 schema/integrity 同时通过才进入 `rolled_back`，不自动删除恢复材料；
- settings、snapshot 和 candidate 的 identity 已持久化但下一状态未落盘时，只复验既有证据并补写状态，不重建对象或猜测无 identity 现场；
- guard-bound 驱动在每个写入、产品 validation 前后与 rollback validation 前后通过平台 port 重新证明静止；核心不定位或启动平台 executable；
- macOS adapter 从 source/target `ProductManifest.json` 固定解析双端 helper，target preflight 负责全部 checkpoint，target/source 双端分别形成升级与回滚 evidence；执行前复验 manifest 长度/hash，版本化 distribution identity 与固定安装来源由 M4-P03 绑定；
- 平台 host 负责固定路径、进程静止和文件系统适配，`ime-userdb` 继续独占 schema 与 migration 语义。

该 crate 不进入输入热路径，不承载安装器 UI、SQLite migration SQL、macOS 进程控制或调用方自定义路径。完整边界见 [macOS 数据升级协调器](macos-data-upgrade-coordinator.md)。

M4-P03 以未公证社区 ad-hoc DMG 内的独立用户域 Installer app 承担程序安装事务；Manager、InputMethod 和 Application Support 分别固定到 current-user home 下的 `Applications`、`Library/Input Methods` 与 `Library/Application Support/RadishLex`。`InstallPayloadManifest.json` format v2 绑定 committed layout、target ProductManifest 与显式历史 source assembly 集合；production upgrade 只从外层 receipt 精确 release 选取旧版本 validation/rollback host。`.radishlex-install-v1` 外层 receipt/guard 负责两处程序切换和数据协调的一致性。非终态程序事务必须进入双端 startup gate，不能让旧程序在数据切换后重新启动。完整决策见 [ADR 0008](adr/0008-macos-installation-carrier.md)。

### ime-product-install

`ime-product-install` 是独立于数据协调器的程序事务核心。它显式区分首次安装、升级、修复和默认程序移除，以 source/target ProductManifest、bundle tree 与 canonical code identity evidence 的 SHA-256 表达逻辑产品身份；receipt 不保存绝对路径、签名输出或用户数据。source、staged、backup 和 installed evidence 只能按 operation 阶段追加，首个程序目标提交后失败必须进入程序回滚。

外层 receipt 固定在 `.radishlex-install-v1`，绑定 data-root identity、operation chain、程序逻辑/文件系统身份与稳定失败分类。两个程序目标各自在同文件系统私有目录执行 source preserve、逐端 rename/fsync 和精确 inode 回滚；Unix socket guard 拒绝同一 root 并发 operation。macOS install adapter 逐字节绑定 committed layout，严格复验 payload/product manifest、完整 bundle tree、strict ad-hoc code identity 与 sealed requirement 集合，使用 metadata-preserving copy 填充 staging，并在 source/installed/restored 阶段重复验证。

独立 macOS install coordinator 同时持有外层与数据 guard，以同一 operation ID、data-root inode 和 source/target release 绑定 M4-P02 receipt。每个数据 quiescence checkpoint 同时复验 installed target；数据失败必须先达到 `aborted_preserved` / `rolled_back`，再恢复并复验 source 双程序，外层才能进入 `rolled_back`。两个核心不互相依赖或解释对方 receipt。只读 startup decision 除阻止 active/non-terminal/损坏现场外，还要求当前运行 Manager/InputMethod 身份匹配 `completed` target 或 `aborted_preserved` / `rolled_back` source。完整字段、状态与停止线见 [macOS 程序安装事务](macos-installation-transaction.md)。

### ime-ranker

`ime-ranker` 只消费 RadishLex candidate 和经过 userdb 整理的摘要，不访问 SQLite、Rime 或平台生命周期。

排序因子可以包含：

- engine 原始顺序或分数；
- 用户词权重；
- 有界 frequency；
- 基于时间的 recency 衰减；
- 上下文类别；
- 负反馈；
- suppressed 与 deleted 惩罚。

每次排序必须能输出 explain。权重调整必须依赖固定合成评测集和指标，不依赖单次主观体验。

### ime-crypto

`ime-crypto` 负责客户端密钥与加密边界：

- sync master key、object key、device wrapping key 和 recovery key；
- AEAD envelope、AAD、nonce、ciphertext hash 和算法版本；
- 设备签名、授权签名、撤销签名和恢复记录签名；
- 恢复码 KDF 与资源上下限；
- 平台密钥 backend 抽象和能力元数据；
- secret 生命周期、内存清理和错误脱敏。

协议必须支持算法演进。平台原生 P-256、平台封装的 Ed25519 seed 或其他方案必须使用不同 backend/algorithm ID，并明确各自的不可导出与硬件保护语义。

### ime-sync

`ime-sync` 负责客户端同步协议与编排：

- P2 payload 类型和加密对象外壳；
- 对象版本、base version、cursor 和冲突错误；
- 设备加入、授权、恢复、撤销和 key epoch；
- 下载后的签名验证、解密、合并和本地写回；
- 上传、发现、重试、退避和 `sync_once` 或等价 orchestration；
- transport trait 与生产 HTTPS 实现。

同步 merge 必须包含本地当前状态，并定义与输入顺序无关的稳定版本顺序。至少使用 key epoch、逻辑时钟或对象版本、device ID 和确定性 tie-break；测试必须覆盖交换律、结合律和幂等性。

### ime-sync-runtime

`ime-sync-runtime` 是 Manager 同步执行的产品组合层，依赖 `ime-sync`、`ime-userdb` 与 `ime-crypto` 的稳定公开边界：

- 组合 HTTPS transport、同步 orchestration、crypto provider 和文件型 userdb；
- 管理 Rust-owned run、worker、取消、超时和临时资源生命周期；
- 为受控资格执行生成隔离合成身份与 P2 数据，不能接受调用方提供 payload、userdb 路径、device/domain/key id 或 key material；
- 只向 FFI 返回固定 phase/outcome/error、受限计数与清理结果，不返回 HTTP body、路径、身份、payload 或 secret。

该 crate 不进入输入热路径，不承载 C ABI、Flutter 状态或 Go server DTO。`ime-sync` 不反向依赖 `ime-userdb`，`ime-ffi` 也不直接建立网络和数据库组合；真实用户同步开放前，资格 provider 必须与生产 backend 明确区分并保持 `user_sync_enabled=false`。

资格 request、phase/error、并发取消和清理 contract 见 [ime-sync-runtime 组件说明](../crates/ime-sync-runtime/README.md)。

### ime-ffi

`ime-ffi` 是 Rust core 与平台/Flutter 的稳定边界：

- C ABI 版本与 capability 查询；
- opaque handle、所有权、生命周期和释放函数；
- UTF-8 编码、结构化错误和 panic boundary；
- owner-thread / worker-thread 规则；
- key result、snapshot、candidate、commit 和 manager data view；
- 调用方复制 borrowed view 的明确时机。

公开裸指针接口必须有一致的 `unsafe` 契约和 `# Safety` 文档。FFI 不得丢失平台做正确决策所需的 `consumed`、commit 或错误信息。

### ime-cli

`ime-cli` 提供可重复的领域与集成验证入口，包括真实 engine 输入、词库、学习、rank explain、同步预检和开发期 smoke。CLI 可以暴露调试信息，但不能成为平台热路径或生产同步的隐藏依赖。

## 输入纵向链

真实平台输入链固定为：

```text
system key event
  -> platform normalization
  -> FFI key call
  -> input runtime privacy check
  -> engine push key
  -> composition / engine candidates / optional commit
  -> ime-runtime privacy decision
  -> userdb summary + ranker
  -> versioned key result and snapshot
  -> native candidate UI or text commit
  -> privacy-aware selection / feedback transaction
```

关键约束：

- 平台必须能判断按键是否被消费，未消费按键交还宿主应用。
- engine 产生的即时 commit 不能在 FFI 层丢失。
- candidate display index、ranked index 和 engine selection index 必须有稳定映射。
- 候选选择结果必须同时表达 consumed、optional commit 和选择后的 snapshot；分段拼音候选可能只确定当前音节并继续 composition，平台不得假定每次候选选择都会立即提交文本。
- secure text entry、P0 App 或隐私模式必须在记录学习事件前阻断。
- display index 必须由 runtime 映射回当次快照中的 engine index，平台端不得假定重排后索引等于 engine 原始索引。
- 选择先捕获 input code、候选身份、原始 engine index 和候选数；立即 commit 时写入一次 selection，分段选择则只保留待确认意图，并在后续匹配 commit 时写入。reset、schema/client/privacy context 变化、取消或不匹配 commit 必须丢弃待确认意图。
- manager、后端和网络不可进入每次按键链路。

## Go Sync Server

Go server 负责：

- 单用户部署配置与认证；
- domain、device、公钥、join request、authorization、revocation 和 recovery metadata；
- encrypted object metadata、版本冲突和 blob 存储；
- 备份恢复、审计、迁移和包分发；
- 请求大小、速率、超时、路径和日志边界。

Go server 不做候选排序、云端实时转换、明文合并或客户端密钥恢复。生产部署必须使用外部或内置 TLS、非空认证、受控代理信任、持久化备份和 graceful shutdown。

## Flutter Manager

Flutter manager 负责：

- 本地词库和学习摘要管理；
- 隐私模式、学习开关和 schema 设置；
- 同步状态、设备、恢复和后端连接；
- 安全诊断、导入导出和备份恢复入口。

manager 通过受控 bridge 使用 Rust 能力。M2 先交付本地词库、学习、隐私和诊断，并让正常本地产品运行态携带 native library、使用固定平台目录和真实持久化数据；M3 再交付同步、设备与恢复；M4 闭合 macOS 版本化 distribution identity、安装升级、发布载体和产品打包；M5 增加 Linux Flutter host，并与 Fcitx5 addon 共用 XDG resolver、native library 和同一 userdb。macOS 社区 ad-hoc 继续作为冻结参考 identity，未来 Developer ID/公证必须独立治理。fixture 只能由显式开发开关启用并持续显示演示标识。manager 不进入输入热路径，也不承担排序、合并或密钥策略真相源。

## 平台策略

### macOS

第一真实平台使用 InputMethodKit。Swift / Objective-C 外壳只负责系统输入法生命周期、按键、候选、commit 和 Rust FFI。候选 UI 是输入法进程内唯一的 nonactivating AppKit panel；controller 维护单一 display index，键盘视觉与 Space 选择读取同一 index，鼠标点击也把目标 index 送入同一 controller/Rust selection 路径，不能让平台显示状态与 Rust engine selection 分叉。

候选锚点必须区分 inline session 内的现存字符索引与文档绝对插入位置：前者用于获取当前全局行矩形，后者只用于公开 fallback。panel 最终限制在目标 `NSScreen.visibleFrame` 内，不以屏幕原点代替无效定位。TIS 状态和测试期间 current source 归属由平台目录中的只读工具记录；输入源选择仍由开发者手动完成，工具不得进入输入热路径或修改系统配置。M2 manager 使用非 App Sandbox 本地分发 profile，与输入法共享已验证的用户 Application Support userdb，并固定文件权限、锁和 schema migration 所有权。M4 首个候选继续使用该 Application Support v1；未来只有 App Sandbox、Mac App Store 或其他明确产品要求成立时才通过 ADR 进入 App Group，并先完成迁移、回滚和双端复验。进程级 runtime、session、按键结果、候选窗、TIS 与验收边界见 [macOS InputMethodKit 平台边界](macos-inputmethodkit-boundary.md)，产品装配与发布边界见 [macOS 产品包边界](macos-product-package-boundary.md)。

首个产品 RimeData 使用 committed 来源锁离线装配：RadishLex 维护全拼 schema 与产品配置，词典固定到 Apache-2.0 `rime-pinyin-simp` 完整 commit/hash，并携带逐资产 LICENSE/AUTHORS。upstream `stroke` reverse lookup、`prelude` preset 与其 LGPL 依赖不进入当前候选；未来扩展必须重新完成行为规格、来源、许可证和真实候选验证。

### Linux

第二真实平台已通过 [ADR 0009](adr/0009-second-platform-linux-fcitx5.md) 固定为 Fcitx5。原生 C++/CMake addon 只处理 input context、按键规范化、候选面板、commit、session 生命周期和 Rust FFI；它不直接调用 Rime 私有 API，不读取 SQLite，也不实现 ranker、学习、隐私或同步。

Linux 复用 ABI v9 的 owner-thread personalized Rime session、owned `KeyResult`、display-index selection 与 `LearningContext`。每个活动 input context 使用独立 Rust session；Fcitx5 input panel 消费同次 snapshot，Wayland 与 X11 都不自行发明浮窗协议。addon、Flutter Manager、诊断和未来安装协调层通过单一 XDG resolver 取得数据、配置、持久状态和缓存路径，不能分别拼接 `$HOME` 或复制数据库。Linux privacy 使用独立、严格、原子替换的平台配置真相源，不让输入热路径解析 Manager settings；程序身份只在平台层按经实机评审的固定 allowlist 映射为粗分类，原始值不跨 FFI、不持久化，unknown 继续失败关闭。

M5 先形成 addon/FFI/build contract，再进入真实 Wayland、X11、常见应用、secure/unknown 上下文和同库个人化验收，最后治理 Linux 安装、升级、修复、移除和数据保留。系统 package 事务由独立 `platforms/linux-product/` 承担：它以 root state identity、canonical append-only receipt、regular-file advisory guard、私有 source/target staging 和 `DpkgTransactionPort` 隔离 Debian package 状态；不复用 macOS 双 bundle rename，不读取用户 XDG，也不进入输入热路径。

P05B 已把 actual `.deb` 验证固定为单次有界流：同链计算 package size/SHA-256，严格解析精确三成员 ar、canonical uncompressed USTAR、仅 `control`/`md5sums` 的 control、actual payload inventory 与唯一 manifest，并交叉 evidence、canonical md5 inventory 与 actual `Installed-Size`；同域 pure relationship 另行验证依赖、Debian version 和 dpkg status。事务在每次 mutation/retry 前重验 staged relationship并消费 move-only quiescence permit；store 对 receipt/stage tmp、单侧 artifact crash window、current required slots、mode 提交与直接父目录 `fsync` 失败关闭或在 guard 下精确恢复。旧 operation v1 只保存 structure/pair metadata，不作为恢复 proof。

production Debian 层已固定 `/usr/bin/dpkg` identity、私有 staged argv、清空环境、null stdin、超时、有界诊断、配置与 lifecycle projection；system observer/port 复验 actual package、依赖/版本、root identity、`/proc/*/maps` 静止和完整安装结果。配置验证只规范化 Debian 13 默认 `no-debsig` 与固定 `/var/log/dpkg.log` 的空白/等号写法，其他日志路径、未知项及 root/admindir/force/hook/path override 失败关闭。guard 父目录只接受非 world-writable 或 root-owned 精确 `01777` sticky mode，预创建非 root guard 仍失败关闭。opaque maintenance command 只有在双显式授权、精确 operation ID 与 root-owned package/evidence 输入成立时才可进入 executor。v1 package 不携带 RadishLex 自有 maintainer scripts；外部 scripts/triggers 不构成 transaction completion。共用 startup observer 已把 terminal actual package/dependency relationship 与 component scope 串联；Manager/Fcitx 在业务初始化前通过 additive request/result v1 消费 decision，输入 session/key ABI 仍为 v9。第三套pair完成source install/S1/S2，第四套暴露source chain不连续，第五套为`rolled_back`，第六套形成target `completed`与S3。`b891ed1`实机调用暴露healthy repair在dpkg前提前成功；`698fe1f`现仅允许非repair或已有target proof的恢复采用valid-target快捷路径，首次repair即使产品健康也继续消费permit并apply一次，118项测试覆盖两侧及五类operation。maintenance-refresh v1的required ancestor已同步前移到该提交；`823afca` ARM64 handoff已从未改写target package/evidence形成production-only ELF并通过guest/host verifier，旧handoff只作历史取证。全新S3 clone `394217A7…B6FB`在`Network=[]`、loopback-only、production-only input和严格mutation preflight后，只调用一次production repair；receipt为`repair/same_release/completed`、chain 3，dpkg log明确记录同版`38-2`重装，完整payload与XDG fingerprint保持不变，startup正负向与terminal postflight通过，该clone不得再次调用或复用。首台rollback clone `9FE6265D…AE28`的完整只读preflight因必需result缺失失败关闭，VM后按明确授权删除而host manifest `6b7ac641…a51d`保留。第二台独立clone `BFB3EF09…2720`的运行态断网、target package/receipt与进程静止通过，但私有传输根准备要求目录stat精确为`700|root|root|1`并返回`root-identity`；正式preflight尚未传入或执行，host manifest `40dab9b…680e`已冻结。目录link count为1是高概率控制缺陷，guest实际值未冻结，不能写成产品漂移或确定根因。后续probe必须把目录mode/owner/group/non-symlink身份与普通输入文件`0600`/root/single-link/size/hash分开；正式检查仍以`O_EXCL`写attempt marker，逐阶段记录并原子写terminal result，宿主回拉验真。S3未漂移且十四台VM全停；下一次只另行授权从S3建立第三台全新rollback clone，八个crash checkpoint继续逐项授权。IBus仅在Fcitx5退出后有明确需求时评估。完整边界见 [Linux Fcitx5 平台边界](linux-fcitx5-boundary.md)、[Linux Manager 本地验收边界](linux-manager-local-acceptance.md)、[Linux 安装维护边界](linux-installation-maintenance-boundary.md) 与 [L6 runbook](runbooks/linux-l6-package-matrix.md)。

### Android

使用 Kotlin `InputMethodService` 和 Rust NDK `.so`。键盘 UI v1 使用平台原生实现；Keystore 算法和硬件能力必须用真实设备验证。

### Windows

使用 TSF 薄壳，候选窗优先原生机制，后置于首个平台稳定化。

### iOS

使用 Swift / UIKit Keyboard Extension 和 Rust XCFramework。无 full access 时必须离线可用；App Group、内存限制和审核边界需要真实设备验证。

## 隐私与同步方向

数据分级以 [隐私与同步设计](privacy-sync.md) 为准：

- P0 永不学习、永不同步；
- P1 默认只保留本地原始事件；
- P2 只能作为端到端加密对象同步；
- P3 可以公开下载。

服务端只可见设备、对象、版本、密文大小、时间和必要协议 metadata。任何 debug、fixture、日志、截图或诊断都不得包含真实明文输入、联系人、密码、证件或支付信息。

## Clean-room 原则

允许：

- 阅读公开文档和观察公开行为；
- 编写行为规格、接口约束、黑盒测试和自己的数据结构；
- 使用许可证兼容的底层库作为可替换 adapter。

禁止：

- 复制外部源码、私有函数结构或受版权限制的词库；
- 把 GPL/LGPL 实现搬入核心层；
- 逐行翻译外部实现；
- 用本地兄弟项目路径或私有材料作为正式文档依赖。

## 验证分层

- 核心类型与纯算法：单元测试、property test 和固定 test vector。
- userdb、ranker、sync、crypto：事务/故障注入、跨设备、篡改、排序评测和性能基线。
- FFI：ABI contract、host binding smoke、线程、释放、panic 和无效指针边界。
- Go server：storage conformance、race、HTTP/API、备份恢复、升级回滚和部署 smoke。
- Flutter manager：format、analyze、unit/widget test、真实 FFI smoke 和产品 bundle 检查。
- 平台壳：对应平台 build、自动契约测试与非敏感人工输入 smoke。

默认先运行精确验证；跨边界、阶段交付和发布前运行仓库级门禁。快速或合成验证不能替代真实平台、真实 bundle 或真实设备证据。

## 主要风险

- 输入链风险：FFI 或平台壳若丢失 consumed/commit，会直接破坏宿主按键和文本提交。
- 输入质量风险：没有固定评测时，frequency、recency 和负反馈容易形成不可解释漂移。
- 数据一致性风险：userdb 非事务写入和非确定 merge 会破坏删除、恢复和多设备收敛。
- 密钥风险：平台算法能力、签名绑定、KDF 上限和 secret 生命周期必须在开放同步前验证。
- 平台风险：系统输入法安装、候选 UI、沙盒和生命周期复杂，必须一次只推进一个真实平台。
- 文档风险：状态流水进入稳定文档会制造错误真相源；当前事实只放状态入口和 devlog。

## 本地个人化 MVP 成功标准

M2 退出至少同时满足：

- 真实 engine 输出候选，ranker 能解释并重排。
- 至少一个平台能离线完成日常中文输入。
- 用户选择、负反馈、删除和恢复正确影响后续候选。
- 用户可管理、导入、导出和停止学习，删除不会被旧状态复活。
- Rust core、FFI、userdb、ranker、manager 本地能力和首个平台都有可重复门禁。

M2 不以远端同步、设备授权或最终发布包为退出条件。

## v1 成功标准

在 M2 基础上，v1 还必须满足：

- 两个真实客户端能安全同步 P2 密文，服务端无法读取明文。
- manager 产品包使用真实 FFI、持久化配置和平台文件访问。
- 输入法、manager、native library、`librime`、schema、签名和升级形成可重复产品包。
- 核心、FFI、Go、Flutter、首个平台和发布供应链都有可重复门禁。

## 稳定停止线

- `KeyOutcome`、FFI 生命周期和 librime 全局生命周期未闭合前，不把平台壳视为可用输入法。
- userdb 事务、ranker 评测和删除语义未稳定前，不开放生产同步。
- merge 收敛、签名绑定、KDF 上限、macOS 平台私钥主路径、本地 HTTPS 编排和 Manager 受控资格执行链已有验证；真实用户入口仍须经过独立产品决策与发布级目标部署评审。该评审完成前保持关闭，上述任一既有证据回归时同样失败关闭。
- 每次只推进一条真实平台主线；M5 期间不并行实现 Android、Windows 或 iOS 平台壳。
- Linux addon 不复制共享业务真相源，不自绘候选浮窗，不以合成 host 或编译通过替代真实桌面证据。
- manager 产品模式不得用静默 fixture fallback 代替真实失败。

临时批次的更严格停止线见 `docs/status/current.md` 当前引用的整改专题。

## 专题文档索引

- [当前状态](status/current.md)：当前批次、验证基线、停止线和近期顺位。
- [产品交付路线图](roadmap.md)：产品里程碑、交付物和退出标准。
- [仓库结构](repository-layout.md)：实际目录与模块职责。
- [第二平台 Linux Fcitx5 ADR](adr/0009-second-platform-linux-fcitx5.md)：第二平台选择、进入顺序和发布关系。
- [Linux Fcitx5 平台边界](linux-fcitx5-boundary.md)：addon、FFI、XDG、隐私、构建和验收边界。
- [Engine Boundary](engine-boundary.md)：engine trait 和核心模型。
- [Rime Adapter](engine-rime-adapter.md)：librime adapter、构建与 native smoke。
- [个人化学习](personalization-learning.md)：userdb、ranker、学习和删除语义。
- [FFI Boundary](ffi-boundary.md)：C ABI、所有权、线程和错误语义。
- [macOS InputMethodKit](macos-inputmethodkit-boundary.md)：第一平台的 runtime、按键链、目录和验收边界。
- [隐私与同步](privacy-sync.md)：数据分级、删除、授权和威胁模型。
- [同步 Payload](sync-payload.md)：P2 对象和 payload 边界。
- [同步编排](sync-orchestration.md)：Rust 状态机、discovery cursor、transaction、outbox 与失败恢复。
- [加密边界](crypto-boundary.md)：key、envelope、签名与恢复。
- [同步密钥管理](sync-key-management.md)：设备、恢复、撤销和 key epoch。
- [Sync Server API/Storage](sync-server-api-storage.md)：Go API、metadata、blob 和错误语义。
- [Manager Boundary](manager-ui-boundary.md)：manager 职责和数据可见性。
- [平台私钥策略](platform-private-key-backend-strategy.md)：平台 backend 能力与停止线。
