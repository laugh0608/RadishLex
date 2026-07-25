# macOS 数据升级协调器边界

本文定义 RadishLex M4-P02 在 `application-support-v1` 布局内的数据升级协调器、SQLite migration、产品双端验证、receipt 和失败恢复边界。读者是 `ime-userdb`、macOS Manager、InputMethodKit、产品装配与后续安装载体的维护者。本文不包含真实用户目录操作步骤、安装器选型、Apple 签名凭据、公证流程或历史命令流水；真实安装与发布仍属于 M4-P03。

## 目标与停止线

M4-P02 要证明程序升级不会把用户数据置于只有新版本能打开、旧版本又无法恢复的中间状态。首个发布候选继续使用：

```text
~/Library/Application Support/RadishLex/
  userdb.sqlite3
  userdb.sqlite3-wal
  userdb.sqlite3-shm
  manager-settings.json
  Rime/
```

本批只处理同一 Application Support 容器内的程序版本与 userdb schema 升级，不迁入 App Group，不改变真实用户同步 gate，也不把安装器变成数据库 migration 真相源。

在进程静止、安全快照、隔离 migration、双端验证、原子切换和失败回滚全部形成可复验证据前：

- 不得对真实原库原地执行升级；
- 不得用空数据库、默认 settings、fixture 或静默重建掩盖失败；
- 不得逐个复制活动 SQLite 的主库、WAL 和 SHM 后声称得到一致快照；
- 不得删除原库、备份或 receipt 来解除失败状态；
- 不得把一次成功 migration 称为产品升级闭环。

## 参与者与职责

### 产品升级协调层

协调层负责编排升级，不解释词条、学习事件、同步对象或 settings 业务字段：

- 解析固定 `application-support-v1` 路径和固定文件槽位；
- 拒绝 symlink、非普通文件、异常 owner/mode/link count 和未知 SQLite sidecar；
- 取得 Manager 与 InputMethod 不再持有数据库连接的静止证明；
- 创建 SQLite 一致快照和 settings 保留副本；
- 只在隔离候选副本上请求 `ime-userdb` migration；
- 调度新版本 Manager 与 InputMethod validation host 分别打开候选数据；
- 持久化版本化 receipt，执行原子切换、切换后验证和失败回滚；
- 在重启后依据 receipt 与固定对象身份继续或停止，不猜测现场归属。

协调层不进入输入热路径，不负责停止任意进程，不接受调用方自定义数据路径，也不读取用户表正文决定升级行为。

首个实现使用独立 `ime-product-upgrade` Rust crate 保存平台无关的状态机、receipt schema 和恢复判断。`platforms/macos-product/UpgradePreflightHost` 负责固定数据根、可用容量、双 bundle 运行状态与受控文件打开句柄的只读适配；这避免把平台调用塞入 `forbid(unsafe_code)` 的核心，也避免把产品升级状态塞入 `ime-userdb`、Flutter UI 或 InputMethodKit 薄壳。crate API 与副作用见 [ime-product-upgrade 组件说明](../crates/ime-product-upgrade/README.md)，macOS executable 的固定输入、输出和门禁见 [产品升级宿主说明](../platforms/macos-product/README.md)。

### `ime-userdb`

`ime-userdb` 继续是 SQLite schema、事务 migration 与数据库结构校验的唯一真相源，并提供四类语义分离的产品能力：

1. `inspect_file`：以只读方式读取 schema 并执行完整性检查，不创建数据库、不配置 WAL、不收紧权限、不执行 migration；
2. `estimate_snapshot` / `create_consistent_snapshot`：在只读事务中估算 logical bytes，并通过 SQLite backup API 把 WAL 可见内容复制到调用方已创建的独立空文件，不迁移源或目标；
3. `migrate_and_validate`：对调用方已经证明为隔离候选的文件执行现有事务 migration，随后验证目标 schema 与完整性，并把候选收敛为不带 WAL/SHM/journal 的单文件 `DELETE` journal 状态；
4. `open_read_only_current`：只接受当前 schema，以 read-only/query-only connection 支持 Manager 查询、personalized runtime 候选信号和协调核心最终复验，不创建文件、不配置 WAL、不 migration 或 chmod。

`ime-userdb` 不负责证明候选是否真的隔离，不停止进程，不生成产品 receipt，也不切换 Application Support 文件。

现有运行时 `UserDb::open` 仍可以为正常产品连接执行 migration。升级协调器不能用该入口检查原库，因为它会改变现场。

### Manager 与 InputMethod

Manager 和 InputMethod 不实现 migration。两端分别在真实 bundle 的 `Contents/Helpers/RadishLexUpgradeValidationHost` 提供受控 host，使用 ABI v8、各自产品 native library 和固定路径规则打开数据库：

