# macOS 无 snapshot 源 schema 观察合同

本文供 userdb、升级核心与 macOS 产品协调层维护者审阅，限定准备取消时缺少 snapshot schema 的观察来源、请求绑定、失效与验收条件。状态：**2026-10-17 设计草案，未实施、未采集；不提供生产资格。** 本轮仅改文档，不操作冻结资产、真实库、账户或会话。

上游为[资格证据 reader 合同](macos-cancellation-qualification-reader-contract.md)及[准备取消兼容合同](macos-preparation-cancellation-compatibility.md)。本设计不改变 v1/v2 回执、action `10–13`、双 guard/gate 或最终 marker；生产目录为空，issuer 未接入。源码定位基线为 `ad15bf7`，配套文档提交为 `f16e46b`。

## 已有事实与不能推出的结论

| 当前来源 | 实际提供什么 | 无 snapshot 时能否证明当前 schema |
| --- | --- | --- |
| [`PreparationBinding` / `PreparationFamily`](../../crates/ime-product-upgrade/src/preparation.rs) | source/target、root、前驱关系，以及 DB/WAL/SHM/journal 的身份、长度和 SHA-256 | 不能；`target_schema_version` 是目标约束，family 无 schema 字段 |
| `PreparationReceipt::snapshot_schema_version()` | 已记录 snapshot 的 schema；无 snapshot 返回 `None` | 不能将 `None` 补为目标版本、当前库支持版本或零 |
| [`CancellationSourceReceipt::source_ready()`](../../crates/ime-product-upgrade/src/cancellation_source_record.rs) | 取消收尾绑定的精确 family；维护前要求与请求观察一致 | 能固定待观察对象，不能解释其 SQLite 内容 |
| [取消 release 实物核验](../../crates/ime-product-upgrade/src/cancellation_release_evidence.rs) | 当前 family、原件、旧 outer、索引与历史祖先关系 | 不等于 schema 观察；维护前不会以维护后 snapshot 等价检查代替它 |
| [`UserDb::inspect_file`](../../crates/ime-userdb/src/store/connection.rs) | 通过 SQLite 读取 `user_version`、完整性检查及当前 schema 检查；旧版本主要报告迁移分类 | 不能直接对原库调用并宣称绝无 sidecar 写入，也不能把旧版本分类当作精确历史 layout 已验证 |
| [`estimate_snapshot` / `create_consistent_snapshot`](../../crates/ime-userdb/src/store/snapshot.rs) | 建立源 SQLite 连接，估算或创建一致快照 | 不符合本设计的原库不建立 SQLite 连接条件 |
| [reader 请求绑定](../../platforms/macos-product/InstallCoordinatorAdapter/src/qualification_reader_binding.rs) | 从完整 proof 派生场景，当前仅读取 snapshot schema | 目前无新观察入口，缺失仍为 `SchemaObservationUnavailable` |

