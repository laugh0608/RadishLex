# RadishLex 产品交付路线图

本文档定义 RadishLex 的长期产品里程碑、交付物和退出标准，读者是维护者、实现者和发布审阅者。本文不记录当前批次、逐日流水、提交清单或命令输出；当前状态见 `docs/status/current.md`，临时整改执行见当前状态引用的整改专题，验证流水进入 `docs/devlogs/`。

## 路线原则

- 里程碑以用户可见纵向链为单位，不以 crate、服务或页面目录存在作为完成证据。
- 首个可用版本先证明真实离线输入，再补本地个人化、同步和产品发布闭环。
- 自部署同步保留为 v1 重要能力，但不阻塞首个 macOS 离线 Alpha。
- Manager 的本地管理能力与同步管理能力分开验收；本地词库、学习和隐私管理可以先于真实同步 UI 产品化。
- 第一平台达到可重复日常输入前，不启动第二平台实现。
- 合成 fixture、CLI 输出、设计草案和 local smoke 只能证明对应边界，不替代真实平台、真实 bundle、真实客户端或目标部署证据。
- 个人开发阶段按平台类型维护一条真实主路径证据，不要求为同一平台凑齐所有硬件、虚拟化或 CI 负向矩阵；暂不可得的环境证据必须标记为延期补测，不能伪造，但不得无限冻结后续里程碑开发。

## M0：方向与工程基础

目标：

- 固定项目定位、许可边界、隐私立场和 v1 范围。
- 建立 Rust core、engine adapter、userdb、ranker、crypto/sync、Go server、FFI 和 manager 的工程原型。
- 明确各模块职责、clean-room 原则和验证分层。

交付：

- README、LICENSE、协作入口和正式架构文档。
- Rust workspace、Go sync server 和 Flutter manager 原型。
- 默认仓库验证、开发期 native/FFI smoke 与合成集成测试。

退出标准：

- 架构、隐私、许可、平台策略和模块职责没有互相冲突的稳定口径。
- 工程原型能够支持首个真实平台纵向链开发。
- 文档和测试不把原型能力误称为可安装产品。

M0 只证明方向与工程基础可继续演进，不证明任何产品里程碑已经完成。

## M1：macOS 离线输入 Alpha

第一真实平台固定为 macOS InputMethodKit。本里程碑只要求开发版真实输入链，不要求真实用户同步或完整 manager 产品包。

目标：

- 在真实 macOS 应用输入框中离线完成中文输入。
- 建立平台可消费的 input runtime、版本化 FFI 结果和进程级 librime 生命周期。
- 证明平台薄壳、Rust core 和成熟底层引擎的职责分离成立。

交付：

- 版本化 KeyOutcome ABI，至少返回 `consumed`、可选 `commit` 和最新 snapshot。
- 可供 Swift / Objective-C 使用并受测试约束的 C header 或等价模块边界。
- 进程级 librime setup / initialize / finalize 与多 session 管理。
- `platforms/macos-imk/` 开发版薄壳。
- 安装、启用、移除和非敏感人工 smoke runbook。

退出标准：

- 开发者可以按 runbook 安装并启用开发版输入法。
- 全拼输入、候选导航、数字键或空格选择、提交、取消、重置和未消费按键透传正确。
- 两个 session 不重复初始化或破坏 librime 全局状态。
- 断网时完整输入链可用，输入热路径不进入 manager 或网络。
- 自动契约测试覆盖 KeyOutcome 到平台提交；人工 smoke 在真实应用完成且不含敏感输入。

不阻塞 M1 的事项：

- 真实用户同步、恢复码、设备授权和撤销。
- 完整 manager 产品化。
- 面向普通用户的 librime/schema 最终分发方案、签名和发布安装包；这些属于 M4 交付，必须在 M4 退出前完成。
- VoiceOver 候选导航与 accessibility press 的完整可用性；M1 Alpha 必须如实记录已知限制，不得声明或宣传 VoiceOver 支持。该能力进入受支持范围前必须完成修复和真实辅助功能验收。

## M2：本地个人化 MVP

目标：

- 让真实输入选择安全进入本地学习，并稳定影响后续候选。
- 让用户能够查看、删除、导出、暂停或限制本地学习结果。
- 修正 userdb、ranker、删除和并发访问的正确性基础。

交付：