- 无参数模式固定验证 migration candidate；唯一可选参数 `--post-switch` 固定验证最终 `userdb.sqlite3`，不得接受路径或其他模式；
- Manager 验证管理查询、对应固定 settings 兼容和关闭连接；
- InputMethod 验证 personalized runtime 创建、只读候选信号访问和关闭连接；
- validation 不产生选择、负反馈、导入、同步或其他业务写入；
- 任一端缺失、版本不匹配、打开失败或未关闭连接，候选不得切换。

双端验证不是用同一个 `UserDb::open` 单元测试冒充两个产品宿主。Manager host 通过只读 current-schema connection 执行 active/deleted/import/learning 管理查询；candidate 模式检查固定 `source-settings.json`，post-switch 模式检查固定 `manager-settings.json`，两者都只验证 settings format v1 类型兼容。InputMethod host 从本 bundle 固定只读 `RimeData` 创建短生命周期隔离 Rime user data；锁定 YAML 只允许部署到该临时目录，不得写回 bundle、共享产品数据或 Application Support。随后使用 privacy-mode `LearningContext` 驱动 personalized runtime 的固定 `luobo` 候选读取，不选择、不提交也不学习。两端都在调用前后比较目标数据库全字节并拒绝 WAL/SHM/journal。

协调核心只接受 `UPGRADE_VALIDATION_EVIDENCE_VERSION = 1` 的固定摘要。Manager evidence 必须同时声明目标 schema、管理查询与 settings 检查完成；InputMethod evidence 必须同时声明同一目标 schema、personalized runtime 与候选信号检查完成。Manager 失败或 evidence 漂移优先记录 `manager_validation_failed`，Manager 通过后 InputMethod 失败或 evidence 漂移记录 `input_method_validation_failed`；这两类切换前失败都原子进入 `aborted_preserved`，原固定数据库不变。只有两端精确通过且协调核心再次以 read-only current-schema connection 复验 candidate，receipt 才从 `candidate_migrated` 单步推进 `candidate_verified`。

receipt 的 `candidate_verified` 状态本身是双端成功的持久化证明，不另存 host stdout/stderr、调用路径、候选正文、内容 hash 或可重放的任意布尔数组。调用方必须是后续固定产品协调入口；UI、安装参数和普通业务代码不能直接构造或覆盖 receipt。

### 安装载体

M4-P03 选择的 `.pkg`、`.dmg` 或安装器应用只能调用稳定协调入口并展示结果。它负责程序 bundle 的安装与恢复，不创建数据库 migration SQL，不解析 receipt 内部字段，不删除真实用户数据。

## 受控数据范围

| 对象 | M4-P02 行为 | 边界 |
| --- | --- | --- |
| `userdb.sqlite3` | 一致快照、隔离 migration、双端验证、原子切换和回滚 | schema 与内容只由 `ime-userdb` 解释 |
| `-wal` / `-shm` / `-journal` | 作为 SQLite 状态处理，不作为独立可复制用户文件 | 静止后仍有未知 sidecar 时失败关闭 |
| `manager-settings.json` | 原样保留、固定身份校验、与程序切换一起恢复 | 当前没有 settings migration，不解析或改写业务字段 |
| `Rime/` | 保留并验证根对象身份，不进入 userdb 候选切换 | Rime 部署与数据升级由独立产品边界管理 |
| 其他对象 | 拒绝自动升级 | 不扫描、移动或删除未知内容 |

数据库不存在时协调器记录 `absent` 并跳过数据 migration；新版本在正常首次启动时创建数据库。preflight 本身不得创建空库。空的现存 SQLite 文件则作为 schema 0 候选处理，必须经过隔离 migration 和双端验证。

## 升级状态机

正常状态严格单向推进：

```text
preflighted
  -> quiesced
  -> snapshot_ready
  -> candidate_migrated
  -> candidate_verified
  -> switch_prepared
  -> switched
  -> post_switch_verified
  -> completed
```

状态含义：

- `preflighted`：固定布局、产品版本、schema、对象类型、owner/mode/link 和空间预算已检查；
- `quiesced`：两端进程与所有已知连接均已关闭，并在快照前再次复核；
- `snapshot_ready`：SQLite 一致快照和 settings 保留副本已落盘、权限已验证、父目录已 `fsync`；
- `candidate_migrated`：候选副本已迁移到目标 schema 并通过 `ime-userdb` 校验；
- `candidate_verified`：新版本两个 validation host 都已打开并关闭候选；
- `switch_prepared`：切换所需原对象/候选/备份身份已固化，重新启动产品仍被阻止；
- `switched`：固定文件名已经原子指向候选，原版本数据仍由备份身份保护；
- `post_switch_verified`：两端已在最终固定路径打开并关闭新数据；
- `completed`：升级结果稳定，产品可以启动；备份仍按保留策略存在。