SQLite 的 `user_version` 是应用设置的版本整数，本身不验证表结构；WAL 可以持有尚未 checkpoint 的已提交内容，读取主 DB 文件头不能代表完整视图。SQLite 的 WAL 只读连接也有 sidecar 条件，而 `immutable=1` 会跳过锁和变化检测，不能把它当作原库不变的证明。依据：[文件格式](https://sqlite.org/fileformat.html#user_version_number)、[WAL 读取](https://sqlite.org/wal.html#how_wal_works)、[只读 WAL](https://sqlite.org/wal.html#read_only_databases)、[immutable 参数](https://sqlite.org/uri.html#uriimmutable)。

## 选定的拟议路径与范围

首选**当前 family 的受控私有字节副本，由既有 SQLite 引擎观察**。源侧只通过受验证文件句柄读取字节；SQLite 仅看到本次新建的私有副本。它不是 preparation snapshot，不回填 `SnapshotEvidence`，不进入 archive/inventory/lifecycle，不成为备份、可重放资格包或新的数据真相源。

该方案会增加临时敏感数据副本及创建、资源管理和清理责任。**设计授权不包含这项实施或真实数据复制授权。** 后续须先批准副本位置、生命周期、资源预算及跨模块接口，再仅在全新合成目录实施验证；真实部署还须另行满足产品操作授权。现有只读 reader 的无写入承诺保持，副本管理不能塞入 `review_evidence`。

| 候选路径 | 本设计取舍 |
| --- | --- |
| 从旧 receipt、版本号、历史实验 schema 或 target 常量推测 | 拒绝；不能关联当前 family |
| 原库直接 `mode=ro`、`query_only` 或 `immutable=1` | 不选；不能据选项名称证明源侧零业务写入和一致性 |
| 只读主文件头，或自行归并 WAL 并解释 schema | 不选；前者漏 WAL，后者新增 SQLite 页视图/schema 解释器；下面的有限 WAL 格式准入校验不能代替 SQLite 读取 |
| 专用内存 VFS 或完整内存数据库映像 | 不在最小批次；新增 VFS/FFI、内存预算及 WAL 重建合同 |
| 私有 DB/WAL 副本 + 既有 SQLite | 作为待批准实施路径；必须通过下述完整性、只读与生命周期验收，失败不切换其他路径 |

## 可信来源与职责

可信观察须同时满足：受验证产品中的观察器代码、严格核心 proof 与现场验证、从该现场取得的精确副本、受支持的 SQLite/layout 验证方法。摘要只证明关联，不证明来源可信。

- 升级核心负责既有 proof、索引/祖先、原件及 source family 真相；产品层负责真实 source/target、外层 guard、静止与当次授权。调用方手工构造的 receipt 或裸 `Path` 不可直接晋升为已验证现场。
- 源文件捕获属于升级协调边界；SQLite 内容解释归 `ime-userdb`。拟议观察 API 接受受控副本能力，不接受任意路径、URI、SQL、VFS 名或 `schema=...`。私有能力必须由本次捕获器创建，不能从 JSON 或调用方布尔值构造。
- reader 仍只读取受审证据包和 typed 请求；未来请求接入观察值的接口是单独待批准变更。它不能创建副本、观察原库或生成 `CancellationSourceQualification`。
- 固定的版本/layout 验证 profile 必须来自受信产品，与当前观察器及 SQLite 实际构建身份绑定。`UserDb::inspect_file` 的旧版本 `MigrationRequired`、当前 `supported_schema_version()` 或版本区间不足以证明历史 layout；未定义精确验证器的版本保持 unsupported。不能假定 source 39 的实际 schema 就是当前源码的 `9`。
- 审阅目录中的 source/schema 兼容性条目只证明指定程序的已审行为，不作为本次库的 schema 来源；本次观察也不能反向生成该条目。独立运行证据与生产策略新鲜度仍分别缺失。

## 一次观察的完整流程

以下均为拟议产品调用合同，不是本轮执行步骤。

1. 只进入严格 proof 明确无 snapshot、且维护尚未开始的分支。验证 snapshot identity/schema 同时缺失、阶段合法、handoff 未开始、没有未解释的临时对象；有 snapshot 的分支继续现有路径，不能通过删字段强行降级到本观察器。旧生命周期前驱缺失/损坏不是“首次无 snapshot”。
2. 沿用外层→内层连续 guard，验证当次四项确认、最多 300 秒可撤销授权、双程序静止、source/target 身份和完整现场。控制检查覆盖捕获、SQLite 读取、释放连接及后置复核；观察器不能自己取得系统授权、续期或停止程序。
3. 从严格 proof 的 `archive.source.source_ready` 取得权威 family，同时绑定 preparation 的 initial family 与阶段关系；不由调用方选择 DB/WAL 组合。核验 data/state root、实际祖先链及每个成员的存在/缺失、device/inode/owner/mode/link/length/hash。journal 必须缺失；存在时不运行恢复。打开的成员保持句柄，逐块读取后复核句柄与目录项，原路径不交给 SQLite。
4. 在获准的私有父目录内独占创建本次 scratch 根，拒绝已存在根、链接、ACL 扩权、非私有 owner/mode 或与源/历史/证据根重叠。通过句柄创建普通文件，目录 `0700`、文件 `0600`、link count 1；不使用硬链接、路径重定向或复制原 inode 身份。
5. 按已捕获 family 逐字节复制 DB 和存在的 WAL；不存在与零长度文件是不同状态，不能补建或遗漏原 WAL。每个副本的长度/hash 必须与对应源成员相同。原 SHM 仍参与 family 身份与全量摘要复核，但不复制为 SQLite 的 wal-index：新副本上的索引由 SQLite 自行重建。SHM 仅是瞬态索引，不能作为 schema 来源；依据[SQLite wal-index 说明](https://sqlite.org/fileformat.html#wal_index_format)。原 journal 及 SHM 不得被修改、删除或重建。
6. 副本齐备后再次全量验证源 family 和路径关系与 proof 一致，再允许 SQLite 访问副本。源的 guard 与静止条件在整个窗口保持；失败不得将一个时间点的 DB 与另一个时间点的 WAL 拼成有效副本。
7. 先通过下述 WAL 视图准入，再用固定连接选项：只读主库、无 CREATE、私有连接、单一读事务、`query_only`，不允许迁移、checkpoint、journal mode 改写、ATTACH、扩展加载或任意 SQL。只执行受审的版本读取、完整性与精确 layout 验证；SQL/行内容/底层错误文本不外传。允许 SQLite 仅在本 scratch 中建立必要 SHM；DB/WAL 在连接前、事务结束及显式 close 后必须保持长度/hash 不变，不能以副本可丢弃为由接受 WAL 被删除或截断。现有 `inspect_file` 不满足全部控制和 close 语义，不能直接复用为已交付观察器。
8. 记录该读事务的版本与验证 profile，显式结束事务和关闭连接；busy、损坏、unsupported layout、close 失败均不能返回成功。重新验证源完整 family、原件/祖先、产品/静止、proof 与授权。副本身份/hash 漂移同样拒绝；不重试到碰巧成功。
9. 按事先批准的本次 scratch 生命周期关闭全部句柄、移除仅本次拥有的副本并核验缺失，成功后才返回内存观察结果。清理失败不返回可消费结果，保留私有状态并给出固定故障码；不得转而清理源、历史资产或其他任务的目录。

### WAL 视图准入与明确限制

仅 `integrity_check` 成功不足以证明 WAL 被正确采用：SQLite 可能从其认可的有效前缀读取。本设计因此要求在 SQLite open 前，以独立的有限格式校验器检查被绑定 WAL；它只决定是否准入，不归并页面或计算 schema。校验器归 `ime-userdb`，不是资格证据 reader 的 JSON 解析逻辑。

- 首个拟议 profile 检查数据库格式与页大小、WAL header 的 magic/格式版本/页大小和 checksum，逐 frame 检查长度、页号、当前 generation salts 及累计 checksum，并记录 commit 边界；算术溢出或任何不明形状拒绝。需要用官方[WAL 格式与 checksum 规则](https://sqlite.org/fileformat.html#wal_file_format)和捆绑 SQLite 生成的合成样本交叉验证；不能只看 WAL 文件存在。
- 无 WAL、零长 WAL、仅 header、有效未提交 frame 尾部、一个或多个有效 commit 分别有明确 profile 分支。最终版本仍由 SQLite 在单一事务中读取，未提交变更不参与版本；若有 commit，SQLite 所见页数须与最后准入 commit 的逻辑大小一致。未知 profile、WAL/DB 模式或页大小冲突均拒绝。
- 对 salt 不匹配、损坏、截断或无法解释的尾部，首版一律拒绝，不截短副本后重开。这也可能拒绝 SQLite 正常接受的 WAL reset 残留或崩溃尾部；这是保守支持边界，不称为源已损坏、不自动 checkpoint。若独立资格矩阵需要这些形状，须先扩充并审阅准入规则，不能把它们计为已覆盖。
- 校验器不证明任意两份格式合法的 DB/WAL 在历史上属于同一库；SQLite 没有供本合同直接比较的持久“库 UUID”。归属只能来自本次核心 proof 的完整 family 和捕获关系，不能通过页大小或 salt 相同补出来源。对于 proof 本身已记录的原始内容，观察结论仅限该视图及 layout，不声明历史数据真实或业务语义正确。

这一路径仍新增有限 WAL 格式校验的维护成本，属于后续实施批准项；若不接受该成本，或不能证明实际 SQLite 选项对准入 WAL 的读取行为，保持 unavailable。本轮没有认可任何实际 source 39 family 满足这些条件，也不把简单无 WAL 样本视为主路径验收。

## proof/family 绑定与结果语义

拟议结果 `SourceSchemaObservation` 为内部不可序列化、不可由调用方构造的短命对象；名称仅为设计标识，本轮不新增 API。它必须由上述全流程构造，至少关联以下字段：

| 绑定组 | 必需内容 |
| --- | --- |
| 请求 | 完整 canonical proof 字节/摘要、取消 operation、旧 outer/data 关系、前驱类型及索引关系；不能只比较 operation ID |
| 产品与场景 | 完整 typed source/target、manifest 身份、无 snapshot 分支及 preparation 阶段；版本字符串不是产品身份 |
| 实物 | data/state root、捕获时祖先观察、完整 source_ready family 含成员缺失状态及 initial family 关系；同 hash 不免除 inode 核验 |
| 观察方法 | 固定观察器/profile、实际 SQLite 构建身份、schema 整数、完整性和精确 layout 验证结果；仅整数读取不产生成功对象 |
| 当次控制 | 进程内不可重放调用标识、guard 生命周期、当前检查点、同一任务授权与截止时间；墙钟、mtime、报告文件均不是新鲜度来源 |

源文件与副本 inode 不同是正常现象，须通过捕获映射与字节摘要连接；不得把副本 inode 写进真实 family 或 proof。派生摘要须与完整 typed 对象一致，不能接受外部传来的 `family_sha256` 作为现场已验证标志。

观察成功只说明本次固定 family 的逻辑视图通过指定版本/layout 验证，不能证明 source 39 能正常启动、学习、重启或保持全部业务语义。观察器不返回 `qualified`，也不做旧程序兼容性试开；这些条件继续由独立资格证据和既有核心检查承担。

未来 reader 只有在观察与当前完整请求完全一致、schema 精确落入受审条目且所需验证 profile 可用时，才可把它作为缺失 snapshot schema 的来源。不存在观测、仅有未验证整数或验证器不支持时保持拒绝；`schema=0` 是需单独验证的实际版本值，不能表达缺失或通配。有 snapshot 时若额外观察与其冲突必须拒绝，不能择一通过。

## 失效、新鲜度与错误分类

每次 qualification 回调仍须新观察或重新完成本合同全部验证；不跨 `Begin`、`SourceVerified`、marker 移动/同步与 `Released` 复用结果。重启、重新授权、恢复任务或调用结束都使旧对象失效；没有凭“5 分钟内测过”复用的 TTL。代价超出任务预算即失败，不跳过 hash、不延长授权。

| 条件 | 处理 |
| --- | --- |
| 无受信观察方法、无当前观察或现场能力未建立 | `Unavailable`；当前 reader 的 `SchemaObservationUnavailable` 语义保持 |
| proof、产品、root/祖先、成员存在性、身份、hash 或控制绑定变化 | `EvidenceChanged`；即使 operation 与 schema 整数相同也拒绝 |
| 实测版本/layout 不在精确支持集合 | `UnsupportedSchema`；不迁移、不借 target 常量、不自动降级 |
| 输入损坏、WAL 不一致、短读、IO/busy、close/清理失败、超限 | `VerificationFailed`，保留固定细分原因；不把失败伪装成“没有库” |
| 任务撤销/到期、guard 失效、程序重新运行 | 当次调用拒绝并丢弃观察；产品保留授权/静止失败原因，不生成资格 |
| 策略未知或撤销、独立证据缺行 | 观察本身不能解除；仍按 reader/未来 issuer 合同拒绝 |

全部失效均不触发源恢复、原件清理、重建 snapshot 或 marker 移动。若未来释放流程已进入 marker 移动后的检查点，失败仍报告核心真实中间状态；不承诺自动搬回或取消成功。历史上已正常释放后发生合法学习，不拿旧观察 hash 拒绝普通历史查询；该观察只约束待释放的本次 family。

## 隐私、资源和 scratch 生命周期

- 源侧承诺不发起 SQLite 连接或文件写入，不改数据、sidecar、权限及事务材料；普通文件读取可能受文件系统 atime 策略影响，不承诺 atime 永不变化，也不通过 utime“恢复”原件。mtime/ctime 或已绑定身份异常必须拒绝。
- 副本含与原库相同等级的敏感数据，不属于合成 fixture、日志或证据附件；不得进入 reader pack、仓库、上传、同步、崩溃附件或诊断。只保留固定错误码和必要非敏感计数；不能输出路径、SQL、原始行或原库摘要作为公共日志。
- 不把 source family 套用 reader 附件 256 MiB 上限。实施批准前必须为观察器单独固定 `max_family_bytes`、scratch 空间预留、连接缓存/临时文件预算及 IO 总量上限，并以合成 WAL 最大样本实测；预算未定时禁止生产调用。单线程逐块读取、串行 SQLite 检查，控制期限不超过任务剩余时间且最多 300 秒。调度降载不等于系统调用超时、CPU/RSS 硬配额。
- scratch 位置须排除同步/备份用途并说明系统备份、swap 和崩溃残留边界；私有权限和 unlink 不代表不可恢复擦除。若不能接受明文副本的残留风险，不能以“临时”命名绕过，应另行设计内存/VFS 路线。
- 正常、失败、取消均只清理已核验属于本次调用的对象；授权应预先包含这些生命周期动作。撤销或到期后只允许已批准的资源关闭/清理，不继续查询或返回成功；清理不延长业务授权，无法完成时记录失败与私有残留。进程崩溃后的残留不复用、不自动认领，清理范围另行核验和授权；未知对象原样保留，不能递归删除整个父目录。本轮没有创建任何 scratch 或批准此类清理。

## 负向与正向验收条件

以下是未来实现的验收清单，本轮没有运行这些实验。全部只用全新合成目录；每项不仅断言错误码，还核对原 DB/WAL/SHM 的身份、内容和存在状态、未知对象保留、无 marker/回执/索引变化，以及 scratch 处置结果。

| 编号 | 样本/注入 | 必须观察到的结果 |
| --- | --- | --- |
| S01 | 没有 snapshot、没有 data 前驱，合法私有源与受支持 layout | 新观察可绑定；不从目标推 schema，原 family 不变 |
| S02 | 主 DB 的旧 schema 与 WAL 中已提交的新 schema 不同 | 读取提交后的实际值；与在另一个同字节合成副本上独立 SQLite 查询的结果一致，不只用 mock 返回值 |
| S03 | 多次提交、有效尾部未提交 schema 变更、部分 checkpoint、无 SHM；另设 WAL reset 留存旧 salt 尾部 | 前组分别验证已提交视图；首版对后者明确拒绝且原件不变，不把合法但不支持的形状记为损坏或已覆盖 |
| S04 | DB 缺失/空文件、零长 WAL、WAL 缺失、SHM 有/无 | 逐个区分，不补建源，不把零长或缺失 DB 当 schema 0；合法零长 WAL 的处理须实测 |
| S05 | 提供 target schema、旧实验值、手写 JSON、伪造能力对象或同版本不同 manifest | 不建立可信观察或拒绝请求匹配；生产构造面不能注入裸整数 |
| S06 | 相同 operation，不同 root、proof、source/target、旧 outer/data、前驱或 family | 拒绝；同时覆盖 no-index、v1、v2、重复取消与 outer/data ID 不同 |
| S07 | 保持 DB 不变，只替换/截断/增添 WAL 或 SHM；同字节 inode 替换 | 在捕获前、中、后及消费前各注入，全部拒绝；成员缺失状态也参与绑定 |
| S08 | symlink/hardlink、祖先/root 替换、ACL/owner/mode 漂移、特殊文件、scratch 与源重叠 | 拒绝且不逃逸；不得通过 chmod、搬移或重试修复输入 |
| S09 | journal 出现、热 journal、未知临时对象、索引或历史原件不匹配 | 失败关闭；不运行 SQLite recovery、不当作首次路径 |
| S10 | WAL header/frame/salt/checksum/页大小错配、截断；替换为不符合 proof 的另一 DB 的 WAL | 格式准入或 family 绑定拒绝，不能因 SQLite 忽略 WAL 而误报主 DB schema；不承诺仅靠格式检测 proof 已绑定内容的历史来源 |
| S11 | 修改 `user_version` 但表/索引 layout 不匹配；旧版本无精确验证器；负数/未来版本 | 不返回成功观察；不能仅凭 `integrity_check` 或整数范围通过 |
| S12 | 捕获期间 writer/源程序出现；guard 被替换；授权撤销/到期 | 立即停止继续观察，受控关闭/处置本次副本，原件无业务写入；不能返回之前测到的值 |
| S13 | 副本 hash 错误、漏复制 WAL、复用旧 scratch、只把旧 SHM 当真相 | 拒绝；不能自动改用主 DB、历史副本或来源不明的文件 |
| S14 | SQLite busy、读错、连接关闭失败、磁盘不足、预算超限、清理失败 | 不产生可消费对象；保留细分原因与剩余本次对象，不清理无关文件 |
| S15 | 正常关闭、拒绝或崩溃后检查源目录 | 不出现观察器新建 sidecar、snapshot、guard、receipt 或日志；可写活动仅在登记 scratch 内，崩溃残留不能作为下次资格 |
| S16 | 保存结果后跨回调、跨任务、重新授权、重启或 marker 检查点使用 | 拒绝重放并要求新验证；旧结果不能反序列化或转换资格 |
| S17 | 观察成功但目录为空、独立运行证据缺失或策略新鲜度未知 | 最终取消仍不可用，marker 保留；不得把成功观察转换为默认合格 provider |
| S18 | 曾正常释放后发生合法学习，再查历史 | 普通历史查询不调用旧观察冻结新数据；新的取消请求仍须观察自己的 family |

原件不变的断言须同时配合文件操作范围观测，不能仅靠前后 hash 相同排除“先写后恢复”。合成故障点应覆盖复制中断、事务开始/读版本/验证 layout、close、清理及返回前；SQLite 查询对照不能使用观察器自己的期望常量。

## 后续实施停止线

本轮完成来源盘点、候选路线及验收合同，不声称 schema 缺口已补齐。下一次若批准实施，先固定 scratch 隐私/生命周期和数字预算，再做合成 observer 原型及 WAL 拒绝能力验证；达不到原件零业务写入、有效 WAL 视图或明确错误分类时停止，不直接接 reader/issuer。

源 schema observer 的新增接口、临时数据复制及必要底层能力须按协作规则单独批准。reader 现有无 snapshot 拒绝不变；真实 source 39 运行证据、Q2 隔离及策略新鲜度/撤销仍独立阻塞。检查继续降载串行，本设计批次只运行文档、文本与差异检查，不启动 source、重跑 A1 或执行全仓重型矩阵。
