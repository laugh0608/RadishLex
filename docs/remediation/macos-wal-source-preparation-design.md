# macOS WAL 源库准备与新事务接续设计

本文是面向 `ime-userdb`、数据升级核心和 macOS Installer 维护者的已批准设计，定义新 operation 的源库准备、旧事务保留、持久化恢复与验证范围。它不代表功能已实现，不改变现行 v1 合同，也不授权真实数据库、冻结载体或现场操作。问题及历史证据见 [WAL 整改专题](macos-wal-upgrade-recovery-2026-09.md)，执行顺位见 [current](../status/current.md)。

## 设计结论与批准范围

建议采用“版本化准备记录 + 既有 v1 数据事务”：先保护一致快照，再由 SQLite 把源库准备成 standalone 文件，最后以准备后的身份创建新的 v1 data receipt。正常 runtime 继续使用 WAL，migration 仍只作用于隔离 candidate。

2026-09-15 项目所有者已确认本方案并要求提交文档、继续推进。授权覆盖以下 A–D 仓库实现与独立合成/程序副本资格边界：

1. 增加源库维护 API、内容等价验证、准备记录、恢复判断和隔离故障矩阵。
2. 增加已终结数据事务向新 operation 的保留式接续，以及事务完成后释放活动数据槽的保留式封存；保留固定状态目录 inode 与同一个 data-root guard。
3. 调整 Installer 开始升级的顺序、准备/终态封存的只读投影和明确中止入口；v1 receipt 序列化、原正常数据状态转换和 ABI POD 布局不变，外围生命周期合同扩展。新增 UI action 若需整数映射，先在双方合同中固定并验证未知值拒绝。
4. 修改稳定专题、模块说明和相关门禁；本批不更新依赖、schema、正常输入的 WAL/NORMAL 策略或学习/同步协议。

批准仅覆盖仓库实现和资格验证，不包含真实维护。实际维护会改变源主库内容、长度和 WAL/SHM 状态；准备后的失败保留是“相同用户数据和删除语义”，不能再描述为“准备前主库与 sidecar 逐字节不变”。真实执行还会涉及历史材料的位置变化，必须另行确认精确现场范围。

## 源码约束与新增缺口

| 现有路径 | 已核对的事实 | 对设计的约束 |
| --- | --- | --- |
| [bootstrap](../../platforms/macos-product/InstallerExecutor/src/bootstrap.rs) | 在 `inspect_file` 前记录 source identity；只接受同一 operation，且要求 `previous_operation_id` 为空 | 不能先创建原 v1 receipt，再原地准备源库；新 operation 接续必须显式实现 |
| [receipt](../../crates/ime-product-upgrade/src/lib.rs) | `can_replace` 固定 operation、schema 和已有 artifact；终态不能重开 | 不放宽同一 operation 的 source identity，也不覆盖旧终态 |
| [store](../../crates/ime-product-upgrade/src/filesystem.rs) | 状态目录对象白名单严格；guard 绑定 data root、状态目录 inode 和 socket | 不整体 rename 状态目录后沿用旧 store/guard；未知准备格式必须阻断旧 reader |
| [switch](../../crates/ime-product-upgrade/src/switch.rs) | active/candidate/backup 均无 sidecar 后才记录 backup identity | 保留该门禁；准备成功不能仅凭 WAL 长度为零 |
| [终态加载](../../crates/ime-product-upgrade/src/switch.rs) / [rollback](../../crates/ime-product-upgrade/src/rollback.rs) | `Completed` 和 `RolledBack` 仍验证零 sidecar 与活动库历史长度 | 源码显示正常 WAL 写入后可能阻止再次启动；须覆盖终态释放，不能仅修好首次升级；尚未运行该缺口的复现测试 |
| [Executor](../../platforms/macos-product/InstallerExecutor/src/lib.rs) | 先创建 outer operation，再独立 bootstrap，guard 跨步骤释放 | 新路径须在统一锁顺序和持久准备意图下交接，不能把当前 bootstrap 简单前移 |
| [snapshot](../../crates/ime-userdb/src/store/snapshot.rs) | backup 读取 WAL 一致视图；现有连接主要依赖析构关闭 | 新维护成功条件必须包含显式关闭；snapshot 成功不等于原库可切换 |