失败状态：

- 切换前失败进入 `aborted_preserved`，原固定路径没有变化；
- `switched` 后验证失败只能进入 `rollback_required`；
- 原对象恢复、目录持久化和旧版本兼容复验通过后进入 `rolled_back`；
- 无法证明对象身份、静止状态或恢复完成时保持当前 receipt，不自动跨过阶段。

`completed`、`aborted_preserved` 与 `rolled_back` 是终态。同一 operation receipt 不得从终态重新进入升级；新尝试必须使用新的 operation ID，同时保留旧结果的明确关联。

## Receipt contract

receipt 使用 UTF-8 JSON、固定字段顺序和末尾换行，当前格式为 `radishlex-product-upgrade-receipt-v1`。写入必须采用同目录普通 `0600` 临时文件、文件 `fsync`、原子 rename 和父目录 `fsync`；不得覆盖无法严格解析或身份异常的既有 receipt。

当前持久化布局固定为：

```text
<verified-data-root>/
  .radishlex-upgrade-v1/       # 0700
    receipt.json               # 0600
    receipt.json.tmp           # 仅可能由中断写入留下
    source-snapshot.sqlite3    # snapshot evidence 已固化后存在
    source-snapshot.sqlite3.tmp # backup 或 rename 中断时保留
    migration-candidate.sqlite3 # candidate evidence 已固化后存在
    migration-candidate.sqlite3.tmp # copy、migration 或 rename 中断时保留
    source-backup.sqlite3       # switch_prepared 后保存原 userdb 对象
    source-settings.json        # settings backup evidence 已固化后存在
    source-settings.json.tmp    # copy 或 rename 中断时保留
```

状态目录只接受上述固定文件名。读取前重新验证 data root 与状态目录的 canonical path、device、inode、owner 和 mode；receipt 还必须是 `link count = 1`、不超过 64 KiB 的普通文件，并在打开前、打开后和读取后保持同一身份。写入使用 `create_new` 创建临时文件，完成文件 `fsync` 后再次验证 root、guard 与旧 receipt 身份，再原子替换并 `fsync` 状态目录；写回结果必须与预期 canonical bytes 完全一致。

每次持久化最多推进一个合法状态，artifact 证据只能追加，operation、版本、layout 与既有证据不能被改写。相同 bytes 可以幂等重放；新 operation 不得覆盖仍存在的 operation。发现 `receipt.json.tmp` 时当前实现返回中断写入错误并保留现场，不自动猜测应提交还是丢弃。

settings backup、snapshot 或 candidate 的最终 identity 已写入 receipt 而下一状态尚未持久化时，协调核心允许幂等收敛：再次复验源对象、固定目标 identity、sidecar 与 SQLite schema/integrity，再只补写缺失的下一状态。rename 已完成但 identity 尚未写入、临时文件仍存在或任一对象漂移时仍失败关闭，不从“文件能否打开”反推归属。

`candidate_verified` 不新增 artifact 或 validation 明细字段。协调核心在同一 guard 下复核当前 persisted receipt、candidate identity、sidecar 零残留、evidence version/schema/check bits，并用既有 receipt 原子替换链只写一个状态转换。写入前失败保持 `candidate_migrated`；端点明确失败则持久化 `aborted_preserved` 与对应稳定 failure code。

如果 preflight 记录了 `SourceSettings`，协调层必须在仍为 `quiesced` 时从固定 `manager-settings.json` 创建 `source-settings.json.tmp`。复制使用已打开并复验身份的源文件与 `create_new` 私有目标，不解析、不规范化也不改写 JSON；文件 `fsync`、源身份复验、原子 rename 和目录 `fsync` 后，先把 `BackupSettings` identity 追加到 receipt。settings 不存在时两个槽位都不存在；出现 source/backup 单边证据时不得进入 `snapshot_ready`。

settings 故障注入覆盖 staged 创建、copy、rename 和 receipt evidence。staged 文件或无 identity 的最终备份会使加载失败关闭；identity 已持久化而状态仍为 `quiesced` 时可以读取证据，但不自动跨状态。原 settings 文件保持原字节与身份，路径替换、symlink/hardlink、owner/mode/link 或 byte length 漂移均拒绝。

receipt 只允许保存：

- format、operation ID、固定 data layout；
- source/target 产品版本与 build number；
- source/target userdb schema；
- 当前状态、稳定 failure code 与是否需要人工恢复；
- 固定逻辑槽位的 device、inode、owner、mode、link count 和 byte length；
- 上一 operation ID（仅用于显式重试关联）。

