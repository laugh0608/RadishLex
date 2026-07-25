# macOS 程序安装事务

本文定义 RadishLex M4-P03 双 bundle 安装、升级、修复和默认程序移除的外层事务核心，读者是 `ime-product-install`、Installer、Manager/InputMethod startup gate 与 macOS 平台适配层维护者。本文不展开 Installer UI、真实用户目录操作、Apple 凭据、签名实现、数据 migration SQL 或历史验证流水；UI/驱动见 [macOS Installer App 边界](macos-installer-app-boundary.md)，分发载体见 [ADR 0008](adr/0008-macos-installation-carrier.md)，数据升级仍见 [macOS 数据升级协调器](macos-data-upgrade-coordinator.md)。

## 职责分层

```text
RadishLex Installer.app
  ├─ 展示 operation、目标、进度和稳定错误
  └─ 要求用户手动切换输入源、关闭 Manager
                         │
                         ▼
macOS installation adapters
  ├─ InstallAdapter 固定路径、签名/manifest 与 staging bundle
  └─ InstallCoordinatorAdapter 绑定外层/data receipt、双 guard 与恢复顺序
                         │
             ┌───────────┴───────────┐
             ▼                       ▼
ime-product-install         ime-product-upgrade
  ├─ 外层 receipt/guard       ├─ data receipt/guard
  ├─ 双程序切换/恢复           └─ snapshot/migration/rollback
  └─ startup decision
```

`ime-product-install` 不读取 bundle 内容、不计算 code signature、不停止进程、不接受自定义 bundle 名或最终路径，也不解释数据 receipt；它只接受平台已解析并验证的固定目标父目录，拥有私有事务槽位及 rename/fsync/recovery。`ime-product-upgrade` 不知道程序 staging、backup 或双 bundle 切换状态。Installer 与平台 adapter 不能自己发明状态、跳过 receipt 或根据缺失文件猜测 operation。

## 固定状态位置与权限

外层状态固定在 current-user data root：

```text
Library/Application Support/RadishLex/.radishlex-install-v1/
  receipt.json
  receipt.json.tmp
```

data root 与状态目录必须是 canonical、非 symlink、目标 uid 所有的 `0700` 目录；receipt 必须是单 link、目标 uid 所有的 `0600` 普通文件。当前格式只接受上述两个文件名。未知对象、symlink、hardlink、权限/owner/identity 漂移和中断临时 receipt 均失败关闭并保留现场。

写路径只接受已经由平台安全创建的 data root。`InstallReceiptStore::open` 可以创建固定状态子目录，但不能创建任意父目录、chmod 既有对象或清理未知现场。跨进程 guard 使用绑定 data-root device/inode/uid 的固定 Unix socket；同一 root 同时只能有一个外层事务。

## Operation kind

operation 必须显式声明，不通过目标存在性推断：

| kind | source product | target product | 数据协调 |
| --- | --- | --- | --- |
| `first_install` | 不存在 | 必须存在 | 不需要 |
| `upgrade` | 必须存在 | 必须存在且 release 更高 | 程序提交后需要 |
| `repair` | 必须存在 | 必须存在且 release 相同 | 不需要 |
| `remove_programs` | 必须存在 | 不存在 | 不需要，默认保留全部数据 |

`remove_programs` 不等价于“移除程序并删除数据”。数据删除保持独立授权、独立 receipt 与固定白名单。

## 产品与程序身份

外层 receipt 不保存绝对路径、Team ID 明文、签名输出、用户数据或 host stderr。逻辑产品身份固定包含：

- `product_id`、`product_version`、`build_number`；
- `ProductManifest.json` SHA-256；
- Manager 与 InputMethod 的固定 component、bundle ID；
- 每个 bundle 的 manifest-bound tree SHA-256；
- 每个 bundle 的 canonical code identity evidence SHA-256。

