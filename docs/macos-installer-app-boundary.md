# macOS Installer App UI 与驱动边界

本文固定 M4-P03 独立 `RadishLex Installer.app` 的状态展示、只读驱动、显式用户授权和失败关闭契约，读者是 Installer UI、安装事务、平台适配与产品门禁维护者。本文不包含真实用户目录写入、程序停止、系统输入源修改、Developer ID、公证、DMG 发布或身份绑定终态清理。

## 分层

```text
AppKit presentation
  ├─ 只展示版本化 snapshot
  ├─ 展示固定目标、operation、进度和稳定错误
  └─ 收集显式确认，不解释 receipt
                    │
                    ▼
InstallerDriver
  ├─ read-only install status projection
  ├─ verified installed-product situation
  └─ stale-action authorization
                    │
                    ▼
InstallerExecutor
  ├─ guard 内重新读取 receipt 与平台 preflight
  ├─ prepared 后等待人工静止确认
  └─ 重放程序事务、数据协调与两段终态
                    │
                    ▼
ime-product-install / macOS adapters
  └─ receipt、guard、程序身份与状态机真相源
```

UI 不直接读取 `.radishlex-install-v1/receipt.json`，不根据 bundle 是否存在猜测 operation，也不接收绝对路径、`HOME`、settings、Manager 页面或调用方自报身份。固定目标仍逐字节来自 `packaging/macos/install-layout.json`。

## 只读状态投影

`ime-product-install::inspect_install_status` 只返回：

- `ReadyFirstLaunch` / `ReadyNoInstallState`；
- `OperationInProgress`；
- `TerminalReceipt`；
- `FailedClosed`；
- 既有稳定 startup error、operation kind、receipt state、failure code 和 manual-recovery 标记。

检查缺失 data root 或状态目录时不创建目录、不 chmod；检查 receipt 时不写临时文件、不清理未知对象。活跃 guard 只返回 `ActiveGuard`，不泄露 receipt 或 operation；已经失去 listener 的 guard socket 只作为可由后续 guard acquire 精确接管的崩溃现场，不由只读检查删除。损坏/中断 receipt、未知对象、unsafe root/state、身份漂移或 IO 失败全部失败关闭。

无 receipt 不能证明已安装产品版本。平台必须按固定双目标和 manifest/code identity 形成以下之一：

- not installed；
- installed release older than payload；
- installed release exactly matches payload；
- installed release newer than payload；
- identity unavailable。

只有前三类能分别形成 first install、upgrade 或 repair；更新版本和身份不可得均阻断。产品身份检查不得移入 UI。

## Snapshot v1

`InstallerViewSnapshot` 固定携带：

- contract version；
- phase：`ready`、`awaiting_user_action`、`in_progress`、`completed`、`recovery_available` 或 `blocked`；
- primary/secondary action；
- operation、receipt state、failure code；
- 0–10 的 receipt 持久化进度；
- stable error、manual prompt；
- `retain_application_support` 数据策略。

进度只表示已持久化状态，不预测耗时，也不能在 UI 定时器中自行前进。重启后必须重新读取 receipt：`prepared` 要求用户完成手动静止步骤；其他非终态提供 resume；`aborted_preserved` / `rolled_back` 提供 retry；completed remove 可再次 first install。completed install/upgrade/repair 必须与当前固定双目标产品身份交叉判定，分别重新提供 upgrade、repair/remove 或失败关闭，不能因存在 terminal receipt 永久隐藏后续 operation。

稳定诊断摘要只允许：

```text
phase=<code> action=<code> error=<code> state=<code>
```

不输出路径、operation ID、bundle identity、签名 requirement、PID、命令行或底层错误正文。未知 contract 版本、phase/action/error/state、进度或 prompt 统一投影为 `blocked + refresh + unknown_driver_result`。

## 用户授权

refresh 是唯一不要求确认的 action。first install、upgrade、repair、resume、retry 和 remove 都必须对当前重新读取的 snapshot 调用 `authorize_installer_action`；旧窗口中的 action 若已不再提供，必须以 `ActionNotOffered` 拒绝。

用户确认只形成 `AuthorizedInstallerIntent`：

- mutation action 必须显式确认；
- remove 必须确认默认只删除两个程序并保留 Application Support；
- upgrade、repair、retry、remove 与 `confirm_quiescence` 必须确认已手动切到中立输入源并关闭 Manager；
- 所有 mutation intent 都标记为必须重新执行平台 preflight。

确认框和勾选不是静止、运行身份或 TIS 状态证据。后续 executor 仍须使用公开平台 API、固定 bundle ID 和 manifest-bound adapter 重新取证。Installer 不程序化选择、注册、启用或移除输入源。

## 当前产品壳

`platforms/macos-product/InstallerApp/` 是独立 AppKit bundle，不依赖 Flutter/Manager。它：

- 从产品/install layout 元数据生成版本、build、最低 macOS 与 Installer bundle ID；
- 把 committed `InstallLayout.json` 原样嵌入 Resources；
- 显示固定双程序目标、保留的数据根、operation、进度、恢复动作和稳定摘要；
- 拒绝任何命令行参数；
- 使用 ad-hoc 签名只完成构建完整性验证。

当前默认 bridge 固定为 `driver_unavailable`，因此该 bundle 是 UI/driver contract shell，不执行真实安装。独立 `InstallerExecutor` 已在隔离合成用户域把 authorized intent 接入现有 manifest-bound port、事务核心与 preflight；后续 bridge 只能调用该稳定入口，不能在 AppDelegate 中补复制、rename、删除、receipt 或 TIS 逻辑。

## 隔离执行器

begin/retry/remove intent 先在外层 guard 内回读 current receipt、验证 operation/source/target chain，并要求 target-only manifest-bound preflight 的 version/build 与 InstallPayload target 精确匹配，再持久化新的 `prepared` 并返回 UI。只有重新投影出的 `ConfirmQuiescence` intent 能推进 `quiesced`；执行前再次 preflight，随后按 receipt evidence 幂等完成双 bundle 切换。

first install、repair、remove 进入统一程序终态。upgrade 要求调用边界已经提供同 operation ID、root/release 精确绑定的 M4-P02 receipt，执行器组合数据协调和 upgrade 两段终态；中断在 `final_verified` 时只重新验证 data `completed` 与双程序，不重复 migration。active guard、stale intent、缺失 upgrade context、platform preflight 失败或任何 receipt/identity 漂移均失败关闭，错误日志只能使用稳定 code。

## 验证

```bash
cargo test --locked -p radishlex-ime-product-install --all-targets
./scripts/check-macos-installer.sh
./scripts/check-repo.sh
```

门禁覆盖 absent root 零写入、active/stale guard、非终态重启投影、completed receipt 与后续 operation、产品情况分支、stale/unconfirmed action、四类执行、preflight 阻断、程序 staging 与 upgrade `final_verified` 重启续跑、移除数据保留授权、稳定摘要、未知 snapshot、原生 bundle metadata、固定 layout 和 UI 源码禁止边界。普通门禁不启动 GUI、不访问真实用户目录或系统输入源。
