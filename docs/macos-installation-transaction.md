# macOS 程序安装事务

本文定义 RadishLex M4-P03 双 bundle 安装、升级、修复和默认程序移除的外层事务核心，读者是 `ime-product-install`、Installer、Manager/InputMethod startup gate 与 macOS 平台适配层维护者。本文不包含 Installer UI、真实用户目录操作、Apple 凭据、签名实现、数据 migration SQL 或历史验证流水；分发载体见 [ADR 0008](adr/0008-macos-installation-carrier.md)，数据升级仍见 [macOS 数据升级协调器](macos-data-upgrade-coordinator.md)。

## 职责分层

```text
RadishLex Installer.app
  ├─ 展示 operation、目标、进度和稳定错误
  └─ 要求用户手动切换输入源、关闭 Manager
                         │
                         ▼
macOS installation adapter
  ├─ 固定 user-domain 路径、签名/manifest 与进程证据
  ├─ 两个目标各自文件系统上的 staging/backup/rename/fsync
  └─ 调用 M4-P02 数据协调器
                         │
                         ▼
ime-product-install
  ├─ operation kind、产品逻辑身份与外层 receipt
  ├─ 跨进程 guard、状态转换和失败分类
  └─ 完全只读的终态身份 startup decision
                         │
                         ▼
ime-product-upgrade
  └─ Application Support 内的数据 snapshot/migration/rollback
```

`ime-product-install` 不读取 bundle、不计算 code signature、不停止进程、不接受安装路径、不复制或删除程序、不解释数据 receipt。`ime-product-upgrade` 不知道程序 staging、backup 或双 bundle 切换状态。Installer 与平台 adapter 不能自己发明状态、跳过 receipt 或根据缺失文件猜测 operation。

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
staged_manager / staged_input_method
backup_manager / backup_input_method
installed_manager / installed_input_method
```

staged/installed 必须逻辑匹配 target，backup 必须逻辑匹配 source；重复 slot、删除旧 evidence 或改写旧 identity 一律拒绝。

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

本切面先实现外层 receipt/guard、artifact contract 与 startup decision；程序 rename/fsync、数据协调映射和平台 adapter 在后续切面接入，但状态与替换规则从当前版本起固定。

## Startup decision

Manager 与 InputMethod 必须在 M4-P02 数据 gate、userdb、settings、Rime runtime 和 Flutter/IMK 业务初始化之前调用外层只读 gate。调用方提供从当前运行 bundle 重新形成的 `RunningProgramIdentity`，不能从设置或 UI 参数构造。

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

## 当前实现退出条件

- 独立 crate 不依赖数据升级 crate，也不进入输入热路径；
- 四种 operation 的 source/target 组合与状态顺序具有拒绝测试；
- product/bundle/hash/artifact evidence 严格解析，receipt canonical 且只允许追加证据；
- guard 能拒绝并发 operation，receipt 写入绑定正确 root 与 guard；
- startup gate 对缺失、非终态、终态身份匹配/漂移、remove、损坏、未知对象和中断写均有稳定结果；
- 全部测试只使用合成 `0700` 临时目录，不访问真实 Application Support、程序目标、系统设置、Keychain 或签名凭据。
