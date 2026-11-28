# Linux P05C 普通用户启动检查修复设计

维护端核验私有恢复材料，普通桌面用户凭绑定终态的记录核验当前安装。Debian ARM64 容器已覆盖 Manager 和 Fcitx 的 Rust 启动判定。含修复的 `40-4` 已通过包对准入，普通账号的实际桌面结果待验。[实现与定向验证](../devlogs/2026-W48.md#2026-11-28-普通用户启动检查仓库实现) · [当前阻塞](../status/current.md#下一阶段linux-m5-交付收口)

## 阻塞发生在哪里

11 月 27 日，普通用户已在聚焦的 Qt 字段切到 RadishLex，Fcitx5 在创建引擎前报告 `FailedClosed / PermissionDenied`。此前 B 批从 root 调用双端 FFI 得到 `AllowedProduct`。这两次观察的调用身份不同。Manager GUI 尚无本轮普通用户结果，日志也未记录首次拒绝访问的路径。[P05C 实机记录](../devlogs/2026-W48.md#2026-11-27-qt-普通字段无候选)

| 启动时的读取 | 冻结的 `40-3` 行为 | 修复后职责 |
| --- | --- | --- |
| 安装状态 | 状态根及 `operations/` 为 `root:root 0755`，规范 receipt 为 `root:root 0644`。读取 receipt 时会进一步枚举当前及历史 `0700` 操作目录的内部成员 | 双端继续核验状态根、guard、临时文件、receipt、可见操作目录的名称集合和目录自身身份。目录内部成员由维护端检查。[只读状态检查](../../platforms/linux-product/src/startup/read_only_state.rs) |
| 包与依赖 | 启动时会重新打开 `0600` 的已暂存包和 evidence，验证实际包，再用展开后的完整 `Depends` 校验 dpkg。安装 manifest 的 `hard_dependencies` 含构建替换项，不能还原全部约束 | 维护端从实际包取得依赖清单。桌面端用该清单独立校验当时的 dpkg 状态。[包关系检查](../../platforms/linux-product/src/startup/system_port.rs) · [manifest 与 control 的关系](../../platforms/linux-product/src/debian/relationship/manifest.rs) |
| 当前运行组件 | 双端共用 startup FFI，分别验证 Manager bundle 或 Fcitx addon、RimeData 和 metadata，并交叉校验两份 FFI | 继续按实际组件路径、symbol origin、manifest 摘要和组件清单检查，在 Flutter 或输入引擎业务初始化前作决定。[组件校验](../../platforms/linux-product/src/startup/system_port/manifest.rs) · [startup FFI](../../crates/ime-ffi/src/linux_product_startup.rs) |

## 启动核验记录的权限边界

由维护执行器在终态校验后写一份普通用户只读的**启动核验记录**。安装事务仍以 v1 receipt 为准。每个使用新 gate 的进程都执行同一只读检查，包括从 root 发起的诊断调用。用户输入、XDG、网络和后台特权服务均不参与这个决定。

固定文件为 `/var/lib/radishlex/startup-v1.json`。将它放在 `install-v1/` 旁边，保留旧 reader 对状态根成员的严格白名单。复用受信的 `root:root 0755` 父目录。记录使用规范 UTF-8 JSON、固定字段顺序、单行和末尾换行。上限为 128 KiB，足以容纳现行 64 KiB control 中展开的依赖及 JSON 开销。读写两端校验编码、类型、长度和字段集合，拒绝未知格式、重复字段或重复依赖。

| 字段 | 来源与作用 |
| --- | --- |
| `format` | 固定为 `radishlex-linux-startup-v1`，只接受本格式 |
| `receipt_sha256` | 当前终态 receipt 最终路径规范字节的 SHA-256，绑定状态根身份、operation chain、终态、选中的 artifact、包/evidence/manifest 摘要。下一次同版本 repair 也会产生不同 receipt |
| `depends` | 选中 artifact 经 `VerifiedArtifactRelationship::verify_package` 验证后，按 Debian control 顺序输出每项 `DirectDependency::to_control_text()`，保留架构、比较符和版本。以实际包为独立来源 |

记录必须是固定路径的 `root:root 0644` 单链接常规文件。通过安全父目录、无链接的固定叶名及打开前后身份比对读取。每个操作目录继续使用 `0700`，暂存包和 evidence 继续使用 `0600`。维护端保留当前所有 slot、私有文件 owner/mode/link/device/inode/size/hash 与实际包关系校验。历史操作只校验 v1 实际记录的结构和文件元数据。[维护存储](../../platforms/linux-product/src/store.rs) · [现有包和依赖类型](../../platforms/linux-product/src/debian/relationship/mod.rs)

每次启动实时验证依赖、当前组件及其安装证据。若只有私有暂存字节在终态后变化，检测时点是下次特权复核。维护端必须在再次出具记录、mutation、retry 或恢复前拒绝漂移。给桌面用户开放暂存会扩大恢复材料的读权限。把依赖清单放进新版 receipt 可以减少一个文件，却要求旧 reader 和恢复工具识别新的事务格式。v1 receipt 与核验记录按以下顺序持久化。

## 维护端何时撤销和出具记录

1. **开始新操作。** 在现有独占 guard 下验证原 receipt 和既有记录的路径、身份及绑定。若有合法的旧记录，先撤下它并同步父目录，再复核私有材料、写入新 `prepared` 或进入任何包变更。记录缺失的旧安装按下文的源资格处理。未知文件、绑定错误或不明临时文件保留现场并拒绝操作。即使新操作尚未写入 receipt 就中断，旧记录也不能继续放行。guard 存在期间双端始终阻断。
2. **完成事务。** 保留现有 receipt 状态机及 `fsync → rename → 父目录 fsync` 顺序。在 guard 内回读已持久化的终态，按 `installed_artifact()` 取目标或保留/恢复的源。逐项复核当前操作的暂存关系、历史目录结构、实际包/evidence、当前 dpkg 及其依赖、完整已安装产品。repair 的 source/target 身份相等时可使用本操作已记录且完全匹配的 target slot。只有原身份、实物与当前系统全都一致，才可出具记录。这也要求独立核验 `aborted_preserved` 的源。有程序但证明不足时保留事务原终态，维护调用返回启动资格未就绪。
3. **持久化。** 在同一父目录独占创建 `startup-v1.json.tmp`，以私有模式完整写入，同步内容，设为 `0644` 并再次同步。校验身份后原子 rename，随后同步父目录，并从最终路径回读内容与 receipt。完成这一步才释放 guard 并报告产品可启动。已移除或无源可恢复的首次安装终态保留“无记录”。出现意外记录或临时对象时停止并保留现场。
4. **中断和重入。** `start`、`resume` 及提前结束的终态分支共用上述收尾。receipt 已终态而记录缺失时，桌面端拒绝启动。另行授权的维护端持 guard，以精确 operation 和制品复核私有材料与实际系统。正确记录可幂等确认，缺失时只补同一终态记录。若发现私有材料漂移，须在 guard 下撤销仍与本终态绑定的合法记录并同步父目录，保留私有现场并报错。恢复仅在临时文件身份安全且字节与重新推导结果完全一致时续完发布。部分字节、来源不明或绑定错误保留待处置。终态收尾不再执行 dpkg 或追加 operation。维护入口对新事务、恢复及已终态的重入执行相同的终态复核。[维护入口](../../platforms/linux-product/src/system/host.rs) · [事务终态分支](../../platforms/linux-product/src/coordinator.rs)

## 普通用户如何判定

进入 Manager runner 或 Fcitx factory 时，先查 guard、两种临时文件、状态根和 receipt，再检查记录与实际安装。一次读取有界的 dpkg status 快照，用既有 Debian 解析、版本比较和依赖选择规则验证包名、版本、架构、Installed 状态、完整 Depends 及每个依赖（含字体）。复用组件校验及两份 FFI 等价检查。准许之前再次检查 guard、临时文件、状态根、receipt、记录及 dpkg 快照的身份与内容一致性。中途切换状态时返回拒绝，下一次启动重新读取。维护端仍须在每次实际变更前检查已映射的组件及进程静止条件。[Debian 状态校验](../../platforms/linux-product/src/debian/relationship/status.rs) · [进程静止检查](../../platforms/linux-product/src/system/process.rs)

下表按阻断优先。读到权限错误、I/O 错误或无法确认对象身份时，均失败关闭。沿用 startup request/result v1 和公开枚举。只有可信 receipt 已解码并绑定状态根时才报告其状态。产品拒绝发生在接触 settings、userdb、隐私设置或 Rime 之前。

| 实际状态 | 两个入口的决定 |
| --- | --- |
| 有合法 guard、任一合法临时文件或 receipt 非终态 | `MaintenanceRequired`。即便旧记录完整，也不能覆盖维护中或中断状态 |
| receipt 非法、根身份漂移、可见目录集合或 owner/mode/link 异常、临时对象非法 | `FailedClosed`，保留可定位错误类别 |
| 已有程序且为合法终态，缺启动核验记录（含旧 v1 安装） | `MaintenanceRequired / UnmanagedPackageState`，等待特权端核验。无 receipt 的产品沿用现行缺失或未受管判定 |
| 记录格式或绑定错误、旧记录放回、当前 manifest 或组件漂移 | `FailedClosed`，按包或组件身份错误报告。同版本、同 Depends 也不能复用其他 operation 的记录 |
| dpkg 半配置、产品版本与 receipt 不同，或依赖缺失、架构/版本不符 | 非完整或未受管的包态要求维护。依赖检查失败时按现有依赖错误关闭，两个入口均不得初始化 |
| `completed` 且有目标，或 `aborted_preserved` / `rolled_back` 且有可核验的源 | 只有记录、实际安装、依赖和当前组件全部匹配才 `AllowedProduct / InstalledReceiptVerified` |
| `completed remove` 且 dpkg 为 NotInstalled/ConfigFiles，或首次安装失败且没有源 | 无记录时分别返回 `RemovedProgram`、`ProductNotInstalled`。若残留记录或安装状态矛盾则拒绝并报告异常 |
| development build | 只有系统安装状态、guard、核验记录与临时文件均缺失且 dpkg 为 NotInstalled 时才 `AllowedDevelopment`。该分支仅适用于开发身份 |

## 旧安装与候选过渡

冻结的 `40-2 → 40-3`、原执行器和既有证据保留原身份。新记录无法改变旧二进制中的读目录行为，单独刷新维护执行器也不能修复 Fcitx5 或 Manager。

新包对以冻结的 `40-3` 原包和 evidence 作源。通用 Debian 的 `completed / upgrade / chain=2` 留作 P05C 升级起点，L6 留存的重装为 `completed / install / chain=6`。source-first 要求独立的 `not_installed → completed / install / chain=1`。首轮 seed 保留 9/12 的审阅结论。第二轮独立取证的 12 类材料全部通过审阅，所有者确认后由 R `1ac7e84` 固定。T `1773cae` 已用这些原件通过准入。[首轮审阅](../devlogs/2026-W48.md#2026-11-28-40-3-首装-seed-独立审阅) · [R 基线](../../packaging/linux/source-first-40-4-baseline.json) · [T 准入记录](../devlogs/2026-W48.md#2026-11-28-40-4-目标提交与包对准入)

新包对准入通过后分别审批制品装配与实机升级。新维护端取得 guard，以旧 receipt、实际旧 source 包/evidence、私有 staging 和安装实物重新验证起点，沿正常升级追加新 operation。若从原 P05C 的 chain=2 接续，预期为同链追加至 `completed / upgrade / chain=3`。修复版本的双端与目标终态逐件取证后才进入 P05C。旧 receipt 继续使用原格式，桌面进程只读，放行依据始终来自受控系统安装。

升级中断按同一 operation 恢复。若回到旧 source，记录实际 rollback/abort 结果，普通用户仍受旧 gate 限制。冻结包对的连续 L6、八个自动合同及代表 crash 各有既定适用域。[固定包对合同](linux-m5-source-first-contract.md) · [P05C 验收方案](../runbooks/linux-p05c-acceptance.md)

| 已有证据 | 在新候选中的适用范围 |
| --- | --- |
| 冻结包对的连续六步、冷启动、依赖、字体与 XDG | 保留为旧版本对照。新包对重新核验实际制品、依赖、字体、双 FFI、完整维护链与 XDG，并观察每个适用终态的启动记录出具、撤销或缺失 |
| 八个自动 crash 合同、两个历史 install 中断及旧 `upgrade_quiesced` 恢复 | 旧结果可说明对应 checkpoint、暂存与原事务恢复行为。新代码改变了 prepare 前及 resume 后的处理，须在新候选验证八个自动合同，并为三个代表边界取得修复版本适用的恢复和终态证据。回到旧源时须记录旧 gate 的实际行为 |
| P05C 旧 Qt 故障与本轮仓库非特权回归 | 用于复现条件和预期。新目标须以普通账号实际加载 Fcitx，观察 Qt 的预编辑、中文候选与 Esc 取消，再独立启动 Manager。其余 C/D 批仍按原矩阵验收 |

## 实施与回归的验收条件

下表定义整体验收范围。本轮已覆盖普通用户权限故障、终态补发与撤销、桌面读取中出现维护或文件替换的定向用例。实际通过项见[本周记录](../devlogs/2026-W48.md#2026-11-28-普通用户启动检查仓库实现)。逐操作中断与连续恢复、候选原生组件加载及真实桌面结果须分别取证。

| 层次 | 必须取得的证据 |
| --- | --- |
| 普通用户双端正反例 | 在隔离的 Linux 文件系统按产品固定路径布置 root 拥有的合成安装。子进程使用非零有效 UID/GID，清空特权组和可绕过读权限的 capabilities。先证明它能读 receipt/记录、无法枚举 `0700` 目录或读取 `0600` 文件，再分别走 Manager 和 Fcitx 的生产启动检查。覆盖成功终态、无记录、旧记录、活动 guard、非法 owner/mode/link、漂移 manifest/组件/FFI、依赖缺失或版本不符。核对 gate 自身对系统状态和合成 XDG 零写入，所有拒绝均先于业务初始化。缺少这种执行身份时标记未执行 |
| 维护与崩溃 | 覆盖 install、upgrade、repair、remove/reinstall、显式 rollback、保留源与恢复源。逐一在撤销旧记录、写 prepared、提交终态、写临时文件、rename、释放 guard 之间中断。核对预期 receipt、记录、guard、dpkg 调用次数及 XDG。相同终态补发两次不得再次变更包。同版本 repair 放回旧记录、临时对象异常均须拒绝。仅改变私有暂存字节时，验证维护端能检出、撤销记录，并使下次桌面启动拒绝 |
| 读写并发与开发隔离 | 用可控屏障让 guard 或新 receipt 在桌面读取中途出现，亦覆盖一次完整变更穿过读取窗口。再测试桌面已通过而维护端申请静止许可的顺序。分别证明旧决定不能放行新安装、已映射的运行组件能阻止 mutation。覆盖仅残留记录的 development build 和缺 receipt 的 product build |
| 真实 P05C | 独立授权后，先在离线 Wayland 的 Qt 普通字段确认焦点及当前输入法，人工输入合成 `nihao`，同时观察预编辑与中文候选，按 Esc 后预编辑和候选消失、字段为空。以同一普通用户独立启动 Manager 并确认进入产品模式。两条通过后继续 C 批多应用、多 session、隐私/普通恢复、学习和删除/tombstone、Manager 同库，再单独执行 D 批冷启动持久化 |

使用已有 Debian ARM64 镜像和对应 Linux libtest 可执行文件，运行 `./scripts/check-linux-startup-unprivileged.sh LOCAL_IMAGE LINUX_TEST_EXECUTABLE` 可重放本轮权限回归。脚本断网、禁止拉取镜像，仅在独立 tmpfs 布置合成系统安装，子进程以无附加组和有效 capabilities 的 UID/GID 65534 分别核验两个组件。正式 [startup gate 检查](../../scripts/check-linux-startup-gate.sh)、[事务检查](../../scripts/check-linux-package-transaction.sh)及新候选的连续维护与 crash 验证按适用范围另行授权。