receipt 不保存：

- 用户词、reading、输入 code、选择/负反馈事件或同步 payload；
- 数据库、WAL、settings 或 Rime 文件正文；
- 用户 home、真实绝对路径、进程命令行或环境变量；
- token、密钥、签名、恢复码、wrapped material；
- 数据内容 hash、可跨设备关联的用户数据指纹或调试 SQL。

operation ID 只接受协调层生成的固定长度小写十六进制随机标识。产品版本、build、layout、状态与 failure code 使用封闭类型；未知字段、未知枚举、格式漂移和不一致状态一律拒绝。

普通文件必须保持 `link count = 1`。APFS 目录的 link count 会随目录项变化，只作为观察字段记录，不参与稳定身份等值；目录稳定身份由对象类型、device、inode、owner、mode 与 canonical path 共同证明。

## SQLite 快照与切换语义

### 快照

进程静止是产品升级前置条件，但不能单独证明 SQLite 主文件包含 WAL 中的最新提交。协调器必须通过 SQLite backup API 或等价的 SQLite 一致快照能力生成候选基础，不能用普通文件复制拼装数据库 family。

当前 `ime-userdb` 使用仓库既有 `rusqlite 0.32.1` 的 backup feature 提供两个分离入口：`estimate_snapshot` 在只读事务中读取 schema、`page_size`、`page_count` 并执行 `quick_check(1)`；`create_consistent_snapshot` 只接受调用方已创建的独立空普通文件，以同一只读事务通过 SQLite backup API 复制全部页。backup 完成后目标必须转为单文件 `DELETE` journal、再次通过 `quick_check(1)`，并与源事务的 schema/page 元数据一致；目标不得留下 WAL/SHM，也不执行 migration。schema 0 的零字节数据库按一个将被 materialize 的 SQLite header page 计入预算。

协调层只允许固定 `userdb.sqlite3` 作为源、固定 `source-snapshot.sqlite3.tmp` / `source-snapshot.sqlite3` 作为目标；源主文件与 snapshot 在操作前后都校验 type、device、inode、owner、`0600`、`link count = 1` 和 byte length。当前保守空间预算为 `3 * logical_snapshot_bytes + 64 MiB`，覆盖 snapshot、后续 migration candidate、切换/rollback 工作余量和最低文件系统余量；真实 available bytes 必须由固定 data root 的平台文件系统端口提供，不能接受 UI 或任意路径调用方自报。预算不足或算术溢出在创建临时文件前失败。

snapshot 写入顺序固定为：`create_new` 私有临时文件、SQLite backup/validation、文件 `fsync`、原子 rename、状态目录 `fsync`、先把 snapshot identity 追加到仍为 `quiesced` 的 receipt，再单独持久化 `snapshot_ready`。故障注入覆盖临时文件创建后、backup 后、rename 后和 receipt evidence 后：临时文件残留或已 rename 但尚未记录 identity 时启动加载失败关闭；identity 已持久化而状态仍为 `quiesced` 时允许读取证据，但当前不会自动猜测并推进状态，恢复动作留给后续 crash-recovery 切面。

快照完成后必须：

- 在独立连接上执行完整 `quick_check`；
- 记录原对象和快照对象身份，不记录内容 hash；
- 把候选、备份和 receipt 放在同一受控父目录或已证明支持所需原子语义的位置；
- 保持目录 `0700`、普通数据文件 `0600`，拒绝 hardlink、symlink 和未知对象；
- 在进入 migration 前再次确认原固定路径身份未漂移。

如果空间预算不足以同时保留原库、settings 副本、快照、迁移候选和切换恢复余量，preflight 失败，不边复制边删除旧文件腾空间。当前 crate 已固定 SQLite 工作预算、settings 私有副本和拒绝语义；macOS preflight host 针对固定 data root 同时查询 `volumeAvailableCapacityForImportantUsage` 与普通 available capacity，取两者可用正值中的保守值。后续协调入口必须在 settings copy 前把 settings byte length 纳入总预算，再把原始 available bytes 交给 Rust 预算判断。

### 隔离 migration candidate

协调层只允许从 receipt 已记录身份的固定 `source-snapshot.sqlite3` 创建固定 `migration-candidate.sqlite3.tmp`，不接受调用方自定义源或目标。复制使用已打开并复验身份的 snapshot 文件句柄与 `create_new` 私有目标；复制完成、文件持久化和 snapshot 身份复验后，才把临时 candidate 交给 `UserDb::migrate_and_validate`。该 API 可以在迁移过程中使用 WAL，但返回前必须切回 `DELETE` journal、再次执行完整性与目标 schema 校验，并确认没有 `-wal`、`-shm` 或 `-journal` 残留。

