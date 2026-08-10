# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-10（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、compile-isolated acceptance controller、六步主序列与八点 crash/recovery 合成矩阵已完成。target `e5b6da1`/`2fa1b8c` 的两次 install 分别暴露默认 `dpkg.cfg` 与 `root:root 01777` `/run/lock` 缺口；target `512e8ab` 已完成 source revision 1 install、S1 与公开合成 XDG 的 S2，但 2026-08-10 的首次 upgrade 又在 receipt/package mutation 前暴露 product manifest verifier 将 Debian revision 错误硬编码为 `1`。三台 failure disk 均保留；修复后的第四套真实 ARM64 pair 已从双 clean root 断网构建并冻结独立 handoff，七台注册 VM 全部停止。新 L6 clone、guest input 与 upgrade 重试尚未开始。

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
- L6 matrix 已固定独立 guest、相邻 revision 六步主序列、八个 crash case、probe/evidence 与逐 mutation 授权。acceptance crate 只经默认关闭的 compile feature 取得 hook；controller 终止并等待完整 process group，再证明 group 为空且无 `dpkg` child。production maintenance CLI 已真实运行四次：前两次 install 与本次 upgrade 停在 receipt 前，第三次 source install 完成；acceptance controller 仍未运行。
- target `e5b6da1` 与 `2fa1b8c` 的两个旧 pair、handoff 和 `S0-clean-e5b6da1-deff08b1`/`S0-clean-2fa1b8c-5683d120` 现只作失败取证；其 VM `6F73F6DC-66DF-40EC-86B8-228C1FDA1195`、`EBF12F50-33B1-4711-B693-B57D419EAE2A` 已按单 VM 约束正常停止，运行内存状态不再保留，磁盘、package/status/XDG 与各自空 state/operations root 仍保留；详细 artifact/operation/S0 hash 见 L6 runbook。
- 第三个 pair 固定 source `55351f2`/target `512e8ab`，record SHA-256 `2f2deaed…697`。source install 的 receipt/dpkg status SHA-256 为 `e58b144e…ab8`/`33c4973d…ff1`。本次 upgrade operation ID 只登记 SHA-256 `f3306a5a…fd1`；CLI 返回 1/`ArtifactInvalid`，没有新 operation、receipt、staging 或 dpkg mutation，source package、receipt、dpkg status 与三份 XDG 内容哈希均未变。失败 guest 已关机，config/EFI/qcow2 SHA-256 为 `5111741c…d2b`、`8f36df35…1ee`、`0cb75b38…f387`。
- 第四个 pair 固定 source `55351f2`/target `56dd4de`，在 Debian 13.6 ARM64、仅 loopback 的 user/network namespace 中由两套 `umask 0022` clean root 构建。record SHA-256 为 `70a394ea…139a`；source/target package 为 `55fba51b…280f`/`58ba3589…2814`，production/acceptance ELF 为 `3bb2925f…401c`/`29cb1b1a…ae76`。builder 与宿主独立 verifier、target production Rust 双侧 actual-package parser、8 项逐哈希、mode/link/ELF 与构建后冻结依赖实体均通过；独立 handoff 已原子发布，尚未写入任何 L6 guest。
- S2 identity 为 `S2-source-data-512e8ab-0c2cefd6`；config/EFI/qcow2 SHA-256 分别为 `5111741c54a49068dbbacfd131891b0990a531001769db21942eb88ac6767d2b`、`365b5a170dca95bdf07c0e5e940fafd4580a353e43c141abede71c95f91261bc`、`0c2cefd6b63420adf143e1f1d6e4e70f59841bf1ba56cb54e86d2e7a226f3eea`，local evidence SHA-256 为 `d164de0f5e6e88afbfa76ccd1c7d321d7064bbfd3ee1e275d3d4842d0dfa5c74`。公开合成 XDG fingerprint 为 `f3df287fa0f1da1d5f1fb3169fe607a84ad7fb9b60aab1308ba2eeb410d0b86b`：userdb schema v9/quick_check、1 active/0 deleted、settings/privacy、四个 `0700` 根与三个 `0600` 文件通过，WAL/SHM 和 Fcitx profile absent；package/receipt/dpkg 未变且产品映射为 0。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用当前三个 stopped L6 failure disk；三套失败 pair/handoff/S0、第三套 S1/S2 与额外 WAL-drift snapshot 只保留取证，不得热替换 executable 或在原 guest 重试。第四套 handoff 只能写入后续独立 clone。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一事项

1. revision/profile 修复、pair 双侧生产解析门禁与 source `55351f2`/target `56dd4de` 第四套 handoff 已冻结；不得修改或覆盖第三套失败 handoff、guest input、失败盘和 S2。
2. 另行授权从冻结 S2 建立独立 clone，写入第四套 root-owned input并完成断网只读 preflight；该步骤不生成 operation ID、不运行 maintenance/acceptance CLI 或 `dpkg`。
3. preflight 通过后另行取得 mutation 授权，只执行一次 upgrade，不自动进入 repair 或 crash/retry。
4. 此后再分别授权其余主序列、八个 crash/retry、产品启动、重启及 P05C；旧资产、远端、推送与发布保持关闭。

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

上述仓库入口本身只证明合同与合成状态。第三套真实 pair 已证明 source install，并在 upgrade 前暴露 revision profile 缺口；第四套真实 pair 已由修复后的 production Rust parser 对两侧实际包验证并冻结 handoff，但尚未进入独立 guest。upgrade、acceptance process-group/crash、其余主序列、真实桌面启动、重启、完整 L6 与公开发布仍未证明。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W33.md)