macOS adapter 负责从固定 bundle 和签名 API 形成 canonical evidence，再交给核心。核心只接受 64 位小写十六进制 SHA-256、稳定 bundle ID、正确 component 配对和同一 release。hash 不能替代发布者要求；平台每次 staging、切换、恢复和启动仍须重新验证 code signature、ProductManifest 与固定来源。

receipt 初始保存 source/target product identity。后续 artifact evidence 只能按 slot 追加：

```text
source_manager / source_input_method
staged_manager / staged_input_method
backup_manager / backup_input_method
installed_manager / installed_input_method
```

每条 evidence 同时保存逻辑 bundle identity 和程序目录根的 device/inode/owner/mode。source/backup 必须逻辑匹配 source，staged/installed 必须逻辑匹配 target；同一次移动前后的 source/backup 或 staged/installed 必须保持相同 device/inode。重复 slot、删除旧 evidence 或改写旧 identity 一律拒绝。

source evidence 必须在任何 rename 前持久化。这样中断恢复才能严格区分：

- source 仍在固定目标、backup 不存在：尚未移动；
- source 不在固定目标、相同 inode 已在 backup：rename 已完成但下一 receipt 尚未落盘；
- 两边同时存在、同时缺失或任一 identity 漂移：失败关闭，不按文件名猜测。

## 程序目标与同文件系统事务目录

跨平台 Unix 核心只接受平台已经解析的两个目标父目录，不接受 bundle 名称或调用方自定义目标。component 固定映射：

| component | 固定 bundle 名 |
| --- | --- |
| Manager | `RadishLex Manager.app` |
| InputMethod | `RadishLexInputMethod.app` |

父目录必须是 canonical、非 symlink、目标 uid 所有且不可由 group/other 写入的目录。每个 component 在自己的目标父目录建立当前 operation 独占的私有事务目录：

```text
<target-parent>/.radishlex-install-<operation-id>/
  staged.app
  source-backup.app
```

事务目录必须为 `0700`，只接受两个固定名称。staged、backup 与最终 bundle 必须是同一父文件系统上的非 symlink 目录；核心不跨设备复制或以 copy+delete 冒充原子切换。平台 adapter 负责使用保留 macOS bundle metadata 的方式把已验签 target 填入核心给出的 `staged.app`，核心随后复验目录身份并记录 staged evidence。

程序切换顺序固定为：

1. 双端静止后分别记录 source evidence；首次安装要求两个固定目标都不存在。
2. install/upgrade/repair 的两个 staged bundle 均验签并持久化 evidence 后推进 `target_staged`。
3. upgrade/repair/remove 将 source 原 inode rename 到各自 `source-backup.app`；每次 rename 后同步目标目录与原目录，再记录 backup evidence。
4. Manager 先把 staged 原 inode rename 到固定目标并记录 installed evidence，推进 `manager_committed`。
5. InputMethod 再执行同样动作，推进 `programs_committed`；remove 则以固定目标不存在作为对应 committed 证据。
6. 任一步重启都只根据 receipt evidence 与精确 inode 在 staged/backup/final 三个固定槽位中的位置续跑。

rollback 对每个已经提交的 target 先把失败 target 原 inode 移回 `staged.app`，再把 source backup 原 inode恢复到固定目标；首次安装没有 source，恢复结果是固定目标不存在；remove 没有 target staging，恢复结果是 source backup 回到固定目标。只有两个 component 都达到各自 source 结果，外层 receipt 才能推进 `programs_restored`。

当前切面不递归清理 staged/backup 或历史 operation 目录。成功、回滚和诊断材料的身份绑定清理必须使用后续独立终态动作；不能为了开始下一次 operation 删除未知目录或未复验的 bundle。

## macOS manifest-bound adapter

macOS adapter 的输入只允许：

- 从系统 user-domain API 得到的 authoritative current-user home 与 uid；
- Installer 自身已验证资源中的 `InstallPayload/` 根；
- 发布构建固定的 Manager/InputMethod Developer ID designated requirement 与 Team ID；
- 已持有的外层 receipt store、guard 和对应 component 的 `ProgramSwitchStore`。

