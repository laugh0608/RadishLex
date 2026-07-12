# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前产品里程碑、整改批次、验证基线、停止线和下一步的维护者与协作者。本文不记录历史流水、完整接口字段、长篇实现清单或操作 runbook；详细事实按链接进入路线图、整改专题和 devlog。

## 当前判断

- 复核日期：2026-07-12（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 分支闭环：阶段性 `dev -> master` PR 合并后，必须在下一批常规开发前将最新 `master` merge 回 `dev`，正式口径见 ADR 0001
- 当前产品里程碑：M1 macOS 离线输入 Alpha
- 当前整改主批次：R01A 输入契约、进程级 runtime 与 macOS 基础输入
- 并行质量批次：无；R06A 已退出
- 已完成批次：R00 文档真相源与停止线收敛、R06A 首批质量门禁与 review-only 资产清理
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；受控同步实现与测试可继续

2026-07-12 的 Apple Development 短时实机批次已验证连续中文输入、5×1 候选、主要编辑键、Enter 原文提交和方向选择语义；进程级候选面板修复后未再发生 deactivate 崩溃。R01A 仍未退出：`IMKCandidates` 选择索引正确但视觉高亮不重绘，自动 parent command 区域也仍有菜单问题。build 28 已完整移除；设置列表、TIS、bundle、运行数据和进程均无残留。下一步先形成资料证据与可复验方案，不继续逐 build 试错。

长期产品交付顺序见 [产品交付路线图](../roadmap.md)，当前整改批次、停止线、资产处置和退出条件见 [项目稳定化整改专题](../remediation/2026-07-project-stabilization.md)。

## 已有工程证据

- ABI contract v3 无损返回 `consumed`、可选 commit 和同事件 snapshot；候选选择复用 owned result，输入 C header 已通过 C11/Objective-C contract。
- librime 生命周期已收口到进程级 runtime；多 session、owner-thread、配置冲突、失败回滚和 finalize 已有自动或 native smoke。
- macOS Objective-C 薄壳、contract bundle 与 wrapper smoke 已落地，覆盖按键规范化、commit/snapshot/candidate 复制、reset、schema、线程与 teardown。
- 隔离 `rime-pinyin-simp` 的真实 FFI smoke 已覆盖 composition、完整与分段非首候选、Backspace、Escape、Enter、翻页、方向键高亮与 Space、multi-session 和不存在 schema 拒绝；adapter 以 deployed schema list、原生 current-page selection API 与选择后回读固定可用性。
- native 门禁覆盖隔离 schema/data/license、架构、FFI symbol、递归 dylib closure、逐库签名哈希和外部依赖拒绝；不读取用户 Rime 目录。
- macOS bundle 固定正式 Bundle/mode ID、`LSUIElement`、简体中文 metadata、双语标签与 Retina 列表图标，并对完整依赖闭包签名。
- TextEdit/Codex 实机已覆盖连续输入、中文提交、5×1 候选、数字/翻页/编辑键、Enter 原文和方向键后 Space 提交非首候选；系统修饰键保持未消费。索引日志与提交一致，但视觉高亮仍停在首项。
- GitHub 仓库级 `Protect master via PR` ruleset 已只读复验为 active；`Repo Hygiene`、`Repository Baseline`、`Rust Clippy`、`Flutter Manager`、`Go Quality` 五项均为 required checks，且 strict/up-to-date policy 已启用。R06A 已完成退出。

这些证据证明工程原型可继续演进，不证明真实平台输入、生产同步或产品发布已经完成。

## 已确认阻塞

- R01A 不安装 native 行为矩阵已闭合候选选择与主要编辑按键；真实应用方向键已能改变选择索引并提交对应候选，但 `IMKCandidates` 视觉高亮不重绘。输入菜单的自动 parent command 区域在 `nil`、空菜单和稳定标题菜单下分别表现为空白行、生命周期回归或重复标题。正式 mode 当前保持移除；在完成公开 API、系统 bundle metadata 和可复验 harness 调研前不再安装新 build。client 切换、进程重启、断网、中英文混输及两个应用交叉复核仍未完成。
- 输入 session 未组合 engine、ranker、userdb 与 privacy policy，真实选择没有进入平台学习热路径。
- userdb 用户意图缺少统一事务、WAL/busy 策略；ranker recency/frequency 语义需要修正。
- 同步 merge、签名绑定、KDF 上限、secret 生命周期、HTTPS orchestration 和资源上限尚未达到真实用户开放条件。
- manager 默认 fixture fallback，native library 打包、持久化路径和文件权限尚未产品化。

## 当前停止线

- R01A 完成前，不新增与真实输入链无关的 readiness、evidence、preview、approval、migration review、fake replay 或 no-symbol 资产。
- M3 退出前，不开放真实用户同步、非受控远端数据、恢复码产品成功路径、设备授权产品成功路径或设备撤销产品执行入口。
- 允许使用合成数据、loopback、短生命周期服务和受控集成测试实现同步成功路径，但这些证据不能解锁产品入口。
- 第一平台达到可重复日常输入前，不启动第二平台实现。
- manager 产品模式不得把真实 FFI 失败静默伪装为 fixture 成功；fixture 只能由显式 demo mode 启用并持续标识。
- 合成 fixture、local smoke、CLI 输出和设计草案不能单独作为产品阶段完成证据。
- 输入热路径继续保持本地和离线，不引入网络依赖。

## 下一步顺位

1. 暂停新 build，系统查阅 Apple InputMethodKit/IMKCandidates/输入法菜单公开资料、当前 SDK headers 与系统自带单 mode bundle metadata；形成候选视觉选择和 parent command 区域的明确行为模型。
2. 在不安装系统输入法的前提下设计可复验 harness 或最小 reference probe，先证明事件路由、selection identifier、视觉刷新触发和菜单对象结构，再决定产品代码改法；不继续叠加 `clearSelection`、空菜单、身份占位项或 plist fallback。
3. 方案经人工确认后才申请下一次短时安装，并一次性复核视觉高亮、菜单、client 切换、进程重启、断网、中英文混输和两个应用交叉行为；完成即严格移除。
4. R01A 退出后实施 R02L，修正 userdb 事务、SQLite 并发、recency、frequency 与删除语义；R02L 退出后再由 R01B 接入真实学习，之后关闭整改专题并进入 M3。

## 验证入口

```bash
./scripts/check-repo.sh
./scripts/check-macos-imk.sh
./scripts/check-docs.sh
./scripts/check-text-files.sh
git diff --check
cmp -s AGENTS.md CLAUDE.md
```

native-rime 门禁需要显式隔离 schema/shared data/license；真实平台安装、Keychain/Android connected smoke、Docker 长流程和发布部署需要对应环境或人工授权。

## 最小阅读索引

- [整改专题](../remediation/2026-07-project-stabilization.md)：当前批次、停止线、资产处置和退出条件。
- [产品交付路线图](../roadmap.md)：产品里程碑、交付物和退出标准。
- [技术方案](../technical-plan.md)：稳定架构、职责、平台与验证边界。
- [macOS InputMethodKit](../macos-inputmethodkit-boundary.md)：第一平台的 runtime、按键链、目录和验收边界。
- [仓库结构](../repository-layout.md)：实际目录与未落地边界。
- [隐私与同步](../privacy-sync.md)：数据分级、密钥、删除、恢复与威胁模型。
- [本周周志](../devlogs/2026-W28.md)：本周事实、验证和交接记录。
