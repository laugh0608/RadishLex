# 仓库门禁与测试运行

本文说明常规质量、可靠性专项和真实资格验收的职责、触发及证据。它不授权真实安装、GUI、冻结资产、生产资格实验或完整验证；当前结果见 current 与周志。

## 正式分层与授权

| 层级 | 内容 | 适用条件 |
| --- | --- | --- |
| 常规质量 | hygiene、Rust、contracts、Go、Manager 五个组件 | PR 必需；本地日常按风险定向验证 |
| 可靠性专项 | 安装执行及升级历史的全故障点、全进程退出和深生命周期组合 | 受影响 PR 必需，发布候选全部执行 |
| 平台与真实资格 | 实际安装、连续事务、真实程序资格、人工输入、容量实验 | 按对应 Boundary/runbook 单独授权，不能由前两层代替 |

`./scripts/check-repo.sh` 执行五个常规组件。它通过只能声明常规质量通过，不表示可靠性专项或真实资格已通过。正式 PR 结论由同一被验证 revision 的常规组件和所有适用专项共同形成；不能用旧日志、另一 revision 或局部结果拼成当前通过。

完整常规门禁、完整可靠性专项及完整平台回归，每轮仍须提前说明必要性、具体命令、环境、预计耗时、资源影响并取得明确允许。不能通过组件入口拆分绕过授权；定向静态检查、具体用例和 repository-only smoke 按协作规则执行。失败或中断不自动授权重跑。

热缓存常规入口约 5 分钟、常规 CI 约 10 分钟是优化验收目标，尚非已测承诺；冷编译单列。相关 PR 的可靠性专项可超过该目标。CI timeout 是失败上限，不是性能达标证据，超时不得改为成功。

## 入口和唯一归属

`./scripts/check-repo.sh --plan` 只展示计划，不执行检查。`--component` 选择职责明确的组件，不是 quick 或完整验收替身；旧 `--skip-*` 参数不再提供。

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

`scripts/reliability-cases.json` 固定18个穷举父测试（Executor 2、升级历史16）；执行前先发现清单并严格对照，空选择、漏项或新增未登记项均失败，防止过滤器误写后零测试假绿。专项用 `cargo test ... --features reliability-matrix ... reliability_matrix_` 精确选择父测试；普通测试不重复执行。ignored 子进程入口仍由父测试通过 `--exact` 调用，禁止全 workspace `--ignored` 或 `--all-features`。真实 qualification-harness、observer 三项容量实验仍按各自合同处理，不能借可靠性 feature 自动启动。

历史 fixture 的普通后续学习由80条减为1条，每个样本仍独立创建。80条记录保留为专门回归，明确断言源库长度与旧 receipt 不同，且历史核验、81个活动词与 tombstone 保留。不得通过共享可变 fixture、缓存 hash、省略 fsync 或减少生产实物核验提速。

### 取消 proof 的计算复用

取消 release、archive、source 的严格编码原来会递归校验嵌套 request/preparation；父级先 `validate` 再为长度调用 `encode`，索引派生和材料比对又重复这棵树。现在每次验证调用用内部 `EncodedCancellationProof` 绑定已校验的不可变模型与 canonical 字节；历史 proof 严格解码后直接保留二者，同次派生索引与核对材料复用计算。archive/source 与 data receipt 的长度取自本次已校验编码，不重复执行相同的嵌套校验。

该内部类型不保存文件身份、摘要或目录观察，不跨检查点替代实物核验。公共编码/解码仍完整拒绝无效嵌套对象、非 canonical 内容及超限材料；持久化格式、256 KiB 上限、授权/资格、guard、hash、owner/mode/link、fsync、原件保留与恢复顺序不变。每个检查点仍重新读取实物；历史 proof/索引的前后身份及最新 locator 复核保留。

`load_previous_inventory` 与 `verify_inventory_objects` 在 released 前驱路径确有再次遍历历史链的情况；这两次都是可观察文件状态的检查，期间还调用外部 hasher，不能把它们按纯函数去重。当前只消除同次调用内不变模型的重复编码/结构校验，不缓存历史链核验成功。定向测试继续覆盖同字节 inode 替换、旧祖先漂移、hash 期间索引替换、回调篡改、资格拒绝和真实退出后的新 guard 恢复。

