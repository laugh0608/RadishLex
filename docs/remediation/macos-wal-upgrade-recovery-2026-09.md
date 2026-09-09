# macOS WAL 升级阻塞与恢复方案

本文面向 Installer、数据升级与平台适配维护者，记录 2026-09-09 冻结现场之后的仓库诊断、已批准恢复入口实现、隔离验证及后续 WAL 方案。本文不授权真实恢复，也不提供绕过产品事务的手动 SQL、文件搬移或 receipt 修改命令。当前停止线见 [current](../status/current.md)，真实现场证据见[联合验收入口](../runbooks/macos-rev01-rev02-acceptance.md#build-3940-实机升级暂停2026-09-09)。

## 当前结论

错误呈现与五项 WAL 特征场景完成后，项目所有者另行批准实现显式恢复入口并准备独立恢复 Installer。恢复入口现已实现，Executor 八项恢复父测试、原生 Installer 门禁及真实 source 39/target 40 副本资格通过；长期 WAL 处理仍未实现。冻结 operation `aad9cf8ac2dae71b2b659e96a91ea706` 仍未恢复，本批没有打开真实 SQLite connection、运行真实 helper、点击 Installer 或改变原载体。

现已为切换前事务增加显式中止入口，通过既有状态机保留原数据库并恢复 source 39 双程序；真实恢复待独立载体准备完成及另行授权。之后单独实现面向新 operation 的 WAL 源库准备合同。恢复到 39 仅返回已知基线，其 IMK 上下文缺陷仍在，不能据此关闭 REV-01/REV-02 或完成 build 40 输入验收。

## 已证实的两个缺口

### 执行错误被持久化进度遮蔽

原 `InstallerBridge::production_perform` 在 dispatch 返回错误后，只要新快照有非零 receipt state，就返回该快照；因此 `data_coordinating + error=none` 可以掩盖当前操作失败。

修复后错误分支保留 fresh receipt 的 operation、state 和 progress，但强制 `blocked + refresh`、清除其他 action/manual prompt；已有稳定错误保留，没有稳定错误时使用既有 `unknown_driver_result`。固定日志只输出授权/执行错误类别，不含路径、ID 或底层正文。UI 只启用实际提供的 action，blocked 时只允许 refresh，并说明进度只是已保存阶段。成功刷新仍按重新读取的持久化状态投影；错误不写入 receipt，刷新/重开不代表原失败已经消除。

错误呈现修复本身未扩展 ABI、稳定错误枚举或 receipt 格式。`unknown_driver_result` 明确报告失败，但尚不能在 UI 展示完整的内层 `InterruptedSwitch` 分类；冻结的旧 Installer 不会自动获得修复，也不通过再次点击来补采生产错误。

### WAL 模式与 standalone 切换合同不匹配

当前 snapshot 可以通过 SQLite backup 读取已提交 WAL 内容，而 `prepare_switch` 要求 active/candidate/backup 均没有 WAL、SHM 或 journal。合法 WAL 因此足以让状态停在 `candidate_verified`，返回 `InterruptedSwitch`，不产生 source backup 或执行原库切换。

新增隔离测试使用独立子进程写入真实 SQLite WAL，再绕过该子进程的连接析构退出：主文件字节未变，已提交学习与删除在 WAL 中。完整 Rust 协调链复现上述错误；原库、snapshot、candidate 均可读到两条选择、一条活跃词和一条 tombstone，原库 inode、主文件和 WAL 字节保留。

正常关闭对照同样失败：关闭时 WAL 消失，但持久 WAL 模式的源库经过后续只读 snapshot，又出现零长度 WAL 与 SHM，仍被切换前门禁拒绝。因此正常关闭或只做 checkpoint 不能单独证明 standalone 资格。SQLite 官方说明 WAL 模式跨连接保留，只读打开也可能需要创建 sidecar；WAL 是数据库持久状态的一部分，不能把它与主文件任意分离或删除。[SQLite WAL 文档](https://sqlite.org/wal.html)

这些结果证明当前协调链存在可重复的合同缺口；真实暂停调用没有暴露原始枚举，仍不能声称已确定其唯一失败点或实际进程退出的全部因果链。

## 冻结现场恢复方案：切换前中止并恢复源程序

### 生产入口与范围

已在现有 InstallerDriver/Executor 与 macOS composition 层增加显式“中止本次升级并恢复源程序”动作。ABI v1 的 POD、symbol 与 contract version 保持，action 9 为可加整数映射；旧 reader 对未知值失败关闭。复用 authoritative home、sealed payload/source identity、外层/数据 guard、真实平台 preflight 和程序恢复 adapter；UI 不接受自报路径、operation、source/target 或 receipt 内容。新增 action 的整数映射和未知值失败关闭已覆盖 Rust/Objective-C 双方，完整合同见 [Installer 边界](../macos-installer-app-boundary.md#显式切换前中止恢复)。

只支持外层 upgrade `data_coordinating` 且数据 `candidate_verified`、未开始源库切换的状态；动作必须明确确认。已有已中止或恢复中的同一 operation 只走恢复重放，不再次记录中止。对 `switch_prepared` 或更晚的数据状态、其他 operation、未知现场不提供本动作。入口已实现于新代码，冻结的旧 Installer 没有该动作，其“继续”不能替代它。

### 变更前必须证明

1. 从 sealed 载荷取得唯一 source 39 与 target 40 的完整身份，重新验证当前 Installer、固定双目标程序及各自私有 source backup 的 tree、manifest、strict signature 和 receipt relationship；目录中的未知对象失败关闭。
2. 依固定锁顺序持有外层 install guard、内层 upgrade guard，重新加载并 `verify_current` 两个 receipt，绑定相同 operation、source/target、data root、owner/mode、原库 inode 与已记录 artifact。冻结证据只作比较输入，不能替代 fresh 系统观察。
3. 证明原库仍在 active 路径，source database backup 不存在，snapshot/candidate/settings backup 保持受控关系；与已有冻结记录核对 metadata，并在 guard/静止证明下建立本次主文件与 WAL 摘要基线，恢复前后比较，拒绝新 sidecar、link、owner、mode 或内容漂移。此前没有记录的 hash 不补写为旧证据。只读文件取证不打开真实 SQLite connection，不迁移、checkpoint 或修复数据库。
4. 重新取得中立输入源、Manager/InputMethod 停止及受支持平台检查点的静止证据；无法证明或发现 busy 时保持原状态。人工勾选、旧 PID 或一次静止观察不能替代恢复阶段重验。

以上前置已进入受控生产验证路径。首次中止前独占落盘固定目录的不可改写保留证据，后续恢复检查点重复验证；数据证据覆盖固定 18 个文件槽与可选 Rime 根，原文件只作只读 hash，不打开 SQLite。详细格式与兼容见[数据升级边界](../macos-data-upgrade-coordinator.md#显式切换前中止的保留证据)。

### 合法状态推进与中断恢复

1. 在 guard 内确认仍是切换前的同一份 receipt 后，调用既有 `UpgradeReceipt::abort_preserved(UpgradeFailureCode::SwitchFailed, false)`，通过 store 持久化 `aborted_preserved`。这是既有合法状态转换，不改写 source identity、旧证据或原库内容。
2. 专用 `abort_pre_switch_install_upgrade` 复用原协调器内部 `settle_failed_data`：内层中止终态进入共享恢复路径，重新验证恢复静止条件；外层落到 `rollback_required`，调用两个 `restore_program_source` 恢复原 source 程序。
3. 完整验证已恢复 source 程序，经过 `programs_restored` 后复验并持久化外层 `rolled_back`；终态前复验数据 receipt 与恢复程序关系。snapshot/candidate/settings backup 和事务证据继续保留，数据层保持 `aborted_preserved`，不切换或清理原 DB/WAL/SHM。
4. 中止落盘后、两个程序恢复之间、恢复完成但终态前的中断，必须重新加载同一 operation、重新授权和验身份后重放。未知/漂移或验证不可得时停止，不靠手工复制 app、回写 JSON 或补造 completed 解围。

预期后置条件是源 39 双程序完整身份及原 inode 恢复、原库及 WAL 字节/inode 不变、外层 `rolled_back`/内层 `aborted_preserved` 相互一致，两个 startup gate 与真实程序资格均允许。真实恢复完成还需独立授权启动；恢复本身不包含输入复验、retry 到 40、清理证据或发布。

### 隔离证明与实机边界

| 场景 | 本批结果 | 证明边界 |
| --- | --- | --- |
| 已提交合法 WAL、非正常连接关闭 | 复现 `candidate_verified` / `InterruptedSwitch`，主文件与 WAL 不变 | 真实 SQLite，合成程序/平台 port |
| WAL 正常关闭 | 只读 snapshot 后重建零长 WAL/SHM 并阻断 | 否定“正常退出即可修复” |
| guard 内显式切换前中止 | 既有协调器恢复 source 程序，数据 gate 允许，学习/tombstone 保留 | 测试直接调用既有状态机，没有生产 UI 中止入口 |
| 恢复静止检查失败、程序恢复验证失败、重载后再恢复 | 分别停止于外层协调/rollback，证明资格后终态 | 合成检查点，不是实际 macOS 进程 busy 或 crash |
| active DB 被同字节新 inode 替换 | `verify_current` 拒绝，receipt 字节不变 | 身份漂移负向合同 |

后续已批准实施批新增八项 Executor 父测试：成功保留已提交 WAL/学习/tombstone、四个程序恢复检查点中断重放、外层/内层 guard 与 preflight 拒绝、文件内容/inode/link/mode/sidecar 漂移、部分证据不重建、完整证据丢失禁止普通 resume、跨 root/目标版本授权拒绝、只读投影零创建。真实 payload 资格还覆盖 source backup 未知对象拒绝、中止后 Manager 已恢复而 InputMethod 未恢复的中断、重载证据后完成、source/target 双端 startup 正反对照。资格使用真实 39/40 payload 的新副本、实际程序 tree/strict signature adapter、临时合成用户数据；不证明冻结用户事务或真实 GUI 已恢复。

独立载体脚本 `scripts/macos-product/recovery_payload.py` 的 `qualify` 只在新临时根复制 source/target，`build` 拒绝已存在或与输入重叠的输出。构建只更新开发 Installer，再复制到独立输出、生成 ReleaseIdentity 并签名；封装前后比较内嵌 payload 内容与原输入，原 payload 完整身份/内容不变性另记证据。此脚本不启动 GUI 或安装程序。完整仓库门禁和封装结果完成后，才请求绑定冻结 operation 的真实恢复授权。

## 后续新升级的 WAL 合同

建议保留 runtime 的 WAL 使用策略，在新升级的源库身份最终绑定及程序切换之前，由 SQLite 自身完成 standalone 源库准备。该变化涉及数据写入、源 artifact 身份、持久化阶段和中断恢复，须作为独立设计/实现范围批准，不把当前 v1 已绑定 source 原位转换后强行 resume。

- 使用只打开既有文件、禁止 create/自动 migration 的专用维护连接；在 guard 和静止证明下核对当前 schema、完整性、文件身份及一致快照，不调用会启用 WAL/迁移的普通 `UserDb::open`。
- 用 SQLite checkpoint 接口确认已提交帧被完整处理，再由 SQLite 切换并验证 `journal_mode=DELETE`，显式检查关闭结果；保留严格无 sidecar 的最终条件。`SQLITE_BUSY`、部分 checkpoint 或关闭失败不能冒充完成。[SQLite checkpoint API](https://sqlite.org/c3ref/wal_checkpoint_v2.html)
- 仅 `wal_checkpoint(TRUNCATE)` 不改变持久 journal mode，后续连接仍可能重建 sidecar。正常源库关闭只能作为静止流程的一部分，不能代替最终资格检查。
- 物理准备会改变数据库 header、长度或字节；必须设计带版本和明确持久化阶段的初始源身份、准备意图、准备后身份及受保护快照关系。中断时依据 durable evidence 与实际文件恢复判断，不把旧 receipt 的 source 字段替换为“现在看到的文件”。未知新格式由旧 reader 失败关闭。
- SQLite backup 提供一致快照，但不能单独证明当前源路径已安全切换或已无 WAL。[SQLite backup 文档](https://sqlite.org/backup.html)
- 合同须覆盖 WAL 中未 checkpoint 的学习/删除、正常关闭再打开、读写连接阻塞、checkpoint 部分失败、模式转换前后 crash、身份漂移、候选失败、切换及恢复；每个成功分支重开核验学习和 tombstone，失败分支保留可证明的原数据与证据。

本批未决定新 receipt 的最终序列化设计，也未实现这条新升级路径。当前源库恢复方案不依赖它，二者不共享已经消费的实机授权。

## 前一诊断批验证与证据

- `./scripts/check-macos-install-coordinator.sh` 通过：17 项测试，1 项仅供父测试调用的 ignored 子进程入口；五项新增父测试均通过，Clippy 通过。
- `./scripts/check-macos-installer.sh` 通过：bridge 7 项测试及 driver/executor、Clippy、原生 presentation/build 通过。构建的是开发 Installer，没有覆盖冻结 release 载体或启动 GUI。
- `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 最终为 `Repository baseline passed`。沙盒 Unix guard IO 限制先被记录，匹配验证获准在沙盒外执行；没有下载或安装依赖。
- 日志：`/private/tmp/radishlex-wal-residue-full-coordinator-20260909.log`、`/private/tmp/radishlex-installer-error-gate-escalated-20260909.log`、`/private/tmp/radishlex-installer-wal-review-check-repo-20260909.log`。初始失败和正常关闭对照的新发现日志均保留。

测试源码见 [WAL 协调场景](../../platforms/macos-product/InstallCoordinatorAdapter/src/wal_residue_tests.rs)和 [bridge 合同](../../platforms/macos-product/InstallerBridge/src/tests.rs)。以上不构成真实恢复、正常 39→40 升级完成或新的输入/隐私验收。

## 恢复入口实施批验证与证据

- Executor 恢复测试：8 passed，1 ignored 子进程由父测试执行；日志 `/private/tmp/radishlex-recovery-executor-final-escalated-20260909.log`。实际 guard 冲突、缺失证据、授权与保留检查均失败关闭。
- `check-macos-installer.sh`：driver/executor/bridge、Clippy、原生 action 双向映射与 presentation/build 通过；日志 `/private/tmp/radishlex-recovery-installer-final-escalated-20260909.log`。
- source 39/target 40 的完整实际程序资格套件通过（单一集成入口内含既有恢复及新增 WAL 中止恢复），耗时 487.47 秒。日志 `/private/tmp/radishlex-recovery-payload-qualification-escalated-20260909.log`；证据根 `/private/var/folders/dp/rzjx58m54kng6tn5k5gjw71h0000gn/T/radishlex-recovery-payload-qualification-wg6qpm3r/`，`input-postflight.json` 确认原输入不变，`qualification-result.json` 记录范围。
- 沙盒 Unix guard 限制的初始失败日志保留，必要验证提权不扩大到真实用户目录或 GUI。没有依赖或 lockfile 变化。
- 完整 `CARGO_NET_OFFLINE=true ./scripts/check-repo.sh` 通过，最终 `Repository baseline passed`；日志 `/private/tmp/radishlex-pre-switch-recovery-check-repo-20260909.log`。qualification feature 的 Clippy 另行通过，日志 `/private/tmp/radishlex-recovery-qualified-clippy-20260909.log`。文档、文本、差异与恢复封装脚本路径保留负向检查通过。
