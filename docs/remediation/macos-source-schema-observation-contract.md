# macOS 无 snapshot 源 schema 观察合同

本文供 userdb、升级核心与 macOS 产品协调层维护者审阅，限定准备取消时缺少 snapshot schema 的观察来源、请求绑定、失效与验收条件。状态：**2026-10-18 收尾：获批的仅合成实现留在工作区、尚未提交；完整门禁按用户要求停止，最终验证未完成。生产接入未实施，不提供生产资格。** 合成实现的验证结果与限制见下文实施记录；不操作冻结资产、真实库、账户或会话。

上游为[资格证据 reader 合同](macos-cancellation-qualification-reader-contract.md)及[准备取消兼容合同](macos-preparation-cancellation-compatibility.md)。本设计不改变 v1/v2 回执、action `10–13`、双 guard/gate 或最终 marker；生产目录为空，issuer 未接入。设计源码定位基线为 `ad15bf7`，仅合成实施参数提交为 `ee6a11b`。

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

## 选定路径与范围

首选**当前 family 的受控私有字节副本，由既有 SQLite 引擎观察**。源侧只通过受验证文件句柄读取字节；SQLite 仅看到本次新建的私有副本。它不是 preparation snapshot，不回填 `SnapshotEvidence`，不进入 archive/inventory/lifecycle，不成为备份、可重放资格包或新的数据真相源。

该方案会增加临时敏感数据副本及创建、资源管理和清理责任。**本次追加授权只覆盖下述合成实施包及其 `hooks` / 最小 POSIX 边界，不包含真实数据复制。** 真实部署仍须另行批准生产目录、生命周期及产品操作授权。现有只读 reader 的无写入承诺保持，副本管理不能塞入 `review_evidence`。

| 候选路径 | 本设计取舍 |
| --- | --- |
| 从旧 receipt、版本号、历史实验 schema 或 target 常量推测 | 拒绝；不能关联当前 family |
| 原库直接 `mode=ro`、`query_only` 或 `immutable=1` | 不选；不能据选项名称证明源侧零业务写入和一致性 |
| 只读主文件头，或自行归并 WAL 并解释 schema | 不选；前者漏 WAL，后者新增 SQLite 页视图/schema 解释器；下面的有限 WAL 格式准入校验不能代替 SQLite 读取 |
| 专用内存 VFS 或完整内存数据库映像 | 不在最小批次；新增 VFS/FFI、内存预算及 WAL 重建合同 |
| 私有 DB/WAL 副本 + 既有 SQLite | 作为已批准的合成实施路径；产品接入仍须满足下述完整性、只读与生命周期验收，失败不切换其他路径 |

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

这一路径新增有限 WAL 格式校验的维护成本，已纳入合成实施授权，尚未批准产品接入；若不接受该成本，或不能证明实际 SQLite 选项对准入 WAL 的读取行为，保持 unavailable。本轮没有认可任何实际 source 39 family 满足这些条件，也不把简单无 WAL 样本视为主路径验收。

## proof/family 绑定与结果语义

拟议结果 `SourceSchemaObservation` 为内部不可序列化、不可由调用方构造的短命对象；合成实现位于测试私有模块，不新增产品 API。它必须由上述全流程构造，至少关联以下字段：

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
- source family 预算与 reader 附件预算独立定义；下面的合成验证 profile 固定首批数字，实际容量结果只按下文合成实测记录，不代表真实库已经适配。单线程逐块读取、串行 SQLite 检查，控制期限不超过任务剩余时间且最多 300 秒。调度降载不等于系统调用超时、CPU/RSS 硬配额。
- scratch 位置须排除同步/备份用途并说明系统备份、swap 和崩溃残留边界；私有权限和 unlink 不代表不可恢复擦除。若不能接受明文副本的残留风险，不能以“临时”命名绕过，应另行设计内存/VFS 路线。
- 正常、失败、取消均只清理已核验属于本次调用的对象；授权应预先包含这些生命周期动作。撤销或到期后只允许已批准的资源关闭/清理，不继续查询或返回成功；清理不延长业务授权，无法完成时记录失败与私有残留。进程崩溃后的残留不复用、不自动认领，清理范围另行核验和授权；未知对象原样保留，不能递归删除整个父目录。本批只创建合成 scratch；正常及预定故障的 fixture 处置在实施授权内，意外残留仍须单独登记和授权。

