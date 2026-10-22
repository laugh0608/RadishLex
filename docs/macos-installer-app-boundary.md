# macOS Installer App UI 与驱动边界

本文固定 M4-P03 独立 `RadishLex Installer.app` 的状态展示、只读驱动、显式用户授权和失败关闭契约，读者是 Installer UI、安装事务、平台适配与产品门禁维护者。本文不包含真实用户目录写入、程序停止、系统输入源修改、Developer ID、公证、DMG 发布或身份绑定终态清理。

## 分层

```text
AppKit presentation
  ├─ 只展示版本化 snapshot
  ├─ 展示固定目标、operation、进度和稳定错误
  └─ 收集显式确认，不解释 receipt
                    │
                    ▼
InstallerDriver
  ├─ read-only install status projection
  ├─ verified installed-product situation
  └─ stale-action authorization
                    │
                    ▼
InstallerExecutor
  ├─ guard 内重新读取 receipt 与平台 preflight
  ├─ prepared 后等待人工静止确认
  └─ 重放程序事务、数据协调与两段终态
                    │
                    ▼
ime-product-install / macOS adapters
  └─ receipt、guard、程序身份与状态机真相源
```

UI 不直接读取 `.radishlex-install-v1/receipt.json`，不根据 bundle 是否存在猜测 operation，也不接收绝对路径、`HOME`、settings、Manager 页面或调用方自报身份。固定目标仍逐字节来自 `packaging/macos/install-layout.json`。

## 只读状态投影

`ime-product-install::inspect_install_status` 只返回：

- `ReadyFirstLaunch` / `ReadyNoInstallState`；
- `OperationInProgress`；
- `TerminalReceipt`；
- `FailedClosed`；
- 既有稳定 startup error、operation kind、receipt state、failure code 和 manual-recovery 标记。

检查缺失 data root 或状态目录时不创建目录、不 chmod；检查 receipt 时不写临时文件、不清理未知对象。活跃 guard 只返回 `ActiveGuard`，不泄露 receipt 或 operation；已经失去 listener 的 guard socket 只作为可由后续 guard acquire 精确接管的崩溃现场，不由只读检查删除。损坏/中断 receipt、未知对象、unsafe root/state、身份漂移或 IO 失败全部失败关闭。

无 receipt 不能证明已安装产品版本。平台必须按固定双目标和 manifest/code identity 形成以下之一：

- not installed；
- installed release older than payload；
- installed release exactly matches payload；
- installed release newer than payload；
- identity unavailable。

只有前三类能分别形成 first install、upgrade 或 repair；更新版本和身份不可得均阻断。产品身份检查不得移入 UI。

## Snapshot v1

`InstallerViewSnapshot` 固定携带：

- contract version；
- phase：既有安装阶段，以及下述准备/取消的只读维护阶段；
- primary/secondary action；
- operation、receipt state、failure code；
- 0–10 的 receipt 持久化进度；
- stable error、manual prompt；
- `retain_application_support` 数据策略。

进度只表示已持久化状态，不预测耗时，也不能在 UI 定时器中自行前进。重启后必须重新读取 receipt：`prepared` 要求用户完成手动静止步骤；其他非终态提供 resume；`aborted_preserved` / `rolled_back` 提供 retry；completed remove 可再次 first install。completed install/upgrade/repair 必须与当前固定双目标产品身份交叉判定，分别重新提供 upgrade、repair/remove 或失败关闭，不能因存在 terminal receipt 永久隐藏后续 operation。

稳定诊断摘要只允许：

```text
phase=<code> action=<code> error=<code> state=<code>
```

不输出路径、operation ID、bundle identity、签名 requirement、PID、命令行或底层错误正文。未知 contract 版本、phase/action/error/state、进度或 prompt 统一投影为 `blocked + refresh + unknown_driver_result`。

执行 action 返回错误时，bridge 保留重新读取的 receipt state、operation 和持久化进度，但强制 `blocked + refresh`，清除其他 action/manual prompt；保留已有稳定错误，没有时使用既有 `unknown_driver_result`。合法 receipt 甚至 completed 状态不能掩盖当前执行失败。日志只输出固定授权/执行错误类别，不输出底层正文。UI 只启用 snapshot 实际提供的 action，blocked 时只允许 refresh，并提示当前进度不代表上次操作成功。此错误投影不写回 receipt；后续 refresh/重开重新读取持久化状态，不能据其恢复普通投影断言失败原因已经消除。