adapter 内嵌 committed `packaging/macos/install-layout.json` 字节。payload 内 `InstallLayout.json` 必须逐字节一致，`InstallPayloadManifest.json` 必须严格绑定 layout、`ProductManifest.json`、版本/build、两个 component-to-target 映射和保留数据语义。payload/product 根、manifest、bundle 与路径链中的真实目录不得由 symlink 或 hardlink 替换；未知顶层对象、字段、component、文件记录或目标映射均失败关闭。

authoritative home 必须是 canonical、非 symlink、目标 uid 所有且不可由 group/other 写入的真实目录。adapter 只从 committed 相对路径形成：

```text
<home>/Applications
<home>/Library/Input Methods
<home>/Library/Application Support/RadishLex
```

它不读取 `HOME`、不展开 `~`、不接受自定义 bundle 名、父目录或删除目标。本切面要求两个程序目标父目录与 data root 已由受控 Installer bootstrap 安全创建；任一缺失、alias、owner/mode 或 inode 漂移都拒绝，不在身份验证过程中顺便创建或 chmod。

### Bundle tree 与 code identity

adapter 对 payload target、已安装 source、staged、installed 和 restored bundle 使用同一 bundle tree v1 算法：按 UTF-8 相对路径排序，对每个单 link 普通文件绑定 path、size、SHA-256，对每个内部相对 symlink 绑定 path 与 target，并拒绝其他对象、绝对/逃逸/broken symlink 和路径重复。目录本身不进入 tree hash；其内容和路径链安全性仍须复验。target tree 必须与 `ProductManifest.json` 的完整 component file records 一致。

code signature 验证固定为：

1. 发布要求只接受五段 `and` 连接的 Developer ID Application designated requirement：精确 bundle identifier、`anchor apple generic`、Developer ID 中间证书 OID、Developer ID Application leaf OID 和匹配 Team ID 的 leaf OU；不接受 `or`、ad-hoc 或宽泛 requirement；
2. `/usr/bin/codesign --verify --deep --strict -R=<expected designated requirement> <bundle>`；
3. 独立读取 Identifier、TeamIdentifier、CDHash、Signature、CodeDirectory 与 designated requirement；
4. 要求 bundle ID、Team ID 和 designated requirement 与该 component 的发布要求精确一致；
5. 将上述固定字段编码为 `radishlex-macos-code-identity-v1` 后只把 SHA-256 写入逻辑程序身份。

receipt 不保存 requirement、Team ID、Authority、CDHash 或 `codesign` 输出原文。验证进程的 stdout/stderr 不进入错误、日志或诊断。Developer ID 要求尚未冻结时只能使用测试注入的合成 verifier，不提供 production ad-hoc fallback，也不把当前 ad-hoc 产品装配冒充发布身份。

### Staging 填充与阶段复验

production copy 使用系统 `/usr/bin/ditto`，保留 resource fork、extended attributes、ACL、quarantine 与 HFS compression，不叠加覆盖既有 `staged.app`。复制前必须复验 guard、operation/component/固定目标、payload manifest、target tree 与 target code identity；复制完成后对 staged tree 和 code identity 重新形成同一逻辑身份，递归同步普通文件与目录，再由核心记录 staged filesystem evidence。

重启时若 `staged.app` 已存在但 receipt 尚无 staged evidence，adapter 只接受完整 tree/code identity 与 target 精确匹配的对象，完成同步后补记 evidence；部分复制、内容漂移、签名失败或未知对象保持现场并失败关闭。本切面不递归删除或覆盖无 evidence 的失败 staging；恢复/清理必须在后续以固定 operation、精确对象身份和显式动作实现。

source evidence 记录前、target commit 后和 source restore 后都必须由 adapter 重新计算 tree hash 并复验 code signature，分别匹配 receipt source、target、source logical identity。核心的 inode evidence 证明“同一个目录对象被移动”，adapter 的 tree/code evidence 证明“该对象仍是预期程序”；两者不能互相替代。

