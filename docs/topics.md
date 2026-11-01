# 能力专题与产品交付地图

本文面向维护者、实现者与审阅者，连接整体产品规划、跨阶段能力、既有子专题和任务 / 验收来源。它不修改 Boundary、ABI、安装事务、冻结证据、平台顺位或操作授权，也不保存另一份进度表。

整体目标由[项目说明](../README.md)和[技术方案](technical-plan.md)定义；M0–M5、后续平台与 Future Topic 的范围和退出标准见[路线图](roadmap.md)。实际阶段、近期顺位与停止线只从[current](status/current.md)进入。

## 层级与来源

- 大专题按跨阶段能力划分，不能与 crate、平台目录或里程碑一一等同；一次产品里程碑通常整合多条能力。
- 子专题复用正式 Boundary、设计文档、Reference 或稳定章节。runbook 是操作与验收来源，devlog 是历史证据，不重复创建一套同名任务卡。
- M5-P01–P05、P05A/B/C、L1–L6、B1–B6、V1–V5、REV 和 Q2/F0/W0 保持各自原含义；交付批次、阻塞、验证计划与审阅问题不重新编号为另一层专题。
- current 维护阶段判断、跨专题顺位和冻结边界；子专题维护范围、局部实施与关闭条件；runbook / 周志维护精确版本、环境与证据。地图只登记主要归属与协同。

里程碑退出、实现、自动验证、实机验证、用户开放与公开发布分别判断。旧平台、旧候选或旧 pair 的证据不能自动覆盖新候选；地图中的验收入口不构成运行授权。

## 大专题

| 能力主线 | 价值与范围 | 主要阶段 / 协同 | 目标范围完成条件 |
| --- | --- | --- | --- |
| 本地输入与引擎 | session、composition、candidate、commit、engine adapter 与 runtime 热路径 | M0/M1；所有后续平台复用，个人化与平台接入协同 | 正常输入、取消、重置、多 session、透传与错误有真实链路证据；输入不依赖网络或 Manager |
| 个人化、隐私与本地数据 | 学习、排序、userdb、删除 / 恢复、并发、迁移与隐私策略 | M2 及后续平台；同步和安装维护协同 | 独立预期、原子提交、隐私零学习、删除防复活与重启成立，平台复验单列 |
| 平台输入与管理体验 | FFI、macOS / Linux 薄壳、Manager 本地管理、设置与诊断 | M1/M2/M5 及后续串行平台；输入和本地数据协同 | 对应平台的真实输入、生命周期、同库管理、失败与辅助功能按原范围验收；不把能力桥当完整 IME |
| 加密同步与服务连接 | P2 密文、设备 / 密钥 / 恢复、同步编排、服务与官方 / 自部署选择 | M3 与后续同步开放；Manager、平台 key backend 和 userdb 协同 | 合成资格、平台 backend、双客户端、目标部署与用户总 gate 分开，不从 M3 历史退出推导真实用户同步开放 |
| 安装、升级与产品载体 | 程序 / 数据事务、package、source / target 身份、startup gate、恢复与数据保留 | M4/M5-P05；平台、本地数据与供应链协同 | 实际载体、原始状态、连续事务和独立环境证据完整；静态声明、CLI 成功或分散现场不能代替 |
| 质量、供应链与发布 | 输入质量、验证分层、来源 / 许可、依赖身份、候选交付与统一发布评审 | 贯穿各阶段；按原平台顺序整合其他主线 | 原验收与适用门禁、未验证面和发布身份可追溯；自动通过不等于实机、签名、公证或公开发布 |

## 子专题主要归属

下表每个正式来源入口指定一个主要归属；其他能力引用它时只登记协同，不复制合同或状态。表中列出主要阅读入口，不替代技术方案的完整索引。