## 准备与取消的只读维护投影

查询先检查固定数据根的维护记录，再决定是否使用旧 outer 事务投影。准备 marker、取消请求、源库收尾、封存证明及唯一 lifecycle 索引由 upgrade 核心的 `inspect_maintenance` 检查；Executor 另外核验严格 outer canonical、root、operation 和 source/target release/manifest 关系。旧 outer 的 completed/aborted/rolled_back 不能覆盖活动维护状态，也不能继续提供 retry/remove。

| phase | 展示含义 |
| --- | --- |
| `preparing_source` | 新准备等待确认、已有准备待续跑，或已观察到 handoff、等待专用产品接续 |
| `cancellation_requested` | 仅已保存取消请求 |
| `cancellation_finishing_source` | 已记录源库收尾，尚未完成 |
| `cancellation_preserving` | 源库收尾已有记录，封存或索引发布尚未完成 |
| `cancellation_awaiting_qualification` | 完整证明和索引就绪，活动请求 marker 仍阻断最后释放 |
| `cancellation_completed` | 完整历史、祖先、当前旧 outer 和 marker 最终位置已核验 |

维护阶段的基础投影只提供 `refresh`，无 secondary action/manual prompt；operation 为 upgrade，receipt state 为 none，进度为 0，界面隐藏旧 receipt 进度。新准备、准备续跑与取消保存经专用只读准入后，按下文 `11–13` 合同提供动作和静止提示。完整 handoff 或终态封存经 Executor 与生产 Bridge 核验后，`preparing_source` 可提供专用 `continue_prepared_upgrade`、实际 outer state/progress 和手动静止提示；这表示等待当次确认，不表示升级完成。`cancellation_completed` 只描述持久化取消结果，不重新认证旧程序资格，也不授权启动、重试或移除；活动请求存在时绝不显示该阶段。

读取使用现有严格 decoder 和共享物理关系检查，不创建目录、不取得或清理 guard、不写回/补同步记录、不打开 SQLite。活动/未知 guard 阻断；已确认拒绝连接且身份稳定的 stale guard 原样保留。读取前后核对 guard、私有文件和索引身份，并复读结果；缺失索引但有历史、孤立记录、未知/临时对象、证明损坏或身份漂移均失败关闭。历史取消完成后的正常学习不受旧运行库摘要约束；中间状态仍核对其绑定的物理材料。只读观察不能代替执行时的双 guard、逻辑等价、静止或独立资格。

Bridge ABI v1 的结构大小、字段、symbol 与 contract version 保持；phase 整数 7–12 为上述六项的顺序扩展。新 Rust/C/Objective-C 映射一致，旧 reader 对未知 phase/action 失败关闭。action `10` 只接受完整接续投影与 preparing_source/upgrade/实际 outer 进度/静止提示组合；`11–13` 只接受下文列出的无 v1 进度组合。其他维护动作、旧 receipt 进度或提示的矛盾组合仍拒绝。稳定摘要仍仅含既有四个 code 字段。

普通 v1 Executor 在取得 outer guard 后再次检查维护状态，拒绝查询后已过期的旧 intent；upgrade bootstrap 前也检查，避免先创建新的数据事务。专用 `10–13` 不得经普通 v1 Executor 执行。新准备/续跑、取消保存和交接后的升级接续各自进入专用任务；维护查询不调用写入口。独立资格提供者及最终取消释放动作未开放。

## 新准备 Executor 任务入口

`authorize_source_preparation` 绑定合法旧 outer 终态与完整 source/target，`authorize_preparation_resume` 绑定精确准备记录；两者均要求当次显式动作、数据保留、中立输入源和 Manager 关闭四项确认，生成最长 5 分钟、可撤销的内存任务。生产 `prepare_upgrade_source` 不接受 UI 路径或可替换 observer，固定使用安装适配器与既有 root/manifest 绑定的 native 双程序静止/容量观察。

同一次调用连续持有 outer → inner guard，顺序为：严格核对旧 data/lifecycle 前驱 → 独占保存旧 outer canonical 字节并同步/回读 → 绑定原件物理证据并持久化 reservation → 创建同 ID 的 artifact-free `prepared` outer → 源库准备 → 旧槽保留 → 新 v1 handoff。固定封存槽为 `.radishlex-install-history-v1/<previous-outer-operation>/receipt.json`；首次 reservation 的 `previous_install_evidence` 固定状态目录、历史目录与文件身份，后续不能改认。只有完整取消生命周期证明允许复用已有原件；无意图的部分初始化、临时文件、冲突、身份漂移保留阻断。

