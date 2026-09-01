# Linux L6 收敛与本地资产生命周期

本文固定 M5-P05B 的收敛退出口径、当前 UTM 资产账本、保留/归档/删除建议和未来安全清理控制器边界。读者是准备收口 Linux L6 或维护本地 UTM 资产的维护者。本文不授权启动、克隆、注销、删除或清理任何 VM、快照、guest 数据、host evidence，也不把账面磁盘占用表述为实际可回收空间。

## 1. 收敛判断

P05B 不再把八个 crash checkpoint 全部执行为真实系统阻塞项。八个场景仍由 committed matrix、compile-isolated acceptance controller、checkpoint/resume 合同、fake runner 和 canonical evidence schema 完整覆盖；真实系统只保留三个具有不同恢复边界的代表场景：

1. `install_prepared`：已闭合，覆盖首次安装 prepared、完整 process group crash、合法 guard 与 exact resume。
2. `install_artifacts_staged`：已闭合，覆盖 target-only staging、fresh boot、延迟结果消歧、`completed` 与正常停止。
3. `upgrade_quiesced`：尚未执行，覆盖已有 source、运行程序静止、upgrade quiesced 与 exact resume。

`upgrade_before_dpkg`、`upgrade_after_dpkg`、`upgrade_rollback_required`、`upgrade_before_source_restore` 和 `upgrade_after_source_restore` 的真实系统执行转为后续 hardening，不再阻塞 P05B 或 M5 退出。它们不得从 matrix、测试、validator 或文档合同中删除，也不能把未执行的真实系统结果写成通过。

P05B 剩余阻塞项为：

- 在资产收敛后完成一次真实 `upgrade_quiesced`；
- 在一台新的 disposable guest、同一 release pair 和同一连续 session 中依次闭合 `install_source`、`upgrade_target`、`repair_target`、`rollback_source`、`remove_source`、`reinstall_target`；
- 同一连续 session 复验 dependency/font、startup 正负向、XDG 零写入/保留、process/package-manager lifecycle、断网与 guest reboot 对照；
- P05C 仍使用另一台独立 guest，并重新取得系统授权。

真实 VM 数量受预算约束：常驻注册锚点目标为 5 台，任一时刻最多增加 1 台 P05B disposable target，因此 P05B 注册预算上限为 6 台。注册项已由历史基线 22 台降至 11 台，但在进一步收敛至 5 台前仍不创建 `upgrade_quiesced` target；一个 disposable target 形成持久证据并退休后，才允许创建下一台。失败不会自动增加替代 clone。

## 2. 审计基线与计量口径

本账本的历史基线来自 2026-08-30 已冻结的 complete v2 清单；当前注册数再由三批冻结 delete manifest 的精确成员 delta 推进，不为更新账本追加 UTM 查询：

- inventory：`/Users/luobo/VirtualMachines/RadishLex-L6-Registration-Shell-d75818f-Upgrade-Quiesced-Move-Complete-Control-v2/utmctl-list-postupdate.json`；
- canonical inventory：22 台全部 `stopped`，hash `1074717ca5ff9f8666e53974b9002d1c4697a33de877e5cdfbdcf24299f4714f`；
- 物理资产：23 个 `.utm` bundle，其中 22 个已注册，`Debian13-ARM64-DependencyFrozen.utm` 故意未注册；
- bundle 的 `du` 合计为 220.08 GiB；其中 14 个 UTM 默认 Documents bundle 合计 130.05 GiB；
- 7 个 APFS snapshot 的 `du` 合计为 65.25 GiB；
- Data volume 当次只读观察约有 527 GiB 可用，不构成立即容量事故。

三个batch先后删除11个注册bundle后，当前为12个物理`.utm`、11个注册项和1个故意未注册的DependencyFrozen bundle；snapshot仍为7个。此处不以`df`变化反推实际回收量。

表中 GiB 是 `du -sk / 1048576` 的账面值。APFS clonefile 和 clone 共享物理块，逐项求和会重复计算共享块；删除候选的账面值只用于排序，不能承诺同等可回收容量。host evidence、handoff 和 control root 体积远小于 VM/snapshot，默认保留而不是优先清理。

以下使用两个路径缩写：

- `OP` = `/Users/luobo/VirtualMachines`
- `DOC` = `/Users/luobo/Library/Containers/com.utmapp.UTM/Data/Documents`

