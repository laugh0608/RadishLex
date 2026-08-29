# RadishLex 协作约定

本文件为人工与 AI 协作者提供 RadishLex 的启动级长期约束。对话开始或结束总结时称呼项目所有者为 `萝卜SAMA`。

## 定位与优先级

- 本文件只保留任何任务开始时都必须知道、跨任务且跨阶段成立的约束，不承载当前里程碑、批次、日期、提交、证据编号、实机清单、临时门禁或下一步。
- 当前任务中项目所有者的明确要求优先于仓库默认流程；若要求会改变架构、协议、隐私、许可证、依赖、平台或运行时边界，仍应先说明影响并确认范围。
- 项目定位以 `README.md` 为准，许可条款以根 `LICENSE` 的 RadishLex Source-Available License 为准，文档导航以 `docs/README.md` 为准，当前阶段、顺位、停止线和“当前不做”只读 `docs/status/current.md`。
- 只读取当前任务需要的最少文档；历史事实需要追溯时再进入 `docs/devlogs/`、`docs/archive/` 或由 `current` 激活的临时专题。
- 详细任务推进、授权、文档分层和交付规则见 `docs/agent-collaboration.md`。

## 称呼与语言

- 对话开始或结束总结时称呼项目所有者为 `萝卜SAMA`。
- 默认使用中文说明、讨论和编写文档。
- 代码、技术标识、配置键、命令、路径和引用名保留原文。

## 协作原则

- 先判断请求属于回答、诊断、设计、实现、验证、发布还是系统操作；回答、诊断和评审不自动授权修改、提交、外部写入或系统动作。
- 开始任务先检查 Git 状态并阅读直接相关文档。若基线与用户给出的 commit、ahead 或 clean 状态不一致，不 reset、覆盖或清理，先报告差异。
- 架构、公共协议、隐私、加密、许可证、平台接入、FFI ABI、安装事务、依赖或阶段边界变更，实施前说明目标、边界、影响和验证方式并等待批准。
- 小型缺陷、文案、纯清理或不改变既有边界的验证补漏，在需求与范围明确时可以直接推进。
- 不同合理解释会明显改变用户行为、数据模型、兼容边界、外部影响或风险时先澄清；已批准方案发生实质范围变化时重新确认。
- 从根因、长期维护、系统一致性和可验证性出发，完整覆盖真实主路径；不得用占位、半成品、玩具实现、异常吞噬或层层 fallback 冒充交付。
- 在满足需求和质量的前提下控制修改范围；不做无关重构，不为“架构感”增加没有真实收益的抽象。
- 工作区可能含用户或并行任务的改动；只修改本批文件，保留无关变化，遇到重叠先查明来源与影响。

## 必须单独授权的操作

- 安装或更新依赖，下载 SDK、模型或数据，改变全局工具链，或新增会修改 lockfile、系统环境和外部缓存的命令。
- 启动长期服务或 GUI，修改系统输入法、权限、Keychain、证书、会话、全局配置、`/usr`、`/var`、dpkg、systemd 或 Fcitx profile/autostart。
- kill、restart、合成按键、自动点击或其他可能干扰用户会话、进程和人工验收的动作。
- 推送、创建或更新 Pull Request、Release、tag、远端设置，以及任何对外发布、部署或消息发送。
- 授权只覆盖当前任务中已说明的目标、命令、环境和影响；命令、目标或运行影响发生实质变化时必须重新确认，不沿用历史会话授权。
- 可直接读写仓库，运行只读 Git、既有格式化、静态检查、测试和 repository-only smoke，并做范围清楚的小型本地提交。
- 默认先在沙盒内验证；权限、网络、证书或 PATH 限制导致关键构建或测试失真时，只为必要验证申请最小范围提权，提权不扩大任务授权。

## 任务文档路由

| 任务 | 优先读取 |
| --- | --- |
| 当前阶段、停止线、下一步 | [当前状态](docs/status/current.md)；不足时再读[路线图](docs/roadmap.md) |
| 项目定位、架构与目录 | [项目说明](README.md)、[技术方案](docs/technical-plan.md)、[仓库结构](docs/repository-layout.md) |
| Agent 协作、授权与验证 | [Agent 协作与执行规则](docs/agent-collaboration.md)、[文档入口](docs/README.md) |
| Engine、runtime、ranker、userdb | [Engine 边界](docs/engine-boundary.md)、[Rime adapter](docs/engine-rime-adapter.md)、相关专题 |
| 隐私、同步、加密与设备 | [隐私与同步](docs/privacy-sync.md)、[加密边界](docs/crypto-boundary.md)、同步与密钥专题 |
| FFI、Manager 与平台壳 | [FFI 边界](docs/ffi-boundary.md)、Manager 专题、对应平台 Boundary |
| 安装、升级、实机与系统操作 | 对应平台安装 Boundary、`docs/runbooks/` 及 `current` 明确引用的现场入口 |
| 分支、贡献与发布流程 | [贡献指南](CONTRIBUTING.md)、[分支与 PR ADR](docs/adr/0001-branch-and-pr-governance.md) |

## 项目长期边界与 Engine 边界约束