新 outer 创建前中断只允许 `reserved` 与精确旧终态接续；创建后必须保持同一个 `prepared` outer。每个检查点复核源/目标实物、双锁、当前授权、静止及封存材料，回调前后相同字节的 active outer 文件替换也拒绝。旧运行库在前次终态后的合法学习不受历史长度约束，旧私有材料仍逐件验证；无 data、未释放终态、v1/v2/取消前驱不混用。

成功仅返回 `preflighted` 数据回执，outer 仍为 `prepared`；没有程序切换或正常启动资格。Bridge 的 `11/12` 连接本任务，普通 bootstrap 的空 data 前驱限制保持。此入口不代表 B 段退出、旧程序资格或真实用户升级完成。

### handoff 后的只读检查与严格重绑

准备查询复用 handoff 的物理关系核验，覆盖 receipt、保护快照和 marker 的已绑定移动位置；不调用 SQLite 等价检查。marker 已位于历史时，核心返回 `PreparationHandedOff` 及活动 v1 receipt，复验准备证明、保护快照、旧 inventory/lifecycle 全链和当前 receipt 的不可变字段及原始 artifact 子集。后续 v1 新快照、候选、备份使用原 v1 reader；`switch_prepared` 的三种 rename 场景沿实际槽位检查。查询前后核对活动 receipt、准备 proof、索引与 guard 身份；Executor 另核验 sealed 旧 outer、当前 outer 产品/前驱、outer/data 状态组合，并前后复验 outer 文件身份。没有证明时不能把活动新事务当成旧终态或首次路径。

`rebind_prepared_upgrade` 由已经持有 outer guard 的调用方使用，取得并返回 inner guard，绑定精确 preparation 与已存在的 v1 链。它不创建 receipt、不补同步、不打开 SQLite，也不赋予 mutation 权限；未知或部分终态释放材料交由后续专用流程，不能被重绑认领。首个 v1 mutation 前仍须 fresh 任务授权、程序/静止观察及 handoff 持久化重验。完整交接或终态封存关系可提供 `preparing_source + continue_prepared_upgrade`；部分交接/取消状态继续只刷新，普通 intent 仍阻断。ABI v1 POD/symbol/version 不变。

### 专用接续准入与待开放动作合同

`authorize_prepared_continuation` 绑定精确新 outer 与 `HandoffReady` preparation，要求四项当次确认、最多 5 分钟且可撤销的任务授权。当前准入范围严格为无程序 artifact 的 outer `prepared` 与原始 data `preflighted`；已推进、取消、部分交接、错误产品/root/前驱均不进入此入口。旧 `authorize_preparation_resume` 不能代替接续授权。

`admit_prepared_continuation` 先拒绝授权 root 与 store 不匹配的调用，再取得 outer → inner 双锁，核验完整旧历史与新交接关系，再同步旧 outer 原件及祖先目录、当前 outer、handoff receipt、保护快照与 proof 及各自移动的两侧父目录。`HandoffReady` 重放也必须重新同步活动 data receipt；此前记录可见不能证明 rename 已持久化。各检查点前后核验任务期限、当前回执、封存证据与文件身份，并观察实际 source/target 双程序和 manifest/root 绑定的 native 静止状态；交接原语在锁内复验 SQLite 源库/快照等价。相同字节的 outer 或 proof inode 替换也拒绝。

成功返回 `PreparedContinuationTask`，持有双锁且不推进任何 v1 状态；不输出 stores、guards 或可序列化的执行凭证。`revalidate` 在同一双锁下重复整个准入检查。任务释放、撤销、到期或进程退出后，不能把曾经准入的结果用作后来调用的授权。此入口只完成接续前的授权与持久化重验。`continue_prepared_upgrade` 消费此任务，在同一 outer → inner 双锁内连接程序 staging/preserve/逐端 commit、v1 数据协调及终态释放，不经普通 executor 重新取锁。

进展态另用 `authorize_prepared_upgrade_resume` / `resume_prepared_upgrade`，精确绑定当前 outer、完整 preparation 和 `PreparedUpgradeData::Active` 或 `Releasing`，重取四项确认、最长 5 分钟及可撤销任务。活动 v1 通过 handoff 严格 reader 重绑并补同步实际文件/目录，不改回执 inode；封存开始后的移动位置由 terminal reader 核验。合法 `rollback_required` / `programs_restored` 可在这个专用任务下恢复，未知恢复状态及未绑定临时材料保留阻断。