## 3. VM bundle 账本

“证据归档后删除候选”表示持久 host/guest manifest 继续保留，而 raw `.utm` package 只有在逐项 preflight 通过并取得另一份精确删除授权后才可删除。它不是当前删除授权。当前没有经过验证的“只注销但保留 bundle”路径；plain `utmctl delete` 会同时移除注册项与 package，不得把它描述为 archive。

| # | Bundle / UUID | GiB | 当前角色 | 建议处置 |
| --- | --- | ---: | --- | --- |
| 1 | `DOC/RadishLex-Debian13-ARM64-L6-1ebbdab.utm`<br>`9C5638D7-0F97-4BD8-8A83-ABCDFCADAC6C` | 9.00 | 第五套 rolled-back terminal | 证据归档后删除候选 |
| 2 | `DOC/RadishLex-Debian13-ARM64-L6-56dd4de.utm`<br>`A3F757B1-CE75-4F23-9509-CAD033260AA1` | 9.44 | 第四套 chain mismatch | 已删除；证据保留 |
| 3 | `DOC/RadishLex-Debian13-ARM64-L6-80e49ce-crash-install-prepared-v2.utm`<br>`FD24ADFF-B160-46F0-B100-10BAAF17C056` | 9.39 | 旧 pair crash 失败/诊断现场 | 已删除；证据保留 |
| 4 | `DOC/RadishLex-Debian13-ARM64-L6-80e49ce-reinstall.utm`<br>`E671DB9C-5E2C-447B-9425-8D91D2CFD465` | 9.06 | reinstall completed terminal | 已删除；证据保留 |
| 5 | `DOC/RadishLex-Debian13-ARM64-L6-80e49ce-remove.utm`<br>`5EA2BAA2-B9A1-46CC-B496-B37826DD27A2` | 9.47 | remove completed terminal | 已删除；证据保留 |
| 6 | `DOC/RadishLex-Debian13-ARM64-L6-80e49ce-repair.utm`<br>`A3022255-ED08-4C66-92D3-075A6BB93107` | 9.45 | repair aborted-preserved | 已删除；证据保留 |
| 7 | `DOC/RadishLex-Debian13-ARM64-L6-80e49ce-rollback-v3.utm`<br>`EFD15599-7177-4D55-BF17-173EE1F0BBDD` | 9.46 | rollback completed terminal | 已删除；证据保留 |
| 8 | `DOC/RadishLex-Debian13-ARM64-L6-80e49ce.utm`<br>`193179D5-2595-4628-A063-9EFB73F8EC05` | 9.44 | 第六套 target completed；已有 S3 | `fourth-batch-v1` prepare 已闭合；delete 未授权 |
| 9 | `DOC/RadishLex-Debian13-ARM64-L6-823afca-repair.utm`<br>`394217A7-BFC9-43C8-94E6-539FF3F2B6FB` | 9.46 | repair completed terminal | 证据归档后删除候选 |
| 10 | `DOC/RadishLex-Debian13-ARM64-L6-b891ed1-repair.utm`<br>`BE3579E0-B150-438D-ABE0-53A8D46137F8` | 9.45 | repair completed-noop | 已删除；证据保留 |
| 11 | `DOC/RadishLex-Debian13-ARM64-L6-d75818f-crash-install-artifacts-staged-v3.utm`<br>`5B19AEF1-0F29-40B6-8F24-1B1929117DAB` | 9.39 | 第二场景 host start 失败现场 | 已删除；证据保留 |
| 12 | `DOC/RadishLex-Debian13-ARM64-L6-d75818f-crash-install-artifacts-staged-v4.utm`<br>`50B75F88-493D-42C0-A1DC-054DEC478038` | 8.85 | 第二场景 completed/stopped-verified terminal | `fourth-batch-v1` prepare 已闭合；delete 未授权 |
| 13 | `DOC/RadishLex-Debian13-ARM64-L6-d75818f-crash-install-artifacts-staged.utm`<br>`B0B826F6-D7D3-433C-8987-E2D6993A87B3` | 9.39 | 第二场景旧 start 失败现场 | 已删除；证据保留 |
| 14 | `DOC/RadishLex-Debian13-ARM64-L6-d75818f-crash-install-prepared-v3.utm`<br>`3EC83EB9-094B-492B-9756-D47E61C593B9` | 8.79 | 第一场景 completed terminal | `fourth-batch-v1` prepare 已闭合；delete 未授权 |
| 15 | `OP/Debian13-ARM64-CleanBase.utm`<br>`21197987-AEBB-46E6-ABDC-B9762F5C0CE4` | 7.06 | rescue clean base | 保留；常驻注册锚点 |
| 16 | `OP/Debian13-ARM64-DependencyFrozen.utm`<br>`755199B1-1C18-4441-8A1E-D423C8DE0022` | 9.39 | immutable COW source；故意未注册 | 保留；不启动、不改写 |
| 17 | `OP/Debian13-ARM64-L6-2fa1b8c.utm`<br>`EBF12F50-33B1-4711-B693-B57D419EAE2A` | 9.40 | 第二套 pre-receipt failure | 已删除；证据保留 |
| 18 | `OP/Debian13-ARM64-L6-512e8ab.utm`<br>`99FF4B3F-4894-4913-BBE3-4934DC27BEEB` | 8.72 | 第三套 source terminal；已有 S2 | 证据归档后删除候选 |
| 19 | `OP/Debian13-ARM64-L6.utm`<br>`6F73F6DC-66DF-40EC-86B8-228C1FDA1195` | 9.42 | 首套 pre-receipt failure | 已删除；证据保留 |
| 20 | `OP/Debian13-ARM64.utm`<br>`755199B1-1C18-4441-8A1E-D423C8DE0022` | 11.65 | 通用 builder；与未注册冻结源共享历史 UUID | 保留；常驻注册锚点 |
| 21 | `OP/RadishLex-L6-PairBuilder-2fa1b8c-v2.utm`<br>`E2F5624C-B0F9-4102-B149-5F72C1851D49` | 17.14 | 隔离 release-pair builder | 保留；常驻注册锚点 |
| 22 | `OP/RadishLex-L6-Registration-Shell-d75818f-Upgrade-Quiesced-v1.utm`<br>`0BAA7355-A55A-463E-97FE-82A785E252A7` | 0.00 | `upgrade_quiesced` registration-only shell | 保留至第三场景 target 建立；随后另评估退休 |
| 23 | `OP/RadishLex/VMs/RadishLex-Debian13-ARM64.utm`<br>`E0168AA6-AFDA-4C81-9327-590DAC48C49B` | 17.24 | P04 已验收现场 | 保留；常驻注册锚点，不作 P05 mutation |

