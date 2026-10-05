# macOS 产品安装协调组合层

本文说明 `radishlex-macos-product-install-coordinator` 如何在不合并两个核心职责的前提下，把外层程序安装事务与 M4-P02 数据升级协调器组合为同一升级 operation，并完成 upgrade 产品终态。读者是 Installer 和 M4-P03 维护者。本文不包含 Installer UI、真实用户目录操作、进程停止、签名凭据或公证；完整状态机见 [macOS 程序安装事务](../../../docs/macos-installation-transaction.md)。

## 绑定契约

组合层开始任何数据写入前必须同时证明：

- 外层与数据 receipt 使用同一 32 位小写十六进制 `operation_id`；
- 两个 receipt 和两个 store 指向同一 Application Support device/inode/uid/`0700` 根；
- source/target product version 与 build 精确一致；
- Manager/InputMethod `ProgramSwitchStore` 与外层 receipt、guard、component 顺序一致；
- 外层 receipt 是当前已持久化值，状态只允许从 `programs_committed` 进入 `data_coordinating`。

两个核心保持独立：`ime-product-install` 不解析数据 receipt，`ime-product-upgrade` 不知道程序 staging/backup。组合层同时持有两层 guard，通过 `UpgradeCoordinatorPort` 调度数据状态，并通过 `InstallProgramValidationPort` 复验已安装或已恢复双 bundle 的逻辑身份。

## 成功与失败

- 每个 M4-P02 quiescence checkpoint 只有在 target preflight 与已安装双 bundle 身份同时成立时才通过；
- 数据 `completed` 且 target 双 bundle 再次复验后，外层推进到 `data_settled`；
- 终态动作在 `final_verified` 和 `completed` 前分别复验 data receipt/guard、root/release、data `completed` 与 installed 双 bundle，任一点中断都从已落盘状态重新取证；
- 数据 `aborted_preserved` 或 `rolled_back` 时，外层先持久化 `rollback_required`，再按精确 inode 恢复 source 双程序；
- source 双程序逻辑身份复验和核心 filesystem evidence 都成立后，外层才能经过 `programs_restored` 进入 `rolled_back`；
- 静止、target 身份或 restored source 身份暂不可证明时，保留两个 receipt 和全部恢复材料，重启后从已持久化状态续跑。

## 验证

```bash
./scripts/check-macos-install-coordinator.sh
```

普通测试使用同一个合成 Application Support 根、合成双 bundle 和合成 userdb，覆盖成功、两段终态、`final_verified` 重启续跑、candidate 失败、post-switch 失败、静止丢失、target 身份漂移、source 身份暂不可得，以及 operation/release/root 绑定拒绝。

`qualification-harness` 集成测试由产品协调门禁提供受 marker 保护的系统临时根、独立 source/target ProductManifest/InstallPayload 和真实构建双 bundle。它覆盖：

- data `completed` 后在 `final_verified` 中断，释放双 guard、回读双 receipt，再重新取证完成；
- 真实双端 candidate host 成功后注入失败，精确恢复 source Manager/InputMethod，并只允许 source 运行身份；
- Manager target 已提交而 InputMethod 尚未提交时释放 guard，外层 gate 阻止双端，再从 receipt 续跑到完整 target 终态；
- active install/data guard、外层非终态与终态 target/source 身份的两层只读 gate 决策。

资格入口只接受固定 temp marker、私有合成 home/payload 和严格 ad-hoc identity；production 还要求 sealed requirement 集合。两类测试均不会访问真实用户目录、系统设置、Keychain 或网络，也不清理 staging/backup/历史 operation 材料。

## 准备取消的外层绑定

`CancellationArchiveAuthority` 为专用取消封存协调器提供真实 outer guard 校验、共同 data root、source/target release/manifest 绑定以及 `ime-product-install` 的严格回执核验。旧回执必须是匹配 source 的合法终态，新回执可空或精确匹配无 artifact 的 `prepared`；活动槽暂缺时仍核验已绑定原件，不能以普通 `persist` 倒退覆盖。回调前后验证 guard，并要求 `CancellationProductAuthority` fresh 证明源程序、静止和当前授权。

此桥已由 Installer Executor 的取消任务组合实际双 bundle 观察、专用 native 预检和任务授权；Bridge/UI action 仍未开放。`tests/cancellation_archive.rs` 使用隔离目录、真实双 guard/回执和合成产品观察，普通安装协调门禁以 `qualification-harness --test cancellation_archive` 精确运行此目标；不会运行需要真实载体环境的其他资格目标。取消 `preserved` 仍保留内层阻断 marker，完整释放由下述独立 authority 承接。

## 取消释放的双 guard 与资格绑定

`CancellationReleaseAuthority` 消费 `CancellationReleaseStore` 的完整证明，持有真实 outer guard；inner guard 由核心在回调两侧核验，取得顺序仍为 outer → inner。每次回调严格核对共同 data root、source/target release 与 manifest、旧/新原 canonical，以及当前活动 outer 必须已恢复为同一旧合法终态。封存原件存在不能代替当前槽回读，普通 v1 持久化规则保持。

`Preserve` 只需 fresh 产品/静止/当前任务授权。`QualifiedSource` 还要求 `CancellationSourceQualificationAuthority` 每次复验独立证据，并返回绑定精确双程序身份、source schema 和本次完整证明的 `CancellationSourceQualification`；相同 release/manifest 但双程序不同、其他 root/取消证明、缺失资格或 snapshot schema 不匹配均拒绝。资格对象构造只作结构检查，不执行或证明旧程序验证；维护前没有 snapshot schema 时，提供者必须另证本次 source family 的 schema，不能套用 target schema。

资格回调后再次观察产品/静止，并在两类外部回调前后复验 outer guard 和活动回执；数据核心继续复验源 family、证明及全部材料。`failure()` 保留最近一次 authority 回调的具体失败类别，调用方仍须优先处理核心错误。marker 已移动后的失败不报告成功；重载必须重新取得 guard 和资格，不能凭 marker 缺失认定完成。

`tests/cancellation_release/` 纳入既有 `cancellation_archive` 测试目标，覆盖维护前后、outer 创建前后、错 root/双 guard/产品/证明/schema、各 marker 检查点资格失效、回调期间 outer/源库漂移、产品复验失败与移动后恢复。这些测试中的资格与静止观察仍为合成。Executor 已组合安装适配器的程序实物观察、专用 native 静止/容量预检和可撤销任务授权，并提供分别保留材料与最终释放的双锁任务入口；独立旧程序资格提供者及 Bridge/action 尚缺，不能开放取消按钮。具体调用边界见 [Executor 说明](../InstallerExecutor/README.md#准备取消任务)。
