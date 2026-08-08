# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-08（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- production 代码与合成门禁已闭合；L6 format v1、compile-isolated acceptance checkpoint/evidence controller、六步主序列、八点确定性 crash/recovery 合成矩阵与 canonical 脱敏 evidence 已完成。Debian 13 ARM64 已从 source `55351f2` revision 1 与 target `e5b6da1` revision 2 的独立 clean root 原子生成并独立复验真实 pair；独立 L6 guest、专用用户、root-owned handoff 与 `S0-clean-e5b6da1-deff08b1` 已准备并复验，Linux `/proc`、`dpkg` mutation、外部 package lifecycle、系统安装与 P05C 仍未执行。真实用户同步继续关闭。

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
- guard 已改为 mode `0600`、零长度、单 link regular file 的 advisory exclusive lock；active contender 拒绝，stale unlocked file 可由维护流程重新取得。目录/文件创建固定 mode，原子替换和新目录项后同步直接父目录。
- 五类 operation 共用可恢复状态机；每次 mutation 或 retry 都重新验证 staged relationship并取得 move-only quiescence permit。首次安装失败恢复携带已取证 recovery target；target/source proof 已持久化时重入不重复 mutation。
- production system observer 只读固定 dpkg status/config、root owner/mode/link 与 actual staged `.deb`；executor 仅接受固定 `/usr/bin/dpkg`、typed argv、清空环境、null stdin、15 分钟上限和有界诊断。concrete port 在每次 retry 重建 relationship、检查依赖/版本、消费 quiescence permit，并以完整 manifest 验证 installed/absent 结果。
- process quiescence 扫描 `/proc/*/maps` 的固定路径与 device/inode，匿名映射不误报，缺权限、畸形或竞态不确定性失败关闭；它不 kill、restart 或操作会话。受控 CLI 是 opaque command，只接受精确 operation ID、root-owned 同名 `.deb`/evidence，以及 `--authorized-system-mutation` 与 `--preserve-user-data` 双显式授权。
- 只读 startup observer 已把 terminal staging 中的 actual `.deb`、完整 dpkg dependency relationship 与 component inventory 串联；Manager 在 Flutter 前、Fcitx 在 Engine/input FFI/XDG/Rime 前取得 permit。字体 family/glyph/owner 与真实 process/package-manager 行为仍留给 L6。
- 未知 package state、半配置、active/异常 guard、tmp/nonterminal receipt、缺失/多余 slot、symlink/hardlink、宽权限或 owner/mode/link/hash/version/ABI 漂移均失败关闭并保留现场。
- L6 matrix 已固定独立 guest、相邻 revision 六步主序列、八个 crash case、probe/evidence 与逐 mutation 授权。acceptance crate 只经默认关闭的 compile feature 取得精确 hook，不轮询 receipt；controller 终止并等待完整 process group，再证明 group 为空且无 `dpkg` child。canonical checkpoint evidence 只保存 operation ID hash 与稳定分类，不含原值、PID、路径、proc/dpkg 原文或用户数据；实际 controller/CLI 仍未运行。
- release-pair contract 固定 source `55351f2`/`26.7.1+38-1` 与 target `e5b6da1`/`26.7.1+38-2`。真实 pair record SHA-256 为 `a9bcf35762b460a23ad9bc062611f8d5edb57e7303861bbcb99e1efb40703dfd`；source/target package 分别为 `b41e32db76388ad18cdeb60e4b40fb8e28710556df87d53bfa5b275ff2ce028c`、`8209c0161609fde3b798628e5c3460e6237c8618f2d26f1452063540c7541295`，production/acceptance ELF 分别为 `037199abe73559e2cd10013f0930f1f44cf9126ac7987169da11f2933a706fc1`、`c4f6282341c6f68f997b1f5d8d2d1b5dec2d96b60e2f387717594d5a0a9523f4`。该记录只冻结 L6 输入，不证明 package mutation 或 startup。
- 本地 L6 VM 固定为 `/Users/luobo/VirtualMachines/Debian13-ARM64-L6.utm`，UTM UUID `6F73F6DC-66DF-40EC-86B8-228C1FDA1195`；guest 用户为 `radishlex-l6`，root-owned pair 位于 `/var/tmp/radishlex-l6-inputs`。未注册 S0 恢复点固定为 `/Users/luobo/VirtualMachines/RadishLex-L6-Snapshots/S0-clean-e5b6da1`，local evidence SHA-256 为 `50dcc48bfba7f65fce56184b2a21183f8e263611eb0f86d62cdafc6f306b1f08`；当前所有 VM 均关机，controller evidence root 与 RadishLex package 均 absent。

## 停止线

- 未获授权不得在真实 guest 运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 install/repair/remove/rollback。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步顺位

1. 独立 Debian 13 ARM64 L6 guest、专用用户、root-owned pair handoff 与 `S0-clean` 已准备；下一步另行授权执行 source revision 1 的首次 install mutation，并在任何产品进程启动前复验 terminal receipt、dpkg/package inventory、字体与五个 XDG absent。
2. 再按 matrix 逐项授权 upgrade、repair、rollback、remove、reinstall、八个 controller crash/retry、source restore、startup、重启与 XDG 对照；每一步单独授权并保留现场。
3. L6 自动证据闭合后另行授权 P05C 独立 guest 实机；旧资产清理、远端、发布、真实同步和其他平台继续独立排期。

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

上述仓库入口本身只证明合同与合成状态；另行形成的 canonical ARM64 pair record 已冻结真实 package/executable identity。当前仍没有调用 maintenance/acceptance CLI 或 `/usr/bin/dpkg`，不证明真实 process-group/dpkg mutation、桌面安装、L6 完成或公开发布。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W32.md)
