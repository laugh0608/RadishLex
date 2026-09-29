# ime-product-upgrade 组件说明

本文说明 `radishlex-ime-product-upgrade` 的职责、持久化对象、只读/写入边界和验证入口，面向维护产品数据升级协调器、FFI 与平台宿主的开发者。本文不包含安装器 UI、真实 Application Support 操作步骤、发布进度或历史验证流水；macOS 产品级状态机与回滚约束见 [数据升级协调器边界](../../docs/macos-data-upgrade-coordinator.md)。

## 组件定位

`ime-product-upgrade` 是产品数据升级协调核心。receipt/state 类型不依赖平台 API，当前文件系统编排、身份检查与 Unix socket guard 只在 Unix 构建中提供。它保存严格的 receipt、文件身份、SQLite 一致快照、settings 保留副本、隔离 migration candidate、启动门禁和双端 validation evidence。SQLite schema 与 migration SQL 仍由 `ime-userdb` 独占，macOS 固定路径、容量和进程检查由平台宿主提供。

```text
macOS product host
  ├─ fixed Application Support path / uid / capacity / quiescence
  └─ Manager + InputMethod validation evidence
                         │
                         ▼
                 ime-product-upgrade
                  ├─ receipt + guard
                  ├─ identity checks
                  ├─ snapshot/candidate orchestration
                  └─ startup decision
                         │
                         ▼
                     ime-userdb
                  schema / migration / SQLite checks
```

该 crate 不进入输入热路径，不停止进程，不接受任意产品数据布局，不解释用户词、学习事件、同步对象或 settings 业务含义，也不负责安装包、程序 bundle 切换或备份清理。

## 固定对象与权限

当前只支持 `application-support-v1`。经平台解析并验证的 data root 必须是目标 uid 所有、canonical、非 symlink 的 `0700` 目录。协调状态固定在：

```text
<data-root>/.radishlex-upgrade-v1/
  receipt.json
  receipt.json.tmp
  source-snapshot.sqlite3
  source-snapshot.sqlite3.tmp
  migration-candidate.sqlite3
  migration-candidate.sqlite3.tmp
  source-backup.sqlite3
  source-settings.json
  source-settings.json.tmp
```

状态目录必须为 `0700`，普通文件必须为 `0600`、单 link 普通文件。普通 v1 store/startup reader 只接受上述白名单对象；下节专用准备入口另识别准备记录和保护快照槽，不能据此放宽普通 reader。未知文件、symlink、hardlink、owner/mode 漂移和中断临时对象均失败关闭并保留现场。

`UpgradeReceiptStore::open` 属于升级协调写路径：状态目录不存在时可以创建并持久化它。产品普通启动不能调用该入口，必须调用完全只读的 `inspect_startup_gate`。

恢复 UI 可以使用 `UpgradeReceiptStore::open_existing` 和 `load_for_recovery_inspection`：只打开已有状态，检查 canonical receipt、snapshot/candidate/settings/switch relationship，不创建目录或打开 SQLite。inspection 对 guard socket 做连接探测，活动或未知状态阻断；connection refused 且身份稳定的 stale socket 保留给后续授权执行，不删除。产品普通启动仍只用 `inspect_startup_gate`，不会采用恢复 inspection 放宽 guard 规则。

## Receipt 与状态转换

receipt format 固定为 `radishlex-product-upgrade-receipt-v1`，最大 64 KiB，使用 canonical UTF-8 JSON 和单个末尾换行。operation ID 是 32 位小写十六进制；release、layout、schema 和既有 artifact identity 在同一 operation 内不可改写，artifact 只能追加。

正常状态只允许逐步推进：

```text
preflighted -> quiesced -> snapshot_ready -> candidate_migrated
  -> candidate_verified -> switch_prepared -> switched
  -> post_switch_verified -> completed
```

切换前失败只能进入 `aborted_preserved`；`switched` 后失败先进入 `rollback_required`，精确恢复后进入 `rolled_back`。`completed`、`aborted_preserved` 和 `rolled_back` 是终态，不能在同一 operation 内重新开始。

