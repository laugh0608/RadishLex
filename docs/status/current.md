# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前产品里程碑、整改批次、验证基线、停止线和下一步的维护者与协作者。本文不记录历史流水、完整接口字段、长篇实现清单或操作 runbook；详细事实按链接进入路线图、整改专题和 devlog。

## 当前判断

- 复核日期：2026-07-15（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 分支闭环：阶段性 `dev -> master` PR 合并后，必须在下一批常规开发前将最新 `master` merge 回 `dev`，正式口径见 ADR 0001
- 当前产品里程碑：M1 macOS 离线输入 Alpha
- 当前整改主批次：R01B 真实学习纵向闭环
- 并行质量批次：无；R06A 已退出
- 已完成批次：R00 文档真相源与停止线收敛、R01A 输入契约与 macOS 基础输入、R02L 本地 userdb/ranker 正确性、R06A 首批质量门禁与 review-only 资产清理
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；受控同步实现与测试可继续

2026-07-12 实机已验证连续输入、5×1 候选、主要编辑键、Enter 和方向选择提交；`IMKCandidates` 程序化选择的 engine/FFI/Space 索引一致，但视觉不重绘。2026-07-13 的单 mode probe 又证伪 controller-first fallback 与显式 panel `keyDown:`；两次测试均已零残留清理，实验代码已回退，不再生成第三个签名 probe。

2026-07-14 的正式 `build 30` 已完成 Apple Development 签名、用户级安装、一次注销/登录、系统设置添加和实体键盘观察，随后完整清理。实体输入时看到候选条、右方向后高亮移动且 Space 提交移动后候选，但系统开启了“自动切换到文稿的输入法”，事后 TIS 显示 TextEdit 已切回系统拼音；因此这组事件和提交不能归属为 RadishLex 正式通过证据。候选窗同时被观察到固定在屏幕左下角。清理后精确 TIS 为 `matches=0 enabled=0 selected=0`，用户级 bundle、隔离运行数据、精确进程均不存在。

当前 SDK 契约确认 `attributesForCharacterIndex:` 接收 inline session 内的字符索引，不是末尾插入位置。正式实现以 marked range 派生合法索引，保留 `firstRectForCharacterRange:actualRange:` 的文档绝对插入位置 fallback；动态 contract 覆盖越界索引返回有限高度 `(0,0)`、末尾/中间 cursor、无 inline session 和最终 panel 锚点，产品构建号升至 `31`。

`build 31` 已在明确授权后完成 Apple Development 重建、严格签名、安装副本哈希复核、用户级安装和系统设置添加，全程无需注销。初次出现“设置已添加但菜单不发布 source”；经系统设置真实移除并重新添加后，菜单项可由开发者手动选择。自检后的只读 TIS 通知监视精确记录 RadishLex mode 为本组 current source，开发者实体输入确认候选出现、跟随文字光标、右方向迁移到第二项且 Space 提交该项，无异常；返回 Codex 后监视才记录切回系统拼音。因此 build 31 的光标锚点与本组视觉/提交同 index 正式通过。

同一冻结 `build 31` 的后续集中验收在精确 RadishLex source 归属下确认鼠标点击第二候选可提交，且无需重新点击 TextEdit 即可继续产生 composition，鼠标路径与宿主焦点通过。随后 VoiceOver 以 `Control-Option-Right Arrow` 导航到第二候选时，旁白焦点已位于第二项，但候选条视觉高亮仍停在第一项；`Control-Option-Space` 执行 accessibility press 后提交的也不是第二项。本轮按当时停止线立即停止，没有继续边缘定位、外观、长候选、多屏/全屏、输入菜单或生命周期/离线矩阵，也没有在安装现场重建。该缺陷证据保留，但当前产品推进决策将 VoiceOver 完整可用性降为 M1 Alpha 已知限制，不再阻塞 R01A 退出，也不单独触发 `build 32`。

验收后清理经历设置项回流和 TIS 缓存竞态：TextEdit 文稿先切回系统拼音，通过系统设置真实移除；删除 bundle/隔离数据并终止进程后，再以系统设置公开界面刷新现有列表与可添加目录。最终现有列表和“添加 -> 简体中文”目录均无 RadishLex，`matches=0 enabled=0 selected=0`，bundle、运行数据、进程与本轮临时构建目录均无残留；未使用 `TISDisableInputSource`、私有配置或注销，“自动切换到文稿的输入法”仍保持关闭。该时点 R01A 因主输入路径的平台与生命周期矩阵尚未完成而继续开放，后续退出结论见 build 32 记录。

