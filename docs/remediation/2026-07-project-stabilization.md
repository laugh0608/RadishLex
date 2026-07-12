# RadishLex 项目稳定化整改专题（2026-07）

本文档是 2026 年 7 月全仓审计后的临时执行专题，读者是当前整改实现者和审阅者。它只记录整改范围、批次顺序、停止线、资产处置和退出条件；不替代路线图、稳定架构、协议、隐私边界、runbook 或 devlog。

## 文档状态

- 状态：生效，范围已于 2026-07-11 收窄
- 审计基线：`dev` 分支，提交 `089c174`
- 已完成：R00 文档真相源与停止线收敛、R06A 首批质量门禁与 review-only 资产清理
- 当前主批次：R01A 输入契约、进程级 runtime 与 macOS 基础输入
- 并行质量批次：无；R06A 已退出
- 真实用户同步：保持关闭
- 关闭方式：R01A、R02L、R01B 与 R06A 全部退出后，将稳定结论写回正式文档，再把本文移入 `docs/archive/` 并从当前状态入口移除

## 一、整改结论

RadishLex 的本地优先、隐私可信、可解释学习、可删除同步、engine adapter 和平台薄壳方向保持不变。此前主要偏差是技术模块、同步准备和“尚未开放能力”的证明资产增长快于真实用户纵向链。

整改目标因此收窄为：

1. 打通 macOS 真实离线输入。
2. 修正进入真实输入热路径前必须正确的 userdb/ranker 语义。
3. 让真实选择在隐私策略约束下进入学习并影响候选。
4. 建立首批真实 CI 门禁，并删除 review-only 生产源码与 replay 资产。

同步收敛、生产同步、manager 同步 UI 和最终产品打包仍然重要，但它们属于 `docs/roadmap.md` 的 M3/M4 正常产品里程碑，不再作为本次临时整改专题的关闭条件。

## 二、整改范围

本专题必须推动以下结果：

- 平台能获得按键是否消费、即时 commit 和最新 snapshot。
- librime setup/initialize/finalize 收口为进程级 runtime，多 session 不重复初始化。
- macOS InputMethodKit 开发版能在真实应用中离线输入中文。
- selection、negative feedback、delete 和 explicit restore 具备事务性。
- SQLite 并发策略、recency、frequency、suppress/delete/restore 语义可复验。
- 真实选择安全写入 userdb，并参与后续候选重排。
- Clippy、Flutter CI 和 Go race 进入常态门禁。
- Rust `src/` 和 Flutter 产品模型不再承载 approval、migration review、no-symbol 或 future command 审批状态机。

## 三、非目标

整改期间不推进：

- 第二条真实平台主线。
- 真实用户远端同步、恢复码产品成功路径、设备授权产品成功路径或设备撤销产品执行入口。
- Windows、Android、iOS 和 Linux 多平台并行落地。
- 自研完整拼音引擎。
- OIDC、管理控制台、多租户或云端实时转换。
- 最终发布签名、普通用户安装包、完整 librime/schema 分发和供应链发布门禁。
- 新增 readiness、evidence、preview、approval、migration review、fake replay 或 no-symbol 证明层。

## 四、全局停止线

1. R01A 完成前，不新增与真实输入链无关的 future command、native export、approval review、evidence bundle 或 no-symbol 资产。
2. M3 退出前，真实用户同步、非受控远端数据和 manager 产品同步入口保持关闭。
3. 允许使用合成数据、loopback、短生命周期服务和受控集成测试实现并验证同步成功路径；这些证据不能解锁产品入口。
4. 第一平台达到可重复日常输入前，不启动第二平台实现。
5. manager 产品模式不得在真实 FFI 加载失败时静默回退 fixture；fixture 只能由显式 demo mode 启用并持续标识。
6. `local_smoke`、合成 fixture、readiness 状态和设计草案不得单独作为产品阶段完成证据。
7. 输入热路径继续保持本地和离线，不得因整改引入网络依赖。
8. 平台安装、系统输入法启用、系统目录写入和发布动作继续遵守人工授权边界。

如必须突破停止线，应先修改对应稳定边界或 ADR 并取得人工确认；不得通过增加新的 gate 模型绕过决策。

## 五、已确认问题基线

以下问题已经确认，整改实现不再重复证明问题是否存在：

