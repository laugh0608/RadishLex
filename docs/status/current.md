# RadishLex 当前状态

本文是日常推进短入口，供维护者判断里程碑、验证基线、停止线和下一步。详细设计与历史进入边界文档、runbook 和周志。

## 当前判断

- 复核日期：2026-07-27（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品
- 当前主批次：M5-P02 Fcitx5 addon、共享 FFI 与开发构建
- 已退出：M0-M3；M4 macOS build 38 单版本产品验收已冻结；M5-P01 第二平台决策与运行边界
- 真实用户同步：保持关闭；只允许合成数据与受控集成测试

## M5-P02 当前实现

首个可审阅批次已经建立 `platforms/linux-fcitx5/` 的真实 C++17/CMake addon 工程、ABI v9 owned projection、Fcitx input panel/session 接线、共享 XDG resolver、addon/input method metadata 和 `./scripts/check-linux-fcitx5.sh`。自动 contract 覆盖 Unicode/named key、modifier/phase、owned `KeyResult`/snapshot、display-index selection、owner-thread、reset/free/shutdown，以及 XDG 默认/override、`0700`/`0600`、relative path、symlink、宽权限和 production/test override 隔离。

ABI 审计确认 personalized Rime session、`LearningContext`、同事件 snapshot 与 display-index selection 可直接复用 v9，本批没有增加 Fcitx 私有 ABI。候选内容和顺序只来自 Rust snapshot；Fcitx candidate list 维护可见 cursor，数字、Space 和鼠标最终统一回传 Rust display index，C++ 不解释 engine index。

当前机器为 macOS，未安装 CMake、Fcitx5 development package 或 Linux runtime；Apple clang 平台无关 contract 已通过，但 addon 本体尚无真实 Linux/Fcitx5 编译或运行证据。普通非 terminal context 暂以 `context_known = 0` 失败关闭个人化；Linux Manager privacy 配置、真实桌面输入和安装维护分别留在 P04、P03 与 P05。

## macOS 冻结基线

M4-P01 已形成离线双 bundle 产品装配。根 `version.json` 是版本/build 唯一人工真相源；当前修复候选递增为 Radish CalVer `26.7.1 (38)`。产品 metadata 固定 macOS 13.0、FFI ABI v9、userdb v9、RimeData v2 和 `community-adhoc-v1`；ProductManifest v3 绑定双 bundle tree、许可证、版本与 schema。

RimeData 来源锁固定 `radishlex_pinyin`、Apache-2.0 `pinyin_simp` 词典和逐资产许可证；首发不携带 LGPL `prelude`、`stroke`。真实 `librime 1.17.0`、native FFI、递归 dylib 与 manifest 门禁已通过。

M4-P02 已闭合只读 inspection、一致快照、隔离 candidate/settings、receipt/guard、原子切换、双端验证、精确 inode 回滚和逐 checkpoint 静止证明。隔离资格使用真实双 bundle/helper 与合成 Application Support 覆盖失败、回滚和重启恢复，不冒充真实历史发布。

M4-P03 固定 DMG + 独立用户域 Installer app。Manager、InputMethod 和 install state 位于 authoritative current-user home，不请求管理员权限：

```text
Applications/RadishLex Manager.app
Library/Input Methods/RadishLexInputMethod.app
Library/Application Support/RadishLex
Library/Application Support/RadishLex/.radishlex-install-v1
```

`ime-product-install` 已统一 first install、upgrade、repair、remove 的 source/target、receipt/guard、双 component 程序切换与恢复。外层终态共用两段动作：精确复验 receipt、guard、双 switch store 与最终程序结果后分别持久化 `final_verified`、`completed`；upgrade 还复验 data receipt/guard、Application Support identity、release、data `completed` 与 installed 双 bundle。中断后从已持久化阶段重新取证续跑，不清理 staging、backup 或历史 operation。

独立外层 `radishlex_product_install_startup_gate` 已进入 ABI v9。Manager/InputMethod 均先执行外层 gate，再执行数据 gate，之后才允许 Flutter、IMK、Rime、userdb、settings 初始化。运行身份只由当前真实用户域 bundle、Info.plist、完整 tree 和 code identity 形成，不接受 UI、settings、`HOME` 或调用方自报。active guard、非终态/损坏 receipt、未知对象、身份漂移、completed remove 与未知 FFI 结果均失败关闭；日志只保留稳定 decision/error/state。

## 社区发布身份

首发不加入付费 Apple Developer Program，明确采用 `community-adhoc-v1`：

