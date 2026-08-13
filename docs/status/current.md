# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-13（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、acceptance controller 与合成矩阵已完成。第五套 upgrade 自动恢复 source并形成 `rolled_back`；修复后的第六套 pair/clone 已在断网 preflight 后只执行一次 upgrade，target `38-2` terminal 为 `completed`，十台 VM 全停。

## P04 冻结基线

M5-P03/P04 已在 UTM Debian 13 ARM64 完成 Wayland/X11、GTK/Qt/Electron/Firefox/Terminal、候选交互、生命周期、断网、隐私、Linux Release Manager、同库学习、删除/恢复、导入导出与重启验收。生产 allowlist 仅为精确 `firefox-esr -> browser`；password、terminal、unknown 与 Qt `Sensitive` 不学习，GTK4 `PRIVATE` 按 unknown 失败关闭。

同一 guest/XDG userdb 最终为 user terms 2、selection events 5、suppressed 0、deleted 1、import batches 2。Manager 导入检查零写入、普通导入防 tombstone 复活、P2 导出、Manager/Fcitx 与桌面会话重启均已通过。该证据不再复跑；guest staging、backup、userdb、导入导出文件和临时服务继续原样保留，不作为 P05 安装、回滚或清理目标。

## P05 固定边界

- 首个载体为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository、Release 或通用 Linux 包。
- layout 绑定 Manager、两份同 hash/不同 inode FFI、addon、完整 RimeData/source/license、desktop/icon 与 product manifest。
- CJK/Latin 文本字体使用发行版 hard dependency `fonts-noto-cjk`、`fonts-dejavu-core`；payload 仅允许固定 Material Icons 图标字形。
- `install`、`upgrade`、`repair`、`remove`、`rollback` 默认对用户 XDG 零写入并完整保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 完全相同的 release pair。
- v1 package 明确不含 RadishLex 自有 maintainer scripts；外部 package scripts/triggers 只投影 dpkg lifecycle，不能代表产品事务完成。

## P05A 与载体证据

`packaging/linux/` 已固定产品版本、ARM64 multiarch、依赖、组件路径、ABI/schema/settings/privacy 和 RimeData lock；rootfs 装配器复验 Manager/addon、canonical manifest、inventory、owner/mode/link、双 FFI、metadata、desktop、字体例外和禁止路径。committed `e1ce740` 已在 Debian 13.6 ARM64 由全新源码形成真实 Manager/addon rootfs 证据，但未安装 package。

确定性载体固定三成员 ar 与 canonical uncompressed USTAR、root owner/mode、`md5sums`、SHA-256 evidence 和解包 rootfs 重验。committed `ce74981` 的同一真实 rootfs 连续两次生成逐字节一致的 `.deb` 与 evidence；package database 保持 `not-installed`。

新增 production-only `VerifiedArtifactRelationship::verify_package`：在同一有界流中计算 `.deb` size/SHA-256，严格解析精确三成员 ar、canonical USTAR、仅含 `control`/`md5sums` 的 control、actual data inventory 与唯一 manifest，并逐项交叉 evidence、control、manifest、payload、canonical md5 inventory 与 actual `Installed-Size`。同域 relationship 校验依赖、Debian 版本关系和 dpkg status；现已同时接入 mutable system port 与只读 startup observer。

## P05B 事务与启动状态