- `ime-ffi` 丢弃 `KeyOutcome` 的 `consumed` 与即时 `commit`。（已由 R01A 第一代码批关闭）
- 仓库缺少供 Swift / Objective-C 消费并受测试约束的 C header 或等价模块边界。（已由 R01A 第一代码批关闭）
- 输入 session 未组合 engine、ranker、userdb 和 privacy policy。
- librime 全局 setup/initialize/finalize 仍由单个 engine session 隐式承担。（已由 R01A 第二代码批关闭）
- userdb 用户意图缺少统一事务，未配置 WAL、busy timeout 和明确并发策略。
- userdb 将毫秒时间戳写入 `recency_score`，ranker 又将其裁剪为 `0..1`；frequency 无界线性增长。
- 同步 merge 缺少稳定设备级 tie-break，签名绑定、KDF 上限、HTTPS orchestration 和资源上限仍未达到开放条件。
- manager 默认 fixture fallback，正常产品包没有携带真实 FFI 与持久化路径。
- PR workflow、ruleset 模板与远端 active ruleset 均已纳入 Flutter、严格 Clippy 和 Go vet/race required checks；远端五项 required checks 与 strict/up-to-date policy 已只读复验。MSRV 和 native bundle 仍属后续独立门禁。
- Rust review-only manager sync command 源码与 Flutter review-only 模型/fixture 已删除；配套文档和 ADR 已在迁移有效安全断言后归档并退出默认阅读链。

## 六、整改批次总览

| 批次 | 名称 | 状态 | 退出结果 |
| --- | --- | --- | --- |
| R00 | 文档真相源与停止线收敛 | 已完成；旧专题文案随 R01A 清理 | 当前入口、长期路线和停止线基本一致 |
| R01A | 输入契约、进程级 runtime 与 macOS 基础输入 | 进行中；基础按键与方向选择语义已形成实机证据，候选视觉高亮和 parent command 菜单仍阻塞，暂停新 build 转入资料调研 | 真实应用可离线完成基础中文输入 |
| R02L | 本地 userdb/ranker 正确性 | 待开始 | 学习、删除、并发和排序语义正确 |
| R01B | 真实学习纵向闭环 | 待开始，依赖 R01A 与 R02L | 真实选择影响后续候选且受隐私策略约束 |
| R06A | 首批质量门禁与 review-only 资产清理 | 已完成 | Clippy/Flutter/Go race 入门禁，审批模型退出生产源码 |

每次只能有一个产品主批次进行；R06A 是并行质量工作流，不得抢占真实输入目标。

## 七、R00：文档真相源与停止线收敛

R00 已完成入口文档、协作认知和整改停止线的主要纠偏。2026-07-11 又将 engine、learning、crypto、FFI 与 manager 稳定专题中的旧阶段流水删减，并补齐 macOS InputMethodKit 平台边界；剩余旧阶段引用只存在于 R06A 待归档或删除的 review-only 资产。

R00 完成不代表代码问题已经修复，也不代表任何产品里程碑已经退出。

## 八、R01A：输入契约、进程级 runtime 与 macOS 基础输入

### 目标

先形成不依赖同步和 manager 的真实离线输入链。

### 必须完成

- 升级版本化按键结果，返回 `consumed`、可选 `commit` 和最新 snapshot。
- 固定 key result、snapshot、candidate view、error 和字符串的所有权与释放责任。
- 提供受测试约束的 C header 或等价 Swift/Objective-C 模块边界。
- 建立进程级 input runtime，收口 librime setup/initialize/finalize 和多 session 生命周期。
- 补 owner-thread、多 session、reset、schema 切换、异常释放和进程退出测试。
- 按 `docs/macos-inputmethodkit-boundary.md` 新增 macOS InputMethodKit 薄壳，只处理系统生命周期、按键、候选、commit 和 Rust FFI。
- 编写开发版安装、启用和移除 runbook；真实安装仍需人工授权。

### 当前完成证据（更新至 2026-07-12）

