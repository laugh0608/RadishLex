# RadishLex Installer App

本文说明 M4-P03 独立原生 Installer App 的展示壳和构建门禁，面向 Installer、产品装配与发布维护者。本文不包含真实安装执行器、真实用户目录写入、进程停止、系统输入源修改、Developer ID、公证或 DMG。

当前 AppKit 壳只消费 `InstallerDriver` v1 snapshot，展示：

- 固定 Manager/InputMethod 目标和默认保留的 Application Support；
- receipt 派生的 operation phase、0–10 进度与重启继续动作；
- 手动切换到中立输入源、关闭 Manager 的提示；
- 只含 `phase/action/error/state` 的稳定诊断摘要；
- 首次安装、升级、修复、继续、重试和程序移除的显式确认文案。

未知字段、未知 contract 版本和未知 action 失败关闭，只允许刷新。UI 不读取 receipt、不解析 `HOME`、不接受路径参数、不调用 TIS 修改 API，也不把用户勾选当作平台静止证明。

当前默认 bridge 明确返回 `driver_unavailable`，因此构建出的 App 仍是不可执行真实安装的 contract shell。后续切面必须把 snapshot 与授权 intent 绑定到隔离事务执行器并重新取证，不能在 UI controller 中补文件操作。

```bash
./scripts/check-macos-installer.sh
```
