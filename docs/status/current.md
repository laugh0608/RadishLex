# RadishLex 当前状态

本文是日常推进短入口，供维护者判断里程碑、验证基线、停止线和下一步。详细设计与历史进入边界文档、runbook 和周志。

## 当前判断

- 复核日期：2026-07-27（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前里程碑：M4 产品发布候选
- 当前主批次：M4-P03 安装载体与发布供应链
- 已退出：M0-M3、M4-P01 双 bundle 产品装配、M4-P02 Application Support v1 数据升级协调器
- 真实用户同步：保持关闭；只允许合成数据与受控集成测试

## M4 稳定事实

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

ad-hoc 只提供包内完整性与事务 identity，不提供 Apple 发布者认证；DMG 不签名、不公证。build 35 传播 quarantine；build 36 对 `0444` dylib 清理失败并在提交前关闭，失败 receipt/staging 已移入可恢复备份。adapter 现以 `ditto --noqtn` 排除传播，逐节点审计无 quarantine，并重复 identity 复验后才记录 evidence。build 37 已完成独立下载与真实用户域安装、输入、repair、remove 证据，但因 Installer 生命周期代码变化不再作为最终载体。

`26.7.1 (38)` 已重新装配双 bundle、Installer 和社区 DMG。DMG 大小 `32196093` bytes，SHA-256 `f171e74bdc0a429655a84b30429481bce3926b17076d09298feed77d9ce4ce4e`，evidence 明确 `apple_notarized=false`；ProductManifest、InstallPayload、strict ad-hoc identity、挂载复验、载体 contract、Installer 精准门禁与完整仓库基线均通过。`dev` 已推送到 `642cbb4`，远端 draft 已改名为 build 38 并仅含 DMG、checksum、evidence 三项匹配资产；Release 仍为 draft、`publishedAt=null`，无正式 Git tag。用户通过已登录 Chrome 形成的独立下载副本大小与 SHA-256 精确匹配本地候选，逐字节比较一致，并带有 Chrome 写入的真实 quarantine 和 GitHub Release 来源元数据；该候选尚未人工放行或进入真实用户域验收，不能继承 build 37 的对应成功断言。

用户安装必须先核对 SHA-256，再使用“系统设置 → 隐私与安全性 → 仍要打开”。终端 fallback 只作用于复制到 `$HOME/Applications` 的精确 `RadishLex Installer.app`，不使用 `sudo`，不得对宽泛目录递归移除 quarantine。完整步骤见 [macOS 社区 ad-hoc DMG Runbook](../runbooks/macos-release-carrier.md)。

## Installer 与升级

Installer UI/driver snapshot v1、restartable executor 与原生 bridge ABI v1 已落地。UI 只展示 verified operation、固定目标、receipt 进度、稳定错误与默认保留数据语义；所有 mutation 都重新投影并要求显式确认，未知版本、action、枚举、authorization flag 或进度失败关闭。AppKit 壳现提供标准 Application 菜单和 `⌘Q` Quit action，关闭最后窗口仍终止进程；正常退出不清理或改写持久化事务，菜单结构与 key equivalent 已进入原生 contract 门禁。

生产 bootstrap 使用 `geteuid/getpwuid_r` 和当前 executable 固定形成 home/resources，严格读取 sealed release identity 与内嵌 InstallPayload。first install 只在显式 action 后创建缺失的固定目录；既有对象不 chmod。repair、默认程序移除和恢复共用真实 mutation port。

upgrade 只从当前外层 receipt 精确选择 `UpgradeSources/<version>-<build>`，在任何 mutation 前复验历史 source manifest/tree/ad-hoc identity、release 顺序与 helper。缺失 source 返回 `driver_unavailable`；错误、重复、身份漂移或非历史 source 返回产品身份阻断。首发 `UpgradeSources` 为空，不伪造上一版本。

build 37 的 first install 与 repair 均先到 `prepared` 静止边界，再完成双 bundle 事务；安装后逐节点 quarantine 为零，ProductManifest、sealed release identity、strict ad-hoc identity 和固定用户域启动均通过。公开合成输入 `zhongwen` 正常展示候选并提交“中文”。repair 替换双 bundle但保留 Application Support、Rime 与 userdb inode；默认 remove 到达 `remove_programs/completed`，双 bundle 与 TIS 已清零，数据、receipt、sidecar 和历史 operation 保留。

## 当前停止线

- 不宣称社区包具备 Developer ID、Apple 公证、Gatekeeper 自动通过或 Apple 已验证。
- 首个正式版本继续关闭真实用户同步，不开放恢复码、设备授权、撤销或轮换的产品成功入口。
- P0 永不学习/同步；P1 原始事件只留本地；P2 只允许端到端加密对象。
- 输入热路径保持完全本地；Go server 不解密、不排序、不保存明文用户词或候选偏好。
- 不同时展开第二真实平台主线，不为形式统一把 Application Support v1 迁入 App Group。
- 不自动清理 staging、backup、历史 operation 或身份绑定终态材料。

## 下一步顺位

1. build 37 首次安装、固定路径启动、双端 startup gate、Manager/输入 smoke、repair 与默认 remove 证据链已闭合；当前真实用户域停在保留 Application Support 和历史事务材料的 completed remove 现场。
2. build 38 已完成标准 Application 菜单、`⌘Q`、版本传播、双 bundle/Installer/DMG 装配、自动门禁、`dev` 推送、远端 draft 三项资产替换及 Chrome 独立下载；下载副本的大小、SHA-256、逐字节比较与真实 quarantine 均已复验。
3. 下一步使用同一 build 38 下载副本人工复验 `⌘Q`、关闭最后窗口、`prepared` 退出后重新打开续跑、首次安装、固定路径启动、输入 smoke、repair 与默认程序 remove；不得复用 build 37 的 bundle identity、载体摘要或实机成功断言。
4. 新候选通过后整理发布说明，明确社区 ad-hoc、未公证、SHA-256、人工放行、默认 remove 保留数据和真实用户同步关闭；push、tag 和正式 Release 仍需分别授权。
5. 恢复空数据基线、清理本轮 operation 或处置历史材料必须另取固定白名单与 receipt 绑定授权；不得把默认保留数据的 remove 冒充数据卸载。
6. 首发形成后把该 assembly 作为下一版本真实历史 source，证明跨发布 upgrade、重启续跑与 source 回滚；真实用户同步继续关闭。

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
./scripts/check-repo.sh
./scripts/check-docs.sh
git diff --check
```

真实系统安装、输入源变更、用户数据、公开上传和 Release 仍需对应授权。

## 阅读索引

- [路线图](../roadmap.md)
- [macOS 产品包边界](../macos-product-package-boundary.md)
- [macOS 程序安装事务](../macos-installation-transaction.md)
- [macOS Installer App 边界](../macos-installer-app-boundary.md)
- [macOS 数据升级协调器](../macos-data-upgrade-coordinator.md)
- [macOS 社区 ad-hoc DMG Runbook](../runbooks/macos-release-carrier.md)
- [本周周志](../devlogs/2026-W31.md)