- ABI contract v3 统一按键处理与候选选择的 Rust-owned `RadishLexKeyResult`，无损返回 `consumed`、可选即时 commit 和同事件 snapshot。
- 失败时 `result_out` 保持为空；owner-thread、空指针、非法 key event、borrowed view 和释放路径已有 host contract 测试。
- `crates/ime-ffi/include/radishlex_input.h` 已覆盖输入侧 ABI，并通过 C11、Objective-C 编译和 Rust function pointer / layout 测试。
- 旧 `push_key` / `push_key_event` 只保留为兼容入口，真实平台主契约切换为 `radishlex_session_handle_key_event`。
- 新增进程级 `RimeRuntime`，统一持有 native API、运行配置与字符串生命周期，并串行化所有 librime 调用。
- 两个 session 与零 session 间隙共享一次 setup / initialize；已初始化期间目录或 deploy 配置冲突返回结构化错误，deploy / session 创建 / schema 选择失败会回滚，只有显式 process shutdown 且零活动 session 时才 finalize。
- 首个成功初始化的 Rime session 固定进程 runtime owner thread；跨线程创建或 shutdown 返回 `InvalidState`，平台壳不能把多个 client 分散到任意线程直接调用 librime。
- adapter stub API 精确验证初始化、创建、销毁和 finalize 次数；`ime-ffi` 增加隔离数据目录下的 gated 双 session peer-release smoke。
- 新增 `platforms/macos-imk/` Objective-C 薄壳：`NSEvent` 规范化、ABI v3 key/selection result、即时 commit、snapshot/candidate 复制、原生 `IMKCandidates`、稳定候选 index、reset/cancel、schema 和 owner-thread 均由同一 wrapper 收口。
- `RLXProcessRuntime` 为每个 input controller 创建独立 session，并在进程 teardown 时先逐个 invalidate session，再调用 `radishlex_rime_runtime_shutdown`；生产条件编译分支不允许回退 demo engine。
- `./scripts/check-macos-imk.sh` 可在不安装系统输入法时构建 contract `.app` bundle，运行 Objective-C → C ABI → Rust session smoke，并检查 plist、rpath、dylib、完整开发签名与关键 symbol；production 分支另有 `-fsyntax-only` 编译门禁。
- `./scripts/check-macos-imk-native.sh` 增加显式 gated native bundle 门禁：拒绝真实用户/runtime 数据目录和 symlink，要求 schema/default/license，固定 deploy policy，并检查架构、三个关键 symbol 与全部 copied data 哈希清单；全部非系统 dylib 会递归封装、改写到 bundle 内 `@rpath`，逐库许可证、签名后哈希和外部绝对依赖拒绝已有自动验证。
- wrapper contract 扩展到完整命名键表、全部 modifier、modifier release、补充平面 Unicode、中文/emoji UTF-8 byte cursor 到 UTF-16 unit 转换、scalar 中间 cursor 拒绝和无 index 候选拒绝。
- native-rime release dylib 已使用现有显式 Homebrew include/lib 构建并复核 `session_new_rime`、`session_handle_key_event` 与 `rime_runtime_shutdown` 导出。
- 采用官方 Apache-2.0 `rime-pinyin-simp` 固定上游 commit，在临时隔离目录保留许可证和来源记录，并移除对其他 schema/preset 的外部依赖；没有读取或修改真实用户 Rime 目录。
- native bundle 已携带上述隔离全拼 shared data 通过架构、依赖、symbol、许可证和哈希清单门禁；真实 librime FFI smoke 已复验 composition、候选、commit、两个 session 共享 runtime 与 peer release 后继续输入。
- native smoke 发现 librime 对不存在 schema 的 `select_schema`/`get_current_schema` 返回过于宽松；adapter 现先读取已部署 schema list，再选择并精确回读。不存在或回读不一致会返回结构化错误，创建期还会销毁 session 并回滚 runtime，对应 stub 与真实 FFI 回归均已覆盖。
- 2026-07-12 新登录确认旧 v13 仍无 source；reference/product probe 将问题定位为失败 Bundle ID/安装路径的 TIS 负缓存。正式身份已固定为 `org.radishlex.inputmethod.macos`、`org.radishlex.inputmethod.macos.Pinyin`、`RadishLexInputMethod.app` 与 `LSUIElement`。用户级 Apple Development v19 已安装、加入并启用；TextEdit 已验证 Space 中文提交与 composition 期间 `Command-N` 未消费，Codex 输入框已人工确认 5×1 原生横排候选可见。
- Rime adapter 现拒绝把 Control/Option/Command 修饰字符降级为普通字母；M1 全拼有候选时由 adapter 以稳定 candidate selection 语义处理 Space，不再依赖临时 schema 的 delimiter/key binder。selection result 保留 optional commit 与更新后的 snapshot，覆盖分段候选只推进 composition 的情况。
- 后续 build 20-28 的短时实机批次验证连续输入、数字选择、翻页、Backspace、Escape、Enter 原文提交和方向键选择对应候选；候选面板收口为进程级对象后未再出现 deactivate 崩溃。grid candidate 现通过缓存 attributed candidate 的公开 identifier 映射显示索引，方向键索引日志与 Space 最终提交一致。
- R01A 仍被两个 InputMethodKit UI 问题阻塞：程序化 `selectCandidateWithIdentifier:` 成功且选择语义正确，但视觉高亮不重绘；系统自动生成不可选择的 parent source/command 区域，`nil`、空菜单与稳定标题菜单分别产生空白行、生命周期回归或重复标题。删除根级 source/icon metadata 与 `clearSelection` 均已被实机证伪。
- build 28 已按 runbook 严格移除；系统设置仅剩系统拼音与美国输入法，公开 TIS 枚举 `matches=0`，用户级 bundle、运行数据和精确进程均不存在。下一判断点改为先完成公开资料、当前 SDK header、系统单 mode bundle metadata 与 reference harness 调研，不再逐 build 试错。

