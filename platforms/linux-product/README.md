# RadishLex Linux product transaction

本目录承载 Debian 系统级产品安装维护的事务合同与 Manager/Fcitx 共用只读 startup decision，不承载 `.deb` 组包，也不直接实现 mutable `dpkg`、服务启停或桌面会话操作。

当前 crate 固定以下边界：

- `install`、`upgrade`、`repair`、`remove`、`rollback` 五类操作共用可恢复状态机；
- 首次系统状态检查后先写入 `prepared` 收据，再把 source / target `.deb` 与 evidence 复制到当前操作的 root-only 私有暂存目录；
- `/var/lib/radishlex/install-v1/receipt.json` 为 append-only 当前收据，`operation_chain` 只允许从终态追加新操作；
- `/run/lock/radishlex-install-v1.lock` 是 mode `0600` 的 Unix socket guard，用 inode 身份阻止并发事务和错误清理；
- `dpkg`、产品载荷验证和恢复动作经 `DpkgTransactionPort` 注入；仓库测试只使用 fake port 与临时目录；
- startup observer 直接读取固定 `/var/lib/dpkg/status`，严格检查 root/guard/tmp/receipt、package 状态与 terminal staging proof；Manager scope 复验完整 bundle tree，Fcitx scope 复验 addon/FFI/RimeData/两份 metadata，并验证两份产品 FFI equivalence、owner/mode/link/hash、ABI 与 data contract；
- observer 不调用 `dpkg`，不创建或清理 state，不读取或写入 XDG/userdb/settings/privacy/Rime；`development-staged` 与 `debian-system-product` 由编译身份隔离；
- additive `radishlex_linux_product_startup_gate` request/result v1 由 Manager 在 Flutter 初始化前、Fcitx factory 在 Engine/input FFI/XDG/Rime 初始化前消费；C++ binding 用 `dladdr`/canonical path 把 startup/error 与全部 Fcitx input symbols 绑定到精确 sibling FFI，拒绝 loader interposition；输入 session/key ABI contract 仍为 v9；
- Manager、Fcitx5、用户 XDG 数据、Rime 用户数据、备份和 P04 guest 资产均不在本 crate 的读写范围。

当前 startup scope 不冒充全 package runtime inventory；mutable `DpkgTransactionPort`、maintainer scripts、外部 dependency/font/version relationship validation、privileged maintenance command、Debian ARM64 L6 matrix、安装命令或系统集成尚未实现，也没有构建或实机运行新的 ARM64 startup-enabled payload。不得把本 crate 的自动测试等同于 Linux 实机安装验收。完整边界见 `docs/linux-installation-maintenance-boundary.md`。
