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
- `radishlex-linux-maintenance` command 类型不可由调用方直接构造，CLI 同时要求小写 32 hex operation ID、root-owned canonical package/evidence、`--authorized-system-mutation` 与 `--preserve-user-data`；它不是公开 installer。`radishlex-linux-artifact-verifier` 是 release-pair 构建期只读入口，以同一 production parser 验证 source/target 两侧 actual `.deb`，不创建 receipt 或运行 dpkg；
- v1 package 不允许 RadishLex 自有 maintainer scripts。外部 dependency scripts/triggers 只形成 dpkg observation，不能推进 product receipt 或代表完成；
- startup observer 只读固定 `/var/lib/dpkg/status`、guard/tmp/receipt、terminal actual package/dependency relationship 与 component identity；Manager/Fcitx 在 Flutter/Engine/input FFI/XDG/Rime 之前消费 additive request/result v1，输入 session/key ABI 仍为 v9。
- L6 format v1 固定独立 Debian 13 ARM64 guest、不同 commit 的相邻 Debian revision、六步主序列、八个 crash checkpoint、字体/startup/XDG/procfs probe 与逐 mutation 授权；实现和执行说明见 [`docs/runbooks/linux-l6-package-matrix.md`](../../docs/runbooks/linux-l6-package-matrix.md)。
- production crate 的 `l6-acceptance-checkpoints` feature 默认关闭，正式 maintenance main 只绑定 disabled sink，也不接受环境或路径覆盖；独立 [`platforms/linux-l6-acceptance`](../linux-l6-acceptance/README.md) compile identity 才能连接八个确定 checkpoint、完整 process group 终止与 canonical evidence。
- [`packaging/linux/l6-release-pair.json`](../../packaging/linux/l6-release-pair.json) 将 S2 terminal source `55351f2` revision 1 package/evidence 精确绑定为 prior-terminal chain anchor，并把 target 固定为 clean descendant revision 2。builder 先 exclusive-copy/fsync frozen source、只构建 target，再用 target production Rust verifier 逐侧解析并冻结 maintenance/acceptance ELF；同版本 source 重建与 evidence 漂移均失败关闭。
- [`packaging/linux/l6-maintenance-refresh.json`](../../packaging/linux/l6-maintenance-refresh.json) 在package冻结后另行绑定第六套record、target package/evidence、旧production ELF与repair fix祖先；builder只构建较新production maintenance和临时artifact verifier，不构建或替换`.deb`，也不包含acceptance executable。

当前 source install、S1/S2、pre-receipt failure、chain mismatch 与第五套 `rolled_back` recovery 均已取证。第六套upgrade形成target `completed`与S3；真实repair、rollback、remove与reinstall也已闭合，六类operation各有独立证据且XDG零漂移。首个`install_prepared`的startup共享目录漂移已修复，第三台clean clone闭合checkpoint/exact resume。第二个case `install_artifacts_staged`的typed合同、首次安装回归、clone身份与host单次start/clone控制已闭合；所有既有start均在host失败关闭且未进入guest。v7完整诊断manifest `206aa335…7b56c`已定位UTM接收start AppleEvent后的AppKit主窗口断言，未见对应reply或QEMU事件且零mutation；foreground launch transport v2已绑定v4 prepared七项证据、精确package与两次descriptor磁盘身份，16项repository-only合成门禁通过但未实机。新v4 clone `50B75F88…8038`已以clone/prepared manifest `f76d1943…ff6e2`/`c40c55d9…144b`冻结为DependencyFrozen clean磁盘、stopped target；下一步先单独授权关闭驻留UTM app，进程归零后再单独授权唯一foreground start。证据见L6 runbook，完整边界见 [`docs/linux-installation-maintenance-boundary.md`](../../docs/linux-installation-maintenance-boundary.md)。