这些证据关闭输入结果、header、进程级 librime runtime、不安装平台 wrapper/contract、隔离 native schema bundle、真实 FFI 调用链、Apple Development 自包含 bundle、TIS 枚举/启用、快捷键未消费、Space/非首候选提交和主要编辑按键子项，不代表真实应用输入矩阵或 UI 已完成。R01A 下一判断点是先形成候选视觉选择与输入菜单的可复验方案，再补 client 切换、进程重启、断网和双应用交叉证据。

### 退出场景

- 全拼产生 composition 与候选。
- 数字键、空格或平台选择提交正确候选。
- 未消费按键交还宿主应用。
- Backspace、Escape、Enter、方向键、取消、重置和 schema 切换行为可复验。
- 两个 session 不重复初始化或破坏 librime 全局状态。
- 断网时全部输入能力可用。

R01A 不要求学习已经接入；真实学习在 R02L 正确性完成后由 R01B 收口。

## 九、R02L：本地 userdb/ranker 正确性

### 目标

在真实选择进入长期学习前，修正本地数据与排序语义。

### 必须完成

- 将 add/restore、selection、negative feedback 和 delete 的多表写入改为事务。
- 配置 WAL、busy timeout、连接策略、文件权限、迁移和损坏恢复边界。
- 将 recency 改为以时间戳为输入的确定性衰减。
- 将 frequency 改为有界或对数增长。
- 明确 suppress、delete 和 manual restore 优先级。
- 为删除身份使用规范化复合身份或抗碰撞稳定标识；同步前不得继续依赖 64 位 FNV 作为唯一身份。
- 建立固定合成排序评测集与输入热路径性能基线。

### 退出证据

- 故障注入证明用户意图要么完整提交、要么完整回滚。
- manager/IME 并发访问不会因默认配置频繁产生 `SQLITE_BUSY`。
- recency 单调衰减、frequency 有界，explain 对应每个实际生效因子。
- 删除、旧状态、显式恢复和备份恢复形成闭环。
- 排序记录 Top-K、MRR 或等价指标，并记录候选重排延迟。

## 十、R01B：真实学习纵向闭环

### 目标

把已正确的本地学习能力接入真实平台输入链。

### 必须完成

- input runtime 组合 engine、ranker、userdb 和 privacy policy。
- 保留 display index、ranked index 与 engine selection index 的稳定映射。
- 真实选择写入事务化 selection，并影响后续候选。
- secure text entry、P0 App 和隐私模式在记录前阻断学习。
- manager 只读取或修改 Rust 真相源，不复制排序和隐私逻辑。

### 退出证据

- 在真实应用连续选择合成词后，后续候选顺序发生可解释变化。
- 重启输入法后学习结果保持。
- P0 与隐私模式输入不产生 selection、weight 或 user term。
- 删除与显式恢复在真实输入链中遵守 R02L 语义。

## 十一、R06A：首批质量门禁与 review-only 资产清理

### 立即门禁

- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- PR CI 增加 Flutter format/analyze/test 和 Go test/race/vet。
- R01A 触及的 FFI 裸指针接口具有一致的 `unsafe` 契约与 `# Safety` 文档。
- native-rime 保留独立 macOS job；默认无 native 环境的 portable baseline 继续存在。

MSRV、最终 native bundle presence、依赖安全和许可证扫描属于 M4 发布门禁，不阻塞 R01A 第一批代码，但必须在产品发布候选前进入 CI。

### review-only 资产处置原则

- 生产源码只保存真实 ABI、真实领域模型和必要兼容层。
- ownership、panic、redaction 和 transient secret 断言迁入真实 contract test 或稳定边界文档。
- approval、admission、migration review、evidence bundle、fake replay 和 no-symbol 断言直接删除或归档，不转化为新的运行时状态机。
- 简单的“能力未开放”只通过 capability 缺席、显式 disabled state 和产品测试表达。