- receipt 在 mutation 前持久化 `prepared`，source/target `.deb` 与 evidence 进入 root-owned operation staging 并取证。`receipt.json.tmp`、stage tmp、单侧 artifact 已持久化与 canonical mode 提交前的 owner-only 崩溃窗口均由 guard 下的维护入口复验恢复；startup 只读阻断且不清理。current operation 必需 slot 必须精确，历史旧 operation v1 只验证 structure/pair metadata，不保存旧 hash proof且不用于恢复。
- guard 已改为 mode `0600`、零长度、单 link regular file 的 advisory exclusive lock；active contender 拒绝，stale unlocked file 可由维护流程重新取得。共享锁父目录只接受非 world-writable 或 root-owned 精确 `01777` sticky mode；预创建的非 root guard 仍由文件身份检查拒绝。目录/文件创建固定 mode，原子替换和新目录项后同步直接父目录。
- 五类 operation 共用可恢复状态机；每次 mutation 或 retry 都重新验证 staged relationship并取得 move-only quiescence permit。首次安装失败恢复携带已取证 recovery target；target/source proof 已持久化时重入不重复 mutation。
- production system observer 只读固定 dpkg status/config、root owner/mode/link 与 actual staged `.deb`；executor 仅接受固定 `/usr/bin/dpkg`、typed argv、清空环境、null stdin、15 分钟上限和有界诊断。concrete port 在每次 retry 重建 relationship、检查依赖/版本、消费 quiescence permit，并以完整 manifest 验证 installed/absent 结果。
- `bb84d4a` 规范化 Debian 13 默认 dpkg 配置，`f0415ad` 接受 root-owned 精确 `01777` sticky `/run/lock`。第三个 target `512e8ab` 的 production identity `f716fad3…de62` 能完成 source install，但其 manifest profile 仍拒绝 revision 2；`56dd4de` 已改为校验 package version 与 manifest 的 product/build 一致并接受任意 canonical positive Debian revision，release-pair builder 同时新增 target 生产 Rust verifier 对 source/target 两侧的实际包复验。
- process quiescence 扫描 `/proc/*/maps` 的固定路径与 device/inode，匿名映射不误报，缺权限、畸形或竞态不确定性失败关闭；它不 kill、restart 或操作会话。受控 CLI 是 opaque command，只接受精确 operation ID、root-owned 同名 `.deb`/evidence，以及 `--authorized-system-mutation` 与 `--preserve-user-data` 双显式授权。
- 只读 startup observer 已把 terminal staging 中的 actual `.deb`、完整 dpkg dependency relationship 与 component inventory 串联；Manager 在 Flutter 前、Fcitx 在 Engine/input FFI/XDG/Rime 前取得 permit。第三次 install 后两份已安装 FFI 均通过只读 production gate，返回 `AllowedProduct + InstalledReceiptVerified + completed`；未启动 Manager、Fcitx、Flutter、Engine 或用户数据层。
- 未知 package state、半配置、active/异常 guard、tmp/nonterminal receipt、缺失/多余 slot、symlink/hardlink、宽权限或 owner/mode/link/hash/version/ABI 漂移均失败关闭并保留现场。
- L6 matrix 已固定独立 guest、相邻 revision 六步主序列、八个 crash case、probe/evidence 与逐 mutation 授权。acceptance crate 只经默认关闭的 compile feature 取得 hook；controller 终止并等待完整 process group，再证明 group 为空且无 `dpkg` child。production maintenance CLI 已真实运行四次：前两次 install 与第三套首次 upgrade 停在 receipt 前，第三次 source install 完成；acceptance controller 仍未运行。
- 前两套分别在 dpkg config 与 guard parent 停止；第三套完成 source install 后由 target revision profile 在新 receipt 前失败关闭；第四套双侧 actual package/preflight 通过，但重建 source 不等于 terminal installed artifact，因 chain 不连续在 operation ID/CLI 前停止。四套现场、pair、handoff、snapshot 与 XDG 均保留，精确 identity/hashes 见 L6 runbook。
- release-pair contract 现将 S2 source package/evidence 的文件名、size、SHA-256 与 `prior-terminal-installed-artifact-v1` policy 固定进 committed JSON。builder CLI 只接受这两个 source 文件、一个 clean target root 与 absent output；先以独立 helper 做 canonical evidence/identity/exclusive-copy/fsync，再构建 target，并以 target production Rust verifier 逐侧解析 actual `.deb`。同版本 source bytes 或 evidence 漂移均在发布前失败。
- 第五套 pair `d2661cc0…ed15` 精确复用 S2 source `09ed1228…bec`，target 为 `cdac2f32…7c26`；断网构建、双 verifier 与 absent `.incoming` 原子发布通过，builder package/state/XDG 未创建，旧 handoff 未覆盖。
- 2026-08-11 确认 unavailable 根因是 UTM 4.7.5 必填 `Network` 键被删除，而非已证实的 bookmark 损坏；经授权以唯一差异 `Network=[]` 修复并重建同 UUID 注册。修复后 preflight 从启动起仅 `lo` 且双路由为空；record/source/target、receipt/dpkg、20 项依赖、字体、双 startup gate `0:1:2:2:6`、XDG `f3df287f…b86b`、WAL/SHM/profile absence 与产品映射全部通过。UTM guest-agent 的命令级返回/空输出偏差以大文件回读、可靠退出合同和 startup 正负对照消除；临时 probe 仅在 guest tmpfs，已删除并证明 absent。宿主证据 `9ecd7f54…3089`/`bf2e0591…d09f` 以 `0600` 冻结；关机后 config/EFI/qcow2 为 `402a5840…3e9`/`bcdab060…ce8`/`fc552276…8f10`，qcow2 零打开句柄、九台全停。
- 获得单步授权后 mutation 前各项只读 gate 再次通过；resolved 20 项依赖清单文本摘要为 `28fe1065…6aac`，release-pair 的 production relationship 摘要 `2ee2b7e5…5743` 不是同一序列化口径。production maintenance 只调用一次，exit 0/stdout 为 `maintenance_outcome=rolled_back`：dpkg log 证明先安装 `38-2`，随后恢复 `38-1`。新 receipt `55171bef…6611` 为 `upgrade/target_newer/rolled_back`，failure `target_validation_failed` after `package_mutating`，source proof installed、target proof null、manual recovery false；双 startup `0:1:2:2:11`、XDG/网络/进程静止通过。证据 `98319405…2378`/`64d5395f…e3d8` 已冻结；关机后 disk 为 `402a5840…3e9`/`cb8a697b…bd65`/`e684f829…8904`。
- 第六套只执行一次 production upgrade，exit 0/stdout `maintenance_outcome=completed`。receipt `ccbc4cd0…1e60` 为 `upgrade/target_newer/completed`，target proof `b211d940…d09c`；dpkg audit/verify、startup 正负向、XDG `f3df…b86b`、网络与进程 postflight 通过。evidence `044c7a25…64e`/`9cc50665…f695` 已冻结；关机后 config/EFI/qcow2 为 `00ba…6456`/`0846…9741`/`6b22…b137`，十台全停。
- S2 identity `S2-source-data-512e8ab-0c2cefd6` 与公开合成 XDG fingerprint `f3df287f…b86b` 保持冻结：userdb schema v9/quick_check、1 active/0 deleted、settings/privacy 与固定 metadata 通过，WAL/SHM/Fcitx profile absent；完整磁盘/evidence hash 见 L6 runbook。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步（2026-08-13）

