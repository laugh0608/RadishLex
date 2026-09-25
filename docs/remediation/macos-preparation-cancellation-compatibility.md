# macOS 准备中止的旧版本兼容补充设计

状态：**待项目所有者确认，未授权实施新增事务路径。** 本文补充[已批准的 WAL 源库准备设计](macos-wal-source-preparation-design.md)，只处理新 v1 handoff 前的主动取消。2026-09-25 完成双层 v1 合同复核与回归测试；本文描述的取消证明、外层封存/恢复和生命周期索引均尚未实现。执行状态以 [current](../status/current.md) 为准。

## 需要确认的调整

建议以独立、严格版本化的准备取消证明表示用户意图。取消完成后，当前真实程序仍是 source，旧版本读取经过核验的原 outer 终态回执，活动 data 状态槽为空；新 Installer 从取消证明识别最近一次维护的真实结果。被取消的新 outer 原字节与准备材料完整封存，不伪造数据事务，不抹去操作历史。

这替代原设计中“维护后创建新 v1 `aborted_preserved`、新 outer 也调用 `abort_preserved`”的主动取消路径。真实失败仍使用既有失败终态，正常完成、回滚和 B6/B7 已实现路径不因此改名。原 v1 字段、失败枚举、`can_replace`、固定状态目录及 guard 身份保持；新增跨 operation 动作只进入专用、持久化的取消协调入口。

确认范围为仓库内的取消记录/索引、专用 outer 封存与原回执兼容恢复、取消接续投影，以及对应合成和独立程序副本资格。维护开始后点击取消，可能需要先完成同一源库准备以证明数据等价；这不等于立即停止 SQLite。范围不包含新依赖、schema、FFI ABI、正常 runtime WAL 策略、真实维护、冻结 39/40 或现场操作。

## 已验证的兼容约束

| 层 | 现行合同 | 对主动取消的影响 |
| --- | --- | --- |
| [数据回执](../../crates/ime-product-upgrade/src/lib.rs) | `aborted_preserved` 必须有已知 `UpgradeFailureCode` 和切换前阶段 | `user_cancelled` 未定义，null 失败码也不合法 |
| [外层回执](../../crates/ime-product-install/src/lib.rs) | `aborted_preserved` 必须有已知 `InstallFailureCode`、提交前阶段，manual recovery 为 false | 同样没有主动取消码，仅修数据层不能解决兼容 |
| 外层正常持久化 | 新事务只能接在匹配的旧终态之后；普通 `can_replace` 禁止逆向恢复 | 恢复兼容回执必须另有完整证明与专用入口，不放宽原方法 |
| 旧 reader | 严格枚举/回执与固定活动目录白名单 | 在 v1 中增加字段、取消状态或未知原因都不能声称旧版本可读 |
| B6 终态封存 | 真实 v1 数据终态，outer/data 同 ID；数据前驱非空 | 不能直接拿它表示尚未创建新 data receipt 的取消 |

[数据合同测试](../../crates/ime-product-upgrade/src/tests.rs)与[外层合同测试](../../crates/ime-product-install/src/tests.rs)均用合法失败终态作为正向对照，验证未知取消枚举、合法 JSON 形状但无失败码的记录被拒绝。外层接续用例同时验证不能逆向 `can_replace`。这些测试只证明仓库 v1 合同，未运行冻结二进制。

不选择用 `snapshot_failed`、`data_coordination_failed`、`io` 等既有失败码承载纯用户意图；没有对应真实失败就不能这样写。也不通过删除全部安装状态、关闭 gate、重开旧终态或把非终态 UI 显示成“取消完成”解决问题。

## 取消入口与数据保持

仅在本次准备存在、真实双程序仍为已绑定 source，且没有持久化 `handoff_intent` 时提供取消。`reserved`、`snapshot_ready`、`maintenance_intent`、`source_prepared`、`previous_archived` 均需按其实际对象关系核验，阶段名称本身不能授权。

取消意图落盘后只能恢复同一取消动作，不再交接新 v1，也不自动重试升级。已有 handoff 意图、新 v1 回执、程序 staging/保留/切换动作或未知对象均不走本入口；保留现有精确恢复路径及其阻断结果。真实 Executor 接线时必须保证准备期间新 outer 仍为无程序 artifact 的 `prepared`。

| 请求时点 | 释放前的源库证明 | 不允许的捷径 |
| --- | --- | --- |
| 维护意图前 | 本次 fresh DB/已有 WAL 仍匹配封存基线；只读操作引起的受限辅助文件变化按原合同核验 | 为了取消而新增 checkpoint/DELETE 维护，或声称所有 sidecar 零变化 |
| 维护意图已写 | 先恢复同一受控准备并证明 standalone、源 schema、全量持久内容/删除语义与保护快照等价、连接已关闭 | 恢复旧物理长度/hash，复制快照覆盖业务库，忽略 hot journal 或 busy |
| 准备成功、旧槽已部分或全部封存 | 重新核验 prepared source、保护快照及每项位置证明 | 重跑旧槽移动、认领无绑定目录，或虚构新 data 终态 |