- engine、ranker、userdb、privacy policy 组合的本地 input runtime。
- 事务化 selection、negative feedback、delete 和 explicit restore。
- SQLite WAL、busy timeout、并发访问、文件权限、迁移和损坏恢复策略。
- 有效 recency 衰减、有界 frequency、明确 suppress/delete/restore 优先级。
- 固定合成排序评测集和输入热路径性能基线。
- 覆盖本地词库、学习、隐私和诊断主要路径的 manager 产品模式。

退出标准：

- 真实候选经 ranker 重排后仍能稳定映射回 engine commit。
- 用户意图要么完整提交、要么完整回滚；并发 manager/IME 访问不频繁产生 `SQLITE_BUSY`。
- secure text entry、P0 App 和隐私模式不会写入学习记录。
- 删除不会被旧事件、旧导入或旧本地状态复活，显式恢复具有新的稳定版本。
- 用户无需 shell 环境变量即可在产品模式管理真实本地数据；fixture 只能通过显式 demo mode 启用。
- 重启后词库、学习设置和排序效果保持，explain 与评测结果一致。

M2 是首个本地个人化 MVP。它不要求远端同步已经开放。

## M3：端到端加密同步 Beta

目标：

- 在不改变本地输入热路径的前提下，让两个真实客户端安全同步 P2 数据。
- 完成确定合并、设备加入、恢复、撤销、key epoch，以及本地 Compose + HTTPS 编排；真实域名、正式证书和目标生产环境演练后移到首个正式版本发布后。
- 保持服务端默认不可信，P1 原始事件继续只在本地。

交付：

- 确定性 merge 与统一冲突表。
- 签名绑定、KDF/解析资源上限、secret 生命周期和跨语言协议 test vector。
- 兼容 `ed25519-v1` / `ecdsa-p256-sha256-v1` 的显式设备签名 profile，以及至少一个创建不可导出私钥并分别通过编译、运行时、产品进程 gated smoke 与资格评审的生产 backend；普通 DPK 软件 key 即使生命周期通过，也不能替代不可导出条件。这些状态不得与用户同步总 gate 混用。
- Apple Secure Enclave P-256 必须使用独立 backend/key identity；repository compiled、runtime、不可导出、hardware-backed、产品资格和用户同步总 gate 分别取证，不得从 token 配置或普通 DPK 证据推导。
- 对象发现、下载、验签、解密、合并、上传、冲突重试和本地 cursor orchestration。
- 设备加入、恢复、撤销和 key epoch 轮换 API。
- Go server 的认证、请求上限、限速、审计、备份恢复和升级回滚闭环。
- 通过真实 Rust bridge 执行的 manager 同步 UI。

退出标准：

- 任意记录顺序得到相同结果，合并满足交换律、结合律和幂等性。
- 两个真实客户端从空状态完成授权、上传、发现、下载、验签、解密、合并和受控重试。
- 旧设备不能复活删除数据，被撤销设备不能获得新 epoch 数据。
- HTTPS、认证、请求/响应上限、慢连接、错误 TLS、限速绕过和日志脱敏通过负向测试。
- manager 不持久化 recovery code、token、wrapped material、signature bytes 或 payload bytes。
- Manager 通过真实 Rust bridge 完成本地 HTTPS 合成 P2 资格执行，覆盖单次运行所有权、并发拒绝、取消、超时、错误脱敏和进程重启；资格入口与普通用户同步入口必须有明确产品区隔。
- 至少一个受支持 macOS 设备上的真实产品 bundle 已完成 signing 与 key-agreement 的独立生命周期、锁定失败关闭、清理和日志脱敏评审；无 Secure Enclave 环境的 `unsupported` 属于发布兼容性补测，不再作为个人开发阶段退出前置条件。

M3 的部署子阶段以短生命周期本地 Compose、Caddy internal TLS、bearer 认证负向响应、权限、备份恢复和日志脱敏通过为退出证据。首个正式版本保持真实用户同步关闭；真实域名、公开 CA 证书、目标生产备份恢复与升级回滚在该版本发布后、准备启用生产同步前单独验收，不能用本地证据冒充。

M3 开发期间，真实用户同步在退出标准全部满足前保持关闭。合成数据、loopback、短生命周期服务和受控集成测试可以实现并验证成功路径，但不能自行解锁产品入口；M3 退出后是否开放仍由后续产品阶段和生产部署证据决定。