## 最小实施方案：仅合成验证

本节固定已批准的 `schema-observer-synthetic-v1` 实施包。它只允许仓库测试 fixture 生成器建立虚构 source、proof 与数据，不能接受真实数据路径参数、环境变量或拖入的文件；路径位于临时目录本身不证明数据是合成的。fixture 来源须由测试内构造能力及创建记录证明，禁止复制冻结资产或真实库来制作 fixture。这里的数字是首批工程验收界限，不是已有性能结论。

### 位置与独占规则

首批固定 macOS 本机目录模板如下；`<euid>` 来自当前进程有效用户 ID，不能由 CLI 或环境覆盖。该固定目录只由本批合成测试独占创建。

```text
/private/tmp/radishlex-schema-observer-synthetic-v1-<euid>/
  batch.json
  case-0001/
    case.json
    source/                 # 本批生成的合成源及事务 fixture
    scratch/
      userdb.sqlite3
      userdb.sqlite3-wal     # 仅源 family 存在时复制
      userdb.sqlite3-shm     # 仅允许 SQLite 在副本上建立
```

- batch 根用独占创建取得本次所有权；任何既有对象，包括正确 owner/mode 的空目录，都返回 `ScratchOccupied`。不检查 PID 后“接管”，不删除、不另换随机根绕过。固定入口也使一次失败残留阻止同一用户继续累积批次；这不是系统级跨用户锁。
- 创建前验证 `/private/tmp` 的实际目录与祖先、受信 owner/mode/sticky 属性；拒绝 root 或 real/effective UID 不同的运行方式，不用 `/tmp` 别名、`TMPDIR`、当前工作目录或 `canonicalize` 后忽略原路径链接。所有新目录 `0700`、普通文件 `0600`，无扩权 ACL；普通文件 link count 1，目录按平台目录规则检查，不能要求目录 nlink 也为 1。不 chmod 或修复已有对象。
- `case-NNNN` 在本批内单调分配、最多 64 个；同一时刻只存在一个活动 case。源与 scratch 为不同的新目录，句柄固定其身份；不通过前缀字符串判断隔离，不允许跨挂载、目录重叠、链接或复用先前 case。
- scratch 的闭合文件集只有上述 DB、按源存在性决定的 WAL，以及可能产生的 SHM；journal、临时数据库、额外子目录或其他文件均使 case 失败。原 `source` 中的事务材料由合成 fixture 合同验证，不将 scratch 清单套用到它们。
- `batch.json` / `case.json` 各最多 16 KiB，只记录测试协议版本、run/case ID、创建者和实物身份、固定阶段/错误码及资源计数。它们不是产品 receipt、schema 观察结果或清理授权；PID、内容 hash、同名标记都不能单独证明残留可删。run ID 使用操作系统随机来源；随机性不能代替实物核验。

该 `/private/tmp` 位置**仅供合成验证**，不表示系统不会备份、交换或保留文件。真实数据的生产目录、备份排除及残留风险尚未批准，不在首批代码中提供默认位置或回退路径；不得因合成测试通过直接接受真实 source。

### 生命周期与失败残留

| 阶段 | 本批可以做什么 | 离开阶段的条件 |
| --- | --- | --- |
| 独占建立 | 测试宿主创建 batch/case、合成 source 和控制记录；尚未观察 | 全部对象来源、owner/mode/身份及 fixture 关闭状态可证 |
| 捕获 | observer 持有已验证 source 与 scratch 目录能力，按合同复制 DB/WAL | 源 family 复核、复制 hash、资源计数和 WAL 准入全部通过 |
| 读取 | 单 SQLite 连接只访问 scratch，执行固定读事务 | 版本、完整性/layout、事务结束和显式 close 全部成功 |
| 处置 | observer 核验 scratch 条目，逐文件移除，最后移除空 scratch 目录 | 所有连接/句柄已结束，目录项身份无漂移且实际缺失 |
| 返回 | 内存结果经过最后 source/proof/授权复核交给测试调用方 | 只有此阶段可返回成功观察；不落盘、不接 reader 或 issuer |
| 测试收尾 | fixture 宿主核验原 source 未被 observer 改写，关闭自有 fixture 后清理 case；批末清理 batch | 只处置本批亲自创建并仍可证明所有权的对象，逐项核验缺失 |