2026-10-23 对同一连续取消代表用例作宿主机 ABBA 对照，两种固定二进制各2样本，均为并发1、nice +19、独立合成目录，不含编译或采样器：

| 指标 | 本次优化前 | 优化后 | 减少 |
| --- | --- | --- | --- |
| 平均墙钟 | 19.486秒 | 8.322秒 | 57.3% |
| 平均CPU | 18.537秒 | 7.430秒 | 59.9% |

原始输出、二进制 SHA-256、独立样本与环境约束见本机 `/private/tmp/rlxcpu-8bj3cqiu/comparison-final.json` 和同目录 `compare-final.py`；本机临时证据不随仓库分发。该结果仅对应 `representative_cancellation_lifecycle_continuous_cancellations_keep_null_and_legacy_data_predecessors` 的现有代表选择，不是完整前驱矩阵或整仓门禁耗时。深历史仍需逐项核验；整仓常规、完整可靠性和 CI 的最终耗时及 Linux 运行结果待另行验证。

## CI 触发与聚合

`PR Checks` 仍只监听目标为 dev/master 的 PR，普通分支 push 不触发。`validation_impact.py` 使用整个 PR 的 merge-base diff，关闭 rename 合并以同时检查删除和新增路径，并沿 workspace 的普通、开发、构建、target 和继承 path 依赖向消费者传播。

安装/升级、userdb、FFI 及相应 macOS 包受影响时要求可靠性专项；工具链、Cargo/lockfile、验证脚本、平台桥及未知路径从严要求。纯文档、已分类 Manager Dart 或 sync-server 改动不单独触发安装恢复专项，但全部常规质量仍必需。空 diff 或无法完成判定不能豁免。

`.github/workflows/quality.yml` 是 PR/tag 共用质量工作流；Rust/原生覆盖 Linux 与 macOS，Go 和 Manager 独立并行。Rust固定1.96.0、Flutter固定3.44.0，Go依go.mod；Cargo 缓存只复用构建产物，Cargo 仍核验输入，不复用门禁结果。首次冷编译/runner差异须单列。

`Candidate Quality` 名称及远端 ruleset 绑定保持不变。仅明确不适用的 reliability 可为 skipped；其他缺失、失败、取消、跳过均拒绝。发布 tag 调用同一质量工作流且强制可靠性专项，避免旧 Release Checks 缺少独立语言质量检查；它仍不代表真实平台发布资格。

## 调度、构建与进程收尾

`run-low-impact.sh` 保留 `CARGO_BUILD_JOBS=1`、`RUST_TEST_THREADS=1`、`GOMAXPROCS=1` 与 `nice +19`，移除强制 `taskpolicy -b`。后台策略实测会放大真实路径成本，且会节流 I/O；调整不恢复高并发、不改变已有进程或全局设置，也不承诺 CPU/RSS 硬配额或风扇静音。沙盒拒绝 nice 时应如实记录；需要精确调度对照时仅对必要短测申请宿主机执行。

测试 profile 仍只优化 sha2 0.10.9 和升级核心，保留 debug assertions/overflow checks；生产 profile 不变。runner 记录每条命令耗时及30秒心跳。POSIX 正常、失败或中断后只回收本命令进程组；非 POSIX 不能声明同等进程树保证。HTTP fixture 持有实际服务 Child，正常及失败均收尾。

不清理未知目录、失败证据或冻结现场。中断保留未完成结论；不恢复旧进程或用旧日志接续成功。

## 调整后的验证

先运行 `python3 -m unittest discover -s scripts -p 'test_repository_*.py'`，验证分层、触发和 fail-closed 聚合；再检查两种测试配置的 Clippy、编译与测试清单，定向运行常规代表路径及恢复用例。完整常规门禁、穷举专项和远端 CI 未执行时必须明确待验证，不能将这些短测写成完整通过。
