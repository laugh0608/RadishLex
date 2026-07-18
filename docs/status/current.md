# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、停止线和下一步的维护者与协作者。本文不记录历史流水、完整字段或操作步骤；详细事实进入稳定边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-18（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M2 本地个人化 MVP
- 当前产品主批次：manager macOS 产品实机验收
- 已完成整改批次：R00、R01A、R02L、R01B、R06A；2026-07 稳定化整改专题已归档
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；受控同步实现与测试可继续

R01A 已以 Apple Development `build 32` 完成真实 TextEdit/Codex、5×1 候选、主要选择与编辑、光标跟随、全屏、输入菜单、双 client、进程重启、离线和零残留证据。M1 Alpha 暂不声明副屏和 VoiceOver 候选操作可用；进入受支持范围前必须修复并重新实机验收。

R02L 已完成 userdb schema v3 的事务、并发、迁移、删除/显式恢复和确定性 ranker 语义。R01B 已把该能力接入 `ime-runtime` 与 macOS 产品 session，并用同一 Apple Development `build 34` 完成真实选择重排、进程重启、删除防复活、显式恢复、隐私模式、unknown、P0、secure 系统路由与最终零残留证据。M1 输入侧真实学习纵向闭环至此关闭，阶段进入 M2 manager 本地产品能力。

## R01B 关闭证据

`build 34` 在 clean HEAD `ee331a5` 使用固定 `librime 1.17.0` 与哈希一致的隔离数据完成 repository/ad-hoc native 重建，五项产物哈希与提交前候选一致；同一输入的 Apple Development 产物完成签名、安装副本一致性和真实验收。

完整序列已证明：TextEdit 首次选择把固定目标从隔离 display/engine `1/1` 重排为 `0/1`；第二次与精确进程重启后的选择把 frequency 推进到 `2`、`3`。delete 产生 tombstone 与一次负反馈、移除 ranker，并在新鲜快照把目标降到 `4/1`；再次选择不能复活。explicit restore 以 `manual_add` 新版本恢复 `0/1`，后续选择从 frequency `1` 重新建立 ranker。隐私模式下同一真实提交保持全库聚合零增量。

同产物补验用全新 userdb 在 TextEdit 建立一次固定学习种子，active/ranker/selection/frequency 均为 `1`。unknown host 与固定 P0 host 中 `时` 均为第 `1` 候选，两组前后全库聚合零增量。secure field 显示 Secure Event Input 已启用，macOS 期间不允许切换到 RadishLex 或系统拼音；解除 secure 聚焦后恢复系统拼音。来源监视未记录 secure 期间 RadishLex source，数据库全量零增量。该项结论固定为“macOS secure 路由旁路，controller secure 分支未由本组实机执行”，不记为 `policy_blocked` 实机通过；controller 分支继续由生产分类 contract 与 native FFI policy 测试约束。

授权 A 普通清理和独立授权 B 数据清理均已完成。最终复验为 TIS `matches=0 enabled=0 selected=0`、bundle/Rime/userdb/sidecar/两个 receipts absent、进程 stopped、隐私键 absent、父目录 empty/`0755`，且无 R01B `/private/tmp` 快照目录。取证与清理全程未读取 P1 原始行或数据库正文。

## M2 当前边界

manager 本地产品运行态的实现与自动门禁已经完成：默认 `product` mode 从 app bundle 加载 ABI v4 native library，固定使用平台 Application Support userdb/settings，路径和权限由 macOS 原生层控制；native、ABI、路径或 userdb 失败会显示结构化启动错误，不会回退 fixture。显式 `demo` 构建持续显示“合成演示数据”。

词库页已区分 active、suppressed、deleted，deleted tombstone 经新 FFI 查询，suppressed/deleted 只能由独立确认动作 explicit restore。manager 隐私设置通过 macOS CurrentUser/AnyHost `CFPreferences` 写入输入 runtime 的真实偏好键并读回；失败会回滚。Rust 文件连接在任何 schema/integrity SQL 前安装 busy timeout，首次 WAL 协商只对锁竞争做有界重试；测试已覆盖八路并发初始化、短时初始化锁、WAL 可见性、输入侧选择、manager 删除、输入侧 tombstone 观察和 manager 恢复。

