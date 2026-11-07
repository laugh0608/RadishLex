# 仓库门禁与测试运行

本文说明常规质量、可靠性专项和真实资格验收的职责、触发及证据。它不授权真实安装、GUI、冻结资产、生产资格实验或完整验证；当前结果见 current 与周志。

## 正式分层与授权

| 层级 | 内容 | 适用条件 |
| --- | --- | --- |
| 日常反馈 | 差异与 hygiene、Rust 格式和 Clippy、会话与安装升级核心测试、Go、Manager | `dev` PR 与本地日常入口 |
| 常规质量 | hygiene、Rust、contracts、Go、Manager 五个组件 | `master` 晋级与发布 tag 必需 |
| 可靠性专项 | 安装执行及升级历史的全故障点、全进程退出和深生命周期组合 | 受影响的 `master` PR 必需，发布候选全部执行 |
| 平台与真实资格 | 实际安装、连续事务、真实程序资格、人工输入、容量实验 | 按对应 Boundary/runbook 单独授权，不能由前两层代替 |

`./scripts/check-repo.sh` 执行五个常规组件。晋级与发布由同一被验证 revision 的常规组件和所有适用专项共同验收。真实平台资格使用对应 runbook 的独立证据。

完整常规门禁、完整可靠性专项及完整平台回归，每轮仍须提前说明必要性、具体命令、环境、预计耗时、资源影响并取得明确允许。不能通过组件入口拆分绕过授权；定向静态检查、具体用例和 repository-only smoke 按协作规则执行。失败或中断不自动授权重跑。

日常 `dev` CI 的作业预算为十分钟，超时视为失败。完整门禁单独记录耗时及缓存状态。2026-11-01 干净 `90e7717` 的本机完整常规用时 12 分 53.82 秒，恢复专项用时 18 分 8.23 秒。具体覆盖见[W44 周志](../devlogs/2026-W44.md)。

本地开发按风险使用定向测试和静态检查；完整验收集中在改动稳定后的明确阶段交付点，不因每个小批次、文档收尾或本地提交自动重跑。涉及核心边界的改动仍保留最终完整验证义务，待验可以如实记录，不能写成已通过或借此开放产品。性能问题先诊断、优化，再为稳定快照制定常规及适用专项计划；不削减安全/恢复覆盖，不用反复全量运行寻找热点。

## 入口和唯一归属

`./scripts/check-repo.sh --plan` 只展示计划，不执行检查。`--component` 选择职责明确的组件，不是 quick 或完整验收替身；旧 `--skip-*` 参数不再提供。

本地日常执行 `./scripts/check-repo.sh --component dev`，使用现有 Rust、Go、Flutter 和锁定的 Manager 依赖。默认检查相对 `HEAD` 的已跟踪文件改动。检查已提交的 PR 范围时使用 `--pr-diff BASE HEAD`，传入两个完整 commit SHA。入口执行 hygiene、workspace Rust fmt 与 Clippy、`ime-core` 的 `session_lifecycle`、`ime-product-install` 与 `ime-product-upgrade` 的库级单测，以及原 Go、Manager 组件。

| 组件 | 命令与覆盖 |
| --- | --- |
| `hygiene` | 文本、文档、治理、相对链接、检查器单测及工作流合同；PR 另检查完整 base/head diff 空白 |
| `rust` | 全仓 fmt、workspace all-targets Clippy、默认目标与 doc-test 各一次；另列 userdb observer、Linux L6 和 macOS qualification-harness 的显式配置 |
| `contracts` | macOS 原生构建/双向映射/展示、跨平台 Linux C++/打包/L6 Python、启动顺序及同步部署合成合同 |
| `go` | sync-server gofmt 检查、vet 与 race test 各一次；Rust HTTP 集成保留为独立跨语言边界测试 |
| `manager` | Dart format、Flutter analyze/test；使用 `--no-pub`，入口不解析或安装依赖 |
| `reliability` | 两个包的 `reliability-matrix` Clippy，随后仅运行 `reliability_matrix_` 父测试 |

统一 Rust 清单在 `scripts/validation_suites.py`。workspace 已覆盖 Linux package/startup、FFI header 和 L6 controller 默认 Rust 测试；contracts 不再嵌套调用这些完整 Rust 专项。L6 Python 合同只由其合同入口执行一次。默认配置和显式 feature 配置分别证明各自语义，不视为可相互替代。

