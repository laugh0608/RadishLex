# RadishLex Installer App

本文说明 M4-P03 独立原生 Installer App 的展示壳和构建门禁，面向 Installer、产品装配与发布维护者。本文不包含真实安装执行器、真实用户目录写入、进程停止、系统输入源修改、Developer ID、公证或 DMG。

当前 AppKit 壳通过静态链接的 `InstallerBridge` ABI v1 消费 `InstallerDriver` snapshot，展示：

- 固定 Manager/InputMethod 目标和默认保留的 Application Support；
- receipt 派生的 operation phase、0–10 进度与重启继续动作；
- 手动切换到中立输入源、关闭 Manager 的提示；
- 只含 `phase/action/error/state` 的稳定诊断摘要；
- 首次安装、升级、修复、继续、重试和程序移除的显式确认文案。

未知字段、未知 contract 版本和未知 action 失败关闭，只允许刷新。UI 不读取 receipt、不解析 `HOME`、不接受路径参数、不调用 TIS 修改 API，也不把用户勾选当作平台静止证明。

ABI 使用固定整数 enum、POD snapshot 与三个已绑定 symbol。Objective-C 只做已知值映射；未知版本、action、error、state、prompt 或进度失败关闭。Rust 隔离 dispatch 已把 fresh snapshot 授权接入 restartable executor，UI controller 不含文件操作。

真实用户域 bootstrap 尚未授权，因此生产 FFI 明确返回 `driver_unavailable`，构建出的 App 不执行真实安装。该阻断来自 Rust bridge，不是 AppDelegate 内的占位 snapshot。

```bash
./scripts/check-macos-installer.sh
```