成功数据先达到 outer `data_settled`，失败数据先恢复双 source 至 `programs_restored`；再将真实 v1 终态及私有材料封存、发布 `release_ready`/索引，以完整证明和实际程序身份结清 outer，最后原 inode 释放 marker 并同步/回读。回调及各程序 rename 边界重复授权、双 guard、物理槽位和专用 native 静止检查；终态前固定 ready proof 的文件身份。`PreparedUpgradeSummary` 只在最后释放验证后返回，正常 WAL 学习不会再受旧 v1 活动回执的历史长度约束。

Driver/Bridge/AppKit 已按以下合同接入 action `10`；新准备/取消保存按下文独立动作合同执行，普通 bootstrap 的空 data 前驱限制保持：

| 环节 | 合同 |
| --- | --- |
| 动作身份 | `ContinuePreparedUpgrade` / `continue_prepared_upgrade` / action 整数 `10` 已完成双向映射；ABI v1 POD、symbol 与版本保持，其他未知值及旧 reader 不认识的 `10` 继续失败关闭 |
| Driver 投影 | 仅完整、fresh 的 handoff/outer/data 关系可提供动作；活动 guard、未知/部分证明、取消材料、产品或 root 漂移只提供刷新或阻断。UI 不传路径、产品身份或 operation ID |
| 授权 | 对本次重新读取的 snapshot 要求四项确认；task 绑定当前精确 outer、preparation、data 状态及产品/root。旧窗口动作、普通 confirm/resume、持久 intent 和上次成功准入都不续权 |
| Bridge dispatch | 从系统 authoritative 用户根与 sealed payload 重建实际适配器，再取得本任务的双锁。先重验持久化，再在同一个任务内执行，不拆成“准入成功后释放锁，再调用普通 executor” |
| 修改前检查 | 程序 staging/preserve/逐端 commit、数据协调的每个修改检查点及终态前，均复验授权、双锁、实际程序槽位与 native 静止；回调后再检查。程序已移动时使用对应阶段的槽位关系，不能一直套用“旧程序仍在安装位置”的准入观察 |
| 中断与重启 | 新授权绑定进展后的 exact outer/data 与不可变交接祖先，并按阶段复验持久化；初始准入仍只接 `prepared/preflighted`；推进态或封存态通过各自精确记录重新授权、核验实际槽位并补同步，禁止 fallback 到初始入口或普通 bootstrap |
| 完成与错误 | 产品完成须同时满足 outer/data 终态与专用终态封存/释放合同；返回准入成功不显示升级完成。撤权、静止丢失、身份漂移或同步失败停止并保留现场，不能吞错或自动降级 |
| 开放门禁 | 同一任务覆盖程序→data→终态、各阶段中断续跑、撤权/漂移矩阵和 Rust/native 双向映射、未知值拒绝；专用检查及完整仓库门禁的本批结果见 W42。该动作不开放新准备或主动取消，独立程序资格与实机另行闭合 |

此合同没有开放真实系统操作；repository-only 文件与进程退出测试不能证明真实断电、WAL/pager 故障或旧程序启动资格。

## 取消 Executor 任务入口

取消执行使用独立的类型化任务，不复用普通 v1 intent 或只读 snapshot 的 refresh：

| 入口 | 授权与结果 |
| --- | --- |
| `authorize_cancellation_preservation` | 固定当前完整 preparation、source/target 双程序身份，要求当次四项确认和最多 5 分钟的可撤销授权 |
| `preserve_preparation_cancellation` | 从已有准备/取消状态接续请求、源库收尾、材料封存和证明/索引发布；成功仅表示等待独立资格，最后请求 marker 保留 |
| `finish_preparation_cancellation` | 使用另外取得的 `authorize_cancellation_release` 授权及显式独立资格提供者，核验同一完整证明后才执行最终释放 |

两个执行入口都先取得 outer guard，再取得 inner guard；同一调用内连续持有双锁，不在阶段切换时释放再取。创建私有历史目录会改变数据根目录链接数，guard 的根匹配使用与原 filesystem/receipt revalidation 相同的 device/inode/owner/mode；state directory 与 socket 身份仍固定，异根、目录替换或权限变化继续拒绝。