后续升级还必须接受正常使用后发生的合法学习。旧终态中的源 identity/hash 是历史事实，不得用它要求今天的 active DB 仍与上次升级时逐字节一致；本次源身份由 fresh 静止观察建立，历史 snapshot/candidate/backup 则继续按原记录保护。

## 备选方案

| 方案 | 判断 |
| --- | --- |
| 仅要求正常退出，或仅 truncate WAL | 不采用；不能证明持久日志模式已改变或后续连接不会重建 sidecar |
| 在旧 v1 operation 的 `candidate_verified` 直接准备并改写 source identity | 不采用；破坏追加式身份和冻结证据关系 |
| 数据 receipt 全面升级 v2，直接新增准备状态 | 暂不采用；旧 source validation host 与回滚后 startup reader 也会拒绝 v2，需另做全套兼容投影 |
| 独立准备记录，完成后交给现有 v1 数据状态机 | 推荐；准备过程有明确持久证明，旧 source/target 的 v1 读取仍可在交接后使用 |

WAL 模式跨连接保留，WAL 包含可能尚未进入主文件的已提交数据，不能任意分离。此处沿用 SQLite 的数据库 family 语义。[SQLite WAL](https://sqlite.org/wal.html)

## 固定对象与准备记录

保留 `.radishlex-upgrade-v1/` 目录本身，建议新增以下固定槽位；产品入口只接受权威 home 和已有 product identity，不接受下列路径的调用方覆盖。

```text
<data-root>/.radishlex-upgrade-v1/
  source-preparation.json
  source-preparation.json.tmp
  preparation-snapshot.sqlite3
  preparation-snapshot.sqlite3.tmp
  terminal-release.json
  terminal-release.json.tmp
  ...既有 v1 固定槽位...
<data-root>/.radishlex-upgrade-history-v1/<operation-id>/
  inventory.json
  preparation.json
  preparation-snapshot.sqlite3
  previous-install-receipt.json
  terminal-release.json
  data/
    ...该旧 operation 原有的 v1 receipt 与固定 artifact...
<data-root>/.radishlex-upgrade-history-v1/
  latest-release.json
  latest-release.json.tmp
```

历史目录的每个槽均有明确用途：旧 operation 的 `data/` 保留其数据事务；新 operation 的 `preparation.json` 与准备快照保存本次前置证明；`previous-install-receipt.json` 保存创建新 outer 前的原 canonical 字节。可选槽缺失必须显式记录，不允许扫描后任意认领。不同 operation 不共享写入目标，已有目标不得覆盖。

`latest-release.json` 是版本化、私有、原子更新的定位索引，仅保存最近封存动作的 outer operation、其 data operation（可空）、release、data-root identity 与封存证明摘要，不承担终态真相源。它只能在对应 `release_ready` 全部验证后更新，并保留前驱值在本次记录中；中断时只接受已记录的旧值或新值。启动须继续交叉检查真正的 release 记录与 outer，不能仅凭索引允许。新升级在无 active receipt 时从该索引定位旧 data operation；中间的 repair 不改变 data 前驱，缺失/冲突索引不通过扫描猜测。既无 active receipt 又无历史目录才属于首次路径；首次准备被明确中止且未创建 data receipt 时，以有证明的空 data 引用记录，不伪造数据终态。

目录为目标用户所有的 canonical `0700`，文件为 `0600` 单链接普通文件；所有目标在同一文件系统。记录格式拟定 `radishlex-source-preparation-v1`，canonical UTF-8 JSON 加末尾换行、上限 256 KiB、严格未知字段/重复字段/版本/阶段拒绝。

准备记录至少包含：

- 新 outer operation、上一个 outer operation、上一个 data operation（允许不存在，不能假定两者总相等）、source/target product identity 与 schema 约束。
- data root 与固定状态目录身份；旧 receipt canonical 摘要及固定 artifact 清单；受控目标的缺失记录。
- 初次观察的 active DB/WAL/SHM/journal 身份，以及快照封存后的源 family 基线。身份包括 device/inode/owner/mode/link/length/SHA-256，目录按稳定字段比较。
- 准备 snapshot 身份、源 schema、每次维护的持久意图和阶段、准备后源身份、交接 receipt 摘要、归档移动进度及中止结果。

初始身份只追加不改写；`prepared_source` 是另一个字段，不能覆盖 `initial_source`。hash 只进入本机私有维护证据，不进入 v1 receipt、普通日志、同步或 telemetry。表内容、SQL、绝对 home、helper 输出和真实输入不进入记录。

记录持久化使用独占创建临时文件、文件 fsync、受控身份复验、原子 rename、目录 fsync、canonical 回读。更新须绑定前一 canonical 记录和单步合法转换；临时文件冲突、记录缺失或无法证明来源的最终对象保持阻断，不自动重建记录。

## 锁、入口与旧版本阻断

锁顺序固定为 outer install guard → inner upgrade guard；所有观察、记录、维护和交接均在本次取得的双 guard 下执行。阶段间可结束调用，但下次必须重新取得两锁、加载记录并做 fresh 产品身份和静止检查，旧观察不能充当许可。

新入口在写 active DB 之前依次完成：

1. 检查前一 outer/data 已终结、当前真实双程序与 source identity 一致、sealed target/source payload 有资格；存在非终态、未知材料或关系漂移则拒绝开始。
2. 预留唯一新 operation ID，保存前一 outer canonical receipt 和旧 data 清单；在固定状态目录落盘准备意图。此时只允许创建本次私有元数据，不打开 SQLite 写连接。
3. 创建相同 ID 的新 outer `prepared`。若中断发生在第 2、3 步之间，新 Executor 只可依据记录和仍匹配的前一 outer 完成这一写入，或明确中止；不能生成另一个 ID。
4. 在 outer 非终态和 preparation 阻断均可回读后，再进入快照和维护阶段；双程序切换必须晚于准备交接完成。

旧数据 reader 不认识 `source-preparation.json` / `terminal-release.json`，根据现有白名单拒绝打开；新 reader 严格识别阶段，准备未交接或终态未封存时同样阻止产品业务初始化。outer 非终态也是独立阻断条件。不能仅把准备记录放在旧 reader 忽略的新目录里，然后声称旧版本已被阻断。

新增 reader 使用专用准备/接续入口核对完整记录与槽位关系。普通 `load_guarded`、snapshot、switch 不得通过忽略未知对象或跳过旧 receipt 验证来“兼容”中间状态。历史 receipt 与活动源身份的不同语义必须由明确阶段解释。

准备记录封存和所有新增 active 槽移出后，活动目录重新满足原 v1 白名单，且新的 v1 data receipt 必须已经存在。源版本 rollback helper 此后才可按固定 v1 路径读取。旧 Installer 不作为新准备流程的恢复执行器；须验证它无法绕过准备记录驱动数据切换。

## 状态与唯一后继动作

下列为准备记录的拟定 typed 状态；阶段成功只能在对应对象 fsync、身份复验及记录持久化后声明。

| 阶段 | 必须已有的持久事实 | 下一动作 |
| --- | --- | --- |
| `reserved` | 新 ID、前一 outer 字节、旧 data inventory、初始文件 family | 精确创建/核验新 outer `prepared` |
| `snapshot_ready` | 新 outer 匹配、准备 snapshot 完整封存、源 schema 与快照一致 | 再验静止与 family，记录维护意图 |
| `maintenance_intent` | 完整快照、维护前 identity、允许的固定 DB family | 仅通过专用 SQLite 维护连接准备或重新验证 |
| `source_prepared` | 源库关闭、DELETE、无 sidecar、内容等价、准备后 identity | 封存旧 data 材料并为新 v1 receipt 腾出固定槽 |
| `previous_archived` | 旧 receipt/artifact 每项都在唯一历史槽，state directory inode 不变 | 写入新的 v1 `preflighted`，source identity 取准备后值 |
| `handoff_ready` | 新 v1 canonical receipt 与准备身份精确相符，历史证据封存 | 移出准备槽并交给既有协调链 |
| `handed_off`（派生结果） | 活动目录仅有合法新 v1 对象、封存的 `handoff_ready` 匹配、双 guard 下回读通过 | 正常推进原 v1 snapshot/candidate/switch/rollback；不回写封存记录 |

无原 data state 的首个升级走同一路径，旧 inventory 显式为空，不创建伪造的旧 receipt。现有无 userdb 的路径保持原能力边界，本批不为“升级”创建空业务库。

`reserved` 到 `snapshot_ready` 之间的文件创建也须先持久化固定槽计划，文件有已记录 identity 后才允许根据它恢复。rename 已发生但没有足够持久 identity 的孤立对象保留并阻断；这种结果不计为自动恢复成功。

## SQLite 源库准备

### 快照先于写连接

使用 `ime-userdb` 的只读一致 snapshot 能力，在新私有槽生成未迁移、同源 schema 的 standalone 快照，验证完整性并显式结束 backup/read transaction、关闭所有连接，再 fsync 和封存身份。只读访问可能产生 SHM 或零长 sidecar，必须记录前后观察；不能把这种访问宣称为全 family 零写入。

此阶段不得改变源主库与已存在 WAL 的内容；如观察到并发数据变化或无法解释的身份变化，拒绝封存快照。正常 SQLite 创建/更新 SHM 的受限变化与源数据修改分开判断，权限、链接和所有权仍严格检查。保护快照失败、close 失败、空间不足时，不进入维护写路径。[SQLite backup](https://sqlite.org/backup.html)

### 专用维护 API

`ime-userdb` 增加不 create、不 chmod、不 migration、不调用普通 `UserDb::open` 的维护入口，由协调层先证明路径和隔离条件：

1. 持久化 `maintenance_intent` 后，才以 READ_WRITE（不含 CREATE）打开固定已有 DB；禁止 attach、自动 schema 修复和任意 SQL 输入。SQLite 打开时的恢复也属于本次受控写入范围。
2. 核对受支持的源 schema、完整性、日志模式；只支持已验证的 WAL/DELETE 输入，其他模式和准备前的未知 journal 失败关闭。维护连接设置有界等待，沿用现有 5 秒上限，不新增无限重试。
3. WAL 输入通过 `PRAGMA main.wal_checkpoint(TRUNCATE)` 取得完整结果，要求 busy=0 且成功 truncate 的两项 frame count 都为 0；错误、busy、部分结果不能推进。DELETE 输入单独识别，不把非 WAL 的 -1 结果冒充完成。
4. 不持有读事务或未释放 statement 时请求 `PRAGMA main.journal_mode=DELETE`，精确检查返回值；不设置 OFF/MEMORY，不由文件 API 删除 sidecar。
5. 检查源 schema 未变化及完整性，释放所有 statement/backup/blob/transaction，显式 `Connection::close` 并检查结果。close 失败保留未完成状态；不能以析构或 `close_v2` 的延迟释放代替已关闭证据。
6. 重新证明静止，验证相同源 inode、安全 metadata 和无 WAL/SHM/journal；fsync 主文件及 data root。只读重开验证 DELETE、结构、与保护快照内容等价，显式关闭，再复验零 sidecar与最终 identity，才记录 `source_prepared`。

checkpoint 结果和关闭语义依据 [SQLite checkpoint API](https://sqlite.org/c3ref/wal_checkpoint_v2.html)、[SQLite close API](https://sqlite.org/c3ref/close.html)。成功 checkpoint 和成功 close 各自只是步骤条件，均不单独代表准备成功。

### 内容等价与物理身份

维护后的主文件字节可合法变化，因此源主文件 SHA-256 不与准备前强求相等。由 `ime-userdb` 提供受支持 schema 的流式内容等价检查：比较 schema 对象、全部持久表/列/记录及 multiplicity，按确定性键与 SQLite storage class 比较值，覆盖内部 sequence、学习、tombstone、配置与同步相关数据；不得只比较行数、活跃词或测试样例。未知 schema 对象/未支持的比较规则拒绝准备，不能忽略它们。

比较只返回固定结果和 schema 分类，不把逐行内容送入 Installer、receipt 或日志；比较失败不得自动恢复 snapshot 覆盖源库。实现前须把支持 schema 的键、rowid/sequence 语义及比较清单变成 `ime-userdb` 合同测试，不能在协调层自造通用 SQL 比较器。

`maintenance_intent` 是唯一允许源主库 length/hash 和已存在 WAL 内容合法变化的窗口；快照阶段仅允许上一节定义的 SQLite 辅助文件变化。维护时源 device/inode/owner/mode/link 仍须匹配，逻辑内容仍须与已封存快照等价；窗口外的源主库或已有 WAL hash 漂移拒绝。重启发现同 inode 的不同业务内容，不能以“SQLite 可能改过”为理由认领。

## 旧事务接续与历史材料

现有 `persist/can_replace` 继续只允许同一 operation。增加独立的接续原语，仅接受准备记录和已证明的终态 inventory，不通过放宽 `can_replace` 来支持跨 operation 覆盖。

逐个把旧 v1 receipt/artifact rename 到 `.radishlex-upgrade-history-v1/<old-data-operation>/data/` 的固定同名槽。保留每个文件 inode、字节及权限；每次 rename 后按目标目录、源目录顺序 fsync，并更新位置证明。原 `.radishlex-upgrade-v1/` 目录始终不移动，guard 身份不变。每个文件只允许“原槽存在/历史槽缺失”或“原槽缺失/历史槽存在且身份匹配”；两处都存在、都缺失、未知对象或目标被占用均阻断，不覆盖或删除。

历史 `SourceDatabase` 等指向运行数据的字段只作历史记录，不能要求把 active DB 也移入历史目录。移动清单只包含该旧事务私有 artifact；旧 receipt、恢复证据与 inventory 一起解释历史关系，不调用旧 API 在新路径上假装原 operation 仍活动。

新 v1 receipt 的 operation 等于新 outer；`previous_operation_id` 等于旧 data operation（首次为 null）。新准备记录另绑定前一 outer；中间发生 repair 等操作时，两条前驱链不得混淆。已支持的 source release/schema 必须由 fresh source 与 sealed payload 证明，不能从前一 receipt 的 target 字段猜测。

准备快照移到新 operation 的私有历史槽继续保留，随后既有 v1 链从准备后的 source 创建自己的 snapshot/candidate。两份快照用途不同：前者保护准备写入，后者服务数据 migration；先保持现有核心的路径和 artifact 合同，不在首批合并二者。

空间预算在既有三份 logical snapshot + 64 MiB 基础上，增加一份准备 snapshot、源主库可能增长的上界、settings 副本和维护 journal 余量；使用 checked arithmetic，每个昂贵写入前 fresh 检查。历史已有文件的 rename 不按重新复制计费；不通过删除旧材料腾空间。

## 中断、明确中止与交接恢复

| 中断位置 | 合法恢复或停止结果 |
| --- | --- |
| 准备意图已写、新 outer 未写 | 匹配前一 outer 后创建相同 ID，或在源未维护时明确中止；不生成新 ID |
| snapshot 创建中、无完整封存身份 | 保留临时对象和原 family，阻断；不得认领为可用保护快照 |
| snapshot 封存后、维护意图前 | 重新核验快照和源 family，可进入维护；此时明确中止无需准备写入 |
| checkpoint 部分完成、模式转换中或关闭结果未落盘 | 保持 `maintenance_intent`；新调用重新取锁和静止证明，仅在安全 family、源 inode 和快照关系成立时经 SQLite 恢复/完成准备，并重新比较全量逻辑内容 |
| 源已准备、完成记录未写 | 按维护意图复核完整后置条件再追加准备后身份，不按“能打开”猜测成功 |
| 旧 artifact 部分移入历史目录 | 按预先持久 inventory 与两个固定槽继续唯一剩余 rename；未知组合保持阻断 |
| 新 v1 receipt 已写、准备 marker 仍在 | 只复验同一 receipt 和 prepared identity，封存准备记录并移出新增槽，不重复维护 |
| 准备 marker 已移出、返回前 crash | 新 v1 receipt 与历史 `handoff_ready` 记录交叉核验，幂等认定交接；不能重新开新 operation |
| 已交接后的 candidate/switch/rollback 失败 | 使用原 v1 状态机；准备记录只提供历史身份关系，不修改原 v1 source 字段 |

明确中止分两种：维护意图前可在证明源业务数据未修改后封存准备记录、保持旧活动槽并结清新 outer；维护意图后必须先证明 prepared source 仍等价、standalone、旧版本兼容，再创建/结清新 v1 `aborted_preserved` 并结清 outer，不能直接回写旧 receipt。准备完成前程序未切换，outer 复用 `InstallReceipt::abort_preserved` 的合法终态，不伪造 `programs_restored`；guard-bound 的数据后置验证和 UI 入口仍须新增并测试。

维护后无法证明内容等价、关闭完成或源安全时，不提供“已中止且可启动”终态。保留原 family、保护快照和记录；从快照替换真实原库属于另行设计的恢复动作，不包含在本方案的自动 fallback 内。此停止结果须向用户明确呈现，不能伪装为普通 retry 可以解决。

交接步骤先写新 v1 receipt，再封存完整准备记录及快照，最后移走阻断 marker；最后一步前后都必须验证不存在“outer 允许 + data 无记录 + 准备未完成”的可启动窗口。出现任何 fsync 错误都不报告成功。

## 终态封存与后续正常 WAL 使用

仅完成准备和新 v1 bootstrap 仍不足以交付：现有 v1 terminal reader 会继续按升级切换现场检查活动库，而正常 runtime 会重新启用 WAL。推荐在新流程内增加明确的终态封存阶段，使旧 source reader 也能在回滚后继续使用；不修改冻结旧二进制，也不要求正常运行放弃 WAL。

该阶段使用独立 `radishlex-terminal-release-v1` 记录，仍采用 256 KiB canonical/严格解析和相同权限规则，固定在活动目录的 `terminal-release.json`，最终保留于本 operation 历史目录。记录绑定 outer/data operation、真实安装版本、数据终态、活动库最终身份、完整 artifact inventory、每项移动身份以及授权释放的结果。它不能用于任何非终态数据 receipt。

顺序固定为：

1. 在双 guard 和静止证明下，以现有精确规则完成 data `completed` / `rolled_back` / `aborted_preserved`，并完成相应外层最终程序验证；outer 仍保持可恢复的非终态。
2. 写入终态封存意图和完整 data inventory，把本 operation 的 data receipt、snapshot/candidate/backup/settings 私有材料逐项移动到其历史 `data/`，保持 inode/字节/权限。活动 `userdb.sqlite3`、settings 与 Rime 不移动。
3. 第 1 步的两端 source/target 兼容验证必须在对应数据 receipt 仍处于固定 v1 槽时完成。封存完成后仅以受控历史验证 API 检查终态证明；不把历史路径传给只接受固定活动路径的旧 host。
4. 历史 inventory 与全部对象持久化后写 `release_ready` 并更新索引。外层 finalization 新增读取该精确证明的分支，仍要求真实双程序 identity 与应安装 release 相符，才持久化 outer `completed` / `rolled_back` / `aborted_preserved`。
5. outer 终态回读匹配后，将 active marker 原 inode 移入历史固定槽并 fsync 两目录。移走 marker 前旧 reader 仍拒绝启动；之后活动目录不含 v1 receipt/artifact，旧 data gate 返回现有 `AllowedNoUpgradeState`，旧 outer gate仍校验源/目标程序身份。界面只有在第 5 步验证后才报告可启动。

`release_ready` 是封存记录的最终字节，是否已释放由 active/history 两个唯一槽和匹配的 outer 终态共同推导，不为补写“成功”修改封存证据。准备中止且未创建新 data receipt 时，记录明确绑定仍有效的旧数据状态或既有历史终态，不伪造本次数据升级已完成。

新 startup/Installer 入口必须识别释放记录并与 outer 及活动目录交叉核对：外层已终结但 marker 还在时只允许完成精确封存，不提供另开事务；active receipt 已不在但无匹配持久历史证明时拒绝。已经授权释放后，活动库长度、内容和合法 WAL 可随正常使用变化，历史快照/receipt 仍严格验证。每个新 operation 重新取得当前源 family，不复用旧内容基线。

这是新增的产品级保留式生命周期，已获仓库实施批准；不能在现有用户现场手动搬走 receipt 解除门禁。必须用冻结 source 39 的真实程序副本验证回滚后启动门禁、正常学习后重启，以及下一次新升级；若旧 source 其他入口仍有固定状态依赖，此方案不具备产品资格，停止扩大实机范围。

## 实施分段与验收矩阵

按以下顺序串行实施，每段完成匹配检查后再进入下一段；支持 schema、历史接续和旧 reader 兼容属于必要工作，不能只交付一个 checkpoint helper。

| 分段 | 实现范围 | 退出条件 |
| --- | --- | --- |
| A：合同与 SQLite | 严格准备记录类型、维护 API、内容等价、显式关闭 | WAL/DELETE、全部受支持源 schema、busy/部分 checkpoint/关闭错误、真实子进程 crash 与数据等价测试 |
| B：持久协调 | guard-bound 准备、inventory、旧槽保留、新 v1 bootstrap、明确中止、终态封存 | 每个文件/目录 fsync、记录和 rename 边界的断点矩阵；新进程重载后精确继续或明确保留阻断 |
| C：产品接线 | Executor、只读投影、startup gate、准备中止、旧 helper 路由、外层终态封存接线 | 新旧 reader 正负对照；双组件在未交接/未封存时拒绝，旧 source 回滚后写 WAL、重启仍允许 |
| D：资格与交接 | 独立合成根、真实 source/target 程序副本、文档和完整门禁 | 同一新 operation 成功链、各失败链、终态后再用新 operation 升级；原输入载体与真实用户目录不变 |

必要矩阵包含：

- 未 checkpoint 的学习、删除/tombstone 与多类持久数据；正常关闭后重开，零长 WAL/SHM，已是 DELETE 的对照。
- 活动 reader/writer、旧进程重新启动、busy/locked、部分 checkpoint、模式转换失败、close 失败、磁盘不足/IO/fsync 失败。
- source、family、snapshot、记录、目录和历史槽的 inode/owner/mode/link/hash 漂移，以及相同字节不同 inode、相同行数不同内容。
- 维护中 SQLite 合法物理变化与非法业务内容变化的正负对照；源逻辑比较覆盖空值、BLOB、文本、数值和重复记录语义。
- 首次升级、前一 `completed`、`aborted_preserved`、`rolled_back`；不同 outer/data 前驱、错误 source/target、重复 ID、旧非终态、部分历史接续。
- 未知新记录版本/字段/阶段、临时残留、丢失 marker/历史证明、旧 reader/旧 Installer、双 guard 冲突、错误 home/root。
- candidate 双端失败、两次 DB rename 中断、post-switch 失败、源版本验证失败再重放；成功和可恢复失败均重开核验学习及 tombstone。
- 新终态后正常学习再次写 WAL，再发起另一个新 operation；不能因先前 hash 已变化而错误拒绝，也不能复用先前 snapshot 冒充本次快照。
- 终态封存各边界的中断，包括 outer 已终态但 marker 未移出、receipt 已移走但 inventory 不全、历史 target 冲突；数据 gate 与 outer gate 任一未满足均不能报告产品可启动。

拟执行检查：相关 Rust fmt/check/test/clippy、`check-macos-upgrade-coordinator.sh`、`check-macos-install-coordinator.sh`、`check-macos-installer.sh`、真实程序副本资格，以及完整 `check-repo.sh`。文档另执行 `check-docs.sh`、`check-text-files.sh` 和 `git diff --check`。资格对所有冻结输入作前后身份比较，合成检查不得表述为真实升级完成。

## 进入实机前必须另行确定

仓库实现和独立产品资格通过后，再确定新候选版本/构建身份、唯一 source、当前数据库与旧事务归档清单、准备写入与停止条件，并请求一次新的实机授权。9 月 9 日恢复授权已消费，本方案不授权复跑恢复、直接重试 target 40、手工 checkpoint、删除 sidecar 或搬移冻结材料。

客户端身份与 composition 隐私实测仍等待新升级真正完成。Linux pair、UTM 操作、M5 顺位、后续平台和公开发布保持当前边界。