维护前未产生保护快照的合法路径显式绑定“无快照”；快照创建中断、未绑定临时文件等不确定状态先保留阻断，不把它们解释为从未发生。维护后快照必须存在且保持。旧 data receipt/artifact 按 inventory 原 inode、字节和权限保留；业务 DB/settings/Rime 绝不移入私有历史槽。已释放的旧 data 历史只核验，不再次搬动。

取消前后程序文件均未移动。必须存在一个已封存、合法终态且 installed product 精确匹配当前 source 的前一 outer；没有这一前驱就不具备本方案的取消释放资格，不以“无安装状态”绕过程序身份核验。前一 data 可为空，两种前驱不得混淆；首次安装不属于此 WAL 升级补充范围。

## 证明、索引与两条前驱链

新增 `radishlex-preparation-cancellation-v1` 严格记录，拟用活动固定槽 `preparation-cancellation.json`，最终在本次 operation 历史目录封存。与准备记录相同：canonical UTF-8 JSON 加换行、256 KiB 上限、未知字段/版本/状态拒绝、目录 0700、文件 0600 单链接、同文件系统受控移动；所有身份只追加不改认。

取消证明至少绑定：

- 本次取消 operation、用户意图、原准备阶段、前一 lifecycle/outer/data 引用，及明确不存在的可选对象。
- data root、两侧状态目录、历史目录身份，原准备证明、保护快照、旧私有 inventory 的摘要和位置。
- source/target 产品身份，未切换的双程序身份，以及本次源 family 基线或 prepared source 等价证明。
- 前一 outer 的原 canonical 字节摘要/文件身份，新 outer 的原 canonical 字节摘要/文件身份（尚未写入时明确为空）。
- 每个封存/兼容恢复槽的预期缺失、意图、实物身份及进度，兼容回执与完整 `cancel_ready` 的对应关系。

复用历史根的 `latest-release.json` 作为唯一定位索引，增加显式区分真实终态释放与准备取消的 v2 格式。新 reader 必须继续严格读取既有 B6/B7 v1 索引/证明；新的取消索引绑定取消证明、legacy outer 和可空 data 引用。索引不是终态真相源，必须核对完整证明、实物、前驱与 active outer；不另建第二份“最新”索引。

首次发布 v2 必须绑定当前合法 v1 索引或有证明的空值，之后只能从精确前驱推进。新格式的正常终态封存证明也必须能绑定取消前驱；仅扩展这一外围生命周期表示，三类真实数据终态的既有核验与写入顺序保持。已封存的 v1 证明不升级、不改写；旧准备/封存工具对未知 v2 失败关闭，不能宣称这些工具前向兼容。冻结 source 的兼容目标是其原 v1 活动回执与 startup gate。

每次索引更新都在本次证明中保存前驱 canonical 值与文件身份，重载只接受记录中的旧值或精确新值，不扫描猜测最近 operation。必须保存完整 v1/v2 前驱链及各版本的严格解析测试；未知版本、错误 proof 类型、循环引用、缺失或身份冲突保持阻断。

取消后下一次新准备分别绑定：最近 lifecycle 是取消 operation；legacy outer 前驱是恢复的旧 outer；data 前驱仍是最后真实数据 operation（可空）。新 outer 的 v1 `previous_operation_id` 继续跟随实际 active outer，以满足原 `can_replace`；取消 operation 留在新的生命周期链中，不能被伪装为 v1 数据前驱。重复取消也必须形成完整链，不覆盖之前取消材料。

若取消时第一次封存了旧活动 data 槽，尚不存在该 data operation 的 B6 release 证明，则其 inventory/位置真相由取消证明绑定。新的前驱解析入口必须显式处理这一类型以及空 data 引用，不能要求 `load_latest_release` 猜造终态释放或伪造同 ID 的新数据事务。

既有 B6/B7 v1 索引继续按原精确证明读取；合法首次路径仍要求有证明的空历史。有新取消历史却缺少索引、或只出现新索引而无证明，都不能回退为旧路径。新 Installer 的查询/恢复/新建入口必须共同解析生命周期链与 v1 事务链，避免 UI 与 Executor 对最近结果的理解不一致。

## 持久化顺序与恢复

始终按 outer install guard → inner upgrade guard 取得双锁，每次动作 fresh 核验 source 产品、静止、私有对象及授权；固定状态目录不移动，guard 不更换。以下是待实现的专用协议，不是人工文件搬移步骤。