五个 macOS/core 独立专项仍保留各自 static/default-tests/extra-tests/native，以及适用的 reliability 阶段。无参数为完整专项；`--phase` 结果只是对应阶段，不声明完整专项通过。

## 安全和恢复覆盖

两个包新增的 `reliability-matrix` 只选择测试，不改变生产逻辑或默认 feature。常规父测试继续验证真实私有文件、SQLite、SHA-256、inode/权限/guard、学习与 tombstone、成功/中止/回滚、授权撤销及严格拒绝未知材料。

| 测试组 | 常规保留 | 可靠性专项保留 |
| --- | --- | --- |
| Executor mutation | 完成/回滚两种结果、首末 mutation 边界及原 proof 保留 | 观察到的每个唯一 mutation 边界，断言不变 |
| 取消 archive | 首末选定边界，授权丢失及真实子进程退出、重新取锁恢复；无前驱/回滚前驱阶段0/5原件保留 | 原有全部选定边界、writer最后访问及46种保存组合 |
| 取消 release | 未绑定 ProofCreated 与已移动 MarkerMoved；维护/未维护、返回失败/进程退出 | 原25边界全部组合及原有断言 |
| 最终取消资格 | 无前驱/回滚前驱、阶段0/5、资格拒绝与原件保留 | 原4种前驱×3阶段×2 outer 组合 |
| 连续取消与历史漂移 | 无前驱连续取消；同字节 inode 替换后零写入拒绝 | 全前驱组合、六种祖先漂移、混合 v1/v2 生命周期 |
| 归档/handoff/terminal release | 普通主路径、冲突、身份漂移及既有恢复测试 | 原全边界授权丢失、全进程退出和 released-predecessor 矩阵 |

`scripts/reliability-cases.json` 按包与 target 固定22个专项父测试（Executor/lib 2、升级/preparation_history 16、升级/source_preparation 4）；执行前逐 target 发现并严格对照，空选择、漏项或新增未登记项均失败。新增 source_preparation 的4项只执行常规层之外的阶段交互组合，与常规层合起来覆盖原完整矩阵。专项用 `cargo test ... --features reliability-matrix ... reliability_matrix_` 精确选择父测试；普通父测试不重复执行。ignored 子进程入口仍由父测试通过 `--exact` 调用，禁止全 workspace `--ignored` 或 `--all-features`。真实 qualification-harness、observer 三项容量实验仍按各自合同处理，不能借可靠性 feature 自动启动。

历史 fixture 的普通后续学习由80条减为1条，每个样本仍独立创建。80条记录保留为专门回归，明确断言源库长度与旧 receipt 不同，且历史核验、81个活动词与 tombstone 保留。不得通过共享可变 fixture、跨观察窗口复用 hash 或历史链核验成功、省略 fsync 或跳过必要实物核验提速。

### 取消 proof 的计算复用

取消 release、archive、source 的严格编码原来会递归校验嵌套 request/preparation；父级先 `validate` 再为长度调用 `encode`，索引派生和材料比对又重复这棵树。现在每次验证调用用内部 `EncodedCancellationProof` 绑定已校验的不可变模型与 canonical 字节；历史 proof 严格解码后直接保留二者，同次派生索引与核对材料复用计算。archive/source 与 data receipt 的长度取自本次已校验编码，不重复执行相同的嵌套校验。

该内部类型不保存文件身份、摘要或目录观察，不跨检查点替代实物核验。公共编码/解码仍完整拒绝无效嵌套对象、非 canonical 内容及超限材料；持久化格式、256 KiB 上限、授权/资格、guard、hash、owner/mode/link、fsync、原件保留与恢复顺序不变。每个检查点仍重新读取实物；历史 proof/索引的前后身份及最新 locator 复核保留。

前述纯计算优化之后，2026-10-24 又合并同一观察窗口内的重复实物核验，见下节。这里的“观察窗口”止于当前验证调用，绝不跨授权回调、写入、重启或后续调用复用结果。历史链仍逐祖先验证，没有引入跨窗口的信任缓存。

2026-10-23 对同一连续取消代表用例作宿主机 ABBA 对照，两种固定二进制各2样本，均为并发1、nice +19、独立合成目录，不含编译或采样器：

| 指标 | 本次优化前 | 优化后 | 减少 |
| --- | --- | --- | --- |
| 平均墙钟 | 19.486秒 | 8.322秒 | 57.3% |
| 平均CPU | 18.537秒 | 7.430秒 | 59.9% |