退出结论：严格 workspace Clippy 已通过；Rust `manager_sync_command` 主模块与四个 review-only 子/测试模块已删除。FFI ownership、owner-thread、copy-before-release、null release 和 panic payload 脱敏断言已归入真实 ABI/`ffi_support` 测试。Flutter action preview、future command protocol、host review、migration、fake replay 和 transient-secret review fixture 已删除，关闭态、隐私优先、错误可见性与 secret 不持久化/不进入 diagnostics 已迁入稳定测试。配套 command contract、checklist、preview、acceptance、FFI boundary/test plan 与 ADR 0006 已移出默认阅读链并归档。PR workflow 与 ruleset 模板已加入 Rust Clippy、Flutter format/analyze/test 和 Go vet/race 独立检查，本机等价命令通过；GitHub 远端 active ruleset 已只读确认五项 required checks 与 strict/up-to-date policy 生效。R06A 于 2026-07-12 完成退出。

### 必须处置的 Rust 资产

- 删除 `crates/ime-ffi/src/manager_sync_command.rs` 及其 `admission.rs`、`migration_review.rs`、`host_test_gate_review.rs` review-only 模块。
- 从 `crates/ime-ffi/src/lib.rs` 移除 review-only module 接线。
- 只迁移与现有真实 FFI 直接相关的 ownership、panic、redaction 和释放负例。

### 必须处置的 Flutter/Dart 资产

- 删除 `manager_sync_action_preview_models.dart`。
- 从 `manager_sync_entry_models.dart` 与 `manager_sync_models.dart` 移除 approval、evidence、migration 和 future command shape。
- 删除 `sync_ffi_rust_host_contract_review_fixtures.dart`、`sync_ffi_rust_host_migration_review_fixtures.dart` 及对应 migration/replay 测试。
- 将 readiness/evidence fixture 收敛为少量真实 disabled-state、隐私模式和错误可见性测试；不保留未来审批目录。
- 保留 transient secret 不持久化、不日志化、不进入 diagnostics 的稳定安全断言，等待 M3 真实命令时复用。

### 文档资产

- `docs/manager-sync-entry-boundary.md` 只保留 UI 职责、secret 和产品停止线。
- `docs/manager-sync-bridge-command-contract.md`、checklist、preview、acceptance matrix、FFI command boundary、test plan 和 ADR 0006 在有效安全规则迁出后归档。
- 被归档文档从 README、当前状态和默认阅读链移除。

### 退出证据

- Rust `src/` 不再包含 review-only manager sync command module。
- Flutter 产品模型不再包含 action preview、approval、migration review 或 evidence bundle 业务层。
- Clippy、Flutter CI 和 Go race 成为可阻止合并的真实门禁。
- 删除前已指出每条保留安全断言的新位置，未迁移的隐私或生命周期断言不被误删。

## 十二、整改后的正常路线

本专题关闭后按 `docs/roadmap.md` 推进：

1. M1/M2 的 macOS 离线输入与本地个人化形成首个本地 MVP。
2. 同步 merge、签名、KDF、HTTPS 和 orchestration 进入 M3，不再使用 R03/R04 作为临时整改编号。
3. manager 本地产品能力属于 M2；同步 UI 属于 M3；最终 bundle、librime/schema 分发、MSRV 和发布门禁属于 M4。
4. 第二平台只有在 M2 退出后评估。

## 十三、批次更新规则

- 本文只更新批次状态、阻塞项、验收证据链接和退出判断，不追加逐日流水。
- 每个可分割实现步骤完成后执行匹配验证；跨边界或批次退出时才运行仓库级门禁。
- 验证命令、提交列表和当天事实写入 `docs/devlogs/YYYY-Www.md`。
- 稳定接口、协议、隐私和平台决策写回正式专题，不以本文作为长期引用源。
- 如果阻塞，记录具体缺失能力和解除条件；不得用新增 review 模型代替实现。

## 十四、总体验收与退出条件

本专题在以下条件同时满足后关闭：

- macOS 开发版能在真实应用中离线完成连续中文输入。
- KeyOutcome、snapshot、commit、FFI ownership 和进程级 librime runtime 有自动契约证据。
- userdb 用户意图具备事务性，SQLite 并发策略可复验。
- ranker 具备有效 recency、有界 frequency、明确优先级和固定评测基线。
- 真实输入选择安全写入 userdb，并影响后续候选；P0/隐私模式阻断有效。
- Clippy、Flutter CI 和 Go race 进入常态门禁。
- review-only Rust/Dart 生产资产已清理，有效安全断言已有真实归属。
- 当前状态、路线图、技术方案、协作入口与实现一致。

关闭本专题不代表 M3 加密同步 Beta 或 M4 产品发布候选已经完成。真实用户同步继续保持关闭，直到 M3 全部退出标准满足。
