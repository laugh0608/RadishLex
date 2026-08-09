# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-09（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- production 代码与合成门禁已闭合；L6 format v1、compile-isolated acceptance checkpoint/evidence controller、六步主序列、八点确定性 crash/recovery 合成矩阵与 canonical 脱敏 evidence 已完成。2026-08-09 的 target `e5b6da1` 与 `2fa1b8c` 两次 install 分别暴露默认 `dpkg.cfg` 与 `root:root 01777` `/run/lock` 缺口，均停在 receipt/package mutation 前；两个 disposable failure overlay 保持运行且离线。包含 `f0415ad` 的 target `512e8ab` 已从双 clean root 形成第三套真实 ARM64 pair、独立 handoff/guest 与 `S0-clean-512e8ab-84d59494`，断网 dpkg/XDG/20 项依赖/字体/sticky lock preflight 通过；第三次 operation ID、maintenance/acceptance CLI 与真实 package mutation 均未开始，真实用户同步继续关闭。

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
- `bb84d4a` 将真实 Debian 13 默认 `no-debsig` 与固定 `/var/log/dpkg.log` 的空白/等号写法规范化为受控配置；`f0415ad` 接受 Debian 标准 `root:root 01777` `/run/lock`，但继续拒绝非 sticky world-writable、异常 sticky mode 与不安全 owner/group。target `2fa1b8c` 的旧 production maintenance identity `919b55dc46958ca55520e0cb9a0fae5c8080f9b58e1bd9142b662af10b7c48eb` 不能用于该修复；第三个 target `512e8ab` 已冻结新的 production identity `f716fad30b6657e108272fd1f7361826773b0ca42e4ac1ce6f78a7ad20aade62`。
- process quiescence 扫描 `/proc/*/maps` 的固定路径与 device/inode，匿名映射不误报，缺权限、畸形或竞态不确定性失败关闭；它不 kill、restart 或操作会话。受控 CLI 是 opaque command，只接受精确 operation ID、root-owned 同名 `.deb`/evidence，以及 `--authorized-system-mutation` 与 `--preserve-user-data` 双显式授权。
- 只读 startup observer 已把 terminal staging 中的 actual `.deb`、完整 dpkg dependency relationship 与 component inventory 串联；Manager 在 Flutter 前、Fcitx 在 Engine/input FFI/XDG/Rime 前取得 permit。第三套 S0 的安装前字体 family/glyph/owner 已通过，package mutation 后字体、startup 与真实 process/package-manager 行为仍留给 L6 postflight。
- 未知 package state、半配置、active/异常 guard、tmp/nonterminal receipt、缺失/多余 slot、symlink/hardlink、宽权限或 owner/mode/link/hash/version/ABI 漂移均失败关闭并保留现场。
- L6 matrix 已固定独立 guest、相邻 revision 六步主序列、八个 crash case、probe/evidence 与逐 mutation 授权。acceptance crate 只经默认关闭的 compile feature 取得精确 hook，不轮询 receipt；controller 终止并等待完整 process group，再证明 group 为空且无 `dpkg` child。canonical checkpoint evidence 只保存 operation ID hash 与稳定分类，不含原值、PID、路径、proc/dpkg 原文或用户数据；production maintenance CLI 已真实运行两次但都停在 receipt 前，acceptance controller 仍未运行。
- target `e5b6da1` 与 `2fa1b8c` 的两个旧 pair、handoff 和 `S0-clean-e5b6da1-deff08b1`/`S0-clean-2fa1b8c-5683d120` 现只作失败取证；其 VM `6F73F6DC-66DF-40EC-86B8-228C1FDA1195`、`EBF12F50-33B1-4711-B693-B57D419EAE2A` 均保持运行且离线。两次 package/status/XDG 未变，只留下各自空 state/operations root；详细 artifact/operation/S0 hash 见 L6 runbook。
- 第三个 pair 固定 source `55351f2`/target `512e8ab`，record SHA-256 为 `2f2deaed6c8886cfcc4751ccc56439bda95f311767587dc74103645664258697`；builder、host handoff 与 root-owned guest input 已逐哈希复验。执行 VM `99FF4B3F-4894-4913-BBE3-4934DC27BEEB` 当前停止，唯一 S0 为 `S0-clean-512e8ab-84d59494`；断网复验后 source/S0 无漂移且 qcow 无句柄，package/state/evidence/五项 XDG absent，dpkg status/audit、20 项依赖、字体与 `01777 root:root` `/run/lock` 均通过。完整 package/ELF/S0/local evidence hash 见 L6 runbook与本周周志。

## 停止线

- 未获授权不得在真实 guest 运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 install/repair/remove/rollback。
- 不停止、恢复、清理或复用当前两个 disposable L6 failure 现场；其两个旧 pair/handoff/S0 只保留取证，不得在原处替换 maintenance executable 后重试。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步事项（2026-08-09）

1. 保持 `e5b6da1` 与 `2fa1b8c` 两个 L6 failure overlay 运行且离线，P04、CleanBase、DependencyFrozen、builders 与第三个基线 guest 关机；不清理任一空 state root，不恢复或覆盖既有 S0/handoff。
2. 只从 `S0-clean-512e8ab-84d59494` 启动第三个 disposable guest，另行生成新的 operation ID，并单步授权 source revision 1 首次 install mutation；复验 terminal receipt、guard/tmp、dpkg status/audit、依赖/字体、完整 inventory、startup decision 与五个 XDG absent 前不启动产品。
3. 首次 install 完成后仍停止，不自动进入 upgrade。其余五步、八个 crash/retry、产品启动、重启、任何 failure 现场处置与 P05C 都继续逐项授权；旧资产、远端与发布保持关闭。

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

上述仓库入口本身只证明合同与合成状态。前两个 canonical ARM64 pair 已在真实 guest 暴露 pre-receipt 环境缺口；第三个包含 `f0415ad` 的 package/executable identity、独立 S0 与断网 preflight 已闭合，但尚未生成 operation ID 或运行 production/acceptance CLI。当前仍不证明 process-group/dpkg mutation、桌面安装、L6 完成或公开发布。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W32.md)
