# RadishLex macOS Installer Bridge

本文说明 AppKit Installer 与 Rust `InstallerDriver` / `InstallerExecutor` 之间的版本化原生边界，面向安装器、平台适配和产品门禁维护者。本文不包含程序停止、系统输入源修改、发布签名、公证或 DMG。

`include/radishlex_installer_bridge.h` 固定 ABI v1 的整数枚举、POD snapshot 和三个已绑定 symbol。Objective-C 只把已知整数映射为 `InstallerDriver` v1 字段；未知版本、枚举、进度或 action 一律失败关闭。

Rust `dispatch_installer_action` 每次从只读 receipt 投影重新授权当前 action，再进入 restartable executor，不接受 UI 自报 operation、路径、receipt state 或产品身份。生产 bootstrap 使用 `geteuid/getpwuid_r` 取得 authoritative current-user home，只从当前 executable 固定反推 `RadishLex Installer.app/Contents/Resources`，并严格读取签名资源 `ReleaseIdentity.json` 和内嵌 InstallPayload。路径、owner、resource、Team ID、双 designated requirement 或 payload identity 任一不可得时稳定返回 `blocked + product_identity_unavailable`。

ad-hoc 开发构建不携带 `ReleaseIdentity.json`，也没有 production fallback；只有后续 Developer ID 切面生成并由 Installer code signature 封存该资源后，bridge 才会继续到真实 driver 装配。当前资源通过后仍保留 `driver_unavailable`，明确表示真实 mutation port 尚未开放。

隔离测试使用合成 `0700` data root，证明 fresh snapshot 重新授权、`prepared` 持久化、重启投影、stale action 与 active guard 阻断。upgrade 的完整执行和 receipt bootstrap 由 `InstallerExecutor` 测试继续覆盖。