## 状态机

通用状态按外部可持久化边界推进：

```text
prepared -> quiesced

first_install:
  quiesced -> target_staged -> manager_committed -> programs_committed
    -> final_verified -> completed

upgrade:
  quiesced -> target_staged -> source_preserved
    -> manager_committed -> programs_committed
    -> data_coordinating -> data_settled
    -> final_verified -> completed

repair:
  quiesced -> target_staged -> source_preserved
    -> manager_committed -> programs_committed
    -> final_verified -> completed

remove_programs:
  quiesced -> source_preserved
    -> manager_committed -> programs_committed
    -> final_verified -> completed
```

`manager_committed` 是首个不可只靠丢弃 staging 收敛的程序变更边界。在它之前失败进入 `aborted_preserved`；从它开始失败进入 `rollback_required`，平台恢复或移除已提交程序后推进 `programs_restored -> rolled_back`。`completed`、`aborted_preserved` 和 `rolled_back` 是终态，同一 operation 不能重开。

`manager_committed` 不表示 Manager 进程运行，只表示 Manager 固定目标的变更已完成并复验；`programs_committed` 表示两个固定目标均已完成该 operation 的目标动作。remove operation 的 committed 状态表示对应目标已确认不存在。

M4-P02 数据 receipt 只能在 `upgrade` 的 `data_coordinating` 阶段运行：

- 数据 `completed` 才能推进外层 `data_settled`；
- 数据 `aborted_preserved` 或 `rolled_back` 都要求外层进入 `rollback_required` 并恢复 source 程序；
- 数据 receipt 非终态、损坏或身份漂移时，外层保持非终态并继续阻止两端启动；
- 外层 `completed` 前必须复验两个已安装 target、数据终态和双端 startup identity。

`InstallCoordinatorAdapter` 已实现上述映射，但不合并两个核心：

- 外层/data receipt 必须使用同一 operation ID、同一 data-root device/inode/uid/mode，并精确匹配 source/target version 与 build；
- 外层与数据 guard 在整个组合调用期间同时存活；每个 M4-P02 quiescence checkpoint 都附带 installed target 双 bundle 逻辑身份复验；
- data `completed` 后再次复验 target，外层才持久化 `data_settled`；
- data `aborted_preserved` / `rolled_back` 后先持久化外层 `rollback_required`，再恢复 source 双程序；只有精确 inode 与 tree/code identity 都复验通过，才允许外层 `programs_restored -> rolled_back`；
- 静止、target/restored 身份或 source validation 暂不可得时保留两个 receipt 和全部恢复材料，外层继续阻止启动并允许按已持久化状态续跑。

外层终态使用四类 operation 共用的 `InstallFinalizationPort`，不复制各 operation 的临时完成逻辑。first install、repair、remove 从 `programs_committed` 进入终态，upgrade 从 `data_settled` 进入终态；每次分别在 `final_verified` 与 `completed` 前复验当前外层 receipt/guard、双 `ProgramSwitchStore` 和 operation 对应的最终程序结果。upgrade 组合层还在两次复验中绑定 data receipt/guard、data root、source/target release 与 data `completed`。第一段落盘后第二段中断时保留 `final_verified`，重启必须重新取得全部证据再完成。

外层 receipt/guard、artifact contract、程序切换恢复、macOS manifest/code-signature adapter、数据协调映射、两段终态与双端启动入口均已实现。隔离真实产品资格进一步证明 `final_verified` 中断续跑、candidate 失败恢复 source 双程序、Manager 单端已提交后的重启续跑，以及 active/non-terminal/terminal 双端启动决策；终态材料清理仍在后续独立切面。

## Startup decision

Manager 与 InputMethod 必须在 M4-P02 数据 gate、userdb、settings、Rime runtime 和 Flutter/IMK 业务初始化之前调用 ABI v9 外层只读 gate。FFI 不接受运行 identity 字段；macOS 实现只从当前 executable 反向绑定固定用户域 Manager/InputMethod bundle，再读取 Info.plist、计算完整 bundle tree 并形成严格 Developer ID code identity。UI、settings、`HOME` 环境变量或调用方自报 bundle/release/hash 均不能成为身份输入。

