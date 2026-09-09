# 产品审阅与改进跟踪：2026-09

本文面向维护者与后续实现者，保存 2026-09-05 综合审阅的证据、风险、建议和关闭条件。源码基线为 `a5345b8`；审阅是文档与关键路径抽查，不是全面安全审计、真实系统验收或发布认证。当前执行顺位与现场停止线仍只读 [current](../status/current.md)。

初次登记只获文档整理授权。2026-09-08 项目所有者先批准隔离诊断与版本核验、REV-01 单一学习存储方案及 REV-02 Rust SQLite 依赖升级与回归，随后批准 macOS 候选准备和本机旧记录的精确保留式归档。2026-09-09 另行批准并完成 build 39 首次安装及固定路径 Manager 启动，随后由项目所有者手动添加/选择输入源，完成普通选词与同会话学习用例；隐私等其余输入矩阵和发布仍未完成。本专题不改写既有里程碑退出记录，不把历史证据扩展为未测能力。关闭事项必须关联修复提交、实际验证及剩余限制；全部处理后按文档规则退出日常阅读链。

## 总体判断

保留本地优先、成熟 engine adapter、Rust 业务真相源、原生平台薄壳和密文同步的主体方向。近期应优先验证“不学习、不丢输入、删除不复活”等核心承诺，补输入质量与性能证据，再收敛验收工具和日常管理体验。平台数量、代码行数和阶段退出记录不能单独代表用户可用性。

下表的优先级是审阅建议，尚不替代现有 M5 系统操作顺位。本文使用“首要、次要、后续”，避免与 P0–P3 数据分级混淆。

| 编号 | 建议优先级 | 事项 | 当前证据性质 | 状态 |
| --- | --- | --- | --- | --- |
| REV-01 | 首要 | Rime 自有学习与隐私控制 | 源码与隔离 native 存储/重启对照 | 仓库修复与隔离 native 回归通过，真实平台复验和质量评测仍开放 |
| REV-02 | 首要 | SQLite WAL-reset 修复版本 | bundled 升级、WAL/旧库回归与新 FFI 身份 | Rust 3.51.3；macOS build 39 已准备，平台实测/Linux 候选/工具链仍开放 |
| REV-03 | 次要 | 学习写入阻塞输入回调 | 同步调用链与 5 秒 busy timeout | 待竞争测试与延迟预算 |
| REV-04 | 次要 | 新词召回与个人化实际收益 | 当前候选重排实现及合成评测 | 待行为规格与评测设计 |
| REV-05 | 次要 | 删词、事件保留与用户预期 | 删除事务和 Manager 文案 | 待保留策略与交互方案 |
| REV-06 | 次要 | MSRV、依赖与 CI 覆盖 | manifest、依赖元数据与 workflow | 待工具链和门禁方案 |
| REV-07 | 后续 | ranker 对持久化模块的依赖 | Cargo 依赖树 | 待相关改动时评估窄类型边界 |
| REV-08 | 后续 | Linux 验收环境维护成本 | 受跟踪文件统计与近期提交主题 | 待工具复用与环境投入评估 |
| REV-09 | 后续 | Manager 日常操作与反馈 | 页面源码抽查 | 待可用性方案与验收 |
| REV-10 | 后续 | 发布反馈节点与许可说明 | 路线图、LICENSE、产品定位 | 待产品决策，不改变发布停止线 |

## REV-01：Rime 自有学习与隐私控制

诊断时（修复前）的源码事实：

