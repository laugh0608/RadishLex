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

状态目录必须为 `0700`，普通文件必须为 `0600`、单 link 普通文件。目录只接受上述白名单对象；未知文件、symlink、hardlink、owner/mode 漂移和中断临时对象均失败关闭并保留现场。

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

`PreparationReceipt` 是已批准 WAL 新升级方案的独立版本化合同：绑定新 operation、两条前驱链、产品摘要、root/state identity、初始源 family、保护快照及准备后 identity。阶段从 `reserved` 单步推进至 `handoff_ready`，canonical 编码最大 256 KiB；未知格式/字段、证据改写和跨 operation 替换失败关闭。它不修改既有 v1 receipt 的 source，也不由类型本身授予文件写入。

`PreparationJournalStore` 提供该记录的持久化层：复用原 v1 状态目录 inode 和 `UpgradeProcessGuard`，只写固定 `source-preparation.json` / `.tmp`；严格校验 root/state 绑定、canonical bytes、前一记录与单步替换。写入经过独占创建、文件 fsync、身份复验、rename、目录 fsync 和回读；相同字节重放也补做文件及目录 fsync。无证明的临时记录、孤立准备快照、未知对象及身份漂移保留并阻断。普通 v1 store/startup reader 继续拒绝准备槽，专用入口不扩展原白名单。

单独调用 journal 的读写方法只证明记录持久化，不认证记录中的 DB、快照、产品和归档。`observe_source_family` 从固定 `userdb.sqlite3` 及 sidecar 读取真实文件身份与摘要；`prepare_userdb_source` 则在匹配 inner guard、已持久化 reservation 和每个 checkpoint 的 `SourcePreparationPort` 确认下推进至 `source_prepared`。它先建立独立保护快照，再持久化 `maintenance_intent`，之后才调用 SQLite 维护；最终复验 DELETE、零 sidecar、原 inode、schema 和全量内容等价，并同步主文件及 data root。准备记录及保护快照保留，原 v1 私有槽不动，不能将 `source_prepared` 解释为已交接或允许启动。

摘要通过可信 `PreparationHasher` 对已打开并前后复验的完整文件流计算；macOS adapter 复用已有 SHA-256 依赖，不接受外部自报摘要。快照和维护前重新计算保守预算：`6 * max(logical_bytes, source_main_bytes) + settings_bytes + 64 MiB`，溢出、容量不足或无法取得 fresh 容量均拒绝。只有 `maintenance_intent` 允许 SQLite 改变主库长度/摘要；主 inode、安全 metadata、快照摘要与逻辑内容仍严格验证。临时/孤立快照不自动认领，未经资格验证的 journal 阻断；维护失败不自动恢复快照覆盖源库。

`maintenance_intent` 下的受限单库 hot rollback journal 可经独立资格检查进入恢复：固定路径、原主 inode、私有 metadata、保护快照摘要、fresh 授权/静止/容量均通过后，先在同一阶段追加并持久化 `journal_recovery` family 身份，再交 SQLite 重放。后续只能接受同一 journal 身份/摘要；WAL/SHM 混存、super-journal 尾标、未知或不完整头部拒绝。源主文件允许同 inode 的页恢复变化，仍须完成 DELETE、零 sidecar 和全量等价复核。日志已消失也必须经过完整准备验证，不能据此推断成功。失败可能已发生 SQLite 物理重放，仍保留记录与快照，不自动覆盖源库。

未使用恢复字段的准备记录继续保持原 canonical 字节；字段仅在持久维护意图阶段追加一次并永久保留，既有严格准备 reader 拒绝含新字段的记录。新 v1 数据 receipt 和 startup 白名单不变。当前证据包括实际 SQLite spill/进程退出、合成 WAL 文件头恢复及组合核心重载；不覆盖 pager 内部任意断电、多库恢复或完整产品升级。

`SourcePreparationPort` 的产品实现必须持有并复验 outer guard、相同 `prepared` operation、受控 source/target 产品、前驱 receipt/inventory 和 fresh 静止观察；合成 port 不证明产品授权。这部分 Executor 接线、旧 operation 接续、明确中止、终态封存和完整产品资格按[已批准方案](../../docs/remediation/macos-wal-source-preparation-design.md)继续；不能在旧 v1 receipt 已绑定后单独调用维护 API。

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