观察器只拥有 scratch 的清理权，不能删除 `source`、case 或 batch。测试宿主拥有合成 fixture 的独立生命周期；这不改变“观察全过程原 family 不变”的断言。源不变须在 fixture teardown 之前验证，不能用删除测试输入掩盖失败。全部清理使用目录句柄和逐项身份核对，不调用递归删除父目录或 glob 清理。

- 正常成功必须先完成 scratch 处置；失败/取消也按同一自有对象规则尝试关闭与清理，失败则不返回可消费对象。任何 close 未确认、未知对象、链接、权限/身份漂移或 unlink/rmdir 错误均保留现场，返回 `ScratchRetained` 及原始固定失败原因，并停止该批剩余 case。
- 进程崩溃后无内存所有权上下文，固定 batch 根保持占用。下一次仅报告占用，不加载旧结果、重开残留 DB、接管进程或按 TTL/PID 清理。先另行只读登记精确路径、身份、owner、条目及进程状态，再针对本次残留单独授权清理；清理输入若再次漂移立即停止。无自动重试、重命名隔离、压缩上传或启动时清扫。
- 对预先列入测试计划的“故意制造残留/崩溃”用例，测试宿主可在验证拒绝后按该计划拆除自己登记的合成注入对象；这属于事先批准的 fixture teardown，不是 observer 的未知残留恢复能力。未登记对象或真实失败不能套用这项许可；子进程退出仅限测试自己安排的故障点，不 kill 用户进程。
- 取消后的处置预先包含在测试包授权内，最多使用下述保留窗口；不得继续 SQL 或生成成功结果。系统调用不返回时，期限不能强制保证终止；不自动 kill/restart，报告状态未定并保留占用，不能写成清理完成。

### 首批数字预算

MiB/GiB 均按 1024 进制。下列入参上限在读取或分配前用 checked arithmetic 验证；超限拒绝，不拆分、截断、提高预算或省略 hash。整个批次经 `run-low-impact.sh`，不得与其他重型构建/测试并行。

| 项目 | `schema-observer-synthetic-v1` 固定值 | 性质与超限处理 |
| --- | --- | --- |
| 并发 | 1 个 batch、1 个 case、1 个 observer、1 个 SQLite 连接；辅助 SQL 线程 0 | 入场约束；既有对象/并发调用拒绝 |
| 源 family | DB ≤ 256 MiB、WAL ≤ 128 MiB、SHM ≤ 8 MiB，三者总计 ≤ 256 MiB；journal 必须缺失 | 身份声明与实际长度双检；零/缺失语义仍按主合同，不凭长度放行 |
| WAL frame | ≤ 262144 个，页大小仅 SQLite 格式合法值 | 格式及计数上限；超过即拒绝 |
| 复制缓存 | 每块 64 KiB，同时持有的文件字节缓冲合计 ≤ 256 KiB | 实现分配上限；禁止整库/整 WAL 读入内存 |
| scratch | 逻辑文件总长 ≤ 272 MiB；其中 DB+WAL ≤ 256 MiB，生成 SHM ≤ 8 MiB | 应用写入前检查；SQLite 自生文件前后检查，不能冒称文件系统硬配额 |
| batch 保留量 | 同时最多 1 份 source 与 1 份 scratch；含控制材料逻辑总长 ≤ 544 MiB | 不累积成功 case；失败保留后停止，无第二个 batch 绕过 |
| 磁盘余量 | 创建 fixture 前、复制前均须可用空间 ≥ 1 GiB | 预检余量，非预分配/独占保证；运行中 ENOSPC 仍失败 |
| 显式文件读取 | 每次 observer 的累计应用层读取 ≤ 3 GiB | 含源全量复核、复制、WAL 准入、副本复核及其控制材料；每块计费，不因重读/缓存命中免计 |
| 显式文件写入 | 每次 observer 的累计应用层写入 ≤ 256 MiB | 仅 DB/WAL 副本；控制记录由宿主管理，各 16 KiB，不允许 observer 写额外材料 |
| SQLite 缓存与临时页 | `cache_size=-8192`（约 8 MiB 建议值）、`mmap_size=0`、`temp_store=MEMORY`、`threads=0` | 连接内设置并回读；验证实际编译配置支持，不改全局 temp 路径或 heap 设置 |
| schema/layout 材料 | 最多 64 个 schema 对象，返回到 Rust 的结构材料累计 ≤ 256 KiB | 固定查询逐行消费，超限拒绝；不输出 SQL/行内容到日志 |
| 内存验收 | 单个专用测试进程峰值 RSS ≤ 96 MiB，排除编译；同进程生成 fixture 的占用一并计入 | 实测验收阈值，非分配器/系统硬上限；不能重新开始采样抹去峰值，超出则性能项失败，不杀进程或削减检查 |
| 观察窗口 | `end = min(任务截止时间, 入场单调时刻 + 120 秒)`；预留最后 10 秒处置，入场剩余不足 15 秒直接拒绝 | 到 `end-10秒` 停止观察、转关闭/清理；超过 end 不可返回成功，不自动续权 |
| SQLite busy / 控制检查 | busy timeout 100 ms；progress handler 每 1000 VM 指令检查撤销/单调期限；所有 IO 块与阶段前后也检查 | 无 busy 重试循环；这是协作式检查，不能保证阻塞 syscall 的硬超时 |
| 诊断输出 | 每 case ≤ 16 KiB、每 batch ≤ 256 KiB | 仅固定错误/计数/用例 ID；截断诊断不得掩盖结果状态，不含原始 SQL、行数据或文件字节 |

