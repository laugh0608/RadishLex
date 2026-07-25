# macOS Installer 执行器

本文说明 `radishlex-macos-installer-executor` 如何把已授权的 Installer intent 接入外层程序事务、manifest-bound 平台 port、只读 preflight 与既有 M4-P02 数据事务。读者是 Installer、平台适配和 M4-P03 门禁维护者。本文不包含真实用户目录、进程停止、输入源修改、Developer ID、公证、DMG 或终态材料清理。

## 两阶段执行

执行器不信任 UI 状态，也不接受路径、运行身份或 release 字段：

1. `BeginFirstInstall`、`BeginUpgrade`、`BeginRepair`、`RetryOperation` 或 `RemovePrograms` 取得外层 guard，回读 current receipt，重新执行 manifest-bound preflight，生成随机 operation ID，并只持久化 `prepared`；
2. Installer 重新投影 snapshot，用户完成中立输入源与 Manager 关闭确认后，`ConfirmQuiescence` 再次执行 preflight，持久化 `quiesced`；
3. 执行器在 guard 内打开固定 Manager/InputMethod `ProgramSwitchStore`，从当前 receipt 重放 source evidence、staging、preserve、逐端 commit、数据协调和两段终态；
4. 非 `prepared` 中断只接受 driver 重新授权的 `ResumeOperation`，沿原 operation ID 和已持久化 evidence 幂等续跑。

first install、repair 和 remove 在 `programs_committed` 后共用核心终态 port。upgrade bootstrap 只从外层 source/target release、固定 data root、只读 userdb schema/identity 与可选 settings/Rime identity 创建 `preflighted` M4-P02 receipt；重启时只接受同 operation、root、release 与 target schema 的既有 receipt。数据 `completed` 后才进入外层 `data_settled -> final_verified -> completed`。若中断在 `final_verified`，重启可重绑 progressed data receipt，只重新证明 data terminal、root/release 与双 bundle，不重新进入数据 migration。

## 失败关闭

- active guard、中断/损坏 receipt、未知对象、root/程序身份漂移由核心或平台 port 保留稳定错误并拒绝执行；
- stale begin/confirm/resume intent、operation/source/target 不匹配和缺失 upgrade context 均拒绝；
- preflight 在任何 receipt 或程序 mutation 前执行，manifest target release 必须与 InstallPayload target 精确匹配，`available_bytes == 0` 不构成证据；
- `InstallerExecutionError::code()` 只输出稳定类别，不包含路径、operation ID、签名正文或底层命令输出；
- 执行器不清理 staging、backup、receipt 或历史 operation 材料。

`InstallerBridge` 已调用本 crate 的稳定入口；真实用户域只读 bootstrap 已接入，但 ad-hoc 构建会先以 `product_identity_unavailable` 失败关闭。Developer ID 资源身份通过后，在真实 mutation port 开放前仍返回 `driver_unavailable`，不把 Rust 文件系统逻辑复制到 AppDelegate。

## 验证

```bash
./scripts/check-macos-installer.sh
```

门禁使用私有合成用户域和真实事务原语，覆盖 first install、upgrade、repair、remove、平台预检阻断、active guard、stale intent、staging 中断续跑，以及 upgrade `final_verified` 重启后完成。不会访问真实 `~/Applications`、`~/Library/Input Methods` 或 Application Support。