## M4：产品发布候选

目标：

- 把 macOS 输入法、manager、Rust native library、librime 和合法 schema/data 形成可安装、可升级、可移除的产品包。
- 统一版本、兼容性诊断、distribution identity、权限和发布门禁。

交付：

- 输入法与 manager 的可重复构建和 bundle presence 检查。
- librime 动态/静态链接、schema/data 来源、许可证、完整性和更新策略。
- App Group、App Support、sandbox entitlement、文件选择和数据库所有权边界。
- 安装、升级、回滚、移除、数据迁移和故障恢复 runbook。
- 外层程序 receipt 与数据 receipt 分层，产品终态分步持久化；Manager/InputMethod 在业务初始化前依次执行外层 install 与数据 upgrade gate。
- InstallPayload 显式绑定可支持的历史 source assembly；production upgrade 只按外层 receipt 的精确 release 选择旧版本 validation/rollback host，缺失或身份漂移在事务写入前失败关闭。
- 产品 metadata 与 ProductManifest 固定版本化 distribution identity；首发 `community-adhoc-v1` 的 strict ad-hoc bundle、未公证 UDIF、SHA-256 evidence、人工放行提示与隔离下载复验绑定同一冻结产物。
- Rust fmt/check/test/clippy/MSRV、Go test/race/vet、Flutter format/analyze/test、native-rime 和 macOS bundle CI。
- 依赖安全、许可证和必要供应链检查。

退出标准：

- 新环境无需手工配置 Homebrew 路径或 shell 环境变量即可安装和使用。
- 输入法与 manager 加载匹配版本的 Rust/native 依赖，升级后用户数据保持。
- FFI、数据库、权限、schema、版本和 bundle 缺失均有明确错误，不静默回退 fixture。
- active/nonterminal/损坏事务、运行 bundle 身份漂移和 completed remove 均在 Flutter/IMK、settings、userdb、Rime 初始化前失败关闭。
- distribution identity、双 bundle/Installer identity、DMG SHA-256 evidence 和用户安装提示精确绑定同一发布候选；社区模式不得宣称 Developer ID、公证或 Gatekeeper 自动通过。
- 发布候选通过自动门禁、安装 smoke 和非敏感日常输入复验。

## 第二平台选择门禁

只有 M2 达到可重复日常输入、稳定本地学习和真实数据管理退出标准后，才评估第二平台。候选优先考虑 Linux Fcitx5 或 Android，依据目标用户、维护成本、平台能力和首个平台证据选择；不得同时展开多条真实平台主线。

Windows TSF 与 iOS Keyboard Extension 后置。任何第二平台都必须复用 Rust core、userdb、ranker、sync 和 privacy 边界，平台壳不得复制业务真相源。

## Future Topic：自研 Rust Engine

只有至少一个真实平台、本地个人化、产品评测和同步边界稳定，并且已有明确替换收益时，才进入自研 engine。

进入条件：

- 已有公开行为规格、输入质量和延迟评测集。
- 成熟底层引擎成为可量化瓶颈，而不是主观偏好。
- 替换不破坏 engine trait、userdb、ranker、FFI、同步和平台壳。
- 实现遵守 clean-room 原则，不复制外部源码、私有结构或受限词库。

## Future Topic：Sync Server Admin Console

这是部署者可选运维界面，不属于 M3 或 M4 的必要交付。进入实现前必须固定管理员认证、scope、脱敏 API 和公网暴露策略。

它只能展示服务健康、migration、存储、备份恢复、升级回滚、认证模式和脱敏 audit 摘要；不得展示明文用户词库、输入历史、候选偏好、token、恢复码、私钥、wrapped material、signature bytes 或 encrypted payload bytes。详细边界见 [Sync Server Admin Console](sync-server-admin-console.md)。

## 里程碑证据规则

- 设计文档证明边界，不能证明产品能力已经实现。
- 单元测试证明局部语义，不能替代跨模块和真实平台证据。
- CLI、fixture 和 local smoke 不得替代系统输入法、真实 bundle 或两个真实客户端。
- 每个里程碑只维护必要退出证据；详细命令和流水写入 devlog 或 runbook。
- 当前里程碑、阻塞项和下一步只在 `docs/status/current.md` 维护。
