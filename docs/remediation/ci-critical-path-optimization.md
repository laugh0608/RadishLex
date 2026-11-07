# CI 精简专题

本文面向维护者与评审者，记录 PR 检查的耗时归因、实施切片和验收办法。首个切片已通过一轮 GitHub Actions 验证。门禁规则和执行顺位分别见[仓库门禁手册](../runbooks/repository-validation.md)和[当前状态](../status/current.md)。

## 耗时归因

PR #12 的首次完整通过用时 **59 分 34 秒**。macOS 常规作业占 **32 分 24 秒**，恢复专项在它结束后才启动，又用 **26 分 35 秒**。Ubuntu、Go 和 Flutter Manager 都在 macOS 常规作业结束前完成。两台 Rust runner 和恢复 runner 的 Rust 缓存均未命中。各步骤和测试组的实测见[周志](../devlogs/2026-W45.md#2026-11-07pr-12-合并与-ci-耗时基线)。

macOS 装配、原生及仓库合同共用 9 分 12 秒。这段检查确保 Installer 消费本轮装配并验证的 `InstallPayload`。长测试的主要成本来自实际执行。macOS 常规 workspace 中，安装执行器和升级历史两组共约 9 分 51 秒。恢复专项里，升级历史单组约 15 分 31 秒。构建缓存能减少编译时间，缩短测试执行和串行等待需要分别处理。

## 首个实施切片

1. `macos-contracts` 在同一 runner 和 checkout 中完成 Manager、InputMethod、`InstallPayload` 装配及原生与仓库合同。Ubuntu 在自己的作业内先跑合同，再跑常规 Rust。
2. `rust-macos` 和适用的 `reliability` 只依赖 `macos-contracts`，成功后在独立 runner 并行。PR 使用本轮合并候选，tag 使用对应 revision。测试作业检出同一候选并各自构建，前置作业的产品包留在产生它的 runner。
3. `Candidate Quality` 继续聚合前置、两端常规检查及适用专项。前置失败或取消时阻断下游并让聚合失败。专项仅在影响分析明确判定不适用时可跳过，发布 tag 继续强制执行专项。

按 PR #12 各段耗时推演，前置约 11 分钟，后续两条路径的较长者约 27 分钟，关键路径估计为 38 分钟。拆分增加一次 runner 准备，实际对照同时记录墙钟与各 runner 的总运行分钟数。

## 对照与验收

- 候选方案使用同一源码和测试输入对照，记录 runner 镜像、架构、工具链、候选 SHA、队列时间与缓存命中。分别记录冷构建和热缓存，按 `Validation Impact` 开始到 `Candidate Quality` 完成统计执行时间，同时保留从 run 创建算起的等待时间。
- 逐项对照旧、新作业的命令、平台、Cargo feature、测试 target 和断言，核对 2 / 16 / 4 共 22 个恢复测试组在同一候选上全部通过。保留独立 fixture、真实子进程退出、授权撤销、材料身份与恢复断言。用缺失装配输入、缺失作业结果和失败作业的合成对照验证门禁拒绝。
- 初步目标为冷构建整轮低于 45 分钟。记录两组的分段耗时和 runner 总分钟数，由评审确定可接受的资源差额。任何覆盖差异须先说明其必要性和等价证据。定向检查与对照命令单列，完整运行按[协作规则](../agent-collaboration.md)取得当轮授权。

本地对照工作流依赖、候选 checkout、装配顺序、组件命令和原 22 项专项清单。聚合单测覆盖前置缺失、失败、取消、跳过及明确不适用的专项。隔离副本缺少产品装配时，payload 脚本按预期拒绝。

PR #13 的 Rust 缓存未命中样本在 **37 分 52 秒**通过 `Candidate Quality`，比基线快 **21 分 42 秒**。两个 macOS 长作业重叠 20 分 56 秒，三项 macOS 作业累计 58 分 18 秒。2 / 16 / 4 共 22 个恢复测试组通过。这一轮达到低于 45 分钟的目标。runner 镜像修订与基线有差异，热缓存表现与耗时波动仍待测。[PR #13 实测](../devlogs/2026-W45.md#2026-11-07-pr-13-远端验证)

## 十分钟日常门禁

项目所有者提出将 CI 压到十分钟以内，并简化日常入口。现有完整口径下，PR #13 的 Ubuntu workspace 测试耗时 11 分 35 秒，macOS 装配与合同作业耗时 13 分 36 秒，恢复专项的升级历史测试体耗时 13 分 28 秒。已有本机记录中，常规全量也用时超过十二分钟。十分钟方案需要重新界定哪些检查阻断日常 PR。[PR #13 日志](https://github.com/laugh0608/RadishLex/actions/runs/42345747026) · [本机门禁记录](../runbooks/repository-validation.md#正式分层与授权)

- 日常进入 `dev` 的 PR 用一个 Ubuntu 作业，调用本地共用的 `./scripts/check-repo.sh --component dev`。它执行差异与 hygiene、workspace Rust fmt 和 Clippy、`ime-core` 的 `session_lifecycle`、安装与升级核心库单测，以及原 Go 与 Manager 组件。
- 阶段晋级 `master` 与发布 tag 继续要求各自候选的完整平台质量和适用的恢复专项。日常 `dev` 的绿灯只证明上述检查范围。macOS 装配、全 workspace 测试及恢复矩阵的完整结果在阶段晋级前取得。
- 先核对入口、路由、工作流聚合和失败语义，再用一轮 Rust 缓存未命中的 `dev` PR 测实际用时。记录作业运行与排队时间。作业预算十分钟，超时视为失败。仅按需要逐轮批准远端或完整验证。

PR #13 的 `508ad84` 只运行 `Dev Quality`，从创建到完成 **3 分 32 秒**，共用入口执行 **2 分 30 秒**。Rust 缓存未命中，Go 与 Flutter 缓存命中。日志显示 `ime-core --lib` 执行零项测试。[首轮日常 CI](https://github.com/laugh0608/RadishLex/actions/runs/42349490232)

改用 `session_lifecycle` 的 `05f3d10` 复测通过，整轮 **1 分 43 秒**，共用入口执行 **48 秒**。三项输入会话测试和 122 项安装、升级核心测试实际通过。Rust、Go 与 Flutter 缓存均命中。[修正后的日常 CI](https://github.com/laugh0608/RadishLex/actions/runs/42350046353)

## 后续切片

- **去重跨平台合同**。先列出两台 runner 实际运行的命令，核对每条的 OS、工具链和文件系统依赖。能证明两端执行同一份平台无关合同的检查归属一个作业。macOS 原生与 Linux 平台相关合同继续在各自环境执行。
- **评估恢复专项分片**。并行编排测完后，若升级历史仍占主导，按现有测试组名称划分独立分片。聚合校验分片并集严格等于已登记清单，每个测试组恰好执行一次。先验证 fixture 与子进程隔离，再衡量新增冷编译、并发开销和 runner 分钟数。
- **评估低风险改动的门禁选择**。纯文档、Go 与 Manager 等变更类型的进一步筛选单独审阅。先证明路径归属、生成物和依赖传播，再用删除、重命名、未知路径及工作流变更做负向对照。待方案批准后更新适用门禁规则。