candidate 成功顺序固定为：复制 snapshot 到临时 candidate、隔离 migration/validation、文件 `fsync`、复验临时文件与 snapshot 身份、原子 rename、状态目录 `fsync`、先把 candidate identity 追加到仍为 `snapshot_ready` 的 receipt，再单独持久化 `candidate_migrated`。当前 schema 返回 `migrated = false`，schema 0 与受支持旧 schema 返回真实源/目标版本；未来 schema、损坏 snapshot、目标版本不符或身份漂移均不得生成可切换 candidate。

故障注入覆盖临时文件创建后、snapshot copy 后、migration 后、rename 后和 receipt evidence 后。临时 candidate 或无 receipt identity 的最终 candidate 会使加载失败关闭；identity 已持久化而状态仍为 `snapshot_ready` 时只允许读取既有证据，不自动猜测并推进状态。原 `userdb.sqlite3` 和 `source-snapshot.sqlite3` 在整个 migration 过程中保持不变。

### 切换

候选只有在 migration 与双端验证都通过后才能进入 `switch_prepared`。切换操作必须有明确的原对象、备份对象和候选对象身份，并通过同文件系统 rename、文件与目录持久化形成可恢复序列。

首个实现固定三个路径，不接受调用方覆盖：

```text
<data-root>/userdb.sqlite3
<data-root>/.radishlex-upgrade-v1/migration-candidate.sqlite3
<data-root>/.radishlex-upgrade-v1/source-backup.sqlite3
```

`SourceDatabase` 与 `BackupDatabase` 表示同一个旧库 inode 在切换前后的两个固定路径；`CandidateDatabase` 表示同一个新库 inode 在 candidate 路径和最终固定路径之间移动。准备切换时先确认 data root、状态目录、原库和 candidate 的 device 相同，backup 路径不存在，三个数据库路径均没有 WAL/SHM/journal。协调器随后把旧库 identity 以 `BackupDatabase` 槽位追加到仍为 `candidate_verified` 的 receipt，再单独持久化 `switch_prepared`。任何 rename 都只能发生在这两个 receipt 写入完成之后。

切换顺序固定为：

1. `userdb.sqlite3 -> source-backup.sqlite3`；复验 backup 仍是 receipt 记录的旧库对象，依次 `fsync` 目标状态目录和源 data root；
2. `migration-candidate.sqlite3 -> userdb.sqlite3`；复验最终固定路径仍是 receipt 记录的 candidate 对象，依次 `fsync` 目标 data root 和源状态目录；
3. 再次复验 backup、最终固定路径、sidecar 零残留和同文件系统身份，最后持久化 `switched`。

不能假设一次 rename 就覆盖完整数据库 family。最终固定路径切换前后必须确保没有遗留 WAL/SHM/journal 使另一代主库被错误重放。切换函数允许从 `candidate_verified`、`switch_prepared` 或已持久化的 `switched` 幂等调用，但只接受以下现场：

| persisted state | 原固定路径 | candidate 路径 | backup 路径 | 唯一动作 |
| --- | --- | --- | --- | --- |
| `candidate_verified` | 旧库 | candidate | 不存在 | 记录 backup identity 并进入 `switch_prepared` |
| `switch_prepared` | 旧库 | candidate | 不存在 | 执行两次 rename |
| `switch_prepared` | 不存在 | candidate | 旧库 | 从第二次 rename 继续 |
| `switch_prepared` | candidate | 不存在 | 旧库 | 复验后只持久化 `switched` |
| `switched` | candidate | 不存在 | 旧库 | 幂等返回，不重复 rename |

其他组合全部失败关闭并保留现场，包括同一对象出现在错误路径、对象缺失或替换、backup 提前存在、candidate 与旧库 identity 混淆、跨文件系统、未知 sidecar 或未知状态目录对象。恢复不能根据“哪个文件能打开”猜测世代，也不能自动删除任何对象。

### 最终路径验证与回滚

`switched` 后协调器在同一 guard 下分别以 Manager/InputMethod helper 的 `--post-switch` 模式打开最终固定 `userdb.sqlite3`。核心只接受 post-switch evidence v1、目标 schema 和两端固定检查位，并再次复验最终路径仍是 receipt 的 `CandidateDatabase` inode、backup 仍是 `BackupDatabase` inode且 sidecar 为零：

- 双端通过后先持久化 `post_switch_verified`；再次复验固定现场与 current schema 后单独持久化 `completed`；
- 任一端失败、evidence 漂移、核心复验失败或完成前重复复验失败都持久化 `rollback_required` 与 `post_switch_validation_failed`，不能直接覆盖为 `rolled_back`；
- `post_switch_verified` 本身是双端成功的持久化证明，崩溃恢复不重放 host，只复验固定现场与 current schema；成功推进 `completed`，失败进入 rollback。