请求/源库阶段在每个检查点核验前一 outer 的封存原件、当前合法 old terminal 或 artifact-free prepared new outer；封存中允许核心证明所绑定的 active outer 暂缺与兼容发布位置。全过程复用安装适配器核验实际 source 双 bundle、sealed target 和 root，再执行专用 native 静止/容量观察，返回后重新核验程序与任务授权。容量直接使用此次专用响应，不能从 UI 或旧 target-only preflight 填入。

持久请求不恢复任务授权。撤销、到期、进程退出后须重新确认；续跑只采用核心能够验证的已绑定位置，未知或未绑定临时文件原样阻断。已进入 handoff、记录变化、错误 root/产品、活动 guard 或证据漂移都不能转入取消。preservation 任务不会调用旧程序资格、不会移动最后 marker；最终入口没有默认资格提供者，资格返回后再次核验 outer、程序、静止与授权。

Bridge 的 action `13` 只连接 preservation，不连接最终释放。生产旧程序资格尚缺，保存成功仍显示等待资格且保持启动阻断；最终取消按钮关闭。回归使用合成用户根、显式合成资格和进程退出，不启动真实 GUI、复制冻结程序或试开真实用户库。

## 用户授权

### 新准备与取消保存动作合同（2026-10-17）

本批按以下合同接线；实现与验证完成情况以 `current` 和当周周志为准。准备、取消保存与最终取消释放是独立任务，不以普通升级或 action `10` 替代。

| 动作 | 身份 | 准入与成功结果 |
| --- | --- | --- |
| 开始源库准备 | `BeginSourcePreparation` / `begin_source_preparation` / `11` | 严格旧 outer 合法终态、实际 source 双程序、sealed target、数据根和前驱链一致；调用现有新准备任务，成功仅为 outer `prepared` / data `preflighted` |
| 继续源库准备 | `ResumeSourcePreparation` / `resume_source_preparation` / `12` | 精确绑定当前准备及旧 outer 原件；只续跑同一准备，完成交接后由另外确认的 action `10` 继续升级 |
| 保存取消请求与材料 | `PreservePreparationCancellation` / `preserve_preparation_cancellation` / `13` | 仅可取消的准备或已有取消请求/收尾/封存；调用现有取消 preservation 任务，成功仅进入等待独立资格，最后 marker 保留 |

三项动作沿用 ABI v1 POD、symbol 和 contract version；既有 `0–10` 不变，未知动作失败关闭。不增加最终取消释放动作，不提供默认合格提供者，不调用最终释放入口。取消保存的确认文字必须说明：取消请求记录后本次准备不能转回升级，必要时继续安全的源库收尾，数据和材料保留，但独立旧程序资格未完成时仍不能启动旧版本。不能用“已取消”描述保存成功。

展示合同：

- 开始和续跑均为 `preparing_source + upgrade + state none + progress 0 + error none`，并提供手动中立输入源/关闭 Manager 提示；开始动作只作为 primary，secondary 为 none。
- 续跑作为 primary；只有严格证明尚无交接意图且可取消时，secondary 才能为 `13`。已记录交接意图后不得请求取消；可验证的部分交接只由准备续跑处理。
- 取消保存可以在可取消的准备阶段作为独立 primary 或续跑的 secondary；已有 `cancellation_requested`、`cancellation_finishing_source`、`cancellation_preserving` 只可提供 primary `13`，并保留上述无 v1 进度和手动提示的合同。
- `cancellation_awaiting_qualification` 与 `cancellation_completed` 继续只刷新；最终取消按钮关闭。完整交接和终态封存仍使用 action `10` 既有投影，不能混入 `11–13`。
- 缺失、未知、未绑定材料或矛盾投影失败关闭；取消封存中的 outer 暂缺只能由核心已有的精确封存位置证明解释，不能退回首次安装或扫描认领。

每次 perform 从 authoritative 用户根和 sealed payload 重新读取、重验提供的动作，取得四项当次确认和最多 5 分钟的可撤销内存任务。control 持有到本次同步调用结束；退出、释放、撤销或到期后不得复用。只读 snapshot 不创建目录/guard、不打开 SQLite、不补同步；执行仍由既有 Executor 在连续 outer → inner 双锁中完成，UI 不传路径、身份或 operation ID。