三批已删除11个raw VM，账面103.32 GiB；剩余6个删除候选账面54.28 GiB。注册列表已从22台降至11台；全部剩余候选通过后才可能降至5台，任一候选未通过preflight时必须继续保留。

第四批只选择其中3台：第六套target/S3、第一场景completed terminal和第二场景completed/stopped-verified terminal，精确账面27.09 GiB。第五套rolled-back与repair completed的既存终态证据树均为历史owner tuple `501:0`；第三套source terminal的最终EFI/qcow2又不同于更早的S2恢复点。三者继续作为候选保留，但不得修改历史证据、放宽`501:20`合同或把S2错误表述为当前bundle逐字节副本；它们须在后续第五批单独建模。

## 4. Snapshot 账本

| # | Snapshot | GiB | 当前角色 | 建议处置 |
| --- | --- | ---: | --- | --- |
| 1 | `OP/RadishLex-L6-Snapshots/S0-clean-e5b6da1` | 9.42 | 旧 pair S0 取证 | 隔离后删除候选 |
| 2 | `OP/RadishLex-L6-Snapshots/S0-clean-2fa1b8c` | 9.40 | 第二套 S0 取证 | 隔离后删除候选 |
| 3 | `OP/RadishLex-L6-Snapshots/S0-clean-512e8ab` | 9.40 | 第三套 clean baseline | 隔离后删除候选；CleanBase 可承担新建基线 |
| 4 | `OP/RadishLex-L6-Snapshots/S1-source-installed-512e8ab` | 9.41 | S2 前 source-installed 恢复点 | 隔离后删除候选；S2 是当前 upgrade 起点 |
| 5 | `OP/RadishLex-L6-Snapshots/S2-preflight-wal-drift-512e8ab` | 8.75 | WAL/SHM 误读漂移取证 | 隔离后删除候选；不得作为恢复源 |
| 6 | `OP/RadishLex-L6-Snapshots/S2-source-data-512e8ab` | 9.42 | `upgrade_quiesced` immutable source/data 起点 | 保留；第三场景与收敛验证的关键锚点 |
| 7 | `OP/RadishLex-L6-Snapshots/S3-target-installed-80e49ce` | 9.44 | 已完成 maintenance/operation 的 target anchor | 保留至 P05B 退出复核 |