只读结果：

| 现场 | decision |
| --- | --- |
| data root 不存在 | `allowed_first_launch` |
| data root 存在，状态目录或 receipt 不存在 | `allowed_no_install_state` |
| `completed` 的 install/upgrade/repair 且 running identity 匹配 target | `allowed_terminal_receipt` |
| `aborted_preserved` / `rolled_back` 且 running identity 匹配 source | `allowed_terminal_receipt` |
| active guard 或任意非终态 receipt | `blocked_install_in_progress` |
| completed remove、终态无可运行 source/target、identity 不匹配 | `failed_closed` |
| 损坏 receipt、中断写、未知对象、unsafe root/state 或 identity 漂移 | `failed_closed` |

startup gate 不创建、删除、chmod、连接 socket 或清理对象。只有三个 `allowed_*` decision 可继续；未知 result version、decision 或 error code 必须由 FFI/平台层失败关闭。

## Receipt 持久化与替换

receipt format 固定为 `radishlex-product-install-receipt-v1`，最大 64 KiB，canonical UTF-8 JSON 只允许一个末尾换行。operation ID 是 32 位小写十六进制；root identity、operation kind、source/target product 与 previous operation ID 在同一 operation 内不可改写。

持久化必须持有匹配 guard，并按以下顺序执行：

1. 复验 data root、状态目录、guard 和已知对象；
2. 编码并验证下一 receipt 与当前 receipt 的单步替换关系；
3. `create_new` 写同目录 `receipt.json.tmp`；
4. 文件 `fsync`，复验 owner/mode/link/length；
5. 再次复验当前 receipt 未变化；
6. 原子 rename 为 `receipt.json`，状态目录 `fsync`；
7. 回读 canonical bytes 与结构化 receipt。

任何不确定状态保留现场，不删除 receipt 或临时对象来绕过 gate。

## 当前实现边界与后续条件

- 独立 crate 不依赖数据升级 crate，也不进入输入热路径；
- 四种 operation 的 source/target 组合与状态顺序具有拒绝测试；
- product/bundle/hash/artifact evidence 严格解析，receipt canonical 且只允许追加证据；
- guard 能拒绝并发 operation，receipt 写入绑定正确 root 与 guard；
- 两个 component 的固定目标、私有事务目录、同设备约束和 source/staged/backup/installed inode 连续性由核心执行；
- preserve、逐端 commit 和 rollback 的每个 rename、目标目录 fsync、源目录 fsync 边界均可注入故障并从精确 inode 现场重试；
- startup gate 对缺失、非终态、终态身份匹配/漂移、remove、损坏、未知对象和中断写均有稳定结果；
- 普通测试只使用合成 `0700` 临时目录；隔离产品资格只在带固定 marker 的系统临时根内使用真实构建 bundle、ad-hoc qualification identity 与合成 Application Support，不访问真实用户目录、程序目标、系统设置、Keychain 或发布签名凭据。
- macOS adapter、真实 bundle 内容/签名复验、M4-P02 状态映射、两段终态、双端 startup 接线与隔离端到端恢复资格已落地；Installer 只读状态投影、显式授权 contract、restartable executor、upgrade data receipt bootstrap、版本化原生 bridge 与独立 AppKit 壳已落地。executor 先持久化 `prepared` 等待重新确认，再从 guard 内 receipt 续跑四类 operation。生产 bridge 已验证 Installer/双 component Developer ID 身份并接入 first install、repair、默认程序移除与恢复的真实 user-domain mutation port；缺失历史 source assembly 的 upgrade 在任何 receipt/program mutation 前返回 `driver_unavailable`。Developer ID 成功产物、正向安装、进程/输入源交互、跨发布 source payload 与身份绑定终态清理仍属于后续切面。
