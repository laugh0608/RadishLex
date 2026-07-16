# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、整改批次、停止线和下一步的维护者与协作者。本文不记录历史流水、完整字段或操作步骤；详细事实进入整改专题、平台边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-16（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M1 macOS 离线输入 Alpha
- 当前整改主批次：R01B 真实学习纵向闭环
- 已完成批次：R00 文档真相源与停止线、R01A 输入契约与 macOS 基础输入、R02L 本地 userdb/ranker 正确性、R06A 首批质量门禁与 review-only 资产清理
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；受控同步实现与测试可继续

R01A 已由 Apple Development `build 32` 的真实 TextEdit/Codex、5×1 候选、主要选择与编辑、光标跟随、全屏、输入菜单、双 client、进程重启、离线和零残留证据完成退出。M1 Alpha 暂不声明副屏和 VoiceOver 候选操作可用；进入受支持范围前必须修复并重新实机验收。

R02L 已完成 userdb schema v3 的事务、并发、迁移、删除/显式恢复和确定性 ranker 语义。R01B 已将其接入 `ime-runtime` 与 macOS 产品 session：secure、P0 和未知上下文使用 engine-only，隐私模式只读排序，正常上下文可学习；真实平台退出证据仍未完成。

## R01B 当前仓库状态

`build 33` 的 ad-hoc 冻结因后续生产分类和验收工具变更而失效，只保留历史记录。`build 34` 代码与工具已提交为 `2464ce4`、`b917827`；提交前 repository/ad-hoc native 门禁和五项候选哈希通过，但尚未在 clean HEAD 重建比对，因此不能进入实机。

当前仓库准备已包含：

- 从正式 controller 抽取生产 `LearningContext` 分类；分类 contract 直接编译该生产源码并覆盖 unknown/P0 host 的固定 Bundle ID，`ValidationHost` 本体只提供对应身份与普通/secure 输入框，仓库门禁不启动 GUI。
- 固定合成用例 `r01b-shi-time-v1`：schema `pinyin_simp`、input `shi`、target `时`、reading absent、context `editor`；隔离初始页为 `是、时、事、使、市`，目标为 display index `1` / engine index `1`。一次真实选择后，fresh isolated non-selection snapshot 可将目标提升到 display index `0`，engine index 仍为 `1`。
- `case-status` 精确检查目标 term/ranker/tombstone 与聚合增减量；删除和恢复再用 fresh isolated Rime user-data snapshot 取证。librime user data 也会改变顺序，UI 不能单独证明 RadishLex ranker。
- `RadishLexPrivacyMode` 固定 CurrentUser/AnyHost 的 CFPreferences domain/key；固定 `0700` state dir 与排他 `0600` receipt 绑定路径和文件身份，先持久化 baseline 再写设置，漂移或不安全时失败关闭。
- 精确进程 stop 只处理命令行匹配固定 bundle executable 的进程，并在删除前复核停止和路径安全。
- 测试 userdb receipt 绑定安装前空父目录与本轮四个 SQLite 文件；只有单独授权且归属、设置恢复、数据库关闭均满足时才执行，父目录始终保留。

提交前已确认精准 Rust/集成测试、`./scripts/check-macos-imk.sh`、`./scripts/check-macos-imk-native.sh` 与 `./scripts/check-repo.sh` 通过；五项候选哈希记录在本周 devlog。该结果不是 clean-HEAD 冻结，提交后仍须用相同输入重跑并逐项比对。

本轮尚未安装或启动输入法、未使用 Apple Development identity、未修改系统设置或隐私键、未终止真实输入法进程，也未打开、迁移或删除真实 userdb。

## 已确认阻塞

- R01B 尚缺同一冻结 `build 34` 的真实选择学习、重启持久化、删除/恢复、隐私分类和最终基线证据。
- secure field 若由 macOS 直接旁路第三方输入法，应记录“系统 secure 路由旁路，controller secure 分支未由本组实机执行”，不能误记为 `policy_blocked`。
- 同步 merge、签名绑定、KDF 上限、secret 生命周期、HTTPS orchestration 和资源上限尚未达到真实用户开放条件。
- manager 默认 fixture fallback、native library 打包和持久化尚未产品化。

## 当前停止线

- `build 33` 只作历史证据；`build 34` 在 clean HEAD 重新通过完整门禁、确认隔离输入与五项产物哈希前，不进入实机授权 A。
- 实机复制前必须用一次完整状态调用重新确认 TIS zero、bundle/Rime/userdb/sidecar absent、删除路径祖先 safe、预存父目录 empty/`0755`、产品进程 stopped，并用 receipt 固定父目录身份及隐私键 absent/显式 false；任一漂移立即取消批次。
- unknown/P0 ValidationHost 只用于合成应用分类与输入框场景，不读取、记录或持久化输入内容；自动门禁不得启动 GUI host。
- R01B 不扩张同步协议、manager 同步 UI、第二平台或新的证明状态机；输入热路径继续完全本地和离线。
- userdb 默认作为用户数据保留。归属不清、隐私设置未恢复、数据库连接未关闭、receipt/路径身份不一致或发现未知条目时，不删除数据，也不关闭 R01B。
- M3 退出前不开放真实用户同步、非受控远端数据、恢复码/设备授权产品成功路径或设备撤销产品入口。

## 下一步顺位

1. 在 clean HEAD 用固定 `librime 1.17.0` 和哈希一致的隔离 `pinyin_simp` 数据重跑 contract、native 与仓库门禁，逐项复核 `build 34` 五项产物哈希并冻结候选。
2. 冻结成功后申请授权 A。授权 A 覆盖 Apple Development 重建/签名、复制前原子基线、用户级安装、系统设置添加、人工 source/实体交互、固定合成用例、进程重启、隐私设置临时变更与恢复、delete/explicit restore，以及保留 userdb/父目录的普通清理。
3. 授权 A 完成普通清理、隐私键恢复、数据库连接关闭和本轮数据归属证明后，再单独申请授权 B。授权 B 只允许精确删除本轮 `userdb.sqlite3`、`-wal`、`-shm`、`-journal`，随后把预存空父目录恢复为 `0755`；不得删除父目录。
4. 只有固定用例的排序、重启、删除/恢复、隐私/P0/secure/unknown 和清理证据全部满足，才关闭 R01B。随后按路线图进入 M2 manager 本地产品能力；真实同步、第二平台与发布打包继续保持停止。

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
