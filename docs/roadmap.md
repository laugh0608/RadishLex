# RadishLex 阶段路线图

本文档定义 RadishLex 的长期阶段顺序、阶段交付物和退出标准，读者是维护者、实现者和发布审阅者。本文不记录当前批次状态、逐日实现流水、提交清单或验证命令输出；这些内容分别进入 `docs/status/current.md`、临时整改专题和 `docs/devlogs/`。

当前阶段与验证基线见 [当前状态短入口](status/current.md)。2026 年 7 月稳定化整改的跨阶段执行顺序见 [项目稳定化整改总专题](remediation/2026-07-project-stabilization.md)。整改批次可以修正早期阶段缺口，但不因此宣称对应产品阶段已经完成。

## Phase 0：方案与边界冻结

目标：

- 固定项目定位、许可边界、隐私立场和 MVP 范围。
- 明确 v1 不从零重写完整中文输入引擎。
- 明确 Rust core、Go server、Flutter manager 和平台薄壳的职责。
- 固定文档真相源、分支策略和验证分层。

交付：

- 根 README、LICENSE 和协作入口。
- 技术方案、仓库结构、隐私同步和阶段路线文档。

退出标准：

- 架构、隐私、许可、MVP 与平台策略没有互相冲突的入口口径。
- 后续阶段的职责和停止线可以从正式文档复验。

## Phase 1：Rust Core 与 Engine 原型

目标：

- 建立 Rust workspace 和平台无关输入领域模型。
- 定义 composition、candidate、commit、key event 和 engine adapter 边界。
- 接入成熟底层引擎并提供 CLI 复验链路。
- 固定 engine 生命周期、错误语义和 clean-room 边界。

交付：

- `ime-core`
- `ime-engine-rime`
- `ime-cli`
- engine 与 session 契约测试

退出标准：

- CLI 能通过真实 engine 完成 `compose -> candidates -> commit`。
- 核心模型不依赖 Rime 私有对象或任何平台生命周期。
- 多 session、schema 切换、错误释放和 engine 全局生命周期有可复验证据。

## Phase 2：个人化学习

目标：

- 建立本地 userdb、选择事件、负反馈和删除语义。
- 实现候选重排、explain 和可管理用户词库。
- 建立隐私分级、导入导出和本地学习开关。
- 确保用户意图的事务性和排序模型的可评测性。

交付：

- `ime-userdb`
- `ime-ranker`
- SQLite schema 与 migration
- 词库和学习 CLI
- 合成排序评测集

退出标准：

- 真实候选能进入 ranker，排序结果能稳定映射回 engine commit。
- 选择、负反馈、删除和显式恢复要么完整提交、要么完整回滚。
- deleted tombstone 能阻止旧事件、旧导入、旧设备和旧备份复活词条。
- recency 随时间衰减，frequency 有界，suppress 与 delete 优先级明确。
- 用户词库导入导出、学习状态和 explain 均可复验，P1 明细不进入 FFI 或同步 payload。

## Phase 3：自部署加密同步

目标：

- 实现 Go 单用户自部署同步服务。
- 实现 P2 加密对象上传、发现、下载、验签、解密、合并和冲突重试。
- 实现设备加入、恢复、撤销和 key epoch 轮换。
- 保持服务端默认不可信，输入热路径完全不依赖后端。
- 当前生产访问控制先使用受控单用户认证；OIDC 作为独立后续专题。

交付：

- `ime-crypto`
- `ime-sync`
- `server/sync-server`
- Docker Compose 与生产部署 runbook
- 跨语言协议 test vector 和多设备集成测试

退出标准：

- 两个真实客户端能从空状态完成授权、上传、发现、下载、验签、解密、合并和重试。
- 任意记录顺序得到相同结果，合并满足幂等性并覆盖离线并发、删除和恢复。
- 服务端无法读取明文 P2 数据，日志和错误响应不泄漏敏感材料。
- 撤销设备不能获得新 epoch 数据，恢复记录可轮换和撤销。
- HTTPS、认证、请求上限、限速、备份恢复和升级回滚达到生产停止线。

## Phase 4：Manager 产品化

目标：