Release 产品门禁已验证 bundle native library、架构、依赖、签名、ABI/必需符号，并直接用 bundle dylib 和临时合成数据跑通删除、tombstone、重启、恢复、再次删除与导出，不启动 GUI、不触碰真实 userdb。M2 尚未退出，因为正常 Release app 的无环境变量 GUI、固定平台路径重启、隐私真实读回和输入法/manager 同库运行仍需在单独授权下完成实机验收。

M2 实机前的数据回滚边界已补齐：manager 从进程启动起使用 `0077` umask，settings 正式文件与原子写临时文件从创建时即为私有权限；固定 profile 的 helper 以不可覆盖 receipt 绑定安装前空父目录，只允许处理本轮 userdb family、`manager-settings.json` 及其临时文件。未知条目、symlink、sidecar-only、身份/权限漂移、打开句柄或 manager 未停止均失败关闭。实机启动前必须先捕获该 receipt，最终删除仍需独立授权。

## 当前停止线

- R01B 已关闭；除非生产输入行为或隐私策略发生回归，不重复完整实机矩阵，也不把 manager 工作重新包装为 R01B。
- manager 产品模式不得在真实 FFI 加载、版本、路径或 userdb 打开失败时静默回退 fixture；不得直接复制 Rust 的排序、删除、恢复或隐私真相源。
- P1 原始选择事件继续只留本地，不进入 manager 展示、诊断、提交记录或同步对象。
- M3 退出前不开放真实用户同步、非受控远端数据、恢复码/设备授权产品成功路径或设备撤销产品入口。
- M2 退出前不启动第二真实平台主线；M4 前不宣称普通用户安装包、最终 librime/schema 分发或发布供应链已经完成。

## 下一步顺位

1. 在 clean HEAD 重跑 manager、FFI、Release 产品 bundle 与仓库门禁，冻结同一 manager/InputMethodKit 输入和产物证据。
2. 按 [macOS manager 产品验收 runbook](../runbooks/macos-m2-manager-product-acceptance.md) 分阶段取得动作时授权，完成正常 Release app 无环境变量启动、固定路径/权限、GUI 删除/恢复和重启持久化。
3. 在输入法与 manager 同时连接同一测试 userdb 的现场，完成双端状态可见、隐私偏好读回与零学习增量，并把系统、进程、偏好和测试数据恢复到可证明基线。
4. 实机证据全部通过后关闭 M2；下一开发批次进入 M3 同步成功路径与安全证据设计。真实用户同步、第二平台和 M4 发布打包继续保持停止。

## 验证入口

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
./scripts/check-manager-product.sh
./scripts/build-manager-macos-product.sh
./scripts/check-repo.sh
./scripts/check-docs.sh
./scripts/check-text-files.sh
git diff --check
cmp -s AGENTS.md CLAUDE.md
```

native-rime 门禁需要显式隔离 schema/shared data/license；真实安装、系统设置、用户数据、Keychain/Android connected smoke、Docker 长流程和发布部署需要对应环境或明确授权。

## 阅读索引

- [产品路线图](../roadmap.md)：里程碑与交付物。
- [manager 边界](../manager-ui-boundary.md)：M2 本地产品职责与 M3/M4 停止线。
- [manager 本地验收](../manager-local-acceptance.md)：自动产品证据、实机缺口与验证入口。
- [macOS manager 产品验收](../runbooks/macos-m2-manager-product-acceptance.md)：M2 实机授权、验收与清理流程。
- [macOS 平台边界](../macos-inputmethodkit-boundary.md)：runtime、隐私、数据与 R01B 稳定结论。
- [R01B 验收 runbook](../runbooks/macos-r01b-personalization-acceptance.md)：关闭证据、授权与可复验流程。
- [技术方案](../technical-plan.md)：架构与职责。
- [仓库结构](../repository-layout.md)：目录边界。
- [隐私与同步](../privacy-sync.md)：数据分级、删除与威胁模型。
- [本周周志](../devlogs/2026-W29.md)：验证与历史流水。