回滚复用现有固定 candidate 槽，不新增任意文件名：

1. `userdb.sqlite3 -> migration-candidate.sqlite3`，把验证失败的新库移回 receipt 已记录的 candidate identity；依次 `fsync` 目标状态目录和源 data root；
2. `source-backup.sqlite3 -> userdb.sqlite3`，把旧库原 inode 恢复到固定路径；依次 `fsync` 目标 data root 和源状态目录；
3. receipt 保持 `rollback_required`，直到 source-release validation evidence v1 与核心只读 `inspect_file` 同时证明恢复库 schema、integrity 和旧版本打开检查完成，才持久化 `rolled_back`。

`rollback_required` 只接受三类可恢复现场：切换后原现场、只把新库移回 candidate 的中间现场、旧库已恢复但尚待 source-release evidence 的现场。每次恢复调用都先重复必要目录 `fsync`；其他对象组合失败关闭。失败的新库、snapshot、settings 副本和 receipt 均继续保留，不在本批清理。

## 启动门禁与中断恢复

Manager 与 InputMethod 在产品启动前必须检查是否存在非终态升级 receipt：

- `preflighted` 至 `switch_prepared`：阻止产品写连接，由协调器确认原库未变后继续或进入 `aborted_preserved`；
- `switched`：阻止正常启动，必须执行最终路径验证或进入 `rollback_required`；
- `rollback_required`：只允许精确恢复动作；
- receipt 无法解析、身份不符或出现未知对象：失败关闭并保留现场，不能删除 receipt 后继续；
- 终态 receipt 可以用于诊断和后续备份清理，但不授权删除数据。

升级协调器的互斥范围是同一用户、同一固定产品数据根。单进程 mutex 不足以跨安装器、Manager 和 InputMethod 进程证明所有权；实现必须使用带身份校验的跨进程 guard，并覆盖异常退出后的恢复。

当前 guard 使用 root-owned、sticky `01777` 的短临时根，固定命名为 `rlx-upgrade-<uid>-<data-root-device>-<data-root-inode>.sock`，socket 必须属于目标用户且保持 `0600`。名称只绑定固定数据根身份，不包含 home、真实路径或用户数据 hash；使用短路径也避免 macOS `sockaddr_un` 长度受深层 Application Support 路径影响。

新调用先验证既有对象确为同用户私有 socket：连接成功视为活动 operation；仅在精确 socket 返回 `ConnectionRefused` 时按异常退出残留删除并重新绑定。symlink、普通文件、owner/mode/type 漂移或其他连接错误一律失败关闭且不删除现场。guard 释放时也只删除与取得时 device/inode 身份完全一致的 socket，不扫描或清理其他临时内容。

macOS preflight host 不接受调用方路径或进程名。它从产品 manifest 固定 Manager/InputMethod bundle ID，通过 `NSRunningApplication` 判断双端是否仍运行，并以系统固定 `/usr/sbin/lsof` 检查已存在的 `userdb.sqlite3` family、`manager-settings.json` 与其原子写临时文件是否仍有打开句柄；输出只包含 format、result、available bytes、quiescent 和稳定 blocker，不回显路径、进程详情或 `lsof` 内容。unsafe root/file、容量不可得或检测工具异常都失败关闭。

这份结果只是同一时刻的只读证据：它不停止进程，也不能阻止旧版本在检测后、snapshot 前重新启动。ABI v8 startup gate 已在 Manager `applicationWillFinishLaunching` 调用 `super` 之前、InputMethod 创建 `IMKServer` 之前接线；只读允许 data root/state absent 与终态 receipt，active guard、所有非终态、损坏 receipt、中断 artifact、未知对象和身份漂移均阻止业务初始化。它不创建目录、不改权限、不连接或清理 guard。

`resume_userdb_upgrade` 已把 `preflighted` 至终态的既有原语纳入同一 `UpgradeProcessGuard`。平台 port 必须在进入 `quiesced`、settings/snapshot/candidate、切换、完成和回滚前重新证明静止，并在 candidate/post-switch/source-release host 返回后再次证明静止；任一 checkpoint 失败都保持最后已持久化状态。该核心驱动不定位 executable、不启动进程、不接受路径；macOS adapter 仍须把每个 checkpoint 接到固定 preflight host，并把双端与 source-release helper 绑定到受 manifest 证明的固定 bundle。

### macOS 固定产品适配器