2026-07-15 再次冻结并签名安装 `build 31` 后，通知型 TIS 监视确认正式 Pinyin mode 覆盖每个测试组。候选窗在屏幕上下左右边缘的放置与限制通过，浅色/深色选中态通过且系统外观恢复原始“自动”；长输入组随即发现真实 panel 展示 9 项候选，与正式 5×1 契约冲突。本轮按停止线停止多屏/全屏、输入菜单和生命周期/离线矩阵，未在安装现场重建；系统设置现有列表与“添加 -> 简体中文”目录、TIS、bundle、运行数据和进程均已零残留，添加 source 时系统自动改变的 🌐︎ 键行为也随移除自动恢复。

根因不是 panel 排版，而是本轮隔离 product-authored `default.yaml` 错误配置 `menu.page_size: 9`，controller/panel 又按设计完整展示 engine snapshot；原动态 contract 只注入恰好五项，未覆盖真实配置分叉。修复保持 engine/display index 一致，不在 UI 层截断：仓库现提供产品 `default.yaml` 模板并固定五项候选页，native bundle 构建覆盖 shared-data 输入中的默认配置，门禁逐字节复核模板并以真实 librime/FFI snapshot 断言候选数为 5。用户可见配置变化使下一候选升为 `build 32`；修复提交为 `e44d92f`。

同日冻结的 Apple Development `build 32` 通过生成/安装哈希一致性与正式 source 通知归属；真实 panel 复验 5×1、长候选压缩与 tooltip，全屏 Space、单一输入菜单项、TextEdit/Codex owner 切换、精确进程重启和离线输入均通过。当前机器只有一个显示器，副屏行为明确记为环境未覆盖，M1 Alpha 不据此声明多显示器保证；VoiceOver 仍是既有已知限制。两次系统设置回流移除后，只有通过应用菜单真正退出 `System Settings` 并等待 `KeyboardSettings.appex` 结束，重开的可添加目录才清除缓存项；最终现有列表与可添加目录无 RadishLex，TIS、bundle、运行数据、生成 app 和进程全部零残留，🌐︎ 键行为恢复原值。R01A 据此完成退出，主批次切换到 R02L。

R02L 已在不接入真实平台热路径的前提下完成本地正确性收口：userdb schema v3 以统一事务承载 add、explicit restore、selection、negative feedback 和 delete，文件库固定 WAL、5 秒 busy timeout、foreign keys、`synchronous=NORMAL`、独立连接和 Unix 私有权限；v1/v2 原子迁移移除 64 位 FNV 唯一身份，未来 schema 与损坏库显式拒绝并原位保留。ranker 改为显式评估时间、确定性 recency、对数有界 frequency/negative contribution、有限分数和稳定原索引 tie-break；固定 5 例合成集达到 Top-1 `0.8`、Top-3 `1.0`、MRR `0.9`，50 候选延迟只记录可复验观测而不设置易波动 CI 墙钟阈值。删除优先于 suppress 和旧状态，只有版本更新的独立恢复入口可清除 tombstone；同步 payload v1 不再把 `manual_add` 推断为恢复。当前主批次据此切换为 R01B。

临时通知监视已收口到现有 `tis_source_status.m`：`--monitor` 先输出精确 current source，再在公开 selected-source 通知到达时通过 CFRunLoop 重读并输出 current source，以 `is_radishlex_pinyin` 明确标识正式 Pinyin mode；同一次切换允许出现重复通知，原 Bundle ID 状态输出和清理调用保持兼容。门禁固定编译、默认输出、通知/RunLoop 结构以及 `TISSelectInputSource`、`TISDisableInputSource` 和私有配置禁用线。用户为避免文稿级 source 占用而主动关闭的“自动切换到文稿的输入法”继续保持关闭，执行者不得自动修改。

长期产品交付顺序见 [产品交付路线图](../roadmap.md)，当前整改批次、停止线、资产处置和退出条件见 [项目稳定化整改专题](../remediation/2026-07-project-stabilization.md)。

## 已有工程证据