5 个 snapshot 删除候选账面合计 46.39 GiB。snapshot 先做同文件系统、no-replace 的隔离改名，再在另一批授权中清除；隔离本身不回收空间。S2、S3 任一身份或持久 manifest 校验失败时，全部 snapshot 清理停止。

## 5. 保留的证据资产

下列资产默认保留，不进入空间清理优先级：

- committed matrix、controller、bindings、测试和文档；
- canonical pair/handoff、refresh handoff、package/evidence archive；
- host/guest manifest、terminal、request、binding、command/result 摘要和清理 manifest；
- operation ID hash-only、脱敏 receipt/dpkg/startup/XDG/network/process 摘要；
- registration shell complete/evidence、S2/S3 manifest 与未来收敛清理证据。

保留证据仍须满足 privacy redline：不得为补齐归档重新拉取 raw operation ID、完整 receipt、完整 dpkg log、真实用户数据或冻结现场中不存在的材料。raw VM 删除前只验证既存材料，不补写或“修复”历史 evidence。

## 6. 安全清理控制器设计

未来控制器必须拆为 repository-only 实现、只读 prepare、VM mutation、snapshot quarantine 和 snapshot purge 五个互不继承授权的阶段。设计本身不授权其中任何系统动作。

### 6.1 静态 allowlist

- committed allowlist 逐项固定 asset kind、精确 UUID、绝对路径、期望 lifecycle action、引用的 terminal/evidence manifest 和 config/EFI/qcow2 identity；
- `retain`、P04、CleanBase、DependencyFrozen、builder、registration shell、S2、S3 永远不能由默认选择进入 mutation；
- 调用方必须逐字传入 batch ID、attempt ID、allowlist digest 和目标 ID；不接受 glob、目录扫描自动扩面、名称前缀或“全部候选”；
- 单个 VM batch 最多 4 台，snapshot batch 最多 2 个；目标变化即产生新 batch、新 attempt 和新授权。
- allowlist内的asset ID、path、UUID和name全局唯一，每个asset必须且只能属于一个batch；跨batch复用或存在未分配asset均失败关闭。

### 6.2 只读 prepare

prepare 只允许读取仓库、冻结 evidence、bundle/snapshot 文件、句柄和一次 plain `utmctl list`。它必须：

1. 绑定 clean committed HEAD、allowlist digest、absent create-new 输出根和精确授权声明；
2. 确认 inventory 与授权输入完全一致、所有注册 VM 均为 `stopped`，且目标 name/UUID 唯一；
3. 以 `lstat`/`openat` no-follow 语义确认目标位于允许根、没有 symlink、owner/mode/link count 合法；
4. 逐项复验 config/EFI/qcow2 或 snapshot identity 与既存 terminal/evidence manifest；
5. 逐项复验持久 evidence manifest 的成员、hash、权限和 privacy 扫描，不要求或生成历史上不存在的证据；
6. 对目标及其关键文件执行两轮零句柄检查；
7. 确认目标没有被 S2/S3、当前 handoff、registration shell、P04、builder 或待执行 batch 引用；
8. 只写 request、binding、preflight、inventory 和 terminal manifest；任一失败闭合 `precondition-rejected`，mutation count 必须为 0。

prepare 结果只能是某一精确 batch 的授权输入，不能直接触发删除，也不能跨 HEAD、inventory 或 asset identity 复用。

2026-08-30 已实现首个repository-only控制：`packaging/linux/l6-asset-retirement-allowlist.json`只列出`first-four-v1`的4台候选并固定两个绝对storage root、UUID/name/path、config/EFI/qcow2和7个既存证据锚点；`scripts/linux-product/l6_utm_asset_retirement_prepare.py`没有mutation命令面，只接受一次plain list、两轮`lsof`和两轮完整asset hash。7项fake-runner覆盖成功、inventory/句柄/磁盘/证据漂移及授权缺失并进入默认L6门禁。

