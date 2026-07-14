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

`build 31` 已在明确授权后完成 Apple Development 重建、严格签名、安装副本哈希复核、用户级安装和系统设置添加，全程无需注销。初次出现“设置已添加但菜单不发布 source”；经系统设置真实移除并重新添加后，菜单项可由开发者手动选择。自检后的只读 TIS 通知监视精确记录 RadishLex mode 为本组 current source，开发者实体输入确认候选出现、跟随文字光标、右方向迁移到第二项且 Space 提交该项，无异常；返回 Codex 后监视才记录切回系统拼音。因此 build 31 的光标锚点与本组视觉/提交同 index 正式通过。

验收后清理经历设置项回流和 TIS 缓存竞态：TextEdit 文稿先切回系统拼音，完整重启设置后对回流项再次真实移除；删除 bundle/隔离数据并终止进程后，再打开现有列表与可添加目录触发公开扫描。最终 `matches=0 enabled=0 selected=0`，bundle、运行数据与进程均无残留；未使用 `TISDisableInputSource`、私有配置或注销。R01A 仍需第二、三阶段平台与生命周期证据，尚未退出。

临时通知监视已收口到现有 `tis_source_status.m`：`--monitor` 先输出精确 current source，再在公开 selected-source 通知到达时通过 CFRunLoop 输出变化，并以 `is_radishlex_pinyin` 明确标识正式 Pinyin mode；原 Bundle ID 状态输出和清理调用保持兼容。门禁固定编译、默认输出、通知/RunLoop 结构以及 `TISSelectInputSource`、`TISDisableInputSource` 和私有配置禁用线。用户为避免文稿级 source 占用而主动关闭的“自动切换到文稿的输入法”继续保持关闭，执行者不得自动修改。

长期产品交付顺序见 [产品交付路线图](../roadmap.md)，当前整改批次、停止线、资产处置和退出条件见 [项目稳定化整改专题](../remediation/2026-07-project-stabilization.md)。

## 已有工程证据

- ABI contract v3 无损返回 `consumed`、可选 commit 和同事件 snapshot；候选选择复用 owned result，输入 C header 已通过 C11/Objective-C contract。
- librime 生命周期已收口到进程级 runtime；多 session、owner-thread、配置冲突、失败回滚和 finalize 已有自动或 native smoke。
- macOS Objective-C 薄壳、contract bundle 与 wrapper smoke 已落地，覆盖按键规范化、commit/snapshot/candidate 复制、reset、schema、线程与 teardown。
- 正式 AppKit panel/component contract 动态覆盖非激活窗口、level、Spaces behavior、五候选、视觉/accessibility selection、appearance、合法 inline character index、绝对 insertion fallback、anchor、owner 接管和完整隐藏；controller contract 覆盖 keyDown/keyUp/modifier、候选变化重置、Space/鼠标/accessibility press 到 Rust commit、Enter/Escape、宿主快捷键和双 client 生命周期。
- 正式 TIS 状态/清理入口按精确 Bundle ID 隔离产品与 reference probe；同一工具的 `--monitor` 已通过公开 selected-source 通知、CFRunLoop 和精确 mode 标识形成实时来源记录，不能用不处理通知的进程内轮询冒充。清理仍以系统设置真实移除为前置，不使用 `TISDisableInputSource` 或私有配置替代。
- 隔离 `rime-pinyin-simp` 的真实 FFI smoke 已覆盖 composition、完整与分段非首候选、Backspace、Escape、Enter、翻页、方向键高亮与 Space、multi-session 和不存在 schema 拒绝；adapter 以 deployed schema list、原生 current-page selection API 与选择后回读固定可用性。
- native 门禁覆盖隔离 schema/data/license、架构、FFI symbol、递归 dylib closure、逐库签名哈希和外部依赖拒绝；不读取用户 Rime 目录。
- macOS bundle 固定正式 Bundle/mode ID、`LSUIElement`、简体中文 metadata、双语标签与 Retina 列表图标，并对完整依赖闭包签名。
- 旧 `IMKCandidates` 实机覆盖连续输入、主要编辑键与提交语义，但视觉高亮不重绘；build 31 的 AppKit panel 已在精确 source 归属下确认候选跟随光标、右方向视觉迁移且 Space 提交同一第二项。
- GitHub 仓库级 `Protect master via PR` ruleset 已只读复验为 active；`Repo Hygiene`、`Repository Baseline`、`Rust Clippy`、`Flutter Manager`、`Go Quality` 五项均为 required checks，且 strict/up-to-date policy 已启用。R06A 已完成退出。

这些证据证明工程原型可继续演进，不证明真实平台输入、生产同步或产品发布已经完成。

## 已确认阻塞

- R01A 的 AppKit candidate panel 已由 build 31 正式关闭光标锚点、右方向视觉迁移与 Space 提交同 index 子项；尚缺宿主焦点、鼠标、VoiceOver、边缘定位、多屏/全屏、输入菜单和 owner/client 生命周期的集中人工证据。
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

1. build 31 已完成本轮签名实机和零残留清理；正式通知监视已进入仓库。不为后续单项自动重装、注销或程序化选择 source，下一集中窗口继续沿用人工切换/交互与公开通知监视分工。
2. 补宿主焦点、鼠标、VoiceOver、边缘定位、多屏/全屏与输入菜单证据，再补 client 切换、进程重启、断网、中英文混输和双应用生命周期；满足退出场景后关闭 R01A。
3. 任一新实机窗口仍须使用冻结产物、明确授权和系统设置真实移除；完成后复核设置列表、可添加目录、TIS、bundle、运行数据和进程全部零残留。
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
