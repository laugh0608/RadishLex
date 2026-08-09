# RadishLex Linux product transaction

本目录承载 Debian 系统级产品的 package relationship、维护事务、固定系统 observer/executor、受控维护 CLI 与 Manager/Fcitx 共用只读 startup decision。它不生成 `.deb`，不启停服务或桌面会话，也不读取/写入用户 XDG；真实 `dpkg` 只允许由 opaque authorized command 进入。

当前合同：

- canonical receipt 绑定 state-root identity、operation chain、source/target artifact、稳定 failure 与 terminal proof；`receipt.json.tmp` 和 artifact stage tmp 只按明确崩溃窗口恢复；
- 私有 operation staging 使用目录 `0700`、文件 `0600`、单 link/固定 inode/size/SHA-256；current operation required slots 必须精确，旧 operation v1 只验证 structure/pair metadata且不用于恢复；
- `/run/lock/radishlex-install-v1.lock` 是 mode `0600`、零长度、单 link regular file 上的 advisory exclusive lock；active contender 拒绝，stale unlocked file 只能由维护 acquisition 重新取得；父目录只接受非 world-writable 或 root-owned 精确 `01777` sticky mode；
- 目录/文件创建固定 mode，receipt 与 staging 的原子替换、新目录项均同步直接父目录；
- 五类 operation 共用恢复状态机。每次 target/source mutation 或 retry 都重新验证 staged relationship并消费 move-only quiescence permit；首次安装恢复携带已取证 recovery target；
- `VerifiedArtifactRelationship::verify_package` 是 production actual-package 唯一入口：从同一有界 `.deb` 流计算 size/SHA-256，严格解析精确三成员 ar、canonical uncompressed USTAR、仅 `control`/`md5sums` 的 control、actual data inventory 和唯一 product manifest，并交叉 evidence、canonical md5 inventory 与 actual `Installed-Size`；同域 pure relationship 另行校验依赖、Debian version 与 dpkg status；两者都不写文件、不调用命令；
- production executor 固定 `/usr/bin/dpkg` identity、私有 staged path、argv、清空后的允许环境、null stdin、15 分钟上限与有界诊断；system observer 只读固定 dpkg status/config、root-owned staging 和完整产品 inventory。配置只接受 `no-pager`、Debian 默认 `no-debsig` 与固定 `/var/log/dpkg.log` 的空白/等号写法，其他值失败关闭；
- concrete `LinuxDpkgTransactionPort` 在 prepare、retry、target/source mutation 与验证阶段重建 actual relationship，检查依赖/版本，消费 move-only `/proc/*/maps` quiescence permit；不自动 kill、restart 或操作用户会话；
- `radishlex-linux-maintenance` command 类型不可由调用方直接构造，CLI 同时要求小写 32 hex operation ID、root-owned canonical package/evidence、`--authorized-system-mutation` 与 `--preserve-user-data`；它不是公开 installer。首次真实运行在旧 validator 的 Debian 默认配置检查处 pre-receipt 失败关闭，未调用 `dpkg`；
- v1 package 不允许 RadishLex 自有 maintainer scripts。外部 dependency scripts/triggers 只形成 dpkg observation，不能推进 product receipt 或代表完成；
- startup observer 只读固定 `/var/lib/dpkg/status`、guard/tmp/receipt、terminal actual package/dependency relationship 与 component identity；Manager/Fcitx 在 Flutter/Engine/input FFI/XDG/Rime 之前消费 additive request/result v1，输入 session/key ABI 仍为 v9。
- L6 format v1 固定独立 Debian 13 ARM64 guest、不同 commit 的相邻 Debian revision、六步主序列、八个 crash checkpoint、字体/startup/XDG/procfs probe 与逐 mutation 授权；实现和执行说明见 [`docs/runbooks/linux-l6-package-matrix.md`](../../docs/runbooks/linux-l6-package-matrix.md)。
- production crate 的 `l6-acceptance-checkpoints` feature 默认关闭，正式 maintenance main 只绑定 disabled sink，也不接受环境或路径覆盖；独立 [`platforms/linux-l6-acceptance`](../linux-l6-acceptance/README.md) compile identity 才能连接八个确定 checkpoint、完整 process group 终止与 canonical evidence。
- [`packaging/linux/l6-release-pair.json`](../../packaging/linux/l6-release-pair.json) 将 source `55351f2` revision 1 与 target revision 2 绑定为同 data contract 的相邻 pair；`build-linux-l6-release-pair.sh` 从两个独立 clean root 构建并强校验两份 `.deb`，再分别冻结 production `--no-default-features` maintenance ELF 与 acceptance ELF。source `55351f2` 与 target `e5b6da1`/`2fa1b8c` 的两个 pair 现均只作 pre-receipt 失败取证；包含 `f0415ad` 的第三个 pair 尚未构建。

当前 production 代码、fake command/crash matrix、L6 format、compile-isolated checkpoint/evidence controller 与 release-pair builder/verifier 已闭合。两次首次授权 install 分别暴露 Debian 默认配置与 sticky lock parent 缺口，均停于 receipt/package mutation 之前；`f0415ad` 已修复后者，但真实 `/usr/bin/dpkg` mutation、字体 family/glyph/owner、外部 package lifecycle 与 L6 完成证据仍不存在。两个 handoff/S0 和离线 failure overlay 继续保留，下一步须重建第三个 pair/handoff/guest/S0。仓库测试只使用 actual 合成 package、fake executor/observer/backend 与临时目录；不得据此宣称系统安装。完整边界见 [`docs/linux-installation-maintenance-boundary.md`](../../docs/linux-installation-maintenance-boundary.md)。