- 输入、候选、学习和 commit 热路径必须本地可用；不要把 RadishLex 做成云端实时输入法 API，同步后端不得进入按键热路径或参与明文合并。
- `ime-core` 管理 session、composition、candidate、commit 和 engine trait；具体引擎生命周期隔离在 adapter，runtime 组合 engine、ranker、userdb 与 privacy。
- `ime-userdb` 是本地学习、删除和同步投影真相源；`ime-ranker` 负责确定性重排与 explain；`ime-sync` / `ime-crypto` 负责客户端协议、合并、设备和密钥。
- `server/sync-server` 只存密文与必要元数据；Flutter Manager 负责设置与管理入口但不进入热路径；平台壳只处理系统生命周期、按键、候选、commit 和 FFI。
- Linux 输入平台优先 Fcitx5，候选使用 input panel；Wayland 不自造浮窗协议。Linux product transaction 不接管输入业务或用户 XDG 数据。
- 模块、目录和平台职责的完整定义以技术方案、仓库结构及对应 Boundary 为准，不在根入口复制实现细节。

## 隐私、数据与错误红线

- P0 数据永不同步，也永不学习：密码、支付、证件、secure text 与隐私模式输入均属于此边界；P1 原始选择和上下文默认只在本地；P2 用户词、摘要和配置只作为端到端加密对象；P3 才可公开下载。
- 删除必须保留 tombstone 或等价的防复活语义并覆盖旧设备、离线冲突和备份恢复；新设备由已有设备或恢复码授权，撤销后轮换后续对象密钥或 key epoch。
- 日志、fixture、截图、诊断和 golden 禁止使用真实输入历史、联系人、密码、证件、支付、密钥、恢复码或生产凭据；只使用合成数据、虚构身份和脱敏聚合。
- FFI 必须明确 ABI、所有权、生命周期、线程、UTF-8、释放和错误语义；crypto、sync、ranker、userdb、平台和安装错误不得静默吞掉或伪装成功。

## 安装、实机与证据红线

- 安装、升级、修复、移除和回滚默认保留用户数据；任何例外必须由明确产品合同和用户授权定义。
- 载体、依赖、安装状态、receipt、staging、guard 与运行中程序必须从实际系统状态交叉验证；脚本退出码、detached 声明、外部 trigger 或分散证据不能单独证明事务完成。
- mutation、retry 和恢复前重新验证输入身份、私有 staged relationship、静止条件和授权；未知状态、半配置或身份、owner、mode、link、hash 漂移均失败关闭并保留现场。
- 被 `current` 或 runbook 标记为冻结的 guest、staging、backup、userdb、导入导出文件、临时服务和证据不得复跑、覆盖、恢复、清理或混用。
- UTM、guest-agent、隔离网络、具体载体、transaction、startup gate、验收矩阵和当前资产规则只以对应 Boundary、runbook 与 `current` 为准；根入口不保存现场副本。
- 静态检查、合成执行器、repository-only 测试和分散实机证据不得表述为真实系统、连续事务、端到端安全或正式发布已经通过。

## 实现、文件与验证

- 代码优先直观命名、早返回、明确类型和浅层职责；不写空转 wrapper、晦涩 factory、动态字符串状态、无目的 fallback 或只转发参数的抽象。
- 单个源码原则上不超过 1500 行，接近 1000 行优先按真实职责拆分；`src/` 与 `scripts/` 使用浅层目录，committed 相对路径默认不超过 180 字符。
- 仓库文本使用 UTF-8 无 BOM、规定换行和文件末尾换行；非 Markdown 不留尾随空格，不为通过门禁批量改写第三方或需保留原格式的材料。
- 每个可分割步骤执行与风险匹配的验证；核心、数据一致性、安全、隐私、用户可见行为和阶段交付扩大门禁，fast/quick 入口不能作为最终门禁。
- Rust 执行格式化、check、相关 test / clippy；Go 执行 gofmt 与相关 test；Flutter 执行 dart format、analyze 与相关 test；平台壳覆盖适用 build / smoke，真实行为保留人工或实机证据。
- 交付时说明实际完成、验证结果、未验证内容和残余风险；没有执行的检查、系统动作或人工验收不得写成通过。
- 架构、协议、隐私、平台、目录、里程碑或验证口径变化时同步对应真相源；每周重要推进追加 Asia/Shanghai 周志。
- 文档改动至少运行 `./scripts/check-docs.sh`、`./scripts/check-text-files.sh` 和 `git diff --check`；阶段性或高风险交付再运行 `./scripts/check-repo.sh`。

## Git 与入口维护

- `dev` 是常态开发与集成分支；串行推进的普通任务直接在 `dev` 开发和提交，不要求主题分支、Pull Request 或额外 worktree。
- 只有项目所有者明确要求、外部贡献、并行写入、确有隔离价值的高风险改动或 hotfix 才创建主题分支；Agent 不自动创建 `codex/*` 等临时分支。
- `dev` 当前不启用 branch protection，直接 push 不触发 GitHub Actions；直接开发按改动范围完成本地验证，需要评审或隔离时再通过 Pull Request 合入 `dev`。
- 提交使用当前用户身份和 Conventional Commits，不添加 AI 署名；代码、文档和治理按主题拆分，提交前复验范围与必要门禁。
- 禁止未授权 `git reset --hard`、checkout 覆盖、force push 或其他破坏性操作；推送、PR、Release、tag 和远端设置始终另行授权。
- 一条规则只有同时满足“跨任务成立、跨阶段成立、必须启动即生效、无法仅由任务路由可靠承载”时，才进入 `AGENTS.md` / `CLAUDE.md`。
- 稳定但按需读取的协作细则进入 `docs/agent-collaboration.md` 或对应专题；阶段状态、临时门禁、“当前不做”和批次事实进入 `current`、runbook 或 devlog，不复制回根入口。
- `AGENTS.md` 与 `CLAUDE.md` 必须逐字一致；修改任一文件时同步另一份，并通过文档检查器验证。
