# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前产品里程碑、整改批次、验证基线、停止线和下一步的维护者与协作者。本文不记录历史流水、完整接口字段、长篇实现清单或操作 runbook；详细事实按链接进入路线图、整改专题和 devlog。

## 当前判断

- 复核日期：2026-07-16（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 分支闭环：阶段性 `dev -> master` PR 合并后，必须在下一批常规开发前将最新 `master` merge 回 `dev`，正式口径见 ADR 0001
- 当前产品里程碑：M1 macOS 离线输入 Alpha
- 当前整改主批次：R01B 真实学习纵向闭环
- 并行质量批次：无；R06A 已退出
- 已完成批次：R00 文档真相源与停止线收敛、R01A 输入契约与 macOS 基础输入、R02L 本地 userdb/ranker 正确性、R06A 首批质量门禁与 review-only 资产清理
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；受控同步实现与测试可继续

R01A 已由冻结的 Apple Development `build 32` 在精确 source 通知归属下完成退出：真实 TextEdit/Codex 覆盖连续输入、5×1 候选、主要选择/编辑/提交、光标跟随、长候选、全屏、输入菜单、双 client、进程重启和离线一致性，系统设置、TIS、bundle、运行数据与进程最终回到零残留。构建 30–32 的失败纠偏、TIS 缓存竞态和清理流水保留在整改专题与周志，不再进入默认阅读链。

M1 Alpha 明确保留两项未声明范围：当前单屏环境未覆盖副屏；VoiceOver 能导航 accessibility 焦点但视觉/提交未跟随，进入受支持范围前必须修复并重新实机验收。

R02L 已在不接入真实平台热路径的前提下完成本地正确性收口：userdb schema v3 以统一事务承载 add、explicit restore、selection、negative feedback 和 delete，文件库固定 WAL、5 秒 busy timeout、foreign keys、`synchronous=NORMAL`、独立连接和 Unix 私有权限；v1/v2 原子迁移移除 64 位 FNV 唯一身份，未来 schema 与损坏库显式拒绝并原位保留。ranker 改为显式评估时间、确定性 recency、对数有界 frequency/negative contribution、有限分数和稳定原索引 tie-break；固定 5 例合成集达到 Top-1 `0.8`、Top-3 `1.0`、MRR `0.9`，50 候选延迟只记录可复验观测而不设置易波动 CI 墙钟阈值。删除优先于 suppress 和旧状态，只有版本更新的独立恢复入口可清除 tombstone；同步 payload v1 不再把 `manual_add` 推断为恢复。当前主批次据此切换为 R01B。

R01B 已完成自动化代码批：新增 `ime-runtime` 统一组合 engine、ranker、userdb 与 privacy policy；每个输入 session 使用独立数据库连接，一次读取当前候选页的排序信号，并保留 display 到 engine index 映射。macOS 产品 session 固定使用 `~/Library/Application Support/RadishLex/userdb.sqlite3`，在每次按键和选择前只传递 secure、privacy、上下文是否已知及粗粒度类别；secure/P0/未知应用不读取或写入 userdb，隐私模式只读排序。真实 librime/FFI native smoke 已证明 selection 记录、分段延迟记录、隐私零增量和 secure 阻断；R01B 仍需授权后的真实 TextEdit 学习、输入法进程重启持久化，以及与安装前数据、路径和权限基线一致的最终回滚证据，当前不得标记完成。

2026-07-16 已完成 R01B 实机前置门禁：macOS `--status` 现在分别报告用户级 bundle、固定删除路径祖先安全性、RadishLex 父目录的存在性/类型/权限、Rime 运行目录、`userdb.sqlite3`、已知 SQLite sidecar 与 `stopped|running_verified|running_unverified|unavailable` 进程状态，悬空 symlink 也按 present/unsafe 处理。隔离 `HOME`、工作目录和仅含受控 Bash 解释器与参数校验 stub 的 `PATH`，配合假 TIS/进程状态队列的动态 contract，已证明普通清理只对命令行匹配固定 bundle executable 的进程执行终止，确认停止并复核固定路径祖先后再删除 bundle/Rime，且必须在第二次 TIS 观测归零后才成功；父目录、主库、WAL、SHM、rollback journal 和无关条目保持，删除路径祖先不安全、source 仍 selected、不可选择 parent 仍 enabled、同名进程身份不符、终止失败、进程不可观测或清理后 TIS 残留时均拒绝或失败。当前只读基线为 TIS `matches=0 enabled=0 selected=0`、`cleanup_path_ancestors=safe`、bundle/Rime/userdb/sidecar 不存在、进程停止，父目录是预存空目录且 mode 为 `0755`；路径状态不推断测试数据所有权。产品 runtime 会把该预存目录收紧到 `0700`，因此后续实机授权与最终基线复核必须显式处理这一权限变化。