`UpgradeReceiptStore::persist` 要求调用方持有匹配的 `UpgradeProcessGuard`。写入顺序为同目录 `create_new` 临时文件、文件 `fsync`、身份复验、原子 rename、目录 `fsync` 和 canonical bytes 回读；不合法的跨状态替换、证据删除或字段漂移都会被拒绝。

`UpgradeReceiptStore::verify_current` 只供持有匹配 guard 的外层组合层使用：它复验 data root、状态目录、guard、对象白名单和当前 canonical receipt 精确相等，不推进状态或写文件。这样外层事务不会以调用方仅在内存中提前推进的 data receipt 开始协调。

## 主要能力

### 新源库准备合同

`PreparationReceipt` 是已批准 WAL 新升级方案的独立版本化合同：绑定新 operation、两条前驱链、产品摘要、root/state identity、初始源 family、保护快照及准备后 identity。类型定义从 `reserved` 至 `handoff_ready` 的单步合同；源库、历史接续及 handoff 编排可依次完成准备、旧槽归档和新 v1 交接；单独调用类型方法仍不证明新事务已持久化。canonical 编码最大 256 KiB；未知格式/字段、证据改写和跨 operation 替换失败关闭。它不修改既有 v1 receipt 的 source，也不由类型本身授予文件写入。

`PreparationJournalStore` 提供该记录的持久化层：复用原 v1 状态目录 inode 和 `UpgradeProcessGuard`，只写固定 `source-preparation.json` / `.tmp`；严格校验 root/state 绑定、canonical bytes、前一记录与单步替换。写入经过独占创建、文件 fsync、身份复验、rename、目录 fsync 和回读；相同字节重放也补做文件及目录 fsync。无证明的临时记录、孤立准备快照、未知对象及身份漂移保留并阻断。普通 v1 store/startup reader 继续拒绝准备槽，专用入口不扩展原白名单。

单独调用 journal 的读写方法只证明记录持久化，不认证记录中的 DB、快照、产品和归档。`observe_source_family` 从固定 `userdb.sqlite3` 及 sidecar 读取真实文件身份与摘要；`prepare_userdb_source` 则在匹配 inner guard、已持久化 reservation 和每个 checkpoint 的 `SourcePreparationPort` 确认下推进至 `source_prepared`。它先建立独立保护快照，再持久化 `maintenance_intent`，之后才调用 SQLite 维护；最终复验 DELETE、零 sidecar、原 inode、schema 和全量内容等价，并同步主文件及 data root。准备记录及保护快照保留，原 v1 私有槽不动，不能将 `source_prepared` 解释为已交接或允许启动。

摘要通过可信 `PreparationHasher` 对已打开并前后复验的完整文件流计算；macOS adapter 复用已有 SHA-256 依赖，不接受外部自报摘要。快照和维护前重新计算保守预算：`6 * max(logical_bytes, source_main_bytes) + settings_bytes + 64 MiB`，溢出、容量不足或无法取得 fresh 容量均拒绝。只有 `maintenance_intent` 允许 SQLite 改变主库长度/摘要；主 inode、安全 metadata、快照摘要与逻辑内容仍严格验证。临时/孤立快照不自动认领，未经资格验证的 journal 阻断；维护失败不自动恢复快照覆盖源库。

`maintenance_intent` 下的受限单库 hot rollback journal 可经独立资格检查进入恢复：固定路径、原主 inode、私有 metadata、保护快照摘要、fresh 授权/静止/容量均通过后，先在同一阶段追加并持久化 `journal_recovery` family 身份，再交 SQLite 重放。后续只能接受同一 journal 身份/摘要；WAL/SHM 混存、super-journal 尾标、未知或不完整头部拒绝。源主文件允许同 inode 的页恢复变化，仍须完成 DELETE、零 sidecar 和全量等价复核。日志已消失也必须经过完整准备验证，不能据此推断成功。失败可能已发生 SQLite 物理重放，仍保留记录与快照，不自动覆盖源库。

未使用恢复字段的准备记录继续保持原 canonical 字节；字段仅在持久维护意图阶段追加一次并永久保留，既有严格准备 reader 拒绝含新字段的记录。新 v1 数据 receipt 和 startup 白名单不变。当前证据包括实际 SQLite spill/进程退出、合成 WAL 文件头恢复及组合核心重载；不覆盖 pager 内部任意断电、多库恢复或完整产品升级。