原始输出、二进制 SHA-256、独立样本与环境约束见本机 `/private/tmp/rlxcpu-8bj3cqiu/comparison-final.json` 和同目录 `compare-final.py`；本机临时证据不随仓库分发。该结果仅对应 `representative_cancellation_lifecycle_continuous_cancellations_keep_null_and_legacy_data_predecessors` 的现有代表选择，不是完整前驱矩阵或整仓门禁耗时。深历史仍需逐项核验；整仓常规、完整可靠性和 CI 的最终耗时及 Linux 运行结果待另行验证。

### 观察窗口与隐藏矩阵收敛（2026-10-24）

`PreparationJournalStore::read_verified_history` 将有256 KiB上限的 proof、locator、inventory 与字节比对统一为一次有界读取：读取前检查私有 pathname 和打开 descriptor，读取同一 descriptor，对实际读取的字节调用 SHA-256，检查完整消费及 descriptor/pathname 的前后 identity、mtime/ctime。摘要直接绑定解码/比对的字节，拒绝增长、缩短、同字节新 inode、权限/link、符号链接和删除漂移；hash/read 错误仍传播。SQLite与任意大小的数据文件仍使用流式 `evidence`，没有把数据库复制进该缓冲区。

历史 `resolve_one` 直接使用该次严格读取的摘要派生索引，材料核验结束后的 proof 身份复核仍保留。`load_verified_previous_inventory` 为取消请求、源库准备检查点、归档检查点和维护只读观察一次加载完整前驱；released 路径不再紧接着通过 `verify_inventory_objects` 重走相同历史链。绑定 context、最新 locator 的末次复核、前后 v1 空槽及新 operation 目录拒绝、guard 和各材料检查仍在；非 released 路径继续验证全部 inventory objects。外层回调前后仍分别执行新的验证；此变化不把任一文件核验成功作为后续检查点的依据。

跨层责任按下表区分，避免把相同持久化原语的所有阶段笛卡尔积隐含在常规父测试中：

| 责任 | 常规层 | 可靠性补充与保留理由 |
| --- | --- | --- |
| 有界证明读取 | 核心单测覆盖字节/摘要绑定、超限、hash期间漂移和未消费完整输入 | 上层继续验证实际SHA-256、目录关系和重启；不以合成digest单测替代集成 |
| 取消请求持久化 | Reserved 的11个检查点 + 其余4阶段的 Recorded；返回失败/真实退出各15场景 | 每种故障模式补齐其余40个阶段×检查点交互；每种模式两层合计55，原断言共110场景完整保留 |
| 取消 source finishing | MaintenanceIntent 的31个写入/维护/语义检查点 + 其余4阶段 SourceReady；两种故障模式各35场景；五阶段成功/幂等及 hot-journal 保留 | 每种模式再补100个交互，每种模式两层合计135，原270场景完整保留 |
| 历史/lifecycle | proof/locator绑定、祖先漂移、学习/tombstone、最新索引末次复核、取消后再升级 | 原16个历史矩阵保持；这是跨操作链的约束，不由单文件持久化测试替代 |
| Executor程序/data衔接 | 11个程序/data切换检查点及2种结果×7个失败恢复点仍保留 | 涉及独立外层程序恢复与组合终态，不能仅因下层也检查guard/hash而删除；原2个mutation/lifecycle矩阵保持 |

四个 source_preparation 父测试原来在常规层暗含380场景，现在常规100 + 可靠性交互280。选择合同测试逐个检查每个原组合恰好归属一层，既不缺项也不重叠；原场景执行器与所有断言共用，fixture仍每个场景独立创建。`source_finishing` 的恢复进度22场景是 hot-journal 特有分支，继续保留，没有并入普通写入覆盖后删掉。独立完整专项包含 default-tests 和 reliability 两阶段；单独 reliability 阶段的280组合不能声称自身覆盖380组合。

同机同一连续取消用例，未改变测试步骤或断言，固定计数探针二进制ABBA各2样本；并发1、nice +19、每次独立合成目录、不含编译，未同时运行其他本批构建/重测：

| 指标 | 窗口合并前 | 合并后 | 减少 |
| --- | --- | --- | --- |
| 平均墙钟 | 7.824674秒 | 6.503900秒 | 16.9% |
| 平均CPU | 6.819863秒 | 5.539001秒 | 18.8% |
| SHA-256调用 | 63,385 | 35,891 | 43.4% |
| 参与hash字节 | 1,092,024,395 | 917,073,404 | 16.0% |