临时通知监视已收口到现有 `tis_source_status.m`：`--monitor` 先输出精确 current source，再在公开 selected-source 通知到达时通过 CFRunLoop 重读并输出 current source，以 `is_radishlex_pinyin` 明确标识正式 Pinyin mode；同一次切换允许出现重复通知，原 Bundle ID 状态输出和清理调用保持兼容。门禁固定编译、默认输出、通知/RunLoop 结构以及 `TISSelectInputSource`、`TISDisableInputSource` 和私有配置禁用线。用户为避免文稿级 source 占用而主动关闭的“自动切换到文稿的输入法”继续保持关闭，执行者不得自动修改。

长期产品交付顺序见 [产品交付路线图](../roadmap.md)，当前整改批次、停止线、资产处置和退出条件见 [项目稳定化整改专题](../remediation/2026-07-project-stabilization.md)。

## 已有工程证据

- ABI contract v4 无损返回 `consumed`、可选 commit、同事件 snapshot、个人化状态和学习处置；candidate view 同时携带 display/engine index，输入 C header 已通过 C11/Objective-C contract。
- `ime-runtime` 已以产品 session 组合 engine、ranker、userdb 与 privacy policy；合成集覆盖重启持久化、映射、分段选择、写失败回滚、读失败降级、删除/显式恢复和 secure/P0/隐私隔离，50 候选延迟只记录观测基线。
- librime 生命周期已收口到进程级 runtime；多 session、owner-thread、配置冲突、失败回滚和 finalize 已有自动或 native smoke。
- macOS Objective-C 薄壳、contract bundle 与 wrapper smoke 已落地，覆盖按键规范化、commit/snapshot/candidate 复制、reset、schema、线程与 teardown。
- 正式 AppKit panel/component contract 动态覆盖非激活窗口、level、Spaces behavior、五候选、视觉/accessibility selection、appearance、合法 inline character index、绝对 insertion fallback、anchor、owner 接管和完整隐藏；controller contract 覆盖 keyDown/keyUp/modifier、候选变化重置、Space/鼠标/accessibility press 到 Rust commit、Enter/Escape、宿主快捷键和双 client 生命周期。
- 正式 TIS 状态/清理入口按精确 Bundle ID 隔离产品与 reference probe；同一工具的 `--monitor` 已通过公开 selected-source 通知、CFRunLoop 和精确 mode 标识形成实时来源记录，不能用不处理通知的进程内轮询冒充。`--status` 已覆盖 bundle、父目录类型/权限、Rime、userdb、sidecar 和进程，只观察固定路径 metadata；动态 contract 固定普通清理保留 userdb family 与父目录。清理仍以系统设置真实移除为前置，不使用 `TISDisableInputSource` 或私有配置替代。
- 隔离 `rime-pinyin-simp` 的真实 FFI smoke 已覆盖 composition、完整与分段非首候选、Backspace、Escape、Enter、翻页、方向键高亮与 Space、multi-session 和不存在 schema 拒绝；adapter 以 deployed schema list、原生 current-page selection API 与选择后回读固定可用性。
- native 门禁覆盖隔离 schema/data/license、产品生成的五项候选页配置、真实 FFI snapshot 页大小、架构、FFI symbol、递归 dylib closure、逐库签名哈希和外部依赖拒绝；不读取用户 Rime 目录。
- macOS bundle 固定正式 Bundle/mode ID、`LSUIElement`、简体中文 metadata、双语标签与 Retina 列表图标，并对完整依赖闭包签名。
- 旧 `IMKCandidates` 实机覆盖连续输入、主要编辑键与提交语义，但视觉高亮不重绘；build 32 的 AppKit panel 已在精确 source 归属下确认五项页、长候选、全屏 Space、方向视觉/提交同 index、双 client、进程重启与离线一致性。
- GitHub 仓库级 `Protect master via PR` ruleset 已只读复验为 active；`Repo Hygiene`、`Repository Baseline`、`Rust Clippy`、`Flutter Manager`、`Go Quality` 五项均为 required checks，且 strict/up-to-date policy 已启用。R06A 已完成退出。
- userdb schema v3 已覆盖多表故障回滚、两个独立文件连接竞争、WAL/权限、v1/v2 迁移、未来版本拒绝、损坏文件保留、规范化删除身份、显式恢复与 P0/P1/P2 隔离；ranker 固定评测覆盖确定性衰减、有界贡献、状态优先级、有限分数与 explain 重构一致性。R02L 已完成退出。

