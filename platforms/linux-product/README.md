# RadishLex Linux product transaction

本目录承载 Debian 系统级产品安装维护的事务合同，不承载 `.deb` 组包，也不直接实现 `dpkg`、服务启停或桌面会话操作。

当前 crate 固定以下边界：

- `install`、`upgrade`、`repair`、`remove`、`rollback` 五类操作共用可恢复状态机；
- 首次系统状态检查后先写入 `prepared` 收据，再把 source / target `.deb` 与 evidence 复制到当前操作的 root-only 私有暂存目录；
- `/var/lib/radishlex/install-v1/receipt.json` 为 append-only 当前收据，`operation_chain` 只允许从终态追加新操作；
- `/run/lock/radishlex-install-v1.lock` 是 mode `0600` 的 Unix socket guard，用 inode 身份阻止并发事务和错误清理；
- `dpkg`、产品载荷验证和恢复动作经 `DpkgTransactionPort` 注入；仓库测试只使用 fake port 与临时目录；
- Manager、Fcitx5、用户 XDG 数据、Rime 用户数据、备份和 P04 guest 资产均不在本 crate 的读写范围。

当前尚未实现真实 `dpkg` adapter、提权入口、安装命令或系统集成；不得把本 crate 的自动测试等同于 Linux 实机安装验收。完整边界见 `docs/linux-installation-maintenance-boundary.md`。