实际 source/target 或前驱资格不满足时不开放新准备，旧普通 bootstrap 的空 data 前驱限制保持，不能用旧升级动作绕过新准备检查。新准备成功不自动串接 action `10`；取消保存不自动请求独立资格或最终释放。错误投影仍为 blocked + refresh，持久化进度不能掩盖本次执行错误。

refresh 是唯一不要求确认的 action。first install、upgrade、repair、resume、retry、remove 和切换前中止恢复都必须对当前重新读取的 snapshot 调用 `authorize_installer_action`；旧窗口中的 action 若已不再提供，必须以 `ActionNotOffered` 拒绝。

用户确认只形成 `AuthorizedInstallerIntent`：

- mutation action 必须显式确认；
- remove 必须确认默认只删除两个程序并保留 Application Support；
- upgrade、repair、retry、remove 与 `confirm_quiescence` 必须确认已手动切到中立输入源并关闭 Manager；
- 所有 mutation intent 都标记为必须重新执行平台 preflight。

确认框和勾选不是静止、运行身份或 TIS 状态证据。后续 executor 仍须使用公开平台 API、固定 bundle ID 和 manifest-bound adapter 重新取证。Installer 不程序化选择、注册、启用或移除输入源。

## 当前产品壳

`platforms/macos-product/InstallerApp/` 是独立 AppKit bundle，不依赖 Flutter/Manager。它：

- 从产品/install layout 元数据生成版本、build、最低 macOS 与 Installer bundle ID；
- 把 committed `InstallLayout.json` 和完整 `InstallPayload/` 嵌入 Resources；
- 显示固定双程序目标、保留的数据根、operation、进度、恢复动作和稳定摘要；
- 拒绝任何命令行参数；
- 使用 ad-hoc 签名只完成构建完整性验证。

AppKit 已静态链接 `radishlex-macos-installer-bridge` ABI v1，并通过三个已绑定 symbol 读取 snapshot、刷新和提交确认。ABI 只使用固定整数 enum 与 POD snapshot；Objective-C 只映射已知值，未知 contract/action/error/state/prompt/进度统一失败关闭。AppDelegate 不含复制、rename、删除、receipt 或 TIS 逻辑。

Installer 必须提供标准 Application 菜单和“退出”命令，`⌘Q` 与关闭最后一个窗口都应终止当前 Installer 进程。正常退出不等价于取消事务：UI 不删除 receipt、guard、staging、backup 或历史 operation，也不把内存 snapshot 写回磁盘；重新打开后始终从持久化 receipt 和 fresh status projection 恢复。若 mutation 正在同步执行，UI 不伪造取消完成或绕过 executor 的持久化边界，异常终止仍由既有 restartable executor 语义接管。

Rust bridge 每次从 fresh status projection 重新授权 action，再进入 `InstallerExecutor`；隔离门禁已证明 `prepared` 持久化、重启投影、stale action 和 active guard。真实用户域 bootstrap 使用 `geteuid/getpwuid_r` 取得 authoritative home，只从当前 executable 固定反推 Installer resources，并复验 sealed `ReleaseIdentity.json` 与内嵌 InstallPayload；不读取 `HOME`、UI 路径或调用方身份。普通开发构建因缺失 identity resource 返回 `product_identity_unavailable`；社区发布构建通过后四类 operation 进入真实 mutation port。upgrade 先复验历史 ProductManifest、完整 tree、strict ad-hoc identity 和 source/target host adapter；缺失 source 在写入前返回 `driver_unavailable`。

`ReleaseIdentity.json` format v2 固定 `community-adhoc-v1`，并记录 target 与全部历史 source 的 Manager/InputMethod exact requirement 有界集合；Installer 自身先通过 strict ad-hoc 结构验证。Installer 不在自身资源内绑定内容相关 `cdhash`。缺失、额外字段、非普通单链接文件、owner 漂移、集合乱序/重复/重叠或 payload signature/tree 漂移全部失败关闭。

first install 的零写入 snapshot 允许固定 data root 尚不存在；只有用户显式确认后，platform adapter 才逐级验证既有 `Library` / `Application Support`，并以 `0700` 创建缺失的 `Applications`、`Library/Input Methods` 和 RadishLex data root。既有目录必须原样满足 owner、canonical path、symlink 和 mode 约束，bridge 不 chmod、不覆盖、不递归创建未知父链。

## 隔离执行器