这些证据已证明 macOS 基础中文输入和自动化个人化接线可继续演进，不证明 R01B 真实个人化退出、生产同步或产品发布已经完成。

## 已确认阻塞

- R01B 自动化链和实机前置观测已完成，但尚缺真实 TextEdit 连续选择改变排序、输入法进程重启后保持、delete/explicit restore、P0/隐私零写入及按测试数据所有权执行的最终基线复核；预存空父目录从 `0755` 收紧到 `0700` 的行为还需纳入实机授权与最终权限处置。
- 同步 merge、签名绑定、KDF 上限、secret 生命周期、HTTPS orchestration 和资源上限尚未达到真实用户开放条件。
- manager 默认 fixture fallback，native library 打包、持久化路径和文件权限尚未产品化。

## 当前停止线

- R01B 只把已经通过 R02L 的本地语义接入真实选择、privacy policy 与 ranker，不借接入扩张同步协议、manager 同步 UI、第二平台或新的证明状态机。
- `--status` 只形成时间点 metadata 证据，不自动声明测试数据归属。当前批次必须在复制前以一次完整调用重新得到 `matches=0 enabled=0 selected=0`、bundle absent、祖先 safe、父目录 present/empty/`0755`、Rime/userdb/sidecar absent、进程 stopped，并同时记录隐私设置键的存在性与原值；其后不得插入系统状态变更，任一字段漂移即取消实机批次，不覆盖、迁移或删除现场。
- M3 退出前，不开放真实用户同步、非受控远端数据、恢复码产品成功路径、设备授权产品成功路径或设备撤销产品执行入口。
- 允许使用合成数据、loopback、短生命周期服务和受控集成测试实现同步成功路径，但这些证据不能解锁产品入口。
- 第一平台达到可重复日常输入前，不启动第二平台实现。
- manager 产品模式不得把真实 FFI 失败静默伪装为 fixture 成功；fixture 只能由显式 demo mode 启用并持续标识。
- 合成 fixture、local smoke、CLI 输出和设计草案不能单独作为产品阶段完成证据。
- 输入热路径继续保持本地和离线，不引入网络依赖。
- M1 Alpha 不声明 VoiceOver 候选操作可用；在后续明确支持或宣传 VoiceOver 前，必须修复并重新完成真实辅助功能验收。

## 下一步顺位

1. R01A 已由 build 32 的五项页、全屏/菜单、双 client、进程重启、离线和零残留证据完成退出；保留 VoiceOver 与副屏环境缺口，不在当前批重复消耗实机窗口。
2. R02L 已以事务回滚、SQLite 文件策略和迁移、规范化删除身份、确定性有界排序、状态优先级、固定合成评测及延迟观测完成退出，并作为 R01B 产品运行时的本地语义基础。
3. R01B 自动化代码批与实机前置门禁已经完成；下一步先在仓库内以高于 `build 32` 的新 `CFBundleVersion` 完成 ad-hoc native 门禁并冻结源码、构建输入与候选，不使用 Apple Development identity。
4. 第一阶段授权逐项覆盖同一输入的 Apple Development 重建/签名、复制前原子基线、用户级安装、系统设置与人工交互、进程重启、隐私设置、父目录 `0755 -> 0700`、CLI delete/explicit restore，以及系统设置真实移除和保留 userdb/父目录的普通清理。普通清理、隐私键恢复、数据库连接关闭和数据归属证明完成后，再申请第二阶段精确授权，只删除本轮 userdb family 并把预存空父目录恢复为 `0755`；当前父目录不得删除，归属不清或设置未恢复时保留数据并维持 R01B 进行中。
5. 任一后续实机回归仍使用冻结产物、人工切换/交互、公开通知监视、明确授权和系统设置真实移除；不因 R01A 退出而降低零残留或来源归属要求。

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
