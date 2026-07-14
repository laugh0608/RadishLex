# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前产品里程碑、整改批次、验证基线、停止线和下一步的维护者与协作者。本文不记录历史流水、完整接口字段、长篇实现清单或操作 runbook；详细事实按链接进入路线图、整改专题和 devlog。

## 当前判断

- 复核日期：2026-07-14（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 分支闭环：阶段性 `dev -> master` PR 合并后，必须在下一批常规开发前将最新 `master` merge 回 `dev`，正式口径见 ADR 0001
- 当前产品里程碑：M1 macOS 离线输入 Alpha
- 当前整改主批次：R01A 输入契约、进程级 runtime 与 macOS 基础输入
- 并行质量批次：无；R06A 已退出
- 已完成批次：R00 文档真相源与停止线收敛、R06A 首批质量门禁与 review-only 资产清理
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；受控同步实现与测试可继续

2026-07-12 实机已验证连续输入、5×1 候选、主要编辑键、Enter 和方向选择提交；`IMKCandidates` 程序化选择的 engine/FFI/Space 索引一致，但视觉不重绘。2026-07-13 的单 mode probe 又证伪 controller-first fallback 与显式 panel `keyDown:`；两次测试均已零残留清理，实验代码已回退，不再生成第三个签名 probe。

2026-07-14 的正式 `build 30` 已完成 Apple Development 签名、用户级安装、一次注销/登录、系统设置添加和实体键盘观察，随后完整清理。实体输入时看到候选条、右方向后高亮移动且 Space 提交移动后候选，但系统开启了“自动切换到文稿的输入法”，事后 TIS 显示 TextEdit 已切回系统拼音；因此这组事件和提交不能归属为 RadishLex 正式通过证据。候选窗同时被观察到固定在屏幕左下角。清理后精确 TIS 为 `matches=0 enabled=0 selected=0`，用户级 bundle、隔离运行数据、精确进程均不存在。

当前 SDK 契约确认 `attributesForCharacterIndex:` 接收 inline session 内的字符索引，不是末尾插入位置。正式实现以 marked range 派生合法索引，保留 `firstRectForCharacterRange:actualRange:` 的文档绝对插入位置 fallback；动态 contract 覆盖越界索引返回有限高度 `(0,0)`、末尾/中间 cursor、无 inline session 和最终 panel 锚点，产品构建号升至 `31`。

`build 31` 已在明确授权后使用 Apple Development identity 重建并完成严格签名、安装副本哈希复核、用户级安装和系统设置添加；当前会话无需注销已识别 source。一次公开 TIS 自动选择诊断精确确认 RadishLex 为 current source，但菜单栏仍显示系统拼音，实体输入表现为 RadishLex；开发者手动经 U.S. 切回系统拼音后恢复一致。该现象只证明菜单栏/SystemUIServer 显示可能滞后，不计作候选功能通过。自动选择工具已停止，当前 RadishLex 为 `matches=2 enabled=1 selected=0`，bundle 与隔离运行数据存在、进程停止，等待人工分组测试后完整清理；R01A 未退出。

长期产品交付顺序见 [产品交付路线图](../roadmap.md)，当前整改批次、停止线、资产处置和退出条件见 [项目稳定化整改专题](../remediation/2026-07-project-stabilization.md)。

## 已有工程证据

- ABI contract v3 无损返回 `consumed`、可选 commit 和同事件 snapshot；候选选择复用 owned result，输入 C header 已通过 C11/Objective-C contract。
- librime 生命周期已收口到进程级 runtime；多 session、owner-thread、配置冲突、失败回滚和 finalize 已有自动或 native smoke。
- macOS Objective-C 薄壳、contract bundle 与 wrapper smoke 已落地，覆盖按键规范化、commit/snapshot/candidate 复制、reset、schema、线程与 teardown。
- 正式 AppKit panel/component contract 动态覆盖非激活窗口、level、Spaces behavior、五候选、视觉/accessibility selection、appearance、合法 inline character index、绝对 insertion fallback、anchor、owner 接管和完整隐藏；controller contract 覆盖 keyDown/keyUp/modifier、候选变化重置、Space/鼠标/accessibility press 到 Rust commit、Enter/Escape、宿主快捷键和双 client 生命周期。
- 正式 TIS 状态/清理入口按精确 Bundle ID 隔离产品与 reference probe；系统设置、TIS 与菜单栏呈现都可能短时竞态。来源归属使用输入期间只读精确 source 记录，清理仍以系统设置真实移除为前置，不使用 `TISDisableInputSource` 或私有配置替代。
- 隔离 `rime-pinyin-simp` 的真实 FFI smoke 已覆盖 composition、完整与分段非首候选、Backspace、Escape、Enter、翻页、方向键高亮与 Space、multi-session 和不存在 schema 拒绝；adapter 以 deployed schema list、原生 current-page selection API 与选择后回读固定可用性。
- native 门禁覆盖隔离 schema/data/license、架构、FFI symbol、递归 dylib closure、逐库签名哈希和外部依赖拒绝；不读取用户 Rime 目录。
- macOS bundle 固定正式 Bundle/mode ID、`LSUIElement`、简体中文 metadata、双语标签与 Retina 列表图标，并对完整依赖闭包签名。
- TextEdit/Codex 实机已覆盖连续输入、中文提交、5×1 候选、数字/翻页/编辑键、Enter 原文和方向键后 Space 提交非首候选；系统修饰键保持未消费。索引日志与提交一致，但视觉高亮仍停在首项。
- GitHub 仓库级 `Protect master via PR` ruleset 已只读复验为 active；`Repo Hygiene`、`Repository Baseline`、`Rust Clippy`、`Flutter Manager`、`Go Quality` 五项均为 required checks，且 strict/up-to-date policy 已启用。R06A 已完成退出。

这些证据证明工程原型可继续演进，不证明真实平台输入、生产同步或产品发布已经完成。

## 已确认阻塞

- R01A 的 AppKit candidate panel 已形成 `build 31` 仓库与冻结安装产物，但尚无人工分组的真实应用通过证据。`build 30` 的实体输入来源不明并暴露左下角定位缺陷；`build 31` 的菜单栏竞态诊断也不计为候选通过。后续由执行者做部署、只读 source 监视和清理，开发者手动聚焦、切换并实体交互，再判定视觉/提交同 index、锚点、宿主焦点、鼠标、VoiceOver、多屏/全屏和生命周期。
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

1. 保持当前已签名安装的冻结 `build 31`，不重建、不注销、不自动选择 source 或注入按键；执行者启动只读 source 监视后，每次向开发者交付一组可判伪实体步骤。
2. 开发者先聚焦目标文稿，再手动选择 RadishLex 并完成该组输入；菜单栏只作辅助观察，输入期间精确 source 不匹配或任一行为异常即停止。测试结束后由执行者完成系统设置移除和 TIS、bundle、运行数据、进程零残留清理。
3. panel 通过后补 client 切换、进程重启、断网、中英文混输与两个应用交叉证据，满足退出场景后关闭 R01A。
4. R01A 退出后实施 R02L；R02L 退出后再由 R01B 接入真实学习，之后关闭整改专题并进入 M3。

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