3 GiB 只约束可由本实现计数的文件读取，**不包含 SQLite VFS 内部读取、OS 预读或文件系统重试，不能称为总磁盘 IO 硬上限**。首批必须另记录 wall time、RSS 和平台可得的 IO 统计，无法取得的项目明确标为未测；不为伪造完整计数新增 VFS/全局拦截器。输入体积、语句集合和期限共同限制 SQLite 工作量，不能据此承诺固定 CPU 百分比。

SQLite 缓存是建议值，`temp_store` 还受编译配置影响，progress 回调也不是任意系统调用的超时器。依据：[cache_size](https://sqlite.org/pragma.html#pragma_cache_size)、[temp_store](https://sqlite.org/pragma.html#pragma_temp_store)、[progress handler](https://sqlite.org/c3ref/progress_handler.html)。首批设置只作用于新建测试连接，不设置 `soft_heap_limit`、`hard_heap_limit`、全局 temp 目录或系统资源限额。数值边界不能在测试超时/超内存后自动放宽；调整须提交原因和新的验收范围。

### 实施接口与依赖边界

- 测试宿主创建 synthetic fixture 与目录能力；协调层只接受本次能力和完整 proof，不提供接受任意原库路径的产品命令。observer 的测试成功不会改变生产目录、reader 的无 snapshot 拒绝或最终取消动作。
- `ime-userdb` 保留 `forbid(unsafe_code)`，使用当前锁定 SQLite/rusqlite 的安全接口。新增默认关闭的 `synthetic-schema-observer` feature，仅启用同版本 rusqlite 的 `hooks`；产品协调层只以 dev-dependency 启用。沿用 `backup` / `bundled` 和现有锁定版本，未改 lockfile、未下载依赖，不新增裸 SQLite FFI 或放宽 userdb 的 unsafe 规则。
- scratch 的目录句柄创建/删除能力与现有只读 reader 文件模块隔离；原模块的 unsafe 例外不能自动覆盖新写入/清理实现。本次已单独批准产品协调层测试模块的最小 POSIX 范围：`mkdirat` 独占建目录、`openat` 配合 `O_EXCL` / `O_NOFOLLOW` 创建或打开、`fstat` / `fstatat` 校验、`unlinkat` 逐文件及空目录删除，以及必要的目录枚举/ACL 读取；文件 IO 和 fd 所有权优先用标准库。沿用已锁定 `libc`，不新增依赖版本；不得包含递归移除、全局 umask、chmod 旧对象、进程控制或任意路径执行。该新模块使用独立的测试态 unsafe 例外，调用集合扩大时仍须重新确认。
- 不新增后台清理器、daemon、GUI、全局注册、系统输入法操作或新的持久产品协议。`batch.json` / `case.json` 只服务合成测试残留辨认，没有迁移、恢复或可信报告导入接口。

### 实施包验收与停止线

在 S01–S18 之外，首批补充下列可检查条件；表格是验收要求，实际执行与尚未覆盖项以实施记录为准，不能把整表视为通过。

| 编号 | 必须验证的边界 |
| --- | --- |
| R01 | 固定根已存在、错误 owner/mode/ACL、替换祖先或并发第二次入场均拒绝；不回退其他根 |
| R02 | source/scratch 句柄错配、跨挂载、同 inode/link、未知 scratch 条目及清理前替换均拒绝；源与无关哨兵原样保留 |
| R03 | 捕获、SQL、close、清理各阶段失败；成功只在 scratch 实际缺失后返回，失败残留阻止后续 batch |
| R04 | 关闭后才可 unlink；每个 unlink/rmdir 错误及故意崩溃都记录可核验状态，不自动接管或递归扫除 |
| R05 | 每项长度/计数/累计 IO 上限的等号与加一边界，组合上限及整数溢出；虚假声明和短读不能绕过 |
| R06 | 小于 15 秒入场、工作截止、撤销和处置超时；保留窗口不能续成业务授权，busy 不循环重试 |
| R07 | 256 MiB family、最大允许 WAL 及多轮完整 hash 的单项容量实测；另测 schema/layout 上限，记录各自最坏 wall/RSS/可得 IO，不用稀疏文件或缓存单样本冒充全部上界 |
| R08 | PRAGMA 回读、实际编译配置、progress handler 中断和无辅助线程；observer 写入范围与 SQLite 生成对象全部可核验 |

先实现并验收目录能力/处置和预算，再验证 SQLite/WAL 观察，最后组合 proof 绑定；任何一层不能达到上述合同都停止接线。未来实施可使用低成本故障注入覆盖错误路径，真实大容量样本只串行运行一次匹配边界；新改动或未解释失败才重跑。不得以 quick 检查或一项性能结果替代完整必要验收。

## 负向与正向验收条件

以下为完整目标验收清单；本批实际覆盖与限制单列在实施记录，不把生产场景或未执行故障计为通过。全部只用全新合成目录；每项不仅断言错误码，还核对原 DB/WAL/SHM 的身份、内容和存在状态、未知对象保留、无 marker/回执/索引变化，以及 scratch 处置结果。

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

## 2026-10-17 合成实施记录

本节区分已实现的测试能力与产品合同。协调层四个模块仅在 `cfg(all(test, target_os = "macos"))` 编译；`ime-userdb` 的 `synthetic-schema-observer` 默认关闭，只有协调层 dev-dependency 启用。无产品 CLI、任意原库路径入口、reader 接入、资格 issuer 或可序列化观察结果。真实 source 39 的 schema 仍未知。

- 文件捕获使用独占固定 batch、目录句柄、无链接打开、owner/mode/ACL 检查与逐项身份核验；DB/WAL 按 64 KiB 块复制，源 SHM 只读核验。成功返回前明确 close、复核源/副本、逐项移除 scratch 并确认缺失；SQLite 或文件句柄 close 未确认均优先保留，不能被此前校验错误覆盖。已见 SHM 的身份不能在再次核验时换成另一 inode。
- WAL 准入按锁定 SQLite 实际生成的样本校验格式及 checksum，最终 schema 仍由单一 SQLite 读事务读取。`synthetic-fresh-schema9-exact-v1` 只支持当前编译 schema 9 的精确 manifest；负数、未来版本或相同版本但不同结构拒绝。它不是对历史 source 39 的 layout 认证。
- WAL 模式主 DB 缺失 WAL 时保守拒绝：不能允许只读 SQLite 在 scratch 补出原 family 没有的零长 WAL。合法无 WAL 样本使用 SQLite 生成的 rollback-journal 模式主库。salt 不同的旧 generation 尾部仍拒绝，不宣称源已损坏；合成矩阵使用真实 SQLite `RESTART` 生成并核对旧 salt 残留，而非仅修改首 frame。`HotJournal` 用例仅验证 journal 存在即阻断，不是热 journal 恢复实验。
- 主 DB 旧版本/WAL 新版本用另一个同字节合成副本上的独立 SQLite 查询对照；部分 checkpoint fixture 构造期间短暂持有 reader/writer 两个连接以生成真实形状，均在封存源前关闭。观察窗口本身仍只有一个连接，没有启动真实程序或 writer。
- 严格核心 proof 覆盖 no-index、旧 data、v1/v2 终态、取消 v2 与重复取消前驱；旧 data 的 operation 与 outer 不同。当前最后 marker 始终保留，完整 proof/family/product/call ID 绑定消费一次，撤销、跨调用或重放拒绝。历史 fixture 的已完成标记不代表当前取消已释放。

专用容量测试均在既有降载入口串行执行，计时进程不包含编译，包含 fixture、完整核心证明构造、观察、消费和收尾；未提高预算：

| 合成样本 | DB / WAL 字节 | 显式读取计费 / 复制写入字节 | 整进程 wall | 峰值 RSS |
| --- | --- | --- | --- | --- |
| DB 恰好 256 MiB | 268435456 / 0 | 2986121324 / 268435456 | 608.44 秒 | 25182208 字节（24.02 MiB） |
| 4096 字节页的 WAL 边界 | 143360 / 134217272 | 1647570540 / 134360632 | 304.23 秒 | 11190272 字节（10.67 MiB） |
| 512 字节页的最大 frame 数 | 28160 / 134217648 | 1646249124 / 134245808 | 309.33 秒 | 12681216 字节（12.09 MiB） |

整进程 wall 不是单 observer 时长；观察与消费成功仍经过原单调期限检查，但未额外测分段耗时。三次 `/usr/bin/time -l` 的 block input/output operation 计数均为 0；它们不是字节数，不能推断实际磁盘无 IO。SQLite VFS/OS 实际 IO 字节未测，完整统计与实际验证记录在[本周周志](../devlogs/2026-W42.md)。全目标 S01–S18 / R01–R08 不能仅凭上述实现描述整体判为通过：本批没有真实 source writer、真实程序静止/授权、实际跨挂载或换 owner、磁盘耗尽、内核真实 close 故障、WAL/pager 内部断电或对任意 syscall 的硬超时证据。owner/device/预算边界有纯函数负向测试，close/unlink/rmdir 用预定故障注入；ACL 在新模块中实际读取检查，但未新增需要 ACL 写能力的系统注入。既有 reader 的 ACL 用例不能冒充本模块的新写入生命周期覆盖。

故意退出子进程用例仅从本次父进程的私有 stdout 管道传递创建记录；父进程验证固定退出码、完整闭合清单、实物身份与摘要后，按预先批准计划拆除自己的 fixture。下一普通 batch 仍只报告占用，不读磁盘 JSON 接管；这不提供意外崩溃恢复能力。实施期间两次意外残留分别重新登记并取得单次精确清理授权，处理经过见周志。

2026-10-18 收尾时，第二次完整门禁已通过前段平台专项、Linux repository-only 与 Go 检查，并在工作区阶段再次通过 userdb observer 及安装协调层测试；随后在升级历史矩阵中由项目所有者要求停止，退出码 `143`，不是完整门禁通过。最后的文件 close 注入断言已被工作区测试覆盖，但改动后的定向全目标 Clippy 尚未复验。后台验证进程组及其临时服务已停止，固定 observer 根已确认缺失；中断的其他合成测试材料与日志保留。后续收口顺序见[接下来的事项](../devlogs/2026-W42.md#接下来的事项)，今晚只提交文档。

## 后续接入停止线

本轮只实施获批的合成验证包，仍不声称产品 schema 缺口已补齐。目录处置、SQLite/WAL 和 proof 绑定均隔离于测试路径；达不到原件零业务写入、有效 WAL 视图或明确错误分类时停止，不直接接 reader/issuer。生产目录与真实敏感副本授权仍未定义，不能沿用合成测试参数开放产品调用。

将源 schema observer 接入产品的新接口、真实临时数据复制及扩大底层能力仍须按协作规则单独批准。reader 现有无 snapshot 拒绝不变；真实 source 39 运行证据、Q2 隔离及策略新鲜度/撤销仍独立阻塞。检查继续降载串行；本实施批次执行合成测试和适用仓库门禁，不启动真实 source、重跑 A1 或执行真实产品资格目标。
