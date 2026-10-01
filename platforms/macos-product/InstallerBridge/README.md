# RadishLex macOS Installer Bridge

本文说明 AppKit Installer 与 Rust `InstallerDriver` / `InstallerExecutor` 之间的版本化原生边界，面向安装器、平台适配和产品门禁维护者。本文不包含程序停止、系统输入源修改、发布签名、公证或 DMG。

`include/radishlex_installer_bridge.h` 固定 ABI v1 的整数枚举、POD snapshot 和三个已绑定 symbol。Objective-C 只把已知整数映射为 `InstallerDriver` v1 字段；未知版本、枚举、进度或 action 一律失败关闭。

`RADISHLEX_INSTALLER_ACTION_ABORT_PRE_SWITCH_UPGRADE = 9` 是 ABI v1 的可加 action；POD、symbol 与 contract version 保持不变。旧 reader 对未知 action 失败关闭，当前 Rust/Objective-C 双向映射和四项 authorization bits 由合同测试覆盖。恢复状态经 Executor 双 receipt 只读检查投影；生产入口不把普通 resume 当作中止恢复。

Rust `dispatch_installer_action` 每次从只读 receipt 投影重新授权当前 action，再进入 restartable executor，不接受 UI 自报 operation、路径、receipt state 或产品身份。生产 bootstrap 使用 `geteuid/getpwuid_r` 取得 authoritative current-user home，只从当前 executable 固定反推 `RadishLex Installer.app/Contents/Resources`，并严格读取 sealed `ReleaseIdentity.json` 和内嵌 InstallPayload。路径、owner、resource、Installer strict ad-hoc identity、双 component exact requirement 集合或 payload identity 任一不可得时稳定返回 `blocked + product_identity_unavailable`。

ad-hoc 开发构建不携带 `ReleaseIdentity.json`，也没有 production fallback。身份、payload 和用户域均通过后，bridge 会从 fresh snapshot 解码已知 authorization bits，装配 macOS install adapter、target preflight、receipt store 和系统随机 operation ID。first install、repair、默认程序移除及其 resume 使用现有真实 mutation port；first install 只在显式 action 后创建缺失的固定目录，绝不 chmod 既有对象。upgrade 在没有 manifest-bound 历史 source assembly 时先返回 `driver_unavailable`，不会先持久化 `prepared` 或切换 bundle。

执行失败不能被有效 receipt 或 completed 进度遮蔽：生产 bridge 保留 fresh receipt 的 operation/state/progress，同时返回 `blocked + refresh`；已有稳定错误保留，否则使用 ABI v1 已有的 `unknown_driver_result`，仅记录固定授权/执行错误类别。错误投影不改 receipt，成功 refresh 仍重新读取持久化状态，不代表先前失败已解决。

隔离测试使用合成 `0700` data root，证明 fresh snapshot 重新授权、`prepared` 持久化、重启投影、stale action、active guard 阻断及执行错误呈现；失败分支覆盖非终态、completed 和已有诊断，保留实际持久化阶段。upgrade 的完整执行和 receipt bootstrap 由 `InstallerExecutor` 测试继续覆盖。