- Installer、Manager 与 InputMethod 使用 strict ad-hoc code signature；
- production adapter 要求 `TeamIdentifier=not set`、`Signature=adhoc`、CodeDirectory ad-hoc flag、primary CDHash 与 designated requirement 全部精确匹配；
- `ReleaseIdentity.json` format v2 绑定 target 与全部显式历史 source 的 Manager/InputMethod requirement 有界、排序、去重集合；两端集合不得重叠；
- Developer ID、Apple Development、unknown requirement、manifest/tree/bundle ID 漂移均失败关闭。

ad-hoc 只提供包内完整性与事务 identity，不提供 Apple 发布者认证；DMG 不签名、不公证。历史 build 35-37 的 quarantine、只读 dylib 清理失败和 Installer 生命周期证据进入周志保留，不作为当前载体。

`26.7.1 (38)` 已重新装配双 bundle、Installer 和社区 DMG。DMG 大小 `32196093` bytes，SHA-256 `f171e74bdc0a429655a84b30429481bce3926b17076d09298feed77d9ce4ce4e`，evidence 明确 `apple_notarized=false`；ProductManifest、InstallPayload、strict ad-hoc identity、挂载复验、载体 contract、Installer 精准门禁与完整仓库基线均通过。远端现有 build 38 draft 仍未发布、`publishedAt=null`，无正式 Git tag；当前本地 `dev` 继续保留未推送推进。Chrome 独立下载副本与本地候选逐字节一致，并带有真实 quarantine 和 GitHub Release 来源元数据；标准 Application 菜单、`⌘Q`、关闭最后窗口和 `prepared` 事务重开续跑均通过。

build 38 首次安装已到达 `first_install/completed`，receipt 精确绑定 `26.7.1 (38)` 且无 failure/manual recovery；固定 Manager/InputMethod 双 bundle 均通过严格 codesign、ProductManifest、ReleaseIdentity 与逐节点无 quarantine 复验。Manager 已从固定用户域 executable 启动，正常通过双 startup gate、加载保留的合成词库并显示 `local_only`，`⌘Q` 后进程停止且 userdb sidecar 清零。用户手动切换到 RadishLex 拼音后以公开合成输入 `zhongwen` 正常展示候选并提交“中文”，随后切回系统拼音；公开 API 监视确认完整切换序列。

build 38 repair 已到达 `repair/completed`，source/target 均为 `26.7.1 (38)` 且无 failure/manual recovery；替换后的双 bundle 再次通过冻结 ProductManifest、ReleaseIdentity 与无 quarantine 复验。Application Support、Rime、userdb inode 分别保持 `18234715`、`18250146`、`18237317`，证明程序修复未替换保留数据对象。

用户在系统设置中手动移除输入源后完成 build 38 默认程序 remove。receipt 为 `remove_programs/completed`，source 为 `26.7.1 (38)`、target 为空且无 failure/manual recovery；双 bundle 不存在，TIS `matches/enabled/selected=0/0/0`，Manager/InputMethod 进程停止。Application Support、Rime、userdb inode 与 runtime/sidecar/receipt 均保留；这是默认保留数据的 completed remove 现场，不是数据卸载或空数据基线。

用户安装必须先核对 SHA-256，再使用“系统设置 → 隐私与安全性 → 仍要打开”。终端 fallback 只作用于复制到 `$HOME/Applications` 的精确 `RadishLex Installer.app`，不使用 `sudo`，不得对宽泛目录递归移除 quarantine。完整步骤见 [macOS 社区 ad-hoc DMG Runbook](../runbooks/macos-release-carrier.md)。

build 38 现作为 macOS 冻结参考产品，不继续公开发布。真实跨发布 upgrade、source rollback、Developer ID/公证和正式 Release 作为后续 macOS 兼容与统一发布评审事项保留，不冒充已有证据，也不阻塞 M5 第二平台开发。

## Installer 与升级

Installer UI/driver snapshot v1、restartable executor 与原生 bridge ABI v1 已落地。UI 只展示 verified operation、固定目标、receipt 进度、稳定错误与默认保留数据语义；所有 mutation 都重新投影并要求显式确认，未知版本、action、枚举、authorization flag 或进度失败关闭。AppKit 壳现提供标准 Application 菜单和 `⌘Q` Quit action，关闭最后窗口仍终止进程；正常退出不清理或改写持久化事务，菜单结构与 key equivalent 已进入原生 contract 门禁。

