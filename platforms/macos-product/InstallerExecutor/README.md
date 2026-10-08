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

## 新源库准备任务

`authorize_source_preparation` 绑定已观察的合法旧 outer 终态、固定 root 与 source/target 双程序；`authorize_preparation_resume` 绑定精确准备记录。两者均要求四项当次确认，授权最长 5 分钟，可撤销且不能从持久记录续权。生产 `prepare_upgrade_source` 固定组合实际安装适配器、root/target manifest 绑定的双程序静止与容量预检，不公开合成 observer。

入口连续持有 outer → inner 双 guard。首次执行先从严格活动 data 或完整 lifecycle 索引确定真实数据前驱，再把旧 outer canonical 字节独占封存到 `.radishlex-install-history-v1/<previous-outer-operation>/receipt.json`；经文件/目录同步与回读后，将文件及目录身份作为 `previous_install_evidence` 绑定到首次 reservation。旧 outer 活动槽此时不变。只有完整取消生命周期证明可以复用已有封存原件；孤立目录、临时副本或单独存在的相同字节均不被认领。

准备意图持久化后才创建同 ID、无程序 artifact 的新 `prepared` outer，然后接源库保护/维护、旧 data 私有槽保留与新 v1 handoff。每个核心检查点前后重新核验 outer、封存原件、程序实物、静止及任务授权。首次无 data、三类未释放旧终态、已释放 v1/v2 与取消前驱分别处理；取消 ID 不会冒充 data 前驱。已绑定中断重开后重新授权，只继续同一 operation；没有 reservation 的部分初始化保留阻断，不自动清理。

成功仅返回新的 `preflighted` 数据回执，outer 仍为 `prepared`，双程序未 staging/切换，startup gate 仍阻断。`load_preparation_for_operation` 可按精确 ID 读取活动准备或已封存 handoff 元数据供重开任务使用；它不授予执行权限，执行仍复验实物和数据等价。Bridge/action 尚未接入此入口，普通 bootstrap 继续只接受空 data 前驱。

### 接续准入任务

`authorize_prepared_continuation` / `admit_prepared_continuation` 只接精确 `prepared/preflighted` 的完整 handoff。四项确认、期限与撤销绑定在任务内；生产入口固定复验真实双程序和 native 静止。准入重同步旧 outer 历史、活动 outer、交接 receipt/保护快照/proof 与移动两侧父目录，重复 SQLite 等价核验，成功返回持续持有双锁的 `PreparedContinuationTask`。