计数通过 `PreparationHasher` 包装器委托原 `MacOsPreparationHasher` 并累计实际读取字节；包括文件及内存canonical输入，不是磁盘物理读取量。用例包含3次取消、后续升级/terminal release及查询；嵌套span不能相加。前后两次样本的hash次数/字节分别一致。本机证据 `/private/tmp/rlxobserve-6kegny8y/` 内 `comparison.json`、`summary.json`、`compile-command.json`、`tests/` 与 `compare.py` 保存原始输出、参数、探针及二进制SHA-256；诊断前基线探针来自 `/private/tmp/rlxwhy-4lyasvhh/`。这是同用例的运行时收益，与常规测试选择调整分开计量，不能外推整仓耗时。

22个定向集成测试全部通过（含六类祖先漂移、hash期间最新索引替换、callback篡改、真实退出/新guard、hot-journal、16类released材料漂移）；两个新增核心读取单测通过。变更后的常规选择及22项专项发现清单另行复验；本轮未运行完整常规门禁、完整可靠性、CI、Linux宿主或实机资格，不能继承上轮完整通过结论。剩余约0.917 GB hash输入、SQLite等价核验、持久化与Executor真实组合成本仍需按调用窗口继续分析；5分钟目标尚未验收。

## CI 触发与聚合

`Dev Checks` 只监听目标为 `dev` 的 PR，一个 Ubuntu 作业调用 `--component dev --pr-diff BASE HEAD`，汇报 `Dev Quality`。`PR Checks` 只监听目标为 `master` 的 PR，汇报 `Candidate Quality`。普通分支 push 不触发这两个工作流。

`master` 的 `validation_impact.py` 使用整个 PR 的 merge-base diff，关闭 rename 合并以同时检查删除和新增路径，并沿 workspace 的普通、开发、构建、target 和继承 path 依赖向消费者传播。

安装/升级、userdb、FFI 及相应 macOS 包受影响时要求可靠性专项；工具链、Cargo/lockfile、验证脚本、平台桥及未知路径从严要求。纯文档、已分类 Manager Dart 或 sync-server 改动不单独触发安装恢复专项，但全部常规质量仍必需。空 diff 或无法完成判定不能豁免。

`.github/workflows/quality.yml` 是 PR/tag 共用质量工作流；Rust/原生覆盖 Linux 与 macOS，Go 和 Manager 独立并行。Rust固定1.96.0、Flutter固定3.44.0，Go依go.mod；Cargo 缓存只复用构建产物，Cargo 仍核验输入，不复用门禁结果。首次冷编译/runner差异须单列。

Ubuntu 在同一作业内先跑合同，再启动常规 Rust。`macOS Product and Contracts` 独立准备 Manager 锁定依赖与 Homebrew `librime`，从当前 checkout 构建双 bundle 和 Installer payload，并在同一 runner 校验。产品装配和 payload 每轮重建。[产品装配入口](macos-product-assembly.md)

macOS 前置作业成功后，常规 Rust 与适用的恢复专项在各自 runner 并行。两者检出本轮候选，使用各自的 Cargo 构建缓存。macOS 前置、常规 Rust、恢复专项的作业超时分别为 20、30、60 分钟。Ubuntu 合同与常规 Rust 共用 30 分钟。

`Candidate Quality` 聚合 macOS 前置、Ubuntu 和 macOS 常规 Rust、hygiene、Go、Manager 与恢复专项。仅明确不适用的恢复专项允许 skipped，其他缺失、失败、取消和跳过均拒绝。发布 tag 调用同一质量工作流且强制恢复专项。真实平台发布资格按对应 runbook 验收。

## 调度、构建与进程收尾

`run-low-impact.sh` 保留 `CARGO_BUILD_JOBS=1`、`RUST_TEST_THREADS=1`、`GOMAXPROCS=1` 与 `nice +19`，移除强制 `taskpolicy -b`。后台策略实测会放大真实路径成本，且会节流 I/O；调整不恢复高并发、不改变已有进程或全局设置，也不承诺 CPU/RSS 硬配额或风扇静音。沙盒拒绝 nice 时应如实记录；需要精确调度对照时仅对必要短测申请宿主机执行。

测试 profile 仍只优化 sha2 0.10.9 和升级核心，保留 debug assertions/overflow checks；生产 profile 不变。runner 记录每条命令耗时及30秒心跳。POSIX 正常、失败或中断后只回收本命令进程组；非 POSIX 不能声明同等进程树保证。HTTP fixture 持有实际服务 Child，正常及失败均收尾。