macOS adapter 只接受两个已经形成产品装配的根目录，不接受独立 helper 路径、bundle ID、版本、build、schema 或 data root 覆盖：

```text
<source-product-root>/
  ProductManifest.json
  Components/
    radishlex_manager.app/
    RadishLexInputMethod.app/
<target-product-root>/
  ProductManifest.json
  Components/
    radishlex_manager.app/
    RadishLexInputMethod.app/
```

两个根目录都必须是 canonical 的真实目录。adapter 严格解析 `ProductManifest.json`，固定要求 `radishlex-macos`、`application-support-v1`、唯一 Manager/InputMethod component、正整数 build 与 schema，并把 manifest release/schema 与 receipt 的 source/target 字段逐次比较。helper 只能从 component 固定相对路径解析：

- target Manager `Contents/Helpers/RadishLexUpgradePreflightHost`：所有 quiescence checkpoint；
- target Manager 与 InputMethod `Contents/Helpers/RadishLexUpgradeValidationHost`：candidate 和 `--post-switch`；
- source Manager 与 InputMethod `Contents/Helpers/RadishLexUpgradeValidationHost`：恢复后的 `--post-switch` 兼容验证。

每次执行前都必须重新确认 executable 是非 symlink、单 link 普通文件，长度和 SHA-256 与所属 manifest 的 file record 一致；manifest 缺少固定 record、重复 component/path、hash/size 漂移或 receipt release/schema 不一致均失败关闭。adapter 不把 stdout/stderr、绝对路径或 checkpoint 写入 receipt；preflight 只接受严格的 `radishlex-upgrade-preflight-v1` ready JSON，validation host 只以受控退出码形成固定 evidence。source-release evidence 只有旧 Manager 和旧 InputMethod 都在恢复后的最终固定路径成功打开并关闭后才能形成。

`ProductManifest.json` 在 M4-P02 证明组件与 helper 的内容绑定，不单独证明发布者身份。Developer ID、Hardened Runtime、notarization、安装位置所有权和运行前 code signature requirement 属于 M4-P03，不能由 manifest hash 替代。

### 隔离产品协调资格

产品协调资格 harness 只在 Cargo `qualification-harness` feature 下编译，不进入普通产品构建或安装载体。它仍执行与产品相同且受 manifest 绑定的 preflight/validation executable，唯一测试差异是为子进程设置固定 `CFFIXED_USER_HOME`，使 Foundation 用户域解析到合成 home。production runner 必须主动移除调用环境中的 `CFFIXED_USER_HOME`，不能继承或开放这项覆盖。

合成 home 必须同时满足：

- 位于当前系统 canonical temp root 之下，且 home 本身是非 symlink、当前进程可访问的 `0700` 目录；
- 同级资格根存在内容精确为 `radishlex-upgrade-qualification-v1` 的固定 marker；
- `Library/Application Support/RadishLex` 由 harness 以 `0700` 创建，所有数据库/settings 只使用合成内容；
- 产品根、合成 home 和场景名只从仓库固定资格脚本传入测试进程，不形成产品 CLI、UI 参数或 receipt 字段；
- 场景结束删除整个短生命周期资格根，不扫描或修改其他 temp 内容。

资格脚本从同一次构建输出形成 target `0.1.0 (35)` 与 source qualification `0.0.9 (34)` 两份完整装配。source qualification 只改写两端 bundle 的版本/build 元数据并重新 ad-hoc 签名，再以固定测试 metadata 生成独立 manifest；其 helper/native code 与当前受测源码一致，userdb schema 仍为 9。它证明 source/target manifest 路由、真实双端打开、切换和原 inode 回滚，不冒充历史 schema 8 产品二进制。旧 schema migration 正确性继续由 `ime-userdb` 与协调核心的逐版本合成测试证明；未来真实跨发布升级还必须保留并验证实际 source release 装配。

故障场景先让真实 helper 完成对应调用，再在 `UpgradeCoordinatorPort` 结果边界注入稳定 Manager/InputMethod failure、指定 checkpoint 静止丢失或一次 source evidence 不可得。这样既保留真实产品打开证据，也能确定性验证 `aborted_preserved`、最后已证明状态、`rollback_required` 和重启续跑；故障注入不能修改产品 helper、数据库正文或 receipt。

## 稳定错误分类

首批 error code 至少覆盖：

- `unsupported_receipt`、`receipt_inconsistent`、`operation_already_active`；
- `unsafe_data_root`、`unexpected_data_object`、`identity_changed`；
- `process_not_quiescent`、`database_busy`；
- `database_corrupt`、`future_schema`、`migration_failed`；
- `insufficient_space`、`permission_denied`、`snapshot_failed`；
- `manager_validation_failed`、`input_method_validation_failed`；
- `switch_failed`、`post_switch_validation_failed`、`rollback_failed`。