生产 bootstrap 使用 `geteuid/getpwuid_r` 和当前 executable 固定形成 home/resources，严格读取 sealed release identity 与内嵌 InstallPayload。first install 只在显式 action 后创建缺失的固定目录；既有对象不 chmod。repair、默认程序移除和恢复共用真实 mutation port。

upgrade 只从当前外层 receipt 精确选择 `UpgradeSources/<version>-<build>`，在任何 mutation 前复验历史 source manifest/tree/ad-hoc identity、release 顺序与 helper。缺失 source 返回 `driver_unavailable`；错误、重复、身份漂移或非历史 source 返回产品身份阻断。首发 `UpgradeSources` 为空，不伪造上一版本。

build 37 的 first install 与 repair 均先到 `prepared` 静止边界，再完成双 bundle 事务；安装后逐节点 quarantine 为零，ProductManifest、sealed release identity、strict ad-hoc identity 和固定用户域启动均通过。公开合成输入 `zhongwen` 正常展示候选并提交“中文”。repair 替换双 bundle但保留 Application Support、Rime 与 userdb inode；默认 remove 到达 `remove_programs/completed`，双 bundle 与 TIS 已清零，数据、receipt、sidecar 和历史 operation 保留。

## 当前停止线

- 不发布 build 38，不创建正式 Git tag，不修改远端 draft；本地提交不推送，任何远端动作仍需另行授权。
- 保留 build 38 assembly、hash/evidence、验收记录与默认 remove 后的数据语义，不清理或改写为其他基线。
- 不宣称社区包具备 Developer ID、Apple 公证、Gatekeeper 自动通过或 Apple 已验证。
- 首个正式版本继续关闭真实用户同步，不开放恢复码、设备授权、撤销或轮换的产品成功入口。
- P0 永不学习/同步；P1 原始事件只留本地；P2 只允许端到端加密对象。
- 输入热路径保持完全本地；Go server 不解密、不排序、不保存明文用户词或候选偏好。
- M5 只推进 Linux Fcitx5，不并行实现 Android、Windows 或 iOS，不为形式统一把 Application Support v1 迁入 App Group。
- Fcitx5 addon 只承担平台生命周期、按键、框架候选面板、commit 与 FFI；不得复制 engine、ranker、userdb、privacy 或同步逻辑。
- 不自动清理 staging、backup、历史 operation 或身份绑定终态材料。

## 下一步顺位

1. M5-P01 已固定 [第二平台 ADR](../adr/0009-second-platform-linux-fcitx5.md)、[Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)、路线图、架构职责、XDG 数据语义、验证分层和停止线，并通过文档与完整仓库门禁。
2. M5-P02 首个实现批次已完成 ABI v9 审计、C++/CMake addon、owner-thread/session、Fcitx input panel、共享 XDG resolver 与平台无关 contract；下一批需要在获准的真实 Linux 开发环境完成 native-rime cdylib + Fcitx5 addon 编译，修正真实 header/link 差异并固定可持续构建基线。
3. M5-P03 在可持续复验的真实 Linux 环境完成 Wayland 主路径、X11 兼容、常见应用输入、切换/重启、断网和 secure/unknown 上下文验收。
4. M5-P04/P05 依次推进 Linux Manager 同库个人化与 Linux 安装、升级、修复、移除、数据保留；不提前并行 Android IME。

## 验证入口

```bash
./scripts/check-macos-product-metadata.sh
./scripts/check-macos-install-layout.sh
./scripts/check-product-install-core.sh
./scripts/check-macos-install-adapter.sh
./scripts/check-macos-install-coordinator.sh
./scripts/check-macos-installer.sh
./scripts/check-macos-release-carrier.sh
./scripts/check-manager-product.sh
./scripts/check-macos-imk.sh
./scripts/check-macos-upgrade-product-coordination.sh
./scripts/check-linux-fcitx5.sh
./scripts/check-repo.sh
./scripts/check-docs.sh
git diff --check
```

真实系统安装、输入源变更、用户数据、公开上传和 Release 仍需对应授权。

## 阅读索引

- [路线图](../roadmap.md)
- [第二平台 Linux Fcitx5 ADR](../adr/0009-second-platform-linux-fcitx5.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [macOS 产品包边界](../macos-product-package-boundary.md)
- [macOS 程序安装事务](../macos-installation-transaction.md)
- [macOS Installer App 边界](../macos-installer-app-boundary.md)
- [macOS 数据升级协调器](../macos-data-upgrade-coordinator.md)
- [macOS 社区 ad-hoc DMG Runbook](../runbooks/macos-release-carrier.md)
- [本周周志](../devlogs/2026-W31.md)