- ABI contract v3 无损返回 `consumed`、可选 commit 和同事件 snapshot；候选选择复用 owned result，输入 C header 已通过 C11/Objective-C contract。
- librime 生命周期已收口到进程级 runtime；多 session、owner-thread、配置冲突、失败回滚和 finalize 已有自动或 native smoke。
- macOS Objective-C 薄壳、contract bundle 与 wrapper smoke 已落地，覆盖按键规范化、commit/snapshot/candidate 复制、reset、schema、线程与 teardown。
- 正式 AppKit panel/component contract 动态覆盖非激活窗口、level、Spaces behavior、五候选、视觉/accessibility selection、appearance、合法 inline character index、绝对 insertion fallback、anchor、owner 接管和完整隐藏；controller contract 覆盖 keyDown/keyUp/modifier、候选变化重置、Space/鼠标/accessibility press 到 Rust commit、Enter/Escape、宿主快捷键和双 client 生命周期。
- 正式 TIS 状态/清理入口按精确 Bundle ID 隔离产品与 reference probe；同一工具的 `--monitor` 已通过公开 selected-source 通知、CFRunLoop 和精确 mode 标识形成实时来源记录，不能用不处理通知的进程内轮询冒充。清理仍以系统设置真实移除为前置，不使用 `TISDisableInputSource` 或私有配置替代。
- 隔离 `rime-pinyin-simp` 的真实 FFI smoke 已覆盖 composition、完整与分段非首候选、Backspace、Escape、Enter、翻页、方向键高亮与 Space、multi-session 和不存在 schema 拒绝；adapter 以 deployed schema list、原生 current-page selection API 与选择后回读固定可用性。
- native 门禁覆盖隔离 schema/data/license、产品生成的五项候选页配置、真实 FFI snapshot 页大小、架构、FFI symbol、递归 dylib closure、逐库签名哈希和外部依赖拒绝；不读取用户 Rime 目录。
- macOS bundle 固定正式 Bundle/mode ID、`LSUIElement`、简体中文 metadata、双语标签与 Retina 列表图标，并对完整依赖闭包签名。
- 旧 `IMKCandidates` 实机覆盖连续输入、主要编辑键与提交语义，但视觉高亮不重绘；build 32 的 AppKit panel 已在精确 source 归属下确认五项页、长候选、全屏 Space、方向视觉/提交同 index、双 client、进程重启与离线一致性。
- GitHub 仓库级 `Protect master via PR` ruleset 已只读复验为 active；`Repo Hygiene`、`Repository Baseline`、`Rust Clippy`、`Flutter Manager`、`Go Quality` 五项均为 required checks，且 strict/up-to-date policy 已启用。R06A 已完成退出。
- userdb schema v3 已覆盖多表故障回滚、两个独立文件连接竞争、WAL/权限、v1/v2 迁移、未来版本拒绝、损坏文件保留、规范化删除身份、显式恢复与 P0/P1/P2 隔离；ranker 固定评测覆盖确定性衰减、有界贡献、状态优先级、有限分数与 explain 重构一致性。R02L 已完成退出。

这些证据证明工程原型可继续演进，不证明真实平台输入、生产同步或产品发布已经完成。

## 已确认阻塞

- 输入 session 未组合 engine、ranker、userdb 与 privacy policy，真实选择没有进入平台学习热路径。
- 同步 merge、签名绑定、KDF 上限、secret 生命周期、HTTPS orchestration 和资源上限尚未达到真实用户开放条件。
- manager 默认 fixture fallback，native library 打包、持久化路径和文件权限尚未产品化。

## 当前停止线

- R01B 只把已经通过 R02L 的本地语义接入真实选择、privacy policy 与 ranker，不借接入扩张同步协议、manager 同步 UI、第二平台或新的证明状态机。
- M3 退出前，不开放真实用户同步、非受控远端数据、恢复码产品成功路径、设备授权产品成功路径或设备撤销产品执行入口。
- 允许使用合成数据、loopback、短生命周期服务和受控集成测试实现同步成功路径，但这些证据不能解锁产品入口。
- 第一平台达到可重复日常输入前，不启动第二平台实现。
- manager 产品模式不得把真实 FFI 失败静默伪装为 fixture 成功；fixture 只能由显式 demo mode 启用并持续标识。
- 合成 fixture、local smoke、CLI 输出和设计草案不能单独作为产品阶段完成证据。
- 输入热路径继续保持本地和离线，不引入网络依赖。
- M1 Alpha 不声明 VoiceOver 候选操作可用；在后续明确支持或宣传 VoiceOver 前，必须修复并重新完成真实辅助功能验收。

## 下一步顺位

1. R01A 已由 build 32 的五项页、全屏/菜单、双 client、进程重启、离线和零残留证据完成退出；保留 VoiceOver 与副屏环境缺口，不在当前批重复消耗实机窗口。
2. R02L 已以事务回滚、SQLite 文件策略和迁移、规范化删除身份、确定性有界排序、状态优先级、固定合成评测及延迟观测完成退出；真实平台热路径仍未接入学习。
3. 当前进入 R01B，把真实选择接入 privacy policy、userdb 和 ranker，保留 display/ranked/engine index 映射，并用真实应用验证学习持久化与 P0 阻断；不开放真实同步或推进第二平台。
4. 任一后续实机回归仍使用冻结产物、人工切换/交互、公开通知监视、明确授权和系统设置真实移除；不因 R01A 退出而降低零残留或来源归属要求。

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
