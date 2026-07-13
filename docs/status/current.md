# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前产品里程碑、整改批次、验证基线、停止线和下一步的维护者与协作者。本文不记录历史流水、完整接口字段、长篇实现清单或操作 runbook；详细事实按链接进入路线图、整改专题和 devlog。

## 当前判断

- 复核日期：2026-07-13（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 分支闭环：阶段性 `dev -> master` PR 合并后，必须在下一批常规开发前将最新 `master` merge 回 `dev`，正式口径见 ADR 0001
- 当前产品里程碑：M1 macOS 离线输入 Alpha
- 当前整改主批次：R01A 输入契约、进程级 runtime 与 macOS 基础输入
- 并行质量批次：无；R06A 已退出
- 已完成批次：R00 文档真相源与停止线收敛、R06A 首批质量门禁与 review-only 资产清理
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；受控同步实现与测试可继续

2026-07-12 的 Apple Development 短时实机批次已验证连续中文输入、5×1 候选、主要编辑键、Enter 原文提交和方向选择语义；进程级候选面板修复后未再发生 deactivate 崩溃。R01A 仍未退出：`IMKCandidates` 选择索引正确但视觉高亮不重绘，自动 parent command 区域也仍有菜单问题。隔离 root-only reference probe 被 TIS 枚举为单一、可选择的 `TISTypeKeyboardInputMethodWithoutModes`，但不进入系统设置的可添加输入法列表，未能启用；失败 probe 已严格清理。全新 ID/路径的单 mode probe 在 2026-07-13 注销并重新登录后进入系统设置目录，并经授权加入、选择和运行。Codex 输入框中合成字母与固定候选正常出现，首项视觉高亮；按一次右方向键后高亮未移动，Space 仍提交 index 0“候选甲”。这证明当前“方向键返回候选面板、由 callback 同步选择”的 reference 假设失败，但尚不能区分方向事件未被面板消费、面板未改变选择或 callback 未触发。实机已停止；系统设置经过回流后的第二次移除，精确 bundle、独立数据和同名进程已清除，TIS 最终两次为 `matches=0 enabled=0 selected=0`。正式 mode metadata 保持不变。

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

- R01A 不安装 native 行为矩阵已闭合候选选择与主要编辑按键；真实应用方向键已能改变选择索引并提交对应候选，但 `IMKCandidates` 视觉高亮不重绘。输入菜单的自动 parent command 区域在 `nil`、空菜单和稳定标题菜单下分别表现为空白行、生命周期回归或重复标题。单 mode probe 虽在跨登录后进入系统设置并成功运行，但右方向键后视觉高亮和最终提交仍停在 index 0，callback 路径未获得成功证据；当前 probe 方案已失败。正式 mode 当前保持移除，client 切换、进程重启、断网、中英文混输及两个应用交叉复核仍未完成。
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

1. 以 macOS 26.5 SDK header 与本次实机差异为输入，收敛方向事件接收方、`IMKCandidates` 选择状态与 callback 触发边界；当前证据不足以指定根因，不修改正式候选事件模型或输入源身份。
2. 下一单一候选方案只评审 controller 对四方向显式调用候选面板公开 `NSResponder keyDown:`，并记录调用前后 `selectedCandidate`、callback index 与最终提交；Space/Enter 继续留在 controller。未确认方案前不改 probe 或生成新 build。
3. 新 probe 仍以视觉高亮、callback 索引和提交三者一致为一次性判定，经人工确认后才申请短时安装；不得叠加 `clearSelection`、程序化选择、空菜单或 plist fallback。
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
- [下一工作日周志](../devlogs/2026-W29.md)：2026-07-13 起的目标、停止线和交接记录。