随后在clean `4a34ae3`执行唯一只读attempt `l6-asset-retirement-prepare-20260830-v1`：一次list确认22台全stopped及canonical inventory `1074717c…714f`，7个证据锚点、4个bundle的两轮完整hash和两轮零句柄均通过，闭合`prepared`。create-new根`RadishLex-L6-Asset-Retirement-Prepare-20260830-v1`的9项manifest SHA-256为`68ce05b012fa5f821e9d5c47835830123777a08c0ba05c1ea2a70bd6e0d4f44e`；delete/start/clone/move/guest计数均为0。该结果只可作为本批删除授权输入，不自动触发mutation。

2026-08-31的repository-only提交`a2c7f07`新增互不重叠的`second-batch-v1`，固定4台、37.62 GiB、7个既存证据锚点与完整bundle身份。prepare新增窄语义校验，分别绑定旧`install_prepared`失败关闭、两类UTM start失败关闭及maintenance repair completed-noop终态；allowlist加载器同时拒绝跨batch复用和未分配asset。13项prepare回归、默认L6门禁与全仓基线通过；纯文件系统离线复核也通过全部锚点及4个bundle身份。此阶段未调用UTM、未创建prepare输出根，删除控制器仍只接受`first-four-v1`，因此不构成第二批prepare或mutation授权。

项目所有者随后只授权第二批只读prepare。从clean `f99ab2d`执行唯一attempt `l6-asset-retirement-prepare-20260831-second-batch-v1`：一次plain list确认18台全stopped及canonical inventory `bc4b55dd…31eb2`，4台目标唯一在册；7个锚点、两轮12文件零句柄和两轮完整bundle身份均通过。create-new根`RadishLex-L6-Asset-Retirement-Prepare-20260831-Second-Batch-v1`含9项manifest成员，SHA-256为`d802f331ac0d338ae25dc62b193f86446cd8e0191143172b7761e4d6bed8ac60`；terminal为`prepared`，delete/start/clone/move/guest/retry均为0。该结果冻结为后续控制输入，不授权删除；当前删除控制器仍硬锁`first-four-v1`。

`1d671e9`在repository-only范围固定第三个互不重叠的`third-batch-v1`：`EFD15599…BBDD` rollback、`5EA2BAA2…27A2` remove与`E671DB9C…D465` reinstall三台completed terminal clone，账面27.99 GiB。新增`completed-operation-terminal`窄语义，逐类绑定format、UUID、maintenance完成态、receipt尾部、全停/零句柄字段及config/EFI/qcow2身份；rollback使用remove归档树中与原件byte-identical且满足owner `501:20`的既存副本。`394217A7…6682` repair completed clone原始host summary及父树为`501:0`，不满足prepare要求的operator root owner tuple，故失败关闭并移出本批，没有修改历史证据或放宽权限合同。14项prepare与10项delete回归、默认L6门禁通过；纯文件系统双轮复核通过3个锚点与3个bundle，allowlist SHA-256为`038a1522c065006d210091792985f36e721e85c6498699bc9443f34658c8e70a`。该阶段UTM查询为0，未来prepare根保持absent，delete控制器继续在创建输出前拒绝第三批，因此不构成prepare或mutation授权。

2026-09-01项目所有者独立授权第三批只读prepare。从clean `e2098bb`执行唯一attempt `l6-asset-retirement-prepare-20260901-third-batch-v1`：一次plain list确认14台全部stopped、canonical inventory `4005023b…04bac`且三台目标唯一在册；3个终态锚点、两轮9文件零句柄及两轮完整bundle身份均通过。create-new根`RadishLex-L6-Asset-Retirement-Prepare-20260901-Third-Batch-v1`为`0700`，10个文件均为`0600`/owner `501:20`/single-link且零xattr；9项manifest SHA-256为`7bcfa513debbe7e276a509509b44953abe0ecb9218aa78da41c7b533eb5f7f38`。terminal闭合`prepared`，inventory/handle/hash为`1/2/2`，delete/start/clone/move/guest/retry均为0；该授权已消费，不得复跑。

