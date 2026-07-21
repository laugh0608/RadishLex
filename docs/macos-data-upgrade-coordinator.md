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

首个实现使用独立 `ime-product-upgrade` Rust crate 保存平台无关的状态机、receipt schema 和恢复判断。macOS host 负责固定路径、进程与文件系统适配；这避免把产品升级状态塞入 `ime-userdb`、Flutter UI 或 InputMethodKit 薄壳。

### `ime-userdb`

`ime-userdb` 继续是 SQLite schema、事务 migration 与数据库结构校验的唯一真相源，并提供两个语义分离的产品能力：

1. `inspect_file`：以只读方式读取 schema 并执行完整性检查，不创建数据库、不配置 WAL、不收紧权限、不执行 migration；
2. `migrate_and_validate`：对调用方已经证明为隔离候选的文件执行现有事务 migration，随后验证目标 schema 与完整性，并把候选收敛为不带 WAL/SHM/journal 的单文件 `DELETE` journal 状态。

`ime-userdb` 不负责证明候选是否真的隔离，不停止进程，不生成产品 receipt，也不切换 Application Support 文件。

现有运行时 `UserDb::open` 仍可以为正常产品连接执行 migration。升级协调器不能用该入口检查原库，因为它会改变现场。

### Manager 与 InputMethod

Manager 和 InputMethod 不实现 migration。两端分别提供受控 validation host，使用与产品相同的 native library、固定路径规则和公开 ABI 打开候选数据库：

- Manager 验证管理查询、settings 兼容和关闭连接；
- InputMethod 验证 personalized runtime 创建、只读候选信号访问和关闭连接；
- validation 不产生选择、负反馈、导入、同步或其他业务写入；
- 任一端缺失、版本不匹配、打开失败或未关闭连接，候选不得切换。

双端验证不是用同一个 `UserDb::open` 单元测试冒充两个产品宿主。

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
```

状态目录只接受上述固定文件名。读取前重新验证 data root 与状态目录的 canonical path、device、inode、owner 和 mode；receipt 还必须是 `link count = 1`、不超过 64 KiB 的普通文件，并在打开前、打开后和读取后保持同一身份。写入使用 `create_new` 创建临时文件，完成文件 `fsync` 后再次验证 root、guard 与旧 receipt 身份，再原子替换并 `fsync` 状态目录；写回结果必须与预期 canonical bytes 完全一致。

每次持久化最多推进一个合法状态，artifact 证据只能追加，operation、版本、layout 与既有证据不能被改写。相同 bytes 可以幂等重放；新 operation 不得覆盖仍存在的 operation。发现 `receipt.json.tmp` 时当前实现返回中断写入错误并保留现场，不自动猜测应提交还是丢弃；该残留的恢复决策与故障注入仍由后续崩溃恢复切面闭合。

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

如果空间预算不足以同时保留原库、快照、迁移候选和切换恢复余量，preflight 失败，不边复制边删除旧文件腾空间。当前 crate 已固定预算计算与拒绝语义；macOS host 的真实 available-space adapter、进程静止证明和 settings 保留副本仍需后续接入。

### 隔离 migration candidate

协调层只允许从 receipt 已记录身份的固定 `source-snapshot.sqlite3` 创建固定 `migration-candidate.sqlite3.tmp`，不接受调用方自定义源或目标。复制使用已打开并复验身份的 snapshot 文件句柄与 `create_new` 私有目标；复制完成、文件持久化和 snapshot 身份复验后，才把临时 candidate 交给 `UserDb::migrate_and_validate`。该 API 可以在迁移过程中使用 WAL，但返回前必须切回 `DELETE` journal、再次执行完整性与目标 schema 校验，并确认没有 `-wal`、`-shm` 或 `-journal` 残留。

candidate 成功顺序固定为：复制 snapshot 到临时 candidate、隔离 migration/validation、文件 `fsync`、复验临时文件与 snapshot 身份、原子 rename、状态目录 `fsync`、先把 candidate identity 追加到仍为 `snapshot_ready` 的 receipt，再单独持久化 `candidate_migrated`。当前 schema 返回 `migrated = false`，schema 0 与受支持旧 schema 返回真实源/目标版本；未来 schema、损坏 snapshot、目标版本不符或身份漂移均不得生成可切换 candidate。

故障注入覆盖临时文件创建后、snapshot copy 后、migration 后、rename 后和 receipt evidence 后。临时 candidate 或无 receipt identity 的最终 candidate 会使加载失败关闭；identity 已持久化而状态仍为 `snapshot_ready` 时只允许读取既有证据，不自动猜测并推进状态。原 `userdb.sqlite3` 和 `source-snapshot.sqlite3` 在整个 migration 过程中保持不变。

### 切换

候选只有在 migration 与双端验证都通过后才能进入 `switch_prepared`。切换操作必须有明确的原对象、备份对象和候选对象身份，并通过同文件系统 rename、文件与目录持久化形成可恢复序列。

不能假设一次 rename 就覆盖完整数据库 family。最终固定路径切换前后必须确保没有遗留 WAL/SHM 使另一代主库被错误重放；具体 rename 序列和崩溃点需由隔离故障注入测试固定。

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
4. 实现 SQLite 一致快照、空间预算、身份校验与故障注入文件系统端口；
5. 从固定 snapshot 创建隔离 migration candidate，固化 standalone SQLite 与 receipt evidence；
6. 实现 Manager/InputMethod 两个候选 validation host；
7. 实现切换、启动门禁、重启恢复和回滚；
8. 接入产品 manifest、自动门禁和隔离产品构建 smoke；
9. M4-P03 选定安装载体后再编写真实安装升级 runbook。

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
