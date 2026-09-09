# ime-product-install 组件说明

本文说明 `radishlex-ime-product-install` 的职责、receipt、程序身份、同文件系统程序切换、跨进程 guard 与启动门禁，面向 Installer、平台适配层和双端启动维护者。本文不包含真实程序复制、签名 API、Installer UI、数据 receipt 或 migration；完整 macOS 边界见 [程序安装事务](../../docs/macos-installation-transaction.md)。

## 组件定位

该 crate 是 M4-P03 外层程序事务核心，独立于 `ime-product-upgrade`：

- 显式区分 `first_install`、`upgrade`、`repair` 和 `remove_programs`；
- 保存 source/target 产品逻辑身份与只追加的 source/staging/backup/installed evidence；
- 约束从 `prepared` 到终态的单步状态转换；
- 在 `.radishlex-install-v1` 原子持久化 receipt，并以 Unix socket guard 拒绝并发 operation；
- 在两个目标各自的文件系统内固定私有 staging/backup，按 Manager、InputMethod 顺序 rename/fsync，并按精确 inode 恢复中断现场；
- 通过四类 operation 共用的终态 port 分别复验并持久化 `final_verified`、`completed`，第二段中断后重新取证续跑；
- 只读 startup gate 同时检查事务终态与当前运行 bundle 身份。

平台 adapter 负责解析固定 user-domain 父目录、code signature/ProductManifest 复验和保留 macOS metadata 的 staging 填充；独立 `InstallCoordinatorAdapter` 负责把本核心与 M4-P02 数据协调器组合。核心只接受已经验证的两个目标父目录，固定 bundle/事务槽位，不读取 bundle 内容、不计算签名、不停止进程、不依赖 `ime-product-upgrade`，也不递归清理程序或用户数据。

核心向组合层提供只读的 current receipt/guard、`ProgramSwitchStore` binding 校验，以及 `InstallProgramValidationPort`。这些接口只证明程序事务输入仍属于当前 operation；数据 receipt 的 operation/root/release 绑定和成功/失败映射仍由组合层负责。

`InstallReceiptStore::open_existing` 只打开并复验既有状态目录，缺失时失败，不创建对象。`load_for_recovery_inspection` 拒绝中断 receipt、活动或无法判定的 guard；对已确认 connection refused 且前后身份一致的 stale socket 只读保留，后续授权 `acquire_guard` 才按既有机制获取 guard。该 inspection 会探测 socket，不能替代产品 startup gate 的严格只读、存在 guard 即阻断规则。

## 稳定状态

状态目录固定为：

```text
<data-root>/.radishlex-install-v1/
  receipt.json
  receipt.json.tmp
```

receipt format 是 `radishlex-product-install-receipt-v1`。data root/状态目录要求 `0700`，receipt 要求 `0600`、单 link 普通文件。未知对象、临时 receipt、owner/mode/identity 漂移均失败关闭。

产品身份只保存 release、bundle ID 与 ProductManifest/bundle tree/canonical code identity evidence 的 SHA-256，不保存绝对路径、Team ID 明文、签名输出或用户数据。

每个 component 的事务目录固定为 `<target-parent>/.radishlex-install-<operation-id>/`，权限为 `0700`，只允许 `staged.app` 与 `source-backup.app`。程序根只接受目标 uid 所有、同设备、非 symlink 的 `0700`/`0755` 目录；source/backup 与 staged/installed 必须分别保持同一 inode。每次 rename 后依次同步目标目录和源目录，重试只接受 inode 位于预期两个槽位之一的现场。

## 验证

```bash
cargo test -p radishlex-ime-product-install --all-targets
cargo clippy -p radishlex-ime-product-install --all-targets -- -D warnings
./scripts/check-product-install-core.sh
```

测试只使用合成私有临时目录，覆盖四类 operation 两段终态、`final_verified` 重启续跑、部分双端提交、source 恢复和 preserve/commit/rollback 的逐 rename/fsync 边界；跨核心组合另由 `./scripts/check-macos-install-coordinator.sh` 验证。不会访问真实 Application Support、`~/Applications`、`~/Library/Input Methods`、系统设置、Keychain 或签名凭据。