`SourcePreparationPort` 的产品实现必须持有并复验 outer guard、相同 `prepared` operation、受控 source/target 产品、前驱 receipt/inventory 和 fresh 静止观察；合成 port 不证明产品授权。这部分 Executor 接线、明确中止、终态封存的产品接线和完整产品资格按[已批准方案](../../docs/remediation/macos-wal-source-preparation-design.md)继续；不能在旧 v1 receipt 已绑定后单独调用维护 API。

### 历史清单与旧私有槽接续

`capture_previous_inventory` 在 reservation 前核验旧 v1 canonical receipt、终态、数据根和固定私有 artifact，分开绑定新 operation、前一 outer 和前一 data operation。历史清单固定保存在 `.radishlex-upgrade-history-v1/<old-data-operation>/inventory.json`，最大 256 KiB、严格 canonical JSON；目录身份、原 receipt 与各槽 inode/owner/mode/link/length/SHA-256、显式缺失一并封存。调用方仍须持有真实 outer guard 和 fresh 产品/静止授权；核心不通过调用方参数证明外层 receipt 已核验。

`previous_inventory_identity` 复验实物后返回清单文件身份，必须在首次准备记录持久化前通过 `bind_previous_inventory` 绑定，后续不能替换。准备链每个 checkpoint 都核验历史清单及私有材料；错误前驱或材料漂移在 SQLite 写入前即拒绝。旧终态中的运行库 identity 是历史事实，不要求活动库仍保持上次升级的长度/内容；本次 family 由 fresh 准备观察和保护快照证明。

`archive_previous_upgrade` 仅从 `source_prepared` / `previous_archived` 执行，按 receipt、snapshot、candidate、backup、settings 的固定顺序，将实际存在的旧私有槽 rename 到同 operation 的 `data/`。`completed` 的 candidate 已在活动路径，`rolled_back` 的 backup 已恢复到活动路径，因此各自不当作私有槽归档。每次移动先复验授权、guard、源库/快照与全部 inventory，随后同步文件、rename、同步目标/源目录，再在准备记录追加 `archived_slots` 位置证明。原活动状态目录 inode 和 guard 始终保持；活动 DB、settings、Rime、准备快照及 marker 不移动。

恢复只接受按顺序归档的前缀，最多允许一个已有精确 inventory 身份、但位置证明尚未落盘的移动；重新同步后才追加进度。两槽同时存在/缺失、逆序、已记录移动被撤回、未知对象或身份漂移均保留并阻断。inventory 初始化的私有 operation 目录若已存在，不重新创建或认领；尚未创建 operation 的空共享父目录可以重新核验并同步。无旧 receipt 但已有历史目录不能冒充首次升级；有明确已释放前驱时走下述索引接续。

无新增字段的准备记录保持原编码；旧严格 reader 拒绝新增 `previous_inventory_identity` / `archived_slots` 字段。v1 data receipt 与 startup 白名单不变。达到 `previous_archived` 仍保持启动阻断，不表示新 v1 handoff、明确中止、终态释放或产品升级已经完成。

### 新 v1 handoff

`handoff_userdb_source` 消费准备 store，要求明确的新 outer operation ID、同一 inner guard 和每个检查点的 fresh 产品授权/静止确认，成功返回原 v1 store 与 `preflighted` receipt。前驱取旧 data operation，source 取准备后身份，release/schema 取已绑定产品上下文；当前可选 settings 和 Rime 从实物取身份，settings 摘要另在准备证明封存。

先在新 operation 历史目录独占创建 `receipt.json`，同步文件和目录，再于 `previous_archived` 追加不可改认的 `handoff_intent`（两层历史目录身份、canonical 新回执及其文件身份、settings 身份）。随后将该回执原 inode 移入活动槽，同步两目录并验证 canonical 字节，持久化 `handoff_ready`；保护快照移入新历史目录后，最后才把 marker 原 inode 移为 `preparation.json`。全部动作保持状态目录和 guard，不放宽普通 v1 白名单或 `can_replace`。

