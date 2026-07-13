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

2026-07-12 实机已验证连续输入、5×1 候选、主要编辑键、Enter 和方向选择提交；`IMKCandidates` 程序化选择的 engine/FFI/Space 索引一致，但视觉不重绘。2026-07-13 的单 mode probe 又证伪 controller-first fallback 与显式 panel `keyDown:`；两次测试均已零残留清理，实验代码已回退，不再生成第三个签名 probe。

基于 Apple 公开契约，正式实现改为进程级非激活 AppKit panel：controller 的唯一 display index 同时驱动视觉与 Space，鼠标/辅助功能回调同一 Rust selection API；client 全局行矩形、`windowLevel + 1` 和目标屏幕负责定位。`menu = nil` 继续表达没有专用命令，自动 parent 与空白 command 行作为平台限制，不再用占位菜单修补。不安装门禁与 Apple Development `build 29` 的 native 闭包、签名和用户级安装副本已通过；公开 TIS 枚举不可选择 parent 与唯一可选择 Pinyin mode，但系统设置重启后尚未显示可添加项。当前等待一次开发者注销/重新登录刷新公开目录，输入法没有添加、选择或启动，R01A 未退出。

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

- R01A 的 AppKit candidate panel 已通过不安装编译、bundle、index/frame、owner、焦点与辅助功能静态门禁，但尚无真实应用证据。正式 `build 29` 已安装到用户级目录，公开 TIS 可发现 mode；系统设置尚未显示可添加项，需一次开发者注销/重新登录后只读复核。mode 仍未加入、选择或运行；后续必须先复核视觉/提交同 index、宿主焦点、鼠标、VoiceOver、多屏/全屏和生命周期，再补 client 切换、进程重启、断网、中英文混输及两个应用交叉复核。
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

1. 开发者完成一次注销/重新登录后，先只读复核正式 `build 29` 的公开 TIS 枚举、系统设置可添加目录、用户级 bundle 与精确进程；不重建、不复制、不私下启用。
2. mode 出现后在动作时确认下通过系统设置加入并选择，再按正式产品 runbook 只执行一次可判伪 smoke：方向视觉/Space index、鼠标、宿主焦点、VoiceOver、多屏/全屏与 owner 生命周期任一失败即停止并清理，不叠加 `IMKCandidates` fallback。
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