任务不推进 v1、不导出 store/guard 或执行凭证；`revalidate` 可在同一锁内重验，任务退出后必须重新授权。进展后的程序/data/终态执行尚待接入，当前 Driver/Bridge 保持刷新；action `10` 的开放条件见 [Installer 合同](../../../docs/macos-installer-app-boundary.md#专用接续准入与待开放动作合同)。

## 交接后查询与重绑

`inspect_maintenance` 识别 handoff 的已绑定中间位置，以及 marker 已移入历史后的 `PreparationHandedOff`；后者同时返回经过严格 v1 reader 核验的活动数据回执。历史材料仍按准备证明固定身份核验，当前 v1 回执按不可变 operation/前驱/release/schema 和只追加 artifact 关系接续；数据切换中断由原 v1 reader 检查实际槽位，不因活动 DB 暂缺而打开 SQLite 或猜测修复。准备证明缺失、错误前驱及孤立材料不能回退成首次 bootstrap。

`rebind_prepared_upgrade` 要求调用者持有匹配 outer guard，取得 inner guard 后核验封存旧 outer、当前完整产品关系、外层/内层合法状态组合和准备证明，再返回现存 store、inner guard 与数据回执；不创建或重写回执。查询和重绑均不打开 SQLite、不补同步，不宣称持久化边界已确认，也不授予执行权限。后续 mutation 仍须当次任务授权、实际程序/静止观察，并在首个 v1 mutation 前通过准备任务接续确认 handoff 持久化。

当前 UI 将交接后状态继续投影为 `preparing_source + refresh`；普通 intent 保持阻断，尚无新的 action/ABI 映射。下一步固定专用 continuation 的授权、持久化重验、双 guard 生命周期和 Driver/Bridge 合同后再开放执行。

## 准备取消任务

`authorize_cancellation_preservation` 为已有准备记录创建独立、最长 5 分钟的可撤销任务，绑定完整 preparation 和两侧程序身份。`preserve_preparation_cancellation` 按 outer → inner 取得并连续持有双 guard，重新核验准备与 outer 原件，接续取消请求、必要源库收尾、材料保留/兼容恢复及证明/索引发布；返回完整 proof 仅表示等待资格，最后 marker 始终保留。前一 outer 原件必须已经在固定历史槽，不扫描补造；源库维护仍由核心执行。

`finish_preparation_cancellation` 要求针对该完整 proof 另取 `authorize_cancellation_release` 授权，并显式传入可信独立资格提供者；没有默认实现。核心要求资格时才允许最后释放，提供者返回后重新核验 outer、程序、静止和任务有效性。生产入口固定使用真实安装适配器与专用 native 预检，不暴露可替换的合成观察器；预检现保留经验证的实际可用容量供源库预算使用。

重开任务重新授权，执行器不从持久请求续权。各阶段中断按核心证明继续同一 operation，active outer 暂缺也只能沿已有封存证明恢复；未绑定临时文件、handoff 或材料漂移保持拒绝。新准备任务已接入 Executor，Bridge/UI action 尚未接入，真实资格未完成时最终 marker 与按钮继续阻断。细节见 [Installer 边界](../../../docs/macos-installer-app-boundary.md#取消-executor-任务入口)。

## 失败关闭

`MacOsCancellationProductAuthority` 为独立取消释放 authority 提供程序观察组合：复用 `MacOsProductInstallAdapter` 校验精确 data root、sealed target 与安装中的 source 双 bundle，在 `CancellationQuiescenceAuthority` 回调前后重新检查。该回调必须绑定本次完整证明、源程序、当前任务授权与双端静止；只有 target release/容量的旧 preflight 回答不足以填入此接口，也不能用 UI 确认或资格测试成功代替。观察组合不复制或运行产品、不打开用户 SQLite；现有签名验证仍通过受控 `codesign` 执行。

`authorize_cancellation_release` 要求本次显式确认、保留数据确认、中立输入源及 Manager 关闭声明，生成只在内存存活的任务授权，完整绑定 proof 和 source 双程序；期限由调用者指定且最多 5 分钟。任务所有者独立持有 `CancellationTaskControl`，撤销或释放 control 即永久失效；到期/中断后的新任务必须重新确认，不能从持久化 `user_requested` 自动续权。声明不替代实际静止检查。

`MacOsCancellationQuiescenceAuthority` 在每次调用中运行专用 native cancellation preflight，核对实际 root 与绑定 target manifest，并在 helper 返回后复查授权未撤销/到期。程序实物观察仍由上层在其前后执行。普通 v1 preflight 响应不被专用路径接受；没有默认放行或环境开关。

准备/取消的只读维护查询已接入 driver/bridge/UI；六个维护阶段均只提供刷新，普通 v1 执行器在 outer guard 内复核维护记录以拒绝旧 intent。查询不创建目录、取得 guard 或打开 SQLite，损坏/缺失证据继续阻断；完整规则见 [Installer 边界](../../../docs/macos-installer-app-boundary.md#准备与取消的只读维护投影)。取消任务执行入口已接入本 crate，Bridge/action 与独立旧程序资格提供者仍缺失。现有 source validation host 不能证明无新 data receipt 的正常打开/再学习/重启资格；必须通过专用副本验证取得对应证据，缺少时仍阻断最后 marker。产品入口完整接线按[取消设计](../../../docs/remediation/macos-preparation-cancellation-compatibility.md)推进。

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