中断重载只接续持久意图绑定的唯一对象；新回执已活动或 marker 已封存时均交叉核验旧 inventory、准备后 DB、保护快照、当前 settings/Rime、目录和回执。未封存身份的新 operation 目录/文件、两槽冲突、未知记录/版本/临时对象、缺失及身份漂移保持原样并阻断。只有已存在且匹配前驱 inventory 的共享历史根可在新 operation 创建前重新核验；首次路径留下但未绑定的历史根不能认领。marker 移出后的同 operation 重放补做同步并回读，不改写历史证明，也不重复维护 SQLite。

`handoff_intent` 是可选新字段，无字段编码保持；旧严格准备 reader 拒绝含该字段的记录。交接后的 v1 仍非终态，startup gate 继续拒绝启动；原 v1 链创建自己的 migration snapshot，保留独立保护快照。真实 outer/Executor 尚未接线，本能力仅取得仓库合成资格。

### 终态保留封存与释放索引

`TerminalReleaseStore` 复用原状态目录和 guard，独立识别固定 `terminal-release.json`，不放宽普通 v1 reader。当前只接收同一 outer/data operation 的合法 `completed`、`aborted_preserved`、`rolled_back`，且 manual recovery 为 false；原终态加载、零 sidecar、当前 DB/settings/Rime 与回执身份均须成立。不能借此清理已经恢复正常 WAL 的旧事务或冻结现场。

`prepare_terminal_release` 要求 outer 仍非终态。它封存当前实际回执、私有 artifact 与运行数据身份、目录身份、可选 handoff 证明/保护快照、安装 release/product 摘要和前驱索引，新建 `radishlex-terminal-release-v2` 意图后逐槽同步文件并原 inode 移入本 operation 历史 `data/`。已有 v1 意图按原格式恢复，不能改写为 v2。每项按目标/源目录顺序 fsync 后记录不可回退的移动进度；最多一个精确移动可领先记录。源 DB/settings/Rime 不移动，历史准备证明不改写。

完整归档后记录 `release_ready`，原子发布与证明版本匹配的 `latest-release.json`。v2 显式记录 lifecycle operation、legacy outer operation、data operation、安装 release、数据根及带类型的证明摘要；当前只接受 `terminal_release`，三个 ID 必须相同且 data 非空。v1 字段与 canonical 顺序保留。索引旧值和实物身份在 release 意图内保留，重放只接受已绑定旧值或精确新值；已有目标漂移、临时文件、未知对象和身份冲突保留阻断。已有历史而丢失索引不得当作首次 v2；无索引只允许当前 operation 及准备证明明确绑定的旧未释放 inventory。

准备完成仍保留活动 marker，不表示外层已终结或允许启动。`finish_terminal_release` 必须通过 `TerminalReleasePort` fresh 证明匹配同一完整 release 证明的外层终态、安装产品和静止状态，才把 marker 最后原 inode 移为历史 `terminal-release.json`，同步两目录并回读。port 接收完整证明；初始化回调仅为尚未封存的观察草稿，只有 `release_ready` 可绑定外层最终结果。合成 port 的状态和字节比较不能替代真实 outer guard/落盘验证。

`load_latest_release` 只读定位并交叉核验已封存证明、目录、私有材料及准备证据；索引单独存在、marker 仍活动或证明不匹配均不能认定已释放。此查询不是 startup 授权，也不以历史运行 DB hash 拒绝后续正常学习、删除和 WAL。无新 data receipt 的准备中止及真实外层接线仍待完成；本批不新增 v1 失败码或伪造主动取消的错误原因。

