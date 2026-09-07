# 产品审阅与改进跟踪：2026-09

本文面向维护者与后续实现者，保存 2026-09-05 综合审阅的证据、风险、建议和关闭条件。源码基线为 `a5345b8`；审阅是文档与关键路径抽查，不是全面安全审计、真实系统验收或发布认证。当前执行顺位与现场停止线仍只读 [current](../status/current.md)。

项目所有者已授权整理项目文档；这不构成依赖更新、架构或隐私方案变更、运行时修改、真实系统操作及发布授权。本专题登记改进建议，不改写既有里程碑退出记录，不把历史证据扩展为未测能力。关闭事项必须关联修复提交、实际验证及剩余限制；全部处理后按文档规则退出日常阅读链。

## 总体判断

保留本地优先、成熟 engine adapter、Rust 业务真相源、原生平台薄壳和密文同步的主体方向。近期应优先验证“不学习、不丢输入、删除不复活”等核心承诺，补输入质量与性能证据，再收敛验收工具和日常管理体验。平台数量、代码行数和阶段退出记录不能单独代表用户可用性。

下表的优先级是审阅建议，尚不替代现有 M5 系统操作顺位。本文使用“首要、次要、后续”，避免与 P0–P3 数据分级混淆。

| 编号 | 建议优先级 | 事项 | 当前证据性质 | 状态 |
| --- | --- | --- | --- | --- |
| REV-01 | 首要 | Rime 自有学习与隐私控制 | 配置、源码与测试观察范围交叉核对 | 待隔离 native 验证与方案决策 |
| REV-02 | 首要 | SQLite WAL-reset 修复版本 | 锁定依赖、本地依赖源码与上游公告 | 待产物核验与依赖方案批准 |
| REV-03 | 次要 | 学习写入阻塞输入回调 | 同步调用链与 5 秒 busy timeout | 待竞争测试与延迟预算 |
| REV-04 | 次要 | 新词召回与个人化实际收益 | 当前候选重排实现及合成评测 | 待行为规格与评测设计 |
| REV-05 | 次要 | 删词、事件保留与用户预期 | 删除事务和 Manager 文案 | 待保留策略与交互方案 |
| REV-06 | 次要 | MSRV、依赖与 CI 覆盖 | manifest、依赖元数据与 workflow | 待工具链和门禁方案 |
| REV-07 | 后续 | ranker 对持久化模块的依赖 | Cargo 依赖树 | 待相关改动时评估窄类型边界 |
| REV-08 | 后续 | Linux 验收环境维护成本 | 受跟踪文件统计与近期提交主题 | 待工具复用与环境投入评估 |
| REV-09 | 后续 | Manager 日常操作与反馈 | 页面源码抽查 | 待可用性方案与验收 |
| REV-10 | 后续 | 发布反馈节点与许可说明 | 路线图、LICENSE、产品定位 | 待产品决策，不改变发布停止线 |

## REV-01：Rime 自有学习与隐私控制

已确认的源码事实：

- [产品 schema](../../packaging/rime/data/radishlex_pinyin.schema.yaml) 使用 `script_translator`，并设置 `enable_user_dict: true`。
- [runtime](../../crates/ime-runtime/src/session.rs) 的 `LearningContext` 控制 RadishLex SQLite 摘要读取、选择记录和学习结果；引擎处理按键与候选提交仍独立发生。
- 在 [Rime session](../../crates/ime-engine-rime/src/session.rs) 与 [进程 runtime](../../crates/ime-engine-rime/src/runtime.rs) 中未发现把该策略传递到底层学习控制的实现；声明 `set_option` ABI 本身不是接线证据。
- [native 测试](../../crates/ime-ffi/tests/native_rime_behavior.rs) 的隐私选择断言检查学习 disposition 和 RadishLex `selection_events`，未由这些断言证明 Rime 自有学习存储不变。
- [librime 1.13.1 Memory](https://github.com/rime/librime/blob/1.13.1/src/rime/gear/memory.cc) 监听 commit 并调用学习；[ScriptTranslator](https://github.com/rime/librime/blob/1.13.1/src/rime/gear/script_translator.cc) 实现用户词典更新。上游源码用于核对行为，不复制实现。

判断：对仍经过引擎的隐私模式输入，存在未被统一策略覆盖的底层学习路径。SQLite 零增量不能推导整个输入法零学习，engine-only 顺序也不能自动解释为没有既有个人化影响。本次未操作真实输入法、未检查用户数据，也未实测证明泄露或落盘。

建议先以新的隔离目录、产品 schema 和合成输入验证普通、隐私、unknown/sensitive、切换和重启路径；同时检查 RadishLex 数据、Rime 用户词典、日志和候选行为。区分初始化元数据变化与包含输入内容的学习变化。不得复用冻结的 P04/R01B 现场。

待决策：由 RadishLex 独占学习，或将 Rime 学习完整纳入隐私、删除、恢复与 explain 合同。不能只假设某个 option 能同时关闭读取、写入和日志。

关闭条件：批准的方案覆盖所有输入衍生存储，native 正负向验证与重启观察通过，相关平台回归完成，历史证据仍按原观察范围保留。

## REV-02：SQLite WAL-reset 修复版本

[userdb manifest](../../crates/ime-userdb/Cargo.toml) 使用 bundled `rusqlite 0.32.1`，[Cargo.lock](../../Cargo.lock) 锁定 `libsqlite3-sys 0.30.1`。审阅时本地该依赖的 `sqlite3.h` 声明 `SQLITE_VERSION = 3.46.0`；这证明默认 bundled 输入版本，不替代每个已冻结发布产物的版本取证。

[SQLite 官方 WAL-reset 说明](https://www.sqlite.org/wal.html#walresetbug)列出受影响范围、罕见并发条件及修复：3.51.3 及之后版本，另有 3.44.6、3.50.7 回补。RadishLex 的多连接 WAL 布局符合相关必要条件；没有在本次审阅中复现数据损坏。引用核对日期为 2026-09-05，执行前重新核验上游与所选版本。

建议核对 Rust 客户端、Go 服务端及产品内其他 SQLite 使用者各自的实际版本，不能以系统 `sqlite3` 版本替代 bundled library。依赖方案需单独批准，冻结载体不原位替换。

关闭条件：实际产物确认包含修复，依赖和 lockfile 变更可追溯，多连接写入/checkpoint、迁移、备份恢复及适用平台门禁通过。未命中罕见竞争的普通压力测试不能单独证明旧版本安全。

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
