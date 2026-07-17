# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、整改批次、停止线和下一步的维护者与协作者。本文不记录历史流水、完整字段或操作步骤；详细事实进入整改专题、平台边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-17（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M1 macOS 离线输入 Alpha
- 当前整改主批次：R01B 真实学习纵向闭环
- 已完成批次：R00 文档真相源与停止线、R01A 输入契约与 macOS 基础输入、R02L 本地 userdb/ranker 正确性、R06A 首批质量门禁与 review-only 资产清理
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；受控同步实现与测试可继续

R01A 已由 Apple Development `build 32` 的真实 TextEdit/Codex、5×1 候选、主要选择与编辑、光标跟随、全屏、输入菜单、双 client、进程重启、离线和零残留证据完成退出。M1 Alpha 暂不声明副屏和 VoiceOver 候选操作可用；进入受支持范围前必须修复并重新实机验收。

R02L 已完成 userdb schema v3 的事务、并发、迁移、删除/显式恢复和确定性 ranker 语义。R01B 已将其接入 `ime-runtime` 与 macOS 产品 session：secure、P0 和未知上下文使用 engine-only，隐私模式只读排序，正常上下文可学习；真实平台退出证据仍未完成。

## R01B 当前仓库状态

`build 33` 的 ad-hoc 冻结因后续生产分类和验收工具变更而失效，只保留历史记录。`build 34` 已在 clean HEAD `ee331a5` 使用固定 `librime 1.17.0` 与三项哈希一致的隔离数据完成 repository/ad-hoc native 重建，五项产物哈希与提交前候选一致；同一输入的 Apple Development 产物完成签名、安装副本一致性和真实验收。

仓库已具备生产 `LearningContext` 分类、固定合成 case、`case-status`/隔离快照、隐私 receipt、精确进程 stop 和独立授权 B userdb 清理。分类、取证与回滚细节由 R01B runbook 承载；librime user-data 也会改变 UI 顺序，排序归因只认新鲜隔离快照与全库聚合。

本轮真实证据已确认：TextEdit 首次选择把固定目标从隔离 display/engine `1/1` 重排为 `0/1`，第二次与精确进程重启后选择将 frequency 依次推进到 `2`、`3`；delete 产生 tombstone 与一次负反馈、移除 ranker，并在新鲜快照把目标降到 `4/1`、deleted penalty `10`，再次选择不能复活；explicit restore 以 `manual_add` 新版本恢复 `0/1`，后续选择重新从 frequency `1` 建立 ranker；隐私模式下同一真实提交保持全部聚合零增量。

进入 unknown host 后，真实候选显示为 librime 自身已学习后的 `时、是、事、使、市`，该顺序符合 engine-only 可受 engine user-data 影响的边界，不能单独判定异常。但人工异常报告步骤没有先要求取消 composition、切回中立 source 再返回 Codex；随后全库聚合出现四次非固定 case 写入，而固定目标自身未变化。未读取 P1 原文，也不把污染归因给具体应用或正文；批次按停止线终止，P0/secure 未执行。授权 A/B 回滚现已完成：隐私键 absent，TIS/bundle/Rime/userdb/sidecar absent，进程 stopped，预存父目录 empty/`0755`，隔离快照已删除。

## 已确认阻塞

- R01B 尚缺同一冻结 `build 34` 的 unknown/P0/secure 受控补验；排序、重启、删除/恢复、隐私模式与最终零残留已经取得证据。
- unknown/P0 补验必须以全新 userdb 建立一次固定 TextEdit 学习基线；测试宿主无论提交、取消还是候选异常，都要在宿主内结束 composition、切回中立 source 后才允许返回 Codex 报告。任一全库聚合非预期增量立即停止。
- secure field 若由 macOS 直接旁路第三方输入法，应记录“系统 secure 路由旁路，controller secure 分支未由本组实机执行”，不能误记为 `policy_blocked`。
- 同步 merge、签名绑定、KDF 上限、secret 生命周期、HTTPS orchestration 和资源上限尚未达到真实用户开放条件。
- manager 默认 fixture fallback、native library 打包和持久化尚未产品化。

## 当前停止线

- `build 33` 只作历史证据；后续补验只能使用五项 Apple Development 哈希、签名与产品源码均未漂移的同一 `build 34`。任一产品输入变化都必须重新冻结，不得混用本轮证据。
- 实机复制前必须用一次完整状态调用重新确认 TIS zero、bundle/Rime/userdb/sidecar absent、删除路径祖先 safe、预存父目录 empty/`0755`、产品进程 stopped，并用 receipt 固定父目录身份及隐私键 absent/显式 false；任一漂移立即取消批次。
- unknown/P0 ValidationHost 只用于合成应用分类与输入框场景，不读取、记录或持久化输入内容；自动门禁不得启动 GUI host。
- R01B 不扩张同步协议、manager 同步 UI、第二平台或新的证明状态机；输入热路径继续完全本地和离线。
- userdb 默认作为用户数据保留。归属不清、隐私设置未恢复、数据库连接未关闭、receipt/路径身份不一致或发现未知条目时，不删除数据，也不关闭 R01B。
- M3 退出前不开放真实用户同步、非受控远端数据、恢复码/设备授权产品成功路径或设备撤销产品入口。

## 下一步顺位

1. 先提交并验证 R01B 人工交接规则：每组操作必须明确目标宿主；提交或异常都要在该宿主结束 composition、切回中立 source 后才返回 Codex，不跨窗口保留 RadishLex 活跃状态。
2. 对 clean HEAD `ee331a5` 的冻结产物复核产品源码无漂移、Apple Development 五项哈希与签名完全一致；重新取得原子基线和授权 A 后，用全新 userdb 做一次固定 TextEdit 学习种子，再只补 unknown、P0 与 secure 场景。engine-only 的真实 UI 顺序按当时 librime 顺序记录，不预设 display index；只以全库聚合零增量判定。
3. 补验结束仍须完成授权 A 普通清理和独立授权 B 精确 userdb 删除，复验 TIS/bundle/Rime/userdb/sidecar/进程/隐私键及父目录权限全部回到基线。
4. 只有补齐 unknown/P0/secure 且无污染，才关闭 R01B 并按路线图进入 M2 manager 本地产品能力；真实同步、第二平台与发布打包继续保持停止。

R01B 详细步骤、授权边界和证据表见 [macOS R01B 个人化验收 runbook](../runbooks/macos-r01b-personalization-acceptance.md)。

## 验证入口

```bash
./scripts/check-repo.sh
./scripts/check-macos-imk.sh
./scripts/check-docs.sh
./scripts/check-text-files.sh
git diff --check
cmp -s AGENTS.md CLAUDE.md
```

native-rime 门禁需要显式隔离 schema/shared data/license；真实安装、系统设置、用户数据、Keychain/Android connected smoke、Docker 长流程和发布部署需要对应环境或明确授权。

## 阅读索引

- [整改专题](../remediation/2026-07-project-stabilization.md)：批次与退出条件。
- [R01B 验收 runbook](../runbooks/macos-r01b-personalization-acceptance.md)：固定用例、授权与回滚。
- [产品路线图](../roadmap.md)：里程碑与交付物。
- [macOS 平台边界](../macos-inputmethodkit-boundary.md)：runtime、隐私、数据与验证。
- [技术方案](../technical-plan.md)：架构与职责。
- [仓库结构](../repository-layout.md)：目录边界。
- [隐私与同步](../privacy-sync.md)：数据分级、删除与威胁模型。
- [本周周志](../devlogs/2026-W29.md)：验证与历史流水。
