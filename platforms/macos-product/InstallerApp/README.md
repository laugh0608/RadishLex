# RadishLex Installer App

本文说明 M4-P03 独立原生 Installer App 的展示壳、原生 bridge 和构建门禁，面向 Installer、产品装配与发布维护者。本文不包含进程停止、系统输入源修改、公证、DMG 或凭据管理。

当前 AppKit 壳通过静态链接的 `InstallerBridge` ABI v1 消费 `InstallerDriver` snapshot，展示：

- 固定 Manager/InputMethod 目标和默认保留的 Application Support；
- receipt 派生的 operation phase、0–10 进度与重启继续动作；
- 手动切换到中立输入源、关闭 Manager 的提示；
- 只含 `phase/action/error/state` 的稳定诊断摘要；
- 首次安装、升级、修复、继续、重试和程序移除的显式确认文案。

未知字段、未知 contract 版本和未知 action 失败关闭，只允许刷新。UI 不读取 receipt、不解析 `HOME`、不接受路径参数、不调用 TIS 修改 API，也不把用户勾选当作平台静止证明。

ABI 使用固定整数 enum、POD snapshot 与三个已绑定 symbol。Objective-C 只做已知值映射；未知版本、action、error、state、prompt 或进度失败关闭。Rust 隔离 dispatch 已把 fresh snapshot 授权接入 restartable executor，UI controller 不含文件操作。

App 嵌入完整 `InstallPayload`。Rust bridge 从当前 executable 和系统用户数据库固定形成 current-user bootstrap，不读取 `HOME`；ad-hoc 开发构建不含 `ReleaseIdentity.json`，因此稳定返回 `product_identity_unavailable`。Developer ID 身份、sealed resource 和 payload 通过后，first install、repair、默认程序移除进入真实 restartable mutation port；upgrade 缺少历史 source assembly 时在写入前返回 `driver_unavailable`。App 每次 render 只向 stderr 输出 `phase/action/error/state` 稳定摘要，不输出路径、PID、operation ID 或签名正文。

发布候选构建入口为 `RADISHLEX_DEVELOPER_ID_APPLICATION="Developer ID Application: …" ./scripts/build-macos-release-installer.sh`。它不接受 ad-hoc fallback，并负责嵌套 Hardened Runtime/trusted timestamp 签名、manifest 重冻结、release identity 生成与最终封存；身份凭据、公证和上传不进入脚本参数或仓库。

真实用户域负向验收、人工输入源步骤和回退边界见 [Installer 真实用户域验收 Runbook](../../../docs/runbooks/macos-installer-user-domain-acceptance.md)。

```bash
./scripts/check-macos-installer.sh
```
