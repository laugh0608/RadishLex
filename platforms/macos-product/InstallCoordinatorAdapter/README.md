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

资格入口只接受固定 temp marker、私有合成 home/payload 和严格 ad-hoc identity；production Developer ID 路径不变。两类测试均不会访问真实用户目录、系统设置、发布签名身份、Keychain 或网络，也不清理 staging/backup/历史 operation 材料。