| 主要归属 | 子专题 / 原来源 | 任务、验收与证据入口 |
| --- | --- | --- |
| 本地输入与引擎 | [Engine Boundary](engine-boundary.md)、[Rime adapter](engine-rime-adapter.md)、[runtime](technical-plan.md#ime-runtime) | 原 Engine 验证、[native smoke](runbooks/rime-native-smoke.md)与各平台输入 runbook；保留成熟引擎隔离与 clean-room 边界 |
| 个人化、隐私与本地数据 | [个人化学习](personalization-learning.md)、[userdb](technical-plan.md#ime-userdb)、[ranker](technical-plan.md#ime-ranker)、[隐私与同步分级](privacy-sync.md) | 原学习 / 删除 / 排序标准、[CLI 证据](cli-learning-evidence.md)、[个人化实机](runbooks/macos-r01b-personalization-acceptance.md)和各平台复验；P0/P1/P2 边界共享但不重复定义 |
| 平台输入与管理体验 | [FFI](ffi-boundary.md)、[字典接口](ffi-dictionary-reference.md)、[macOS IMK](macos-inputmethodkit-boundary.md)、[Linux Fcitx5](linux-fcitx5-boundary.md) | M1 / M5-P02/P03；[平台调用合同](runbooks/ffi-platform-call-contract.md)、[macOS 开发验收](runbooks/macos-inputmethodkit-development.md)及 Linux 原验收，不扩大 ABI 或平台支持 |
| 平台输入与管理体验 | [Manager UI](manager-ui-boundary.md)、[设置与诊断](manager-settings-diagnostics.md)、[本地验收](manager-local-acceptance.md)、[Linux 本地验收](linux-manager-local-acceptance.md) | M2 / M5-P04；[Manager 产品验收](runbooks/macos-m2-manager-product-acceptance.md)，本地管理与同步资格分列 |
| 加密同步与服务连接 | [密码边界](crypto-boundary.md)、[密钥与设备生命周期](sync-key-management.md)、[恢复流程](production-recovery-flow.md)、[私钥 backend 策略](platform-private-key-backend-strategy.md) | 原 ADR / 算法 profile、平台 key runbook 和 backend 资格；compiled、runtime、非导出、hardware-backed、产品资格与用户总 gate 分列 |
| 加密同步与服务连接 | [payload](sync-payload.md)、[同步编排](sync-orchestration.md)、[服务 API / 存储](sync-server-api-storage.md)、[客户端连接](client-server-connection-plan.md) | M3 与后续同步开放；原矩阵、[本地 HTTPS](runbooks/sync-server-local-smoke.md)、[生产部署](runbooks/sync-server-production-deployment.md)，端点 / 同步域隔离不能由本地探针替代 |
| 加密同步与服务连接 | [Manager 同步入口](manager-sync-entry-boundary.md)、[恢复与设备授权](manager-recovery-device-auth-flow.md)、[同步产品状态](manager-sync-product-status.md)、[readiness 场景](manager-readiness-scenarios.md) | 原资格链和场景；局部 readiness 不能覆盖 current 的真实用户同步关闭状态 |
| 安装、升级与产品载体 | [macOS 产品包](macos-product-package-boundary.md)、[程序安装事务](macos-installation-transaction.md)、[数据升级](macos-data-upgrade-coordinator.md)、[Installer](macos-installer-app-boundary.md)、[升级 FFI](ffi-product-upgrade-reference.md) | M4；原程序 / 数据 receipt、[产品装配](runbooks/macos-product-assembly.md)与[用户域验收](runbooks/macos-installer-user-domain-acceptance.md)，旧 39/40 冻结现场从 current 进入 |
| 安装、升级与产品载体 | [Linux 安装维护](linux-installation-maintenance-boundary.md) | M5-P05A/B/C；[L6 matrix](runbooks/linux-l6-package-matrix.md)、[资产生命周期](runbooks/linux-l6-asset-lifecycle.md)与原 L1–L5 自动门禁；连续 L6、代表 crash、产品行为和独立 P05C 各自取证 |
| 质量、供应链与发布 | [验证分层](technical-plan.md#验证分层)、[里程碑证据规则](roadmap.md#里程碑证据规则)、[发布载体](runbooks/macos-release-carrier.md) | [门禁手册](runbooks/repository-validation.md)、来源 / 许可清单与各批周志；完整门禁仍按逐轮授权执行 |

## 当前激活的整改与交付子专题

仅跟随 current 激活的材料；地图不额外激活历史整改。接续任务顺序见[Linux M5 交付收口](status/current.md#下一阶段linux-m5-交付收口)，系统资源与冻结现场见其相邻停止线。

| 主要归属 | 子专题 / 原来源 | 接续任务与关闭依据 |
| --- | --- | --- |
| 安装、升级与产品载体 | [M5 基线审计](remediation/linux-m5-delivery-baseline-audit.md)、[source-first 合同](remediation/linux-m5-source-first-contract.md) | B1–B6、S/R/T 与 V1–V5 保留。先审 source / 新 pair、实际构建与资源资格，再形成新候选连续 L6、产品行为及独立 P05C；装配与收证入口见[source-first runbook](runbooks/linux-source-first-assembly.md)，实现、自动验证与真实资格分别判断；执行顺位只读current |
| 安装、升级与产品载体 | [macOS WAL 升级恢复](remediation/macos-wal-upgrade-recovery-2026-09.md)、[源库准备](remediation/macos-wal-source-preparation-design.md)、[取消兼容](remediation/macos-preparation-cancellation-compatibility.md) | 程序 / 数据恢复与 action 10–13 的原范围、B 段及最终取消缺口；[联合验收](runbooks/macos-rev01-rev02-acceptance.md)与周志，不设置为 Linux M5 的新增退出前置 |
| 安装、升级与产品载体 | [source39 资格](remediation/macos-source39-qualification-plan.md)、[schema 观察](remediation/macos-source-schema-observation-contract.md)、[资格 reader](remediation/macos-cancellation-qualification-reader-contract.md) | Q2、F0、W0、catalog / issuer 与真实程序资格各自满足原条件；合成结果和方案不能解除当前冻结现场与最终取消关闭状态 |
| 质量、供应链与发布 | [产品审阅 REV](remediation/product-review-2026-09.md) | 跨输入、隐私、数据、安装和工具链的整合审阅；每项关闭条件在原专题，具体修复由对应能力主线承担，不复制第二张状态表 |

M5 审计中旧 VM 数量、路径与 Move/adopt 候选是当时的资源输入；恢复系统工作前必须先读 current 的最新资源与路径绑定条件，不能直接按旧审计操作。平台恢复、VM、完整门禁、真实输入及发布都不是本页附带授权。

## 近期工作的阅读链

1. source-first：current → 基线审计 B1 → source-first 的 S/R/T、首批范围和 V1/V2；装配/seed/target入口见runbook，V3–V5另有环境、材料审阅和运行前置，当前授权只读current。
2. 构建与资源：current 的系统停止线 → B2/B3 → 源码锁、实际 L1–L5 与新 source 资格；不把旧 pair 声明或路径迁移当新 target 通过。
3. M5 产品收口：source / pair 资格 → 代表 crash 与连续 L6 → 当前候选隐私 / 学习 / 删除 / 重启 → 独立 P05C → 原 M5 退出审阅；不把分散证据拼成一个 session。
4. 后置支线：macOS 资格 / WAL、输入质量或性能、同步开放和后续平台各沿 current 与原路线图的进入条件；本批不新建任务优先级或放宽门禁。

未来双语释义、自研 Rust Engine 与 Server Admin Console 继续使用路线图的 Future Topic 边界，临近实施再细化，不能从地图归组推导为当前开发任务。