对用户和日志只返回阶段、稳定 code 与非敏感处置建议。底层 SQLite、文件路径和 validation host 细节只进入本地受控诊断，不进入 receipt、遥测或同步数据。

## 验证矩阵

### 自动隔离验证

必须使用临时合成目录覆盖：

- 数据库 absent、空库、当前 schema、每个仍受支持的旧 schema；
- 未来 schema、损坏 SQLite、歧义旧 schema 和 migration 事务回滚；
- 活动 WAL 数据、busy/locked、未静止连接和未知 sidecar；
- 数据根、数据库、候选、备份或 receipt 为 symlink/hardlink；
- owner/mode/link 漂移、只读目录、磁盘空间不足和 rename/fsync 故障；
- receipt 未知字段、未知状态、字段不一致、篡改和重复 operation；
- 每个持久化阶段之前和之后的故障注入；
- 切换前失败保持原字节可用，切换后失败完成精确回滚；
- 重启恢复幂等，不跨越未被持久化证明的状态；
- receipt、错误和测试日志不包含合成词正文、SQL、绝对 home 或 secret。

### 产品验证

自动状态机和 userdb 测试通过后，还必须以产品构建验证：

- Manager validation host 打开候选并关闭；
- InputMethod validation host 打开同一候选并关闭；
- 切换后的固定路径由两端再次打开；
- 回滚后的原数据由旧版本兼容 host 打开；
- 产品 manifest 的版本、build 和 userdb schema 与 receipt 目标一致；
- 没有启动 GUI、安装输入法、选择输入源或访问真实用户目录。

真实 Application Support、真实进程停止、安装升级和回滚演练必须进入后续专用 runbook，并按仓库规则另行授权。

## 实现顺序

1. 固定本文、状态机、receipt schema、错误枚举和负向解析测试；
2. 在 `ime-userdb` 增加只读 inspection 与显式 migration/validation summary；
3. 实现临时目录内的 receipt 原子持久化和跨进程 guard；
4. 实现 settings 原样保留副本、SQLite 一致快照、空间预算、身份校验与故障注入文件系统端口；
5. 从固定 snapshot 创建隔离 migration candidate，固化 standalone SQLite 与 receipt evidence；
6. 已接入固定 macOS available-space、点时静止探针、双端 startup gate 和 Manager/InputMethod 候选 validation host；
7. 已把双端验证结果以 `candidate_verified` 或端点 failure 原子持久化，并完成固定 backup、同文件系统双 rename、目录持久化与逐边界重启恢复；
8. 已在最终固定路径完成双端验证、`post_switch_verified` / `completed` 和精确 rollback；
9. 已补齐 settings/snapshot/candidate evidence-only 恢复，并实现同一 guard 下逐 checkpoint 复验静止的核心协调驱动；
10. 已实现 macOS 固定 host adapter、source/target manifest 绑定、target Manager preflight 装配和 adapter contract；
11. 已在隔离合成 Application Support 中以真实产品 helper 完成成功、双端失败、静止丢失、回滚和重启恢复协调资格；
12. M4-P03 选定安装载体后再编写真实安装升级 runbook。

## M4-P02 退出标准

M4-P02 只有同时满足以下条件才可退出：

- 原数据库永不由协调器原地 migration；
- WAL 数据通过 SQLite 一致快照进入隔离候选；
- 旧 schema、当前 schema、未来 schema、损坏和并发占用具有稳定结果；
- Manager 与 InputMethod 在候选和最终固定路径分别完成真实产品打开验证；
- 每个持久化阶段的中断都能幂等继续、保留原数据或精确回滚；
- receipt 严格、私有、无敏感内容，且不能被删除或篡改来绕过启动门禁；
- 备份清理是有版本、receipt 与固定目标的后续动作，默认升级不删除恢复材料；
- 全部证据来自隔离合成数据和产品 host，不触碰真实用户数据。

截至 2026-07-25，上述条件已由核心 61 项测试、manifest-bound adapter contract 与真实双产品协调资格覆盖，M4-P02 对首个发布候选退出。资格 source 使用独立 `0.0.9 (34)` metadata、重新签名与 manifest，但因尚不存在上一版正式发布包，native code/schema 与 target 同源；这一限制不影响首发数据协调器退出，也不得被描述成历史二进制兼容证据。M4-P03 必须保留 source/target 程序版本回滚边界；从首个发布包形成后，后续版本必须用实际 source release 装配重跑跨发布资格。

稳定验证入口：

```bash
./scripts/check-macos-upgrade-coordinator.sh
./scripts/check-macos-upgrade-product-coordination.sh
```
