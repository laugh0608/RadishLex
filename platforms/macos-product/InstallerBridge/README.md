# RadishLex macOS Installer Bridge

本文说明 AppKit Installer 与 Rust `InstallerDriver` / `InstallerExecutor` 之间的版本化原生边界，面向安装器、平台适配和产品门禁维护者。本文不包含真实用户目录 bootstrap、程序停止、系统输入源修改、发布签名、公证或 DMG。

`include/radishlex_installer_bridge.h` 固定 ABI v1 的整数枚举、POD snapshot 和三个已绑定 symbol。Objective-C 只把已知整数映射为 `InstallerDriver` v1 字段；未知版本、枚举、进度或 action 一律失败关闭。

Rust `dispatch_installer_action` 每次从只读 receipt 投影重新授权当前 action，再进入 restartable executor，不接受 UI 自报 operation、路径、receipt state 或产品身份。生产 App 已静态链接并实际调用该 ABI；在真实用户域 bootstrap 尚未开放前，导出入口稳定返回 `blocked + driver_unavailable`，不会把隔离资格误当成可安装产品。

隔离测试使用合成 `0700` data root，证明 fresh snapshot 重新授权、`prepared` 持久化、重启投影、stale action 与 active guard 阻断。upgrade 的完整执行和 receipt bootstrap 由 `InstallerExecutor` 测试继续覆盖。