1. 原样保留第五套 `rolled_back` clone、operation staging/receipt、三套本地 evidence 与修复备份；不得启动、重试或与第六套证据混用。
2. 原样保留第六套 target-completed clone、handoff、旧 input、preflight/upgrade evidence；当前 config/EFI/qcow2 为 `00ba…6456`/`0846…9741`/`6b22…b137`，十台均 stopped。
3. 下一批必须另行授权，只从 stopped 第六套现场冻结 S3 target-installed snapshot；不在同批启动 VM 或运行 repair。
4. repair、rollback operation、remove、reinstall、acceptance controller、crash/retry、产品启动与 P05C 保持关闭。

## 验证入口

```bash
./scripts/check-linux-product-metadata.sh
./scripts/check-linux-product-layout.sh
./scripts/check-linux-deb-artifact.sh
./scripts/check-linux-package-transaction.sh
./scripts/check-linux-startup-gate.sh
./scripts/check-linux-l6-contract.sh
./scripts/check-linux-l6-controller.sh
./scripts/check-linux-l6-release-pair.sh
./scripts/check-linux-fcitx5.sh
./scripts/check-manager-linux-product.sh
./scripts/check-repo.sh
./scripts/check-docs.sh
./scripts/check-text-files.sh
git diff --check
```

上述入口证明 prior-terminal anchor、target-only builder、canonical revision 等合成合同。第三套证明 source install，第四套暴露 chain mismatch，第五套证明 target apply/failure/recovery；第六套已证明修复后的 target terminal。repair 及其后主序列、八个真实 crash case、桌面启动、重启、完整 L6 与公开发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W33.md)