- [产品 schema](../../packaging/rime/data/radishlex_pinyin.schema.yaml) 使用 `script_translator`，并设置 `enable_user_dict: true`。
- [runtime](../../crates/ime-runtime/src/session.rs) 的 `LearningContext` 控制 RadishLex SQLite 摘要读取、选择记录和学习结果；引擎处理按键与候选提交仍独立发生。
- 在 [Rime session](../../crates/ime-engine-rime/src/session.rs) 与 [进程 runtime](../../crates/ime-engine-rime/src/runtime.rs) 中未发现把该策略传递到底层学习控制的实现；声明 `set_option` ABI 本身不是接线证据。
- [native 测试](../../crates/ime-ffi/tests/native_rime_behavior.rs) 的隐私选择断言检查学习 disposition 和 RadishLex `selection_events`，未由这些断言证明 Rime 自有学习存储不变。
- [librime 1.13.1 Memory](https://github.com/rime/librime/blob/1.13.1/src/rime/gear/memory.cc) 监听 commit 并调用学习；[ScriptTranslator](https://github.com/rime/librime/blob/1.13.1/src/rime/gear/script_translator.cc) 实现用户词典更新。上游源码用于核对行为，不复制实现。

判断：对仍经过引擎的隐私模式输入，存在未被统一策略覆盖的底层学习路径。SQLite 零增量不能推导整个输入法零学习，engine-only 顺序也不能自动解释为没有既有个人化影响。本次未操作真实输入法、未检查用户数据，也未实测证明泄露或落盘。

建议先以新的隔离目录、产品 schema 和合成输入验证普通、隐私、unknown/sensitive、切换和重启路径；同时检查 RadishLex 数据、Rime 用户词典、日志和候选行为。区分初始化元数据变化与包含输入内容的学习变化。不得复用冻结的 P04/R01B 现场。

当时待决策：由 RadishLex 独占学习，或将 Rime 学习完整纳入隐私、删除、恢复与 explain 合同。不能只假设某个 option 能同时关闭读取、写入和日志。

关闭条件：批准的方案覆盖所有输入衍生存储，native 正负向验证与重启观察通过，相关平台回归完成，历史证据仍按原观察范围保留。

### 2026-09-08：修复前隔离复现与方案讨论

从 `1496901` 的产品实现开始，新增 [native 诊断测试](../../crates/ime-ffi/tests/native_rime_privacy_probe.rs)，使用本机 librime 1.17.0、锁定产品 schema 和全新合成目录。执行方法、48 个独立进程与观察限制见 [隔离诊断 runbook](../runbooks/rime-privacy-probe.md)。结果如下：

| 场景 | 提交后 RadishLex selection_events | 新进程后 Rime 导出词条 | 结论 |
| --- | --- | --- | --- |
| normal | 1 | 1 | 两套存储均学习，正向对照成立 |
| privacy / unknown / secure / sensitive | 各 0 | 各 1 | FFI 返回 skipped_by_policy，但底层仍持久化合成选择 |
| normal→privacy / normal→unknown | 各 0 | 各 1 | 提交前收紧策略不能阻止 Rime 学习 |
| privacy→normal / unknown→normal | 各 1 | 各 1 | 提交时的宽松策略允许此前受限 composition 进入 RadishLex 学习 |
| 临时关闭 Rime user_dict：normal | 1 | 0，未创建 Rime userdb | RadishLex 普通学习仍可执行 |
| 临时关闭 Rime user_dict：privacy / unknown | 各 0 | 0，未创建 Rime userdb | 新库 A/B 中未观察到两套词条学习 |

各场景 baseline 和输入后 cancel 均无导出词条、无 SQLite selection_events。产品 schema 九项场景重开后，受策略阻断 RadishLex ranker 的候选仍由原首候选变为刚选过的第二候选；这把持久化学习与 Rime 读取影响关联起来。指定日志目录未匹配合成选择的 UTF-8 文本，只能保留这一窄观察结论。未操作系统输入法，secure 测试不代表真实系统密码已泄露。

建议下一实现批采用 **RadishLex 独占学习，Rime 提供基础候选**，并为 composition 保留“曾经过禁学上下文”的状态：在 privacy/unknown/secure/sensitive 中产生或继续处理的 composition，恢复普通模式后也不得学习；仍允许完成输入，状态只在对应 composition 完成或 reset 后释放。实现应同时检查按键自动 commit、候选选择、分段选择、schema/context 切换与失败路径，不能只补一个选择入口。

该方案会改变候选质量与数据读取合同，当时尚未批准：停用 Rime 用户词典意味着不再依赖它的历史自学词和新词召回；RadishLex 现有重排不会自动补足缺失候选。既有 Rime 用户数据默认保留，不自动清除、迁移或宣称已物理擦除。实现前需明确产品配置强制性、旧部署配置/缓存的处理，以及混合新旧版本时的兼容边界；临时关闭一个 YAML flag 不足以交付产品修复。新词召回实现如需扩展，仍按 REV-04 单独批准。

备选方案是把 Rime 学习完整纳入统一策略，并补读取、写入、删除、防复活、恢复和 explain 的跨存储合同。它可争取保留 Rime 自学收益，但涉及更大的双存储一致性与 adapter 边界，当前不建议仅靠某个 incognito option 局部修补。

批准后的验证须将当前诊断观察改为明确的隐私正负向断言，补既有词典读取与配置漂移、上下文双向切换、多 session、重启、分段/自动 commit、删除恢复、普通输入质量对照，并运行 Rust/native/全仓及适用平台回归。以上为修复前记录；当时 REV-01 开放，生产实现与产品 schema 尚未修改。

### 2026-09-08：批准后的仓库修复与隔离回归

项目所有者批准提交诊断后继续实现推荐方案，实现提交为 `40cd1bd`。实现为 **RadishLex 独占学习，Rime 基础候选**，没有增加依赖或 FFI ABI：

- 产品 user_dict 明确关闭，adapter 在 native session 创建前检查实际 default 与全部可选 schema；拒绝启学、缺失关闭项、namespaced/custom translator 等未审阅路径。公开 config handles 保留到显式 shutdown，覆盖同进程多 session、切 schema、零 session 间隙与文件替换。部署仅运行 installation/workspace 更新，不触发用户词典升级或 trash 清理。
- runtime 的 composition 策略只能收紧，受限输入返回普通后仍禁学；曾 engine-only 的输入也保持禁读摘要。分段、自动 commit 余串、同 schema、失败重试与直接 engine 修改不提前释放限制，确认 composition/input code 都为空后恢复下一次普通学习。
- 旧 Rime 学习文件保留，未迁移或物理擦除。基础词典候选仍可重排，旧自学词和新词召回收益已停用。普通 native smoke 覆盖分段、翻页、同库学习与删除恢复，以及受限分段/自动 commit 切回普通后禁学、下一次普通输入恢复学习；尚无大规模输入质量损失量化，REV-04 不因此关闭。

存储回归 12 场景 / 48 个独立进程通过：全程普通各一条 SQLite 学习，其他场景零条；空库没有 Rime userdb，三组预置高频合成旧词库逐文件字节不变，基础候选与空库及重启前一致。配置回归六场景 / 12 个进程通过，包含编译配置、用户 custom、未选中不安全 schema，以及真实缓存生命周期。指定日志未匹配合成选择 UTF-8，仅保留窄观察结论。命令、断言与限制见 [runbook](../runbooks/rime-privacy-probe.md)。

Rime 来源锁变化使当前 Linux 产品不再满足冻结 L6 pair 的同合同约束。冻结 JSON、载体与实机证据保持原样；仓库门禁检查冻结声明与拒绝漂移的回归，实际 `validate-contract` / `validate-target` 和构建入口继续拒绝当前新合同。不可把新构建与旧 source package 混配；任何新 pair、系统升级或旧数据现场部署都须另批明确。macOS 旧 build 38 也没有被替换或重新验收，旧进程仍可能沿用旧学习行为。

REV-01 **未整体关闭**：本批完成仓库实现、Rust/native 回归与全仓门禁；仍缺少两平台新构建的真实隐私路由/输入回归、更多 librime 环境及广泛质量评测。该隐私批未改变 SQLite；后续独立批准的升级见 REV-02。

## REV-02：SQLite WAL-reset 修复版本

初审时 [userdb manifest](../../crates/ime-userdb/Cargo.toml) 使用 bundled `rusqlite 0.32.1`，[Cargo.lock](../../Cargo.lock) 锁定 `libsqlite3-sys 0.30.1`，其 `sqlite3.h` 声明 `SQLITE_VERSION = 3.46.0`；2026-09-08 已按独立授权升级，当前结果见本节后文。源码版本不替代每个已冻结发布产物的版本取证。

[SQLite 官方 WAL-reset 说明](https://www.sqlite.org/wal.html#walresetbug)列出受影响范围、罕见并发条件及修复：3.51.3 及之后版本，另有 3.44.6、3.50.7 回补。RadishLex 的多连接 WAL 布局符合相关必要条件；没有在本次审阅中复现数据损坏。引用核对日期为 2026-09-05，执行前重新核验上游与所选版本。

建议核对 Rust 客户端、Go 服务端及产品内其他 SQLite 使用者各自的实际版本，不能以系统 `sqlite3` 版本替代 bundled library。依赖方案需单独批准，冻结载体不原位替换。

关闭条件：实际产物确认包含修复，依赖和 lockfile 变更可追溯，多连接写入/checkpoint、迁移、备份恢复及适用平台门禁通过。未命中罕见竞争的普通压力测试不能单独证明旧版本安全。

### 2026-09-08：版本核验与依赖候选

- 新增 Rust/Go opt-in 身份入口，命令见 [runbook](../runbooks/rime-privacy-probe.md#sqlite-运行时身份)。Rust userdb 测试查询实际链接 SQLite 为 **3.46.0**，source id 为 `2024-05-23 13:25:27 96c92aba00c8375bc32fafcdf12429c58bd8aabfcadab6683e35bbb9cdebf19e`；Go `modernc.org/sqlite v1.53.0` 的测试为 **3.53.2**，source id 为 `2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24`。
- 只读扫描 macOS `26.7.1-38` 的 InputMethod/Manager FFI 二进制，两者都含 `3.46.0` 与同一 SQLite source id。SHA-256 分别为 `d90ce52dca275fd88b2c90a9c4e2ef859f81f9a5768d732bb33f07253c797172`、`9f9117f417d34d0bba528149a0670fffda48859f585851a98060dc008f300ad7`。这是静态内嵌身份，未启动 bundle、未查询其中运行时、未修改冻结载体；Linux 制品与部署服务尚未取证。
- 当日重新核对 [SQLite WAL-reset 公告](https://www.sqlite.org/wal.html#walresetbug)，Rust 版本位于公告受影响范围；Go 测试版本已晚于修复版本。本轮没有执行损坏复现，不能因 Go 通过版本核验就宣称部署服务或所有数据库安全。
- 可审议的具体候选为上游 [rusqlite v0.39.0](https://github.com/rusqlite/rusqlite/blob/v0.39.0/Cargo.toml) 与其 `libsqlite3-sys 0.37.0`，该 tag 的 [bundled header](https://github.com/rusqlite/rusqlite/blob/v0.39.0/libsqlite3-sys/sqlite3/sqlite3.h) 固定 SQLite 3.51.3。该候选用于缩小方案范围，不代表已选择最新版本或已通过兼容验证。
- 下一依赖批建议只升级 Rust 客户端 SQLite 链，保留 bundled、backup 与 WAL 数据格式合同；批准后才解析依赖和更新 lockfile，核查 API/feature 差异及完整依赖图的 MSRV，并验证多连接 write/checkpoint、迁移、备份恢复和两平台新产物身份。若需要提高支持工具链版本，应联动 REV-06 明确批准；不改变 Go 依赖，不替换冻结包，不以系统 SQLite 代替 bundled，也不自维护 SQLite 补丁分叉。

### 2026-09-08：Rust 客户端升级与数据回归

- 依赖与测试本地提交为 `a03c69b`。userdb 和 runtime dev-dependency 同步升级至 `rusqlite 0.39.0`，锁定 `libsqlite3-sys 0.37.0` / SQLite **3.51.3**；保留 bundled、backup 和默认 statement cache。schema 9、WAL/NORMAL、5 秒 busy timeout、FFI ABI、同步协议及 Go 依赖均未改变，REV-03 的等待问题仍独立存在。
- 对照上游 0.33–0.39 release notes 核查 SQL tail 检查、整数转换、Name/backup API 与 feature 差异。现有调用无需源码适配，工作区 check 与 Clippy 通过。native 依赖变化限于 SQLite 链及 hashlink/hashbrown/foldhash；lockfile 新增的 wasm 条件依赖不进入当前原生构建，也不构成新增平台支持。
- MSRV 审核：新 native 传递依赖声明的最低版本不高于 1.65；rusqlite/sys 未声明固定 `rust-version`，上游 [MSRV 策略](https://github.com/rusqlite/rusqlite/blob/v0.39.0/README.md#minimum-supported-rust-version-msrv)只承诺发布时最新 stable。变更前原生依赖图已含要求 1.88 的 `time 0.3.53`，与根 1.80 声明不一致。本机 Rust 1.96.0 验证通过；未安装旧工具链、提高根声明或声称 1.80 通过，REV-06 仍开放。
- 新增三个写者 × 64 轮学习与三种 checkpoint 交错回归，核对 192 条事件、频次、词数与完整 integrity_check；固定读事务证明 reset 返回 busy、读视图不变、释放后可 truncate 并继续写入。这是回归与并发覆盖，不是上游罕见损坏的确定性复现。
- [旧 SQLite 合成 fixture](../../crates/ime-userdb/tests/fixtures/README.md)保留 3.46.0 原始文件字节；升级打开保持 schema 9，继续学习、删除后迟到选择不复活、备份内删除语义和显式恢复后重开均通过。既有旧 schema migration、损坏/未来 schema 拒绝、事务失败回滚与未 checkpoint WAL backup 测试通过。
- 常规身份测试查询 SQLite **3.51.3**，source id 为 `2026-03-13 10:38:09 737ae4a34738ffa0c3ff7f9bb18df914dd1cad163f28fd6b6e114a344fe6d618`。新 `aarch64-apple-darwin` release FFI（native-rime、apple-keychain）静态库 SHA-256 为 `2d1463a91f2d83e7af0ce12408117714575fa004a0bd8f2bbe81f61e5bbb7b93`，dylib 为 `d48528d478ab7b956728ce7ac984d474661db03cd03a82337b70051e4202f9c4`；两者包含新 source id。C probe 从该静态库链接 SQLite，查询同一版本/source id 并完成内存 SQL 和 mock FFI session 生命周期。没有给产品 ABI 新增版本查询入口，也没有调用真实 Keychain。
- SQLite 升级后 48 进程隐私存储与 12 进程有效配置回归通过；具体根、命令、产物身份和最终全仓门禁见 [当周记录](../devlogs/2026-W37.md)。Rust 身份最低版本断言已纳入常规测试，打印命令见 [runbook](../runbooks/rime-privacy-probe.md#sqlite-运行时身份)。

该依赖批未制作或替换 macOS app/DMG、Linux `.deb`，也未执行实机输入/安装事务；后续已批准的候选准备结果如下。REV-02 **未整体关闭**，旧 build 38 与冻结 L6 资产仍保持原身份。

### 2026-09-08：REV-01 / REV-02 macOS 联合候选准备

- 项目所有者批准先准备新 macOS 候选、固定工具链与实际产物身份，并编写后续实机矩阵。版本元数据提交 `5e9b0a8` 将单一产品 build 提升至 39，联动 Linux 版本投影；未生成 Linux 新包、替换冻结 pair 或改变安装合同。
- 完成 `26.7.1 (39)` 双组件和本地 community ad-hoc Installer。ProductManifest、双份 payload、ReleaseIdentity、资源/native 依赖与签名验证通过；双 FFI 包含 SQLite 3.51.3 新 source id，旧 source id 不存在。具体 hash、工具链与边界见[联合验收入口](../runbooks/macos-rev01-rev02-acceptance.md)。
- 最终包内 InputMethod FFI 七个独立合成场景通过：普通学习为一条，六类受限/往返场景为零，commit 保留且无 Rime 自有学习库；包内 Manager FFI smoke 通过。C 上下文注入不等于真实 macOS 隐私路由；此前 native 分段/重启证据不填充本候选实机矩阵。
- 完整仓库门禁、平台元数据与 L6 合同、Flutter analyze 与 99 项测试通过。旧 build 38 的 104 项树记录一致，Cargo/pub 锁与冻结 L6 声明不变。新增 smoke 只使用新建合成临时数据。
- 未安装或启动产品 GUI、未制作新 DMG、未操作系统输入法/Keychain/guest，也未发布。Installer 的历史升级源为空，下一步应先明确首次安装资格，不能直接执行 build 38 → 39 升级。两平台真实复验、Linux 新 pair/产物、输入质量与 REV-06 继续开放。

候选准备后，项目所有者选择本机现有账户测试。旧数据及历史 operation 已按精确授权整体归档，正式安装路径为空；现场与归档证据以[联合验收入口](../runbooks/macos-rev01-rev02-acceptance.md)为准。旧 receipt 与实际 data-root 的 device id 不一致，触发原因及长期身份合同仍待评估；归档没有修复或放宽生产校验。后续从[2026-09-09 明日事项](../devlogs/2026-W37.md#2026-09-09明日事项)继续，不自动导入旧库或沿用归档授权执行安装。

### 2026-09-09：build 39 首次安装与 Manager 启动

- 按项目所有者本次明确授权，在现有账户从重新核验的空正式路径执行本地 Installer 首次安装。`prepared` 与再次静止确认分别执行，receipt 到达 `first_install/completed`；双程序 tree、strict ad-hoc identity、版本与无 quarantine 通过。
- Manager 从固定用户域路径启动，界面显示空词库与 `local_only`，新 userdb 为 `0600`；新 data-root identity 与 receipt 一致。旧资料未导入，原 device id 差异的长期处理仍未解决。具体身份、观察范围与证据见[联合验收入口](../runbooks/macos-rev01-rev02-acceptance.md#本机首次安装与-manager-启动2026-09-09)。
- 这只补齐本候选首次安装和 Manager 启动证据。InputMethod 尚未运行，隐私系统路由、真实输入/学习、删除恢复、重启及质量矩阵仍待执行；REV-01/REV-02 不因此关闭，Linux 新产物/pair 与其他开放项保持原范围。

### 2026-09-09：普通选词与同会话学习

项目所有者完成 TextEdit `shi → 时` 两次选择，反馈候选由 2 升至 1、提交正确且未感到卡顿。只读核对首轮 selection/term/ranker 各增 1，第二轮仅 selection 增 1，精确 `editor` frequency 从 1 增至 2；未发现 Rime 自有 userdb。固定路径 InputMethod 已运行，安装与数据身份不变。该组只覆盖普通选择和同会话候选变化，不替代隐私路由、重启、独立排序对照、输入质量或竞争延迟测试；REV-01/REV-02 继续开放。明细见[联合验收入口](../runbooks/macos-rev01-rev02-acceptance.md#普通输入与同会话学习2026-09-09)。

### 2026-09-09：全程隐私输入与普通恢复观察

Manager 隐私草案首次尚未保存时前置检查未通过；项目所有者保存后，平台 API 与设置文件均为 true。新的 TextEdit `shi → 时` composition 提交正确，全库聚合、目标频次与时间戳零变化，Rime 自有 userdb 为零；该全程隐私用例通过。关闭并保存后平台键为显式 false，目标频次 2→3；同区间全库 selection 增 3，项目所有者随后确认另有 RadishLex 输入，未索取或读取其内容。该区间保留目标词恢复学习结论，不当作单次全库精确用例，不再将额外增量列作未知缺陷。未提交 composition 中途双向切换及其余矩阵仍待执行，REV-01/REV-02 继续开放。具体边界见[联合验收入口](../runbooks/macos-rev01-rev02-acceptance.md#隐私模式与恢复普通学习2026-09-09)。

### 2026-09-09：composition 切换部分执行与上下文差异

项目所有者批准后台隐私控制和新 TextEdit 文稿合成按键。第一组隐私→普通在同一 marked `shi` 保留后提交“时”，全部聚合和固定合成身份零变化；随后新普通输入提交正确且选择事件增 1，却新增 `code` frequency=1，原 `editor` frequency=3 不变。执行者按停止线暂停，后两组未开始；隐私键已恢复 false、临时基线已消费、数据身份未漂移，输入源仍选中且进程运行。该组保留零学习与普通一次学习观察，但不作为 TextEdit 上下文路由完整通过，证据见[联合验收入口](../runbooks/macos-rev01-rev02-acceptance.md#未提交-composition-切换2026-09-09-部分执行后暂停)。

当时源码确认 build 39 控制器忽略 `sender`，按最前台应用分类；`code` 唯一映射为 Codex，与本次差异一致。尚未确定激活时点，也未实测真实敏感应用受影响。当时建议以公开 IMK 输入客户端 bundle identity 作为来源，缺失/无效时 unknown 禁学，不借最前台已知应用放宽；保留 secure/private/sensitive 与 composition 锁定策略。项目所有者随后批准仓库修复及回归，结果如下。

### 2026-09-09：输入客户端身份仓库修复

`6a55782` 已将应用分类来源改为当前 IMK client 的 `bundleIdentifier`；缺失/无效或查询异常保持 unknown，固定异常日志不包含 App ID 或异常正文。九类身份、五条提交路径、独立隐私/secure 信号、策略往返及更新失败合同通过；既有 native 12 场景 / 48 进程存储回归与完整仓库门禁通过。控制器合同记录 bridge 入参和调用顺序，native 回归验证 Rust 持久化，二者不合并宣称真实 IMK 路由通过。ABI/schema、应用允许表和 runtime 策略不变。已安装 build 39 与原候选仍是修复前版本；新候选、安装后实测及剩余矩阵未执行，REV-01/REV-02 继续开放，详情见[联合验收入口](../runbooks/macos-rev01-rev02-acceptance.md#客户端身份来源修复2026-09-09)。

### 2026-09-09：build 40 双组件与受控升级源待确认

版本提交 `1636acf` 后已装配 build 40 双组件，二进制确认包含客户端身份修复且不含 contract 注入入口；包内 InputMethod 七场景与 Manager FFI smoke 通过，ABI/schema、双 FFI 与 Rime 来源不变。原 build 39 候选与实际已安装组件的完整树和签名一致，completed receipt/data-root identity 一致，原载体与锁文件保持不变。本轮未生成 build 40 Installer、安装、执行真实输入或发布；需先单独确认把 build 39 本地候选作为本次 39→40 唯一受控升级源，再按既有 payload/identity 机制封装。不能以本地候选冒充历史正式发布，也不据构建或合成测试关闭 REV-01/REV-02。完整身份与下一范围见[联合验收入口](../runbooks/macos-rev01-rev02-acceptance.md#build-40-双组件准备与升级衔接2026-09-09)。

## REV-03：输入线程的数据库等待

[候选选择](../../crates/ime-runtime/src/session.rs)在返回结果前同步执行 `record_selection`；[学习事务](../../crates/ime-userdb/src/store/learning.rs)取得 `Immediate` 写事务，[连接策略](../../crates/ime-userdb/src/store/connection.rs)配置 `busy_timeout = 5000 ms`。平台收到结果后才向宿主提交文本，存在写锁竞争拖住输入回调的风险。

[SQLite busy timeout](https://www.sqlite.org/c3ref/busy_timeout.html)明确允许锁冲突等待；WAL 不能消除写者竞争。当前“学习失败仍返回 commit”合同保障错误结果，不等于返回延迟已有约束。本次没有实测秒级卡顿。

建议先用文件数据库和合成词测量短锁、长锁、Manager 导入、多 session、checkpoint 与故障条件，记录从按键/选择到 commit 返回的分位延迟和超时结果。选定受支持硬件上的延迟预算后，再决定短等待、显式跳过学习或受控异步方案；异步必须说明队列上限、退出、隐私切换、删除顺序与失败诊断。

关闭条件：批准的延迟与学习持久化合同有可重复竞争测试；commit 不丢失，原始事件不会越过隐私边界，低性能环境的结果如实记录。

## REV-04：新词召回与输入收益

[runtime](../../crates/ime-runtime/src/session.rs)先取得 engine candidates，再读取身份匹配的摘要；[ranker](../../crates/ime-ranker/src/ranker.rs)仅遍历该候选集合。向 RadishLex userdb 添加或导入词条，不等于已有独立候选注入能力；也不能把 Rime 自身学习的收益全部归因于 RadishLex ranker。

现有 [ranker 评测](../../crates/ime-ranker/tests/evaluation.rs)固定 5 个合成 case，Top-1 为 0.8、Top-3 为 1.0、MRR 为 0.9，属于规则回归。[runtime 延迟测试](../../crates/ime-runtime/tests/personalized_session.rs)使用合成引擎和内存数据库，不覆盖真实文件、完整平台回调或锁竞争。

建议先定义新增短语的行为：engine 原候选中没有该词时，导入后的预期是什么；跨页、分段、reading 身份、删除和恢复如何表现。再建立公开或合成输入集，覆盖长句、同音词、中英混输、标点、错拼、分段选择、重启与删除后普通选择。

关闭条件：固定输入集及规模，比较基础引擎与个人化后的 Top-1/Top-3/MRR、选词次数、改善和误伤；记录冷启动、持续输入、并发管理下的延迟。新词召回方案须另行批准，不在文档中冒充已实现功能。

## REV-05：删除与保留策略

[删除事务](../../crates/ime-userdb/src/store/learning.rs)保留 deleted 词条和 tombstone 身份、移除 ranker 权重并追加负反馈；不会同时清除既有 `selection_events`。降权或删除学习偏好也不保证基础 engine 再不提供同文候选。

当前合同解决防复活与显式恢复，不提供全部记录的物理擦除承诺。建议区分“删除学习偏好”“清除原始事件”“清除本地数据”的用户预期，明确 P1 保留期限、清除入口、备份影响及为防复活必须保留的最小信息。不得为了清理空间直接删除 tombstone。

关闭条件：经批准的保留与删除策略明确数据范围、默认值、恢复和同步影响，Manager 文案与实现一致，相应事务与旧状态回归通过。原始事件自动清理、物理擦除和新删除协议均未在本批实现。

## REV-06：工具链与验证覆盖

- [workspace](../../Cargo.toml)声明 `rust-version = 1.80`；锁定的 `base64ct 1.8.3` 依赖元数据要求 Rust 1.85，且该库出现在 [sync](../../crates/ime-sync/Cargo.toml)正常依赖中。这是声明不一致，本次未安装 Rust 1.80 重跑。
- [PR workflow](../../.github/workflows/pr-check.yml)提供 baseline、Clippy、Flutter、Go race/vet；[Release workflow](../../.github/workflows/release-check.yml)仅安排文本与 repository baseline，未完整复用这些检查。
- [check-repo](../../scripts/check-repo.py)具有平台条件分支；Ubuntu 上通过不代表 macOS adapter 或 native product 已验证。当前没有从这两个 workflow 得到完整 MSRV、依赖安全和真实平台产物验证证据。

建议形成日常、集成、候选产物三层检查映射，绑定提交、工具链、feature、平台和产物身份；保留本地 `dev` 工作方式，但使阶段证据可独立复验。MSRV 应根据实际依赖与支持范围决策，不直接把 1.85 当作全 workspace 已验证下限。

关闭条件：支持版本声明和实际最低版本构建一致，关键检查在相应层运行，发布候选能追溯实际平台产物及依赖版本。workflow、依赖和全局工具链未在本批修改。

## REV-07 至 REV-10：维护与产品决策

| 编号 | 依据与建议 | 关闭或决策所需结果 |
| --- | --- | --- |
| REV-07 | [ranker](../../crates/ime-ranker/Cargo.toml)依赖 userdb，传递包含 SQLite、sync、crypto；在相关改动时评估由窄摘要类型承载排序输入，不为此立即做全仓重构 | 依赖方向方案批准，转换与排序语义回归通过 |
| REV-08 | 基线中 `scripts/linux-product` 有 162 个 Python/Shell 文件、88,302 行，其中实现 60,472 行、测试 27,830 行；最近 100 个提交自 2026-08-26 起有 99 个以 Linux 为主题。统计只说明维护规模与主题分布，不代表工时或代码质量 | 分开列产品事务、验收环境与单次历史证据的职责；提出环境故障投入上限、切换条件及真实可复用部分，不创建通用框架占位，不清理冻结材料 |
| REV-09 | [Manager 弹窗](../../apps/radishlex-manager/lib/src/screens/dictionary/dictionary_dialogs.dart)暴露 tombstone、suppressed 等内部词，并要求输入导入路径；建议用户语言、文件选择、预览及错误恢复，技术字段归入诊断 | 完成主要任务的可用性方案、合成数据测试及适用人工验收；原生文件选择涉及的新依赖或权限另行评估 |
| REV-10 | 全平台统一发布会延后反馈；建议讨论稳定平台的小范围试用或提前用户研究，并澄清当前 LICENSE 下试用、自部署与贡献的授权路径 | 明确目标人群、反馈范围、许可与发布决策；现有平台顺序、统一发布政策、同步关闭态和许可证继续有效 |

## 本次审阅证据与后续维护

2026-09-05 在 `a5345b8` 执行：

```bash
cargo test --offline --locked -p radishlex-ime-runtime -p radishlex-ime-ranker
cargo test --offline --locked -p radishlex-ime-userdb --lib
./scripts/check-docs.sh
./scripts/check-text-files.sh
git diff --check
```

前两项共 96 项测试通过；文本与差异检查通过，文档检查对两份已有长文档给出预算警告。未执行完整 repository gate、Flutter/Go 全套、真实平台操作或密码协议全面审计。此处只记录审阅轮次，不替代后续文档整理或实现轮次的验证记录。

更新事项时保留原始证据及其观察范围，追加新的判断、批准与验证；不能仅因文档完成、普通测试通过或阶段曾退出就关闭风险。待决策建议不能自动生成真实系统或外部发布授权。
