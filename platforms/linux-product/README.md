# RadishLex Linux product transaction

本目录承载 Debian 系统级产品的 package relationship、维护事务合同与 Manager/Fcitx 共用只读 startup decision。它不生成 `.deb`，不直接执行 mutable `dpkg`，不启停服务或桌面会话，也不读取/写入用户 XDG。

当前合同：

- canonical receipt 绑定 state-root identity、operation chain、source/target artifact、稳定 failure 与 terminal proof；`receipt.json.tmp` 和 artifact stage tmp 只按明确崩溃窗口恢复；
- 私有 operation staging 使用目录 `0700`、文件 `0600`、单 link/固定 inode/size/SHA-256；current operation required slots 必须精确，旧 operation v1 只验证 structure/pair metadata且不用于恢复；
- `/run/lock/radishlex-install-v1.lock` 是 mode `0600`、零长度、单 link regular file 上的 advisory exclusive lock；active contender 拒绝，stale unlocked file 只能由维护 acquisition 重新取得；
- 目录/文件创建固定 mode，receipt 与 staging 的原子替换、新目录项均同步直接父目录；
- 五类 operation 共用恢复状态机。每次 target/source mutation 或 retry 都重新验证 staged relationship并消费 move-only quiescence permit；首次安装恢复携带已取证 recovery target；
- `VerifiedArtifactRelationship::verify_package` 是 production actual-package 唯一入口：从同一有界 `.deb` 流计算 size/SHA-256，严格解析精确三成员 ar、canonical uncompressed USTAR、仅 `control`/`md5sums` 的 control、actual data inventory 和唯一 product manifest，并交叉 evidence、canonical md5 inventory 与 actual `Installed-Size`；同域 pure relationship 另行校验依赖、Debian version 与 dpkg status；两者都不写文件、不调用命令；
- typed Debian command contract 固定 `/usr/bin/dpkg`、私有 staged path、argv、清空后的允许环境、null stdin、有界诊断、dpkg config 与 lifecycle projection，但没有 executor；
- v1 package 不允许 RadishLex 自有 maintainer scripts。外部 dependency scripts/triggers 只形成 dpkg observation，不能推进 product receipt 或代表完成；
- startup observer 继续只读固定 `/var/lib/dpkg/status`、guard/tmp/receipt、terminal staging 与 component identity；Manager/Fcitx 在 Flutter/Engine/input FFI/XDG/Rime 之前消费 additive request/result v1，输入 session/key ABI 仍为 v9。

当前未实现 production fixed-path system observer/executor、concrete mutable `DpkgTransactionPort`、真实 process quiescence、privileged host/CLI、startup dependency relationship 连接、字体 family/glyph/owner、外部 package lifecycle 或 Debian ARM64 L6。仓库测试只使用合成 package、fake port 与临时目录；不得据此运行或宣称系统安装。完整边界见 [`docs/linux-installation-maintenance-boundary.md`](../../docs/linux-installation-maintenance-boundary.md)。