begin/retry/remove intent 先在外层 guard 内回读 current receipt、验证 operation/source/target chain，并要求 target-only manifest-bound preflight 的 version/build 与 InstallPayload target 精确匹配，再持久化新的 `prepared` 并返回 UI。只有重新投影出的 `ConfirmQuiescence` intent 能推进 `quiesced`；执行前再次 preflight，随后按 receipt evidence 幂等完成双 bundle 切换。

first install、repair、remove 进入统一程序终态。upgrade 在外层 `prepared` 后从外层 source/target、固定 data root、只读 `userdb.sqlite3` schema/identity 与可选 settings/Rime identity bootstrap M4-P02 `preflighted` receipt；若 data receipt 已存在，则只接受同 operation、root、source/target release 与当前 target schema 的持久化链。中断在 `final_verified` 时可重新打开已到 `completed` 的 data receipt，只重新验证终态与双程序，不重复 migration。active guard、stale intent、缺失/漂移 context、platform preflight 失败或任何 receipt/identity 漂移均失败关闭，错误日志只能使用稳定 code。

## 显式切换前中止恢复

`AbortPreSwitchUpgrade` / `abort_pre_switch_upgrade` 只提供给外层 upgrade `data_coordinating`、内层无失败的 `candidate_verified`，以及持有同一份恢复证据的中止/程序恢复重放。它要求显式确认、数据保留、中立输入源与 Manager 关闭四项授权。Rust intent 绑定 operation、data root 的 device/inode/owner/mode、source/target manifest digest 与 build；执行时在 guard 内重验，UI 不能提供路径或身份。ABI v1 的 POD、symbol 和 contract version 不变，整数 action 9 为可加扩展；新 bridge/native 双向映射覆盖此值，旧 reader 对未知值失败关闭。

只读恢复投影使用 `open_existing` 和 `load_for_recovery_inspection`，不 bootstrap、不创建目录、不打开 SQLite。活动或无法判断的 guard 阻断；已确认无监听且身份稳定的 stale guard 只读保留，后续授权执行才按既有机制取得 guard。执行按外层 install、内层 upgrade 顺序加锁，再核对 receipt relationship、真实 preflight、sealed source/target 程序、数据保留证据及恢复各检查点。

首次中止前在固定 data root 下独占创建 `.radishlex-pre-switch-recovery-v1/<operation>/evidence.json`，记录原始双 receipt、受控数据文件身份/摘要与可选 Rime 根身份。文件只读保存原 DB/WAL/SHM 与备份摘要，不打开数据库；详细合同见[数据升级边界](macos-data-upgrade-coordinator.md#显式切换前中止的保留证据)。首次创建后不可重建或改写；证据不完整、缺失或漂移失败关闭。已中止的专用恢复状态不能绕过证据走普通 resume。该证据目录独立于旧状态目录，保留旧 source 的 v1 receipt/startup reader 兼容。

协调器合法持久化内层 `aborted_preserved`，经过共享程序恢复路径到外层 `rolled_back`；每个程序恢复前和两次外层终态前重验真实静止、完整程序身份、双 receipt 与同一份数据证据。失败保留现场，重开须 fresh 授权，不重新记录数据基线。不接受已进入 `switch_prepared` 的数据事务，不提供 checkpoint、清理 sidecar 或自动重试升级。

## 验证

```bash
cargo test --locked -p radishlex-ime-product-install --all-targets
./scripts/check-macos-installer.sh
./scripts/check-repo.sh
```

门禁覆盖 absent root 零写入、显式 first-install 固定目录 provisioning、active/stale guard、非终态重启投影、completed receipt 与后续 operation、产品情况分支、fresh reauthorization、四类执行、preflight 阻断、程序 staging、upgrade receipt bootstrap 与 `final_verified` 重启续跑、历史 source 成功/缺失/重复/内容漂移/receipt release 选择、移除数据保留授权、ABI enum 映射、未知 snapshot/action/authorization bits、原生 bundle metadata、固定 layout、完整内嵌 payload、Installer/双 component release identity 失败关闭、已绑定 Rust symbol、标准退出菜单和 UI 源码禁止边界。普通门禁不启动 GUI、不访问真实用户目录或系统输入源；`⌘Q`、关闭窗口和重启恢复仍需在冻结候选上做人工 UX 复验。

真实用户域负向验收、人工输入源交互、静止检查和回退顺序见 [macOS Installer 真实用户域验收 Runbook](runbooks/macos-installer-user-domain-acceptance.md)。