`8172c49`在repository-only范围固定互不重叠的`fourth-batch-v1`：`193179D5…EC05`第六套target/S3、`3EC83EB9…593B9`第一场景completed terminal与`50B75F88…8038`第二场景completed/stopped-verified terminal，共27.09 GiB。新增窄语义分别绑定保留S3与source terminal一致身份、正常stop后的双重qcow2和零句柄、completed transaction后的唯一graceful stop/全停交叉检查；第二场景另绑定28项terminal-stop manifest。5个既存锚点均为`0600`/owner `501:20`，三套config/EFI/qcow2经两轮纯文件系统复核一致，allowlist SHA-256为`850f31b13e64dfa4a8fb253b30dbb89212beee3e022540d455302cf07caf1a47`。15项prepare、10项delete与默认L6门禁通过；UTM查询、prepare输出和mutation均为0，delete控制器显式拒绝第四批，因此下一步仍只能另行授权只读prepare。

项目所有者随后独立授权第四批唯一只读prepare。从clean `edd58b0`执行attempt `l6-asset-retirement-prepare-20260901-fourth-batch-v1`：一次plain list确认11台全部stopped、canonical inventory `be170018…603b1f3`且三个目标唯一在册；5个终态锚点、两轮9文件零句柄及两轮完整bundle身份均通过。create-new根`RadishLex-L6-Asset-Retirement-Prepare-20260901-Fourth-Batch-v1`为`0700`，10个文件均为`0600`/owner `501:20`/single-link且零xattr；9项manifest SHA-256为`9fca287626c95482c070a7c7a90834adc2eb295d264d2e83a18287e9ccc7b388`。terminal闭合`prepared`，inventory/handle/hash为`1/2/2`，delete/start/clone/move/guest/retry均为0；授权已消费且不得复跑，delete仍须repository-only扩展控制并取得独立mutation授权。

### 6.3 VM mutation

- VM 删除前重新执行 prepare 的全部关键身份、全停和零句柄检查；
- 每台只允许一次 plain `utmctl delete <exact-uuid>`，不得使用 GUI delete、名称匹配、shell expansion、自动 retry 或并行删除；
- 每次调用后只允许一次 plain list，必须证明恰好移除该 UUID、其 package absent、其余 inventory 成员不变且全部 stopped，才可进入下一台；
- exit 0、注册项消失和 package absent 三者必须同时成立；任一矛盾为 `state-indeterminate`，停止 batch，不自动 rollback、重建、补删或继续下一台；
- terminal manifest 固定每次 argv、exit/timed-out、before/after canonical inventory、package observation 和未授权目标不变证明。

当前没有可靠的 unregister-only primitive；如果未来发现该能力，必须先做新的 repository-only 设计和独立授权，不能把 `utmctl delete` 包装成注销。

2026-08-30 项目所有者明确授权执行`first-four-v1`后，新增`l6_utm_asset_retirement_delete.py`：它递归绑定9项prepare manifest与语义、clean committed HEAD、allowlist和固定storage roots，mutation前重新执行一次全停list、两轮完整asset hash与两轮零句柄。随后每台只形成一次`utmctl delete <exact-uuid>`和一次post-list，必须同时满足clean command success、精确单成员inventory delta、其余成员不变/全stopped与package absent；任何矛盾闭合`state-indeterminate`并停止，不retry、rollback、补删或继续下一台。7项fake-runner已进入默认L6门禁；此记录尚不表示真实delete已调用。

随后在clean `93abcb0`执行唯一attempt `l6-asset-retirement-delete-20260830-v1`。preflight重新闭合7个锚点、两轮完整hash、22台全停inventory与两轮零句柄；4次delete均exit 0、无stdout/stderr/timeout，post-list严格为22→21→20→19→18且全stopped，4个精确package均absent。terminal为`deleted`，list/delete为`5/4`，无retry/rollback/start/clone/move/guest；21项manifest SHA-256为`d6369fb297a53213537947b5c168aa68c8af4d97e588f4ffaed09b62e36d0449`。

2026-08-31的repository-only提交`3e8796a`将同一逐台控制精确扩展到`second-batch-v1`：首批与第二批使用两个互斥授权位，未知batch、缺失授权或同时授权均在创建输出根前拒绝；request和terminal按实际batch记录授权与终态原因。10项fake-runner覆盖第二批成功、错授权、未知batch及两批prepare递归绑定；真实`d802f331…8ac60` prepare的9个成员也由新逻辑离线复核通过，未来delete根保持absent。此实现未调用UTM或delete，不构成真实mutation授权。