- 提供词库、学习、隐私、同步、设备和恢复管理界面。
- 通过稳定 bridge 使用 Rust core，不在 Flutter 中复制业务真相源。
- 将真实 FFI、数据库、设置和文件访问打包进正常产品运行态。
- 对不可用能力显示明确原因，不以 fixture 或默认成功伪装。

交付：

- `apps/radishlex-manager`
- Rust manager bridge 与 native bundle
- 本地设置、词库、诊断和同步状态页面
- 平台持久化、文件选择和发布构建验证

退出标准：

- 用户无需 shell 环境变量即可在产品模式管理真实本地数据。
- 应用包携带并加载匹配版本的 Rust FFI，设置和数据库在重启后保持。
- 导入导出使用系统文件访问机制，sandbox 权限与数据目录符合平台要求。
- FFI、权限、数据库和版本失败均明确展示，不静默回退 fixture。
- 真实同步 UI 只有在 Phase 3 退出标准满足后才允许开启。

## Phase 5：第一个真实平台输入法

第一平台固定为 macOS InputMethodKit。Linux Fcitx5 保留为后续桌面候选，但不与第一平台并行展开。

目标：

- 在真实应用输入框中使用 RadishLex。
- 平台壳通过 FFI 获得按键消费结果、commit、composition 和 candidates。
- 用户真实选择进入本地学习，并影响后续候选排序。
- 保持断网可用和平台原生候选交互。

交付：

- `platforms/macos-imk`
- 安装、启用、移除和 smoke runbook
- 平台契约测试与非敏感人工验证记录

退出标准：

- 开发版输入法可按 runbook 安装并完成连续中文输入。
- 按键透传、候选导航、提交、取消、重置和多 session 行为正确。
- 重启后用户词库仍在，学习结果能影响真实候选。
- 输入热路径不发起网络请求，平台壳不承载排序、同步或隐私真相源。

## Phase 6：Android 输入法

目标：

- 使用 Kotlin `InputMethodService` 和 Rust NDK bridge 落地 Android IME。
- 处理生命周期、前后台、键盘 UI、设备密钥和移动端隐私模式。

交付：

- `platforms/android-ime`
- Android native bridge、instrumented tests 和设备矩阵

退出标准：

- 常见 Android 版本与目标设备可稳定输入。
- Rust core 复用成立，Kotlin 不承载用户词库、ranker 或同步真相源。
- 平台密钥能力有真实设备证据，不依赖未经验证的算法假设。

## Phase 7：Windows 输入法

目标：

- 使用 TSF 薄壳接入 Rust core。
- 复用 Windows 原生候选与文本服务生命周期。

交付：

- `platforms/windows-tsf`

退出标准：

- 主流 Windows 版本可稳定注册、输入、升级和移除。
- TSF 壳保持平台适配职责，不复制核心业务状态。

## Phase 8：iOS Keyboard Extension

目标：

- 使用 Swift / UIKit Keyboard Extension 和 Rust XCFramework。
- 默认离线可用，在 full access 边界内提供可选同步。

交付：

- `platforms/ios-keyboard`

退出标准：

- 无 full access 时核心输入可用且不尝试联网。
- 内存、生命周期、App Group 和审核边界有真实设备证据。

## Phase 9：自研 Rust Engine

目标：

- 在产品与评测证明有必要时，逐步替换成熟底层引擎。
- 保持既有 engine trait、用户数据和平台壳兼容。

进入条件：

- 至少一个真实平台、个人化学习和自部署同步已经稳定。
- 已有公开行为规格、评测集和明确的替换收益。
- 不依赖复制外部引擎源码、私有结构或受限词库。

退出标准：

- 自研 engine 在输入质量、延迟、资源占用和兼容性评测上达到既定基线。
- 替换不破坏 userdb、ranker、FFI、同步和平台壳边界。

## Future Topic：Sync Server Admin Console

这是部署者可选运维界面，不属于 Phase 3 或 Phase 4 的退出条件。进入实现前必须先固定管理员认证、scope、脱敏 API 和公网暴露策略。

它只能展示服务健康、migration、存储、备份恢复、升级回滚、认证模式和脱敏 audit 摘要；不得展示明文用户词库、输入历史、候选偏好、token、恢复码、私钥、wrapped material、signature bytes 或 encrypted payload bytes。详细边界见 [Sync Server Admin Console](sync-server-admin-console.md)。