共享 reader 沿绑定的 `previous_index` 逐级复验所有祖先，校验旧 canonical 索引的长度/摘要、data 前驱和 source release 连续性及准备证据中的索引/证明身份；重复 operation、缺失和漂移拒绝。查询前后固定当前索引；释放各写入/发布/marker 检查点前后也复验祖先，不能靠最新证明或产品回调豁免旧材料损坏。v1 格式与历史字节保持，具体合同见[数据协调边界](../../docs/macos-data-upgrade-coordinator.md#终态封存外层确认与释放索引)。

### 已释放前驱向新准备接续

活动 v1 槽为空且明确提供前一 data operation 时，`capture_previous_inventory` 通过共享只读 release reader 定位 `latest-release.json`，核验最终证明、目录、旧回执/私有材料及准备证据；不写新 inventory，不扫描猜测前驱。`previous_inventory_identity` 此时返回 release 证明身份，`release_index_identity` 返回索引身份；调用方在首次 reservation 前使用 `bind_released_predecessor` 同时绑定两者，普通旧槽仍使用 `bind_previous_inventory`。

准备记录新增可选且不可改认的 `previous_release_index_identity`，原字段缺失时编码保持，旧严格准备 reader 拒绝新字段。证明摘要复用 `previous_inventory_sha256`，数据回执摘要仍绑定真正的旧 v1 receipt；前一 outer/data 链分别保留。源 release/product 必须与已释放证明匹配；索引或证明即使同字节换 inode 也拒绝，缺失、冲突、临时文件或提前出现的未绑定新 operation 目录不能进入 SQLite 写路径。

新 family 和保护快照从当前运行数据重新取得，允许上次释放后的合法 WAL 学习/删除。准备和 handoff 各检查点复验已固定的索引、证明及历史材料；已释放槽不再移动，`archived_slots` 保持空，`previous_archived` 只记录既有封存证明通过。新 v1 handoff、后续终态释放和下一次准备继续使用同一套历史校验；多个保留的旧 operation 不被认领或改写。真实 outer/Executor 和明确中止仍待完成，合成产品 port 不构成产品授权。

### 准备取消意图与源库收尾

`PreparationCancellationStore` 消费普通准备 store、复用原目录和 guard，在 handoff 意图出现前持久化独立的 `PreparationCancellationRequest`。记录为严格 canonical、最大 256 KiB 的 `radishlex-preparation-cancellation-v1`，当前只接受 `requested` / `user_requested`；绑定原准备记录及文件 inode/metadata/hash、fresh 源 family，并复验保护快照、旧 inventory/已释放前驱及其私有材料。必须已有前一 outer 绑定；journal、未知或不安全对象、handoff 意图和临时写入残留均拒绝。

`request_cancellation` 的成功只表示取消请求已持久化。文件独占创建、同步、受控 rename、目录同步与回读全部完成后才返回；同请求重试重新同步且不替换 inode，临时文件不自动认领。`PreparationCancellationPort` 每个检查点负责真实 outer guard、前一 outer 原字节、未切换的 source 程序和 fresh 静止证明；核心在回调前后复验实物。当前测试的 port 为合成，不能据此宣称产品授权或完整取消已完成。

普通准备/归档/handoff、v1 reader 与 startup gate 不认识取消槽，继续失败关闭。请求入口不调用 SQLite；`load_guarded` 只用于尚未开始收尾的请求。收尾进度一旦存在，请求重放和原请求加载均拒绝，必须经 `load_source_guarded` / `finish_source` 继续同一取消，不能删 marker 后借普通入口继续。

`CancellationSourceReceipt` 在固定 `cancellation-source.json` / `.tmp` 中以严格 `radishlex-cancellation-source-v1` 绑定请求文件身份和工作准备副本。`finishing` 只可追加既有单 journal 恢复身份或推进至 `source_ready`；原准备 marker、取消请求和保护快照保持。`reserved` / `snapshot_ready` 路径不打开 SQLite，以请求 family 原身份和字节证明保持；维护意图后重新取得授权/静止/容量，完成同一受控准备，复验 standalone、源 schema、全量内容/tombstone 等价、关闭与源文件/数据根同步。已经准备好的路径重新验证等价，不重复归档旧材料。

请求仍拒绝现存 journal；取消收尾意图落盘后，专用恢复才可沿用单 journal 资格检查、精确恢复身份持久化和 SQLite 重放。`CancellationSourcePort` 每个检查点确认 fresh 产品授权；未知临时进度、busy、容量不足、身份/内容漂移均保留阻断。`load_source_guarded` 只核验物理关系，不能代替 `finish_source` 的 fresh 授权与内容复验。源库 `source_ready` 不表示完整取消；后续材料封存与兼容恢复已接入 `CancellationArchiveStore`，v2 生命周期索引和最终 marker 释放仍待完成，详见[已批准的补充设计](../../docs/remediation/macos-preparation-cancellation-compatibility.md)。

### 启动门禁

`inspect_startup_gate(data_root, expected_owner_id)` 不创建、删除、chmod、连接 socket 或清理任何对象：

- data root 不存在：`AllowedFirstLaunch`；
- data root 存在但状态目录/receipt 不存在：`AllowedNoUpgradeState`；
- receipt 为终态：`AllowedTerminalReceipt`；
- active guard 或非终态 receipt：阻止启动；
- 损坏 receipt、中断 artifact、未知对象、unsafe root/state 或身份漂移：`FailedClosed`。

调用方只能把三个 `Allowed*` decision 解释为允许。FFI status、result version、decision 或 error code 无法识别时必须失败关闭。

### 快照、settings 与候选

- `create_settings_backup` 从 receipt 已记录身份的固定 settings 创建原样私有副本，不解析或规范化 JSON。
- `create_userdb_snapshot` 使用 `ime-userdb` SQLite backup API 生成一致 standalone snapshot；空间预算为三份 logical snapshot 加 64 MiB reserve。
- `create_userdb_candidate` 只从已记录 snapshot 创建 candidate，并只对隔离临时文件调用 `UserDb::migrate_and_validate`。

三条写路径都在调用前后复验源对象身份，并按临时文件、内容生成、文件持久化、原子 rename、目录持久化、receipt identity、状态推进的顺序提交。任何阶段失败都不得修改原固定 `userdb.sqlite3`。

settings backup、snapshot 或 candidate 的 identity 已持久化而下一状态尚未落盘时，原 API 会复验固定对象、源对象与 SQLite 结构后幂等完成剩余状态转换。只有 rename 已发生但 receipt 尚无 identity、临时文件残留或对象漂移的现场继续失败关闭，不根据文件内容猜测归属。

### 双端验证证据

`record_candidate_validation` 只接受 `UPGRADE_VALIDATION_EVIDENCE_VERSION = 1`：

- Manager evidence 必须包含目标 schema、管理查询和 settings 检查；
- InputMethod evidence 必须包含同一目标 schema、personalized runtime 和候选信号检查；
- 两端通过时，核心再次以 read-only current-schema connection 复验 candidate，才推进 `candidate_verified`；
- Manager 或 InputMethod 明确失败分别以稳定 failure code 进入 `aborted_preserved`。

receipt 不保存 host 输出、任意布尔数组、绝对路径、数据库正文或内容 hash。`candidate_verified` 状态本身是本 operation 已接受双端证据的持久化结论。

### 原子切换与重启恢复

`switch_userdb_candidate` 只接受固定原库、candidate 与 backup 路径。它先要求三个数据库路径都没有 WAL/SHM/journal，再把旧库 identity 以 `BackupDatabase` 追加到 receipt 并持久化 `switch_prepared`，随后执行旧库到 `source-backup.sqlite3`、candidate 到 `userdb.sqlite3` 的同文件系统 rename。每次跨目录 rename 后按目标目录、源目录顺序执行目录 `fsync`，最终复验两个 inode 与 sidecar 零残留后才持久化 `switched`。

调用可从 `candidate_verified`、`switch_prepared` 或 `switched` 恢复。恢复只接受 receipt 所证明的原路径、单次 rename 后现场、两次 rename 后现场或完整 switched 现场；其他缺失、重复、替换、跨设备、sidecar 或未知对象组合失败关闭且不清理。`switched` 幂等重放不会再次移动文件，backup 默认保留。

合法 WAL 中的提交可以通过 SQLite backup 进入 snapshot/candidate，但该事实不满足源库 standalone 切换条件；正常关闭也可能因后续只读连接重建 sidecar 而被拒。macOS 组合层的显式切换前中止只恢复源程序并保留数据；它不在本核心增加 checkpoint、源库 journal-mode 转换或新升级协议。具体保留证据见[数据升级边界](../../docs/macos-data-upgrade-coordinator.md#显式切换前中止的保留证据)。

### 最终验证与回滚

`record_post_switch_validation` 只在 `switched` 接受最终固定路径的双端 evidence。成功先持久化 `post_switch_verified`，随后由独立完成动作复验相同 inode 与 current schema 并推进 `completed`；验证失败或完成前重复复验失败都进入带 `post_switch_validation_failed` 的 `rollback_required`。

回滚先把失败的新库移回固定 candidate 槽，再把旧库 backup 原 inode 恢复到 `userdb.sqlite3`，每次跨目录 rename 后按目标、源目录顺序 `fsync`。receipt 在文件恢复后仍保持 `rollback_required`；只有 source-release evidence v1 与核心只读 schema/integrity 复验同时通过，才能进入 `rolled_back`。失败的新库和全部恢复材料默认保留。

### Guard-bound 协调驱动

`resume_userdb_upgrade` 从已持久化的 `preflighted` 或任一后续状态续跑，不创建 operation 或接受路径。平台通过 `UpgradeCoordinatorPort` 在每个写入、产品 validation 前后和回滚验证前后重新证明静止，并只返回固定 validation report/evidence；核心在同一 `UpgradeProcessGuard` 下调度现有 settings、snapshot、candidate、switch、completion 与 rollback 原语。

静止无法证明时返回精确 checkpoint，receipt 保持最后已证明状态；source-release evidence 不可得时旧库可已恢复，但 receipt 保持 `rollback_required`。candidate 失败正常收敛到 `aborted_preserved`，post-switch 失败正常收敛到 `rolled_back`，成功收敛到 `completed`。macOS 进程检查、helper executable 定位与 manifest 绑定仍由下一层平台适配负责。

## 错误与隐私边界

`UpgradeFilesystemErrorCode` 和 `UpgradeFailureCode` 是调用方分支使用的稳定分类。面向 UI 或普通日志只能输出阶段、稳定 code 和非敏感处置建议；不得记录真实路径、SQL、用户词、输入历史、settings 正文、secret 或 validation host stderr。

发现不确定现场时应保留 receipt、原库、snapshot、candidate 和 settings 副本，不能通过删除状态目录或创建空库绕过门禁。恢复和备份删除必须由后续带版本、固定目标和身份复验的协调动作完成。

## 开发验证

```bash
cargo test -p radishlex-ime-product-upgrade --all-targets
cargo clippy -p radishlex-ime-product-upgrade --all-targets -- -D warnings
./scripts/check-macos-upgrade-preflight.sh
./scripts/check-manager-product.sh
./scripts/check-macos-imk.sh
```

测试只使用隔离合成目录。生产 preflight executable、Manager/InputMethod validation host 和真实产品不得在普通仓库门禁中对真实 Application Support 执行。

### 取消材料封存与外层兼容投影

`CancellationArchiveStore::preserve_cancellation` 在独立 `cancellation-archive.json` 中固定取消请求、`source_ready` 原件、运行数据身份、旧 inventory/已释放证明、历史目录和 outer 原字节。逐槽原 inode 封存旧私有材料、准备记录、保护快照与源库进度；新 outer 已创建时先封存原件，再从已封存的旧 canonical 字节创建并持久绑定兼容副本，最后发布到固定活动 outer 槽。新 outer 尚未创建时仅核验仍活动的旧回执。

数据核心处理受证明约束的字节和文件关系，不解析外层 v1 状态或接管程序 staging/backup；`ime-product-install` 校验旧终态与无 artifact 的新 `prepared` 合同，组合层持有 outer guard 并绑定 root/产品/授权。每次动作重新核验物理身份；源库等价在接续入口复验；未知或未绑定临时对象失败关闭。固定外层历史槽、文件顺序和产品前置条件见[取消兼容设计](../../docs/remediation/macos-preparation-cancellation-compatibility.md#2026-09-29取消材料封存与-outer-兼容恢复)。

返回 `preserved` 仍保留活动请求与封存进度，旧 reader/startup gate 继续阻断；不发布取消生命周期索引、不解除 marker、不表示完整取消或旧程序资格。真实终态的 v2 索引与接续已接入，取消 proof 类型及其接续仍拒绝。真实产品观察与 Executor 尚未接入；测试只证明仓库与隔离合成范围。