1. 保存不可改认的取消意图，绑定准备记录与前驱，阻止继续 handoff。活动准备 marker 与新增取消 marker 均保持；旧 reader 对未知对象失败关闭。
2. 按请求时点完成源库证明。必要时仅恢复原维护意图，旧 data 私有材料依原 inventory 封存；此前尚未具备 `source_prepared` 的取消归档须由此专用证明授权，不能放宽 B4 原状态转换。
3. 封存本次准备材料、完整旧 inventory 和新 outer 原字节。新 outer 如已活动，先绑定其历史目标缺失，再原 inode 移入本次历史固定槽、同步两侧目录；尚未创建的分支保留明确缺失证明。此时可能暂时没有 active outer，但 data marker 与双 guard 必须仍阻断业务初始化。
4. 在受控临时槽从已封存的前一 outer canonical 字节创建兼容回执，固定新文件身份并同步；其内容、operation、产品、终态不得改写。专用入口才可原子发布到 active outer 槽。若新 outer 从未写入且旧 outer 一直活动，仅核验原件，不重复写入。原历史回执本身保持，不能消费掉唯一证据。
5. 所有材料和兼容回执实物核验通过后持久化 `cancel_ready`，固定最终证明字节并发布生命周期索引。再次核验 active outer 与 source、旧 data 历史、运行数据和全部私有槽，数据活动目录只剩最后取消 marker。
6. 取消 marker 最后原 inode 移为本次历史 `preparation-cancellation.json`，同步两侧目录并回读，才报告已取消并允许旧 source 的正常双 gate 评估。活动 outer 是旧版本兼容事实；最近动作已取消的权威事实是独立取消证明，两者都要保留。

恢复不能调用普通 outer `persist` 强行覆盖：它本来就拒绝逆向替换。专用入口必须同时持有两 guard、匹配完整取消意图、原新 outer/缺失状态和旧 canonical 证明；缺少任何项即失败关闭。恢复的回执是兼容投影，不是把被取消的新 operation 改成旧 operation 或重新打开旧终态。

每次 rename 均要求恰好一个合法位置；两处都有、两处都无、owner/mode/link/hash/inode 漂移、未知临时对象或无意图目录全部保留阻断。文件/目录 fsync、记录写入、索引发布、outer 暂缺及 marker 最后移走等边界均要真实子进程退出矩阵。已经释放后只读查询历史，不以旧运行库 hash 阻止后续正常学习/WAL；中断后若无法证明原静止条件，不能继续执行会改状态的恢复动作。

## 旧程序资格与退出条件

当前 source validation host 的 `PostSwitch` 路径面向已有 v1 事务，不能直接用于无新 data receipt 的准备取消，也不能先制造伪回执再调用 host。取消释放资格必须单独覆盖以下条件：

1. 使用精确绑定的旧 source 程序副本与隔离合成 home，验证原 outer 终态 + 空活动 data 槽的双 gate，以及新旧 Installer 对中间 marker/不完整证明的拒绝。覆盖前一 outer completed/aborted/rolled_back、outer/data 不同 ID、无 data 前驱和重复取消。
2. 对受支持源 schema，以合成 WAL 学习、删除/tombstone、配置和同步数据验证维护前取消与维护后等价 source 的旧 runtime 正常打开、再学习、再启动。源 schema 未变及核心全量等价不能被表述为旧程序资格已完成。
3. 在取消协调器中绑定上述资格覆盖的 source release/双程序身份，并每次 fresh 复核实际安装对象、schema、静止及源内容关系。没有对应 source 资格或无法验证当前对象就不移走最后 marker。程序副本资格不等于对真实用户库执行试开。
4. 合成取消后再次完成准备→新 v1→真实数据终态释放，并复验前面所有取消证明、旧 receipt、保护快照 inode/字节不变；包括 v1→v2 索引接续、连续取消/释放以及版本、前驱和 proof 类型错误绑定。

需先完成核心纯记录/实物证明与故障矩阵，再接真实 outer/Executor 和只读 UI，最后做独立程序副本资格。UI 在维护后提示“完成安全准备后取消”，中断/busy/不兼容均显示仍需恢复，不显示取消成功。新增 UI action 的映射依已批准合同另行固定，不能提前暴露未获资格的按钮。

B 段仍未退出；本补充获确认也不等于产品已支持取消。真实源码/旧二进制若不能满足任一前置条件，应保留阻断并回报设计差异，不削弱 v1 gate。WAL/pager 内部真实断电资格、C–D 产品接线、REV-01/REV-02 和所有冻结现场停止线保持。

## 备选方案与取舍

| 方案 | 取舍 |
| --- | --- |
| 本文独立取消证明 + 原 outer 兼容回执 | 推荐；保留旧 v1 和旧 source，代价是新增受控外层封存/恢复及生命周期前驱协议，需单独确认 |
| 双层 v1 都新增 `user_cancelled` | 旧 source 严格 reader 拒绝；必须升级旧程序或另建兼容路径，超出当前保留冻结 source 的目标 |
| 只允许维护开始前取消 | 可缩小功能，但无法满足已批准的维护后安全取消目标，且外层已创建时仍有双层表示问题 |
| 继续使用既有失败码或留非终态但显示取消 | 不采用；结果与真实原因不符，或仍被 startup gate 阻断 |