监护证据根的私有`umask 077`与被测子进程环境分开设置；子进程沿用本轮批准的构建/测试口径（本机既有口径为`022`），启动前记录实际nice、umask及并发值。监护不得因继承私有证据权限改变被测fixture语义；失败先核对环境及材料，不自动改权限或重跑。采样不等于CPU/RSS硬限额。

fixture在panic时应保留并输出新建根；成功清理只覆盖自身登记的合成材料。Linux startup的`TestSite`已按此保留，macOS reader/observer的诊断与保留范围见对应合同；历史已被旧Drop清理的现场如实标缺失，不补建冒充原件。

不清理未知目录、失败证据或冻结现场。中断保留未完成结论；不恢复旧进程或用旧日志接续成功。

## 调整后的验证

先运行 `python3 -m unittest discover -s scripts -p 'test_repository_*.py'`，验证分层、触发和 fail-closed 聚合；再检查两种测试配置的 Clippy、编译与测试清单，定向运行常规代表路径及恢复用例。完整常规门禁、穷举专项和远端 CI 未执行时必须明确待验证，不能将这些短测写成完整通过。

### 首轮完整验收（2026-10-23）

经项目所有者明确允许，在干净的 `dev / 86fe464` 上串行执行下列两条命令，各一次；常规20分钟、可靠性45分钟停止预算。沙盒预检发现 nice 被拒绝且实际为0，未启动受限环境下的完整门禁；仅为这两条门禁及本批资源采样/收尾使用宿主机权限，核实 nice=19、三个并发预算均为1。没有推送、触发CI、下载依赖、清缓存或操作冻结现场。

```bash
/usr/bin/time -l env CARGO_NET_OFFLINE=true GOPROXY=off GOTOOLCHAIN=local ./scripts/check-repo.sh
/usr/bin/time -l env CARGO_NET_OFFLINE=true GOPROXY=off GOTOOLCHAIN=local ./scripts/check-repo.sh --component reliability
```

| 层级 | 结果/墙钟 | user+system CPU | 进程树RSS采样峰值 |
| --- | --- | --- | --- |
| 常规 | 全部组件通过；874.28秒 | 673.14秒 | 1648MiB（约1.61GiB） |
| 可靠性 | Executor 2项、历史16项全部通过；1059.89秒 | 790.83秒 | 444.08MiB |

两层合计1934.17秒（32分14.17秒），相对旧入口6504.63秒约减少70.3%。旧入口与新入口的检查归属、调度及缓存状态存在差别，这是两次实际执行的场景对照，不是单一优化的等条件速度测量。两层均未触及预算，没有重跑；本批跟踪进程均已退出。预算监护包含收尾的实测分别875.14/1060.01秒，不能与命令墙钟混算。

资源为 `/usr/bin/time -l` 的命令树CPU计时，加每5秒采样的本批进程树RSS/CPU，采样可能错过短峰值。time报告的单进程最大RSS分别609.11/415.88MiB，不等于树内进程RSS总和。常规在Flutter测试期间采样到517.6% CPU、9个进程和1.61GiB RSS；可靠性CPU采样峰值94.3%。因此并发预算1和nice +19不代表整个工具树只用一个核心，也不保证固定RSS或风扇静音；本轮没有擅自修改Flutter并发参数。

常规最慢命令仍为workspace all-targets（696.50秒）；其中Executor常规组221.35秒、升级历史215.03秒、源库准备92.17秒，合计占整层约60.5%。可靠性Executor矩阵305.07秒、升级历史矩阵734.70秒。性能目标尚未收口，继续优化应针对这些实测路径，保留安全和恢复断言。

Manager 99项测试本轮执行通过；Go format/vet通过，race命令成功但5个有测试的包均命中Go测试缓存，不能称作本轮重新运行了这些测试体。若要求禁用该缓存，需要明确计划后再运行，不能自动追加 `-count=1`。本次未执行冷缓存测量、GitHub CI、Linux宿主运行、实机资格或生产observer容量实验。日志/资源/预算状态在本机 `/private/tmp/rlxgate-lx63tnw2/`：`routine.log`、`reliability.log`、`state.json`、两个 `*-resources.jsonl` 与 `analysis.json`。当前门禁结论只绑定 `86fe464`；随后结果文档修改仅执行文档/文本/diff检查，不拼成新revision完整通过。
