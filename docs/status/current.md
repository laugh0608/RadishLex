# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、停止线和下一步的维护者与协作者。本文不记录历史流水、完整字段或操作步骤；详细事实进入稳定边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-17（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M2 本地个人化 MVP
- 当前产品主批次：manager 本地产品模式
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

Flutter manager 已有本地词库、导入导出、学习摘要、rank explain、真实 Dart FFI smoke、脱敏诊断和同步关闭态原型证据，但正常产品运行态尚未退出：

- 正常 manager 构建包尚未携带匹配版本的 RadishLex native library，也未固定受控平台持久化目录与权限。
- manager 尚未与 macOS 输入 runtime 共享真实 userdb；双连接锁竞争、migration 所有权和失败诊断缺少产品证据。
- 未显式配置环境时仍可使用 fixture；产品模式必须明确失败，fixture 只能由持续标识的 demo mode 启用。
- 尚缺无需 shell 环境变量的本地产品 smoke，不能把开发期 FFI smoke 或 widget fixture 当作 M2 退出证据。

## 当前停止线

- R01B 已关闭；除非生产输入行为或隐私策略发生回归，不重复完整实机矩阵，也不把 manager 工作重新包装为 R01B。
- manager 产品模式不得在真实 FFI 加载、版本、路径或 userdb 打开失败时静默回退 fixture；不得直接复制 Rust 的排序、删除、恢复或隐私真相源。
- P1 原始选择事件继续只留本地，不进入 manager 展示、诊断、提交记录或同步对象。
- M3 退出前不开放真实用户同步、非受控远端数据、恢复码/设备授权产品成功路径或设备撤销产品入口。
- M2 退出前不启动第二真实平台主线；M4 前不宣称普通用户安装包、最终 librime/schema 分发或发布供应链已经完成。

## 下一步顺位

1. 以 [manager 边界](../manager-ui-boundary.md) 和 [本地验收口径](../manager-local-acceptance.md) 为 M2 设计入口，先固定 product/demo 启动模式、native library 装载与版本失败语义、平台持久化路径和权限。
2. 让 manager 与输入法通过 Rust 真相源访问同一受控 userdb，明确 migration 所有权，并补双连接 WAL/busy、删除/恢复和损坏保留测试。
3. 在正常 manager 构建包中闭合本地词库、学习摘要、隐私设置、rank explain 与结构化诊断主要路径；产品模式失败必须可见，demo mode 必须持续标识。
4. 完成无需 shell 环境变量的本地产品 smoke 和匹配门禁后再判断 M2 退出；真实同步、第二平台与发布打包继续保持停止。

## 验证入口

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
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
- [manager 本地验收](../manager-local-acceptance.md)：已有原型证据、产品缺口与验证入口。
- [macOS 平台边界](../macos-inputmethodkit-boundary.md)：runtime、隐私、数据与 R01B 稳定结论。
- [R01B 验收 runbook](../runbooks/macos-r01b-personalization-acceptance.md)：关闭证据、授权与可复验流程。
- [技术方案](../technical-plan.md)：架构与职责。
- [仓库结构](../repository-layout.md)：目录边界。
- [隐私与同步](../privacy-sync.md)：数据分级、删除与威胁模型。
- [本周周志](../devlogs/2026-W29.md)：验证与历史流水。
