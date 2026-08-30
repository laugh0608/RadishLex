# RadishLex 文档入口

本文是项目文档的导航与职责说明，面向维护者、实现者和审阅者。它不记录当前批次、提交清单、证据编号、实机现场或命令流水；这些易变事实分别进入当前状态、runbook 和周志。

## 项目定位

RadishLex（萝卜词核）是由 Rust 输入核心、Go 自部署同步后端、Flutter Manager 与平台原生薄壳组成的源代码可见中文输入系统。输入热路径必须本地可用；服务端默认不可信，只承担密文同步、备份、设备和包分发；平台壳不承载 userdb、排序、同步或隐私真相源。许可条款以仓库根 `LICENSE` 为准。

## 先读什么

- [当前状态](status/current.md)：当前里程碑、批次、冻结基线、停止线、下一步和本周周志入口；这是唯一阶段快照。
- [Agent 协作与执行规则](agent-collaboration.md)：任务类型、方案确认、授权范围、文档归位、风险分层验证和根协作入口维护细则。
- [技术方案](technical-plan.md)：稳定架构、模块职责、输入链、平台策略和验证分层。
- [产品交付路线图](roadmap.md)：长期里程碑、交付物和退出标准，不记录逐批进度。
- [仓库结构](repository-layout.md)：实际目录、模块职责与尚未落地的边界。
- [隐私与同步](privacy-sync.md)：数据分级、密钥、删除、恢复和威胁模型。

处理任务时先遵循根协作入口并读取当前状态，再按任务选择最少的稳定专题；涉及授权、范围、验证或文档归位时进入 Agent 协作专题。只有 `current` 明确引用的临时专题才进入日常阅读链。

## 按职责查找

- `docs/adr/`：需要长期追溯的架构与产品决策。
- `docs/runbooks/`：可重复操作步骤、环境前提、验证方式和停止条件。
- `docs/devlogs/`：按 Asia/Shanghai 周记录推进事实、命令、提交与历史流水。
- `docs/remediation/`：由当前状态显式激活的临时整改专题。
- `docs/archive/`：已关闭并退出默认阅读链的历史材料。
- `docs/agent-collaboration.md`：稳定但无需每次启动全量读取的协作与执行细则。
- `docs/*.md`：稳定的架构、协议、平台、隐私、Guide、Boundary 或 Reference。
- 各组件 `README.md`：组件职责、就地开发入口和局部验证方式。

Linux 当前安装工作从 [Linux 安装维护边界](linux-installation-maintenance-boundary.md)、[L6 package matrix](runbooks/linux-l6-package-matrix.md) 与 [L6 资产生命周期](runbooks/linux-l6-asset-lifecycle.md) 进入；输入平台与 Manager 分别见 [Linux Fcitx5 平台边界](linux-fcitx5-boundary.md) 和 [Linux Manager 本地验收边界](linux-manager-local-acceptance.md)。其他专题从当前状态、技术方案或仓库结构继续下钻。

## 维护规则

- `AGENTS.md` 与 `CLAUDE.md` 只保存跨任务、跨阶段且必须启动即生效的长期约束，并保持逐字一致；详细协作流程进入 Agent 协作专题。
- 当前阶段、临时停止线和顺位只更新 `docs/status/current.md`；详细流水只追加当周 devlog，设计事实更新对应稳定专题。
- 同一状态事实不复制到多个稳定入口。入口只给判断和索引，专题解释边界，runbook 描述操作，devlog 保存历史。
- 新增或大改文档需在开头说明用途、读者与不包含内容。优先更新既有文档，关闭后的临时材料移入 archive。
- 架构、协议、隐私、平台、目录、里程碑或验证口径变化要同步对应真相源；普通阶段推进不触碰协作文件。
- 文档改动至少运行 `./scripts/check-docs.sh`、`./scripts/check-text-files.sh` 与 `git diff --check`；文档检查器同时验证两份根协作入口逐字一致，阶段性或高风险交付再运行 `./scripts/check-repo.sh`。
