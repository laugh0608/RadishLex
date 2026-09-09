# macOS Installer 执行器

本文说明 `radishlex-macos-installer-executor` 如何把已授权的 Installer intent 接入外层程序事务、manifest-bound 平台 port、只读 preflight 与既有 M4-P02 数据事务。读者是 Installer、平台适配和 M4-P03 门禁维护者。本文不包含真实用户目录、进程停止、输入源修改、Developer ID、公证、DMG 或终态材料清理。

## 两阶段执行

执行器不信任 UI 状态，也不接受路径、运行身份或 release 字段：

1. `BeginFirstInstall`、`BeginUpgrade`、`BeginRepair`、`RetryOperation` 或 `RemovePrograms` 取得外层 guard，回读 current receipt，重新执行 manifest-bound preflight，生成随机 operation ID，并只持久化 `prepared`；
2. Installer 重新投影 snapshot，用户完成中立输入源与 Manager 关闭确认后，`ConfirmQuiescence` 再次执行 preflight，持久化 `quiesced`；
3. 执行器在 guard 内打开固定 Manager/InputMethod `ProgramSwitchStore`，从当前 receipt 重放 source evidence、staging、preserve、逐端 commit、数据协调和两段终态；
4. 普通非 `prepared` 中断使用 driver 重新授权的 `ResumeOperation`，沿原 operation ID 和已持久化 evidence 幂等续跑；显式切换前中止恢复使用下述专用动作。

first install、repair 和 remove 在 `programs_committed` 后共用核心终态 port。upgrade bootstrap 只从外层 source/target release、固定 data root、只读 userdb schema/identity 与可选 settings/Rime identity 创建 `preflighted` M4-P02 receipt；重启时只接受同 operation、root、release 与 target schema 的既有 receipt。数据 `completed` 后才进入外层 `data_settled -> final_verified -> completed`。若中断在 `final_verified`，重启可重绑 progressed data receipt，只重新证明 data terminal、root/release 与双 bundle，不重新进入数据 migration。

## 显式切换前中止恢复

`inspect_installer_view_with_recovery` 通过两层 store 的 `open_existing` / `load_for_recovery_inspection` 检查原有事务，不 bootstrap、不创建目录、不打开 SQLite。只有外层 `data_coordinating`、内层无失败的 `candidate_verified`，或持有同一证据的合法恢复阶段，才向 driver 提供 `AbortPreSwitchUpgrade`。

执行从外层 guard 到内层 guard，重验 operation/root/source/target、fresh preflight、程序材料与固定数据文件。首次中止前独占落盘本地保留证据；`abort_pre_switch_install_upgrade` 合法推进内层中止，再复用源程序恢复路径，逐端恢复和外层终态前重验。已中止的专用恢复状态、已存在的证据都不能绕过检查走普通 resume；缺失、部分或漂移证据停止，不自动重建。数据库只读 hash，不使用 SQLite connection、不改变 journal mode；Rime 仅核对根身份。

该动作不创建新 operation、不改变 v1 receipt 格式、不实现 WAL 源库准备或后续升级重试。详细证据格式、检查点与旧 reader 兼容见 [Installer 边界](../../../docs/macos-installer-app-boundary.md#显式切换前中止恢复)和[数据升级边界](../../../docs/macos-data-upgrade-coordinator.md#显式切换前中止的保留证据)。

## 失败关闭

- active guard、中断/损坏 receipt、未知对象、root/程序身份漂移由核心或平台 port 保留稳定错误并拒绝执行；
- stale begin/confirm/resume intent、operation/source/target 不匹配和缺失 upgrade context 均拒绝；
- preflight 在任何 receipt 或程序 mutation 前执行，manifest target release 必须与 InstallPayload target 精确匹配，`available_bytes == 0` 不构成证据；
- `InstallerExecutionError::code()` 只输出稳定类别，不包含路径、operation ID、签名正文或底层命令输出；
- 执行器不清理 staging、backup、receipt 或历史 operation 材料。

`InstallerBridge` 已调用本 crate 的稳定入口；普通开发构建因缺失 sealed identity 先以 `product_identity_unavailable` 失败关闭。community ad-hoc 资源身份通过后，first install、repair、默认程序移除和 resume 直接使用本 crate 与 manifest-bound macOS port，不把文件系统逻辑复制到 AppDelegate。production upgrade 还要求 payload 显式绑定真实历史 source assembly；缺失时 bridge 在任何 receipt/program mutation 前返回 `driver_unavailable`。

## 验证

```bash
./scripts/check-macos-installer.sh
```

门禁使用私有合成用户域和真实事务原语，覆盖 first install、upgrade、repair、remove、平台预检阻断、active guard、stale intent、staging 中断续跑，以及 upgrade `final_verified` 重启后完成；另覆盖已提交合法 WAL 保留、恢复各检查点重放、缺失/部分证据、资产漂移、跨 root/target 授权与只读投影零创建。不会访问真实 `~/Applications`、`~/Library/Input Methods` 或 Application Support。