项目所有者随后明确授权`second-batch-v1`唯一真实mutation。从clean `fd6d501`执行attempt `l6-asset-retirement-delete-20260831-second-batch-v1`：preflight重新闭合7个锚点、两轮完整hash、18台全停inventory与两轮零句柄；4次精确delete均exit 0、stdout/stderr为空，post-list严格为18→17→16→15→14且其余成员全stopped，4个package均absent。terminal闭合`deleted`，list/delete为`5/4`，无retry/rollback/start/clone/move/guest；21项manifest SHA-256为`d29ae2733ee5ad0c6175797f31b4dfb7f1aeab4b8ceb3fbaa917091ceb8bd88f`。该授权已消费，不得复跑。

`22dcf30`在repository-only范围把同一逐台控制扩展到`third-batch-v1`：三个batch各有独立且互斥的授权位，第三批资产数精确固定为3；未知batch、缺失/错配/多个batch授权均在创建输出根前拒绝。10项fake-runner覆盖第三批成功terminal、错授权、未知batch及三批prepare递归绑定，默认L6门禁通过。新逻辑对真实`7bcfa513…f7f38`的9项prepare成员完成纯离线递归复核，未来delete根保持absent；未调用UTM或delete，不构成真实mutation授权。

项目所有者随后明确授权`third-batch-v1`唯一真实mutation。从clean `2e49353`执行attempt `l6-asset-retirement-delete-20260901-third-batch-v1`：preflight重新闭合3个锚点、两轮完整hash、14台全停inventory与两轮9文件零句柄；按rollback、remove、reinstall顺序执行3次精确delete，均exit 0且stdout/stderr为空，post-list严格为14→13→12→11、其余成员全stopped，三个package均absent。terminal闭合`deleted/third-batch-v1-deleted-and-verified`，list/delete为`4/3`，无retry/rollback/start/clone/move/guest。create-new根含18项manifest，SHA-256为`74e2ac3cbdced1c39a74beb7cff34163a2bcdec24fa4d0ac431d72aa2c35ebdc`；最终inventory hash为`be17001859eff1fbc4e7fa4d9e9764af662630668521d7a9e54b4f689603b1f3`。授权已消费，不得复跑。

### 6.4 Snapshot quarantine 与 purge

- quarantine 只允许将精确 snapshot 以同文件系统、no-replace rename 移入 create-new batch 目录；目标目录、父目录、manifest 和 S2/S3 身份必须调用前后复验；
- rename 后 snapshot 不得被启动、恢复或作为 clone source，控制写入原路径 absent、新路径 identity 相同和未授权 snapshot 不变证明；
- purge 是另一份授权，只接受已冻结 quarantine manifest 和精确隔离路径；不得递归作用于 `OP`、snapshot 根、batch 父目录或由环境变量/通配符推导的路径；
- purge 后只证明精确隔离成员 absent，不用 `du` 或 `df` 代替身份与删除终态。

### 6.5 失败关闭与授权边界

下列情况全部停止，不自动处理：HEAD 或 inventory 漂移、目标已启动、句柄出现、UUID/name/path/hash/owner/mode/link 漂移、evidence 不完整、S2/S3 漂移、package 部分存在、命令超时、UTM 返回与文件系统矛盾、未授权注册项变化或磁盘空间观察异常。

实际 mutation 每批必须重新向项目所有者列出：精确目标、UUID/路径、账面 GiB、持久证据 manifest、命令次数、失败后保留状态和不可恢复影响。授权不从本设计、历史清理批次或上一批 prepare 自动继承。

## 7. 推荐执行顺序

1. repository-only allowlist、prepare 和 fake-runner 回归已实现并通过门禁；实现阶段未调用 UTM、未改资产。
2. 三个VM退休batch均已分别完成只读prepare、独立授权的唯一mutation并冻结manifest。
3. `fourth-batch-v1`的唯一只读prepare已闭合并冻结；下一步只在repository-only范围扩展其delete授权面，真实mutation仍关闭。其余3个困难候选后续组成独立第五批，不修改历史证据或放宽owner/身份合同。
4. 注册项降至预算后，才创建唯一 `upgrade_quiesced` disposable target；case 闭合后先退休该 target。
5. 建立一台新的连续 L6 guest，完成六类 operation 和完整正常生命周期，然后退休。
6. snapshot 以两项一批 quarantine；确认 P05B 不再依赖后，另行 purge。
7. P05B 收口后重新评估 registration shell 和 S3；P05C 使用独立 guest 与独立授权。
