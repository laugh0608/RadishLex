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
  -> local settings, dictionary, diagnostics, device and sync management
```

客户端本地数据库和 Rust core 是用户数据真相源。Flutter、平台壳和 Go server 都不能各自复制一套候选排序、删除语义、同步状态或隐私策略。

## 产品交付顺序

架构模块可以提前形成原型和受控测试，但产品里程碑按用户可见纵向链退出：

1. M1 先打通 macOS 离线输入，闭合按键消费、候选、commit、FFI、进程级 engine runtime 和平台壳。
2. M2 再闭合本地个人化，让真实选择、删除和反馈以正确事务语义影响后续候选，并提供本地 manager 管理界面。
3. M3 在本地数据语义稳定后开放端到端加密同步、设备授权、恢复、撤销和 manager 同步界面。
4. M4 最后闭合 native library、`librime`、schema、manager、签名、升级和供应链发布门禁。

同步原型、loopback、短生命周期服务和跨语言测试可以在 M1/M2 期间继续演进，但不能进入真实用户产品入口，也不能替代 M3 的退出证据。2026 年 7 月整改专题只负责修复 M1/M2 前置问题和首批质量门禁，不承担 M3/M4 的长期项目管理。

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

选择、负反馈、删除和显式恢复是用户意图，跨表写入必须具备事务性。数据库需要明确 WAL、busy timeout、并发访问、文件权限、备份恢复和 schema migration 策略。

P1 原始事件只在本地用于学习，不得通过 FFI 管理接口或同步 payload 暴露。P2 导出只允许从明确的压缩摘要与用户可管理数据生成。

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

manager 通过受控 bridge 使用 Rust 能力。M2 先交付本地词库、学习、隐私和诊断，并让正常本地产品运行态携带 native library、使用固定平台目录和真实持久化数据；M3 再交付同步、设备与恢复；M4 闭合发布签名、公证、安装升级和最终产品打包。fixture 只能由显式开发开关启用并持续显示演示标识。manager 不进入输入热路径，也不承担排序、合并或密钥策略真相源。

## 平台策略

### macOS

第一真实平台使用 InputMethodKit。Swift / Objective-C 外壳只负责系统输入法生命周期、按键、候选、commit 和 Rust FFI。候选 UI 是输入法进程内唯一的 nonactivating AppKit panel；controller 维护单一 display index，键盘视觉与 Space 选择读取同一 index，鼠标点击也把目标 index 送入同一 controller/Rust selection 路径，不能让平台显示状态与 Rust engine selection 分叉。

候选锚点必须区分 inline session 内的现存字符索引与文档绝对插入位置：前者用于获取当前全局行矩形，后者只用于公开 fallback。panel 最终限制在目标 `NSScreen.visibleFrame` 内，不以屏幕原点代替无效定位。TIS 状态和测试期间 current source 归属由平台目录中的只读工具记录；输入源选择仍由开发者手动完成，工具不得进入输入热路径或修改系统配置。M2 manager 使用非 App Sandbox 本地分发 profile，与输入法共享已验证的用户 Application Support userdb，并固定文件权限、锁和 schema migration 所有权；M4 若转为 App Group 或其他容器，必须先设计迁移、回滚和双端复验，不能静默复制数据。进程级 runtime、session、按键结果、候选窗、TIS 与验收边界见 [macOS InputMethodKit 平台边界](macos-inputmethodkit-boundary.md)。

### Linux

第二桌面候选优先 Fcitx5，其次 IBus。Wayland 下优先使用输入法框架 panel，不自行发明浮窗协议。

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
- merge 收敛、签名绑定、KDF 上限、平台私钥和 HTTPS 编排未验证前，不开放真实用户同步。
- 第一真实平台未达到可日常输入前，不并行启动第二平台。
- manager 产品模式不得用静默 fixture fallback 代替真实失败。

临时批次的更严格停止线见 `docs/status/current.md` 当前引用的整改专题。

## 专题文档索引

- [当前状态](status/current.md)：当前批次、验证基线、停止线和近期顺位。
- [产品交付路线图](roadmap.md)：产品里程碑、交付物和退出标准。
- [仓库结构](repository-layout.md)：实际目录与模块职责。
- [Engine Boundary](engine-boundary.md)：engine trait 和核心模型。
- [Rime Adapter](engine-rime-adapter.md)：librime adapter、构建与 native smoke。
- [个人化学习](personalization-learning.md)：userdb、ranker、学习和删除语义。
- [FFI Boundary](ffi-boundary.md)：C ABI、所有权、线程和错误语义。
- [macOS InputMethodKit](macos-inputmethodkit-boundary.md)：第一平台的 runtime、按键链、目录和验收边界。
- [隐私与同步](privacy-sync.md)：数据分级、删除、授权和威胁模型。
- [同步 Payload](sync-payload.md)：P2 对象和 payload 边界。
- [加密边界](crypto-boundary.md)：key、envelope、签名与恢复。
- [同步密钥管理](sync-key-management.md)：设备、恢复、撤销和 key epoch。
- [Sync Server API/Storage](sync-server-api-storage.md)：Go API、metadata、blob 和错误语义。
- [Manager Boundary](manager-ui-boundary.md)：manager 职责和数据可见性。
- [平台私钥策略](platform-private-key-backend-strategy.md)：平台 backend 能力与停止线。
