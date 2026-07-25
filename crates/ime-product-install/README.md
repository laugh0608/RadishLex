# ime-product-install 组件说明

本文说明 `radishlex-ime-product-install` 的职责、receipt、程序身份、跨进程 guard 与启动门禁，面向 Installer、平台适配层和双端启动维护者。本文不包含真实程序复制、签名 API、Installer UI 或数据 migration；完整 macOS 边界见 [程序安装事务](../../docs/macos-installation-transaction.md)。

## 组件定位

该 crate 是 M4-P03 外层程序事务核心，独立于 `ime-product-upgrade`：

- 显式区分 `first_install`、`upgrade`、`repair` 和 `remove_programs`；
- 保存 source/target 产品逻辑身份与只追加的 staging/backup/installed evidence；
- 约束从 `prepared` 到终态的单步状态转换；
- 在 `.radishlex-install-v1` 原子持久化 receipt，并以 Unix socket guard 拒绝并发 operation；
- 只读 startup gate 同时检查事务终态与当前运行 bundle 身份。

平台 adapter 负责固定 user-domain 路径、code signature/ProductManifest 复验、进程静止、程序 staging/rename/fsync 与 M4-P02 数据协调。核心不接收程序路径，不读取 bundle，不停止进程，也不删除程序或用户数据。

## 稳定状态

状态目录固定为：

```text
<data-root>/.radishlex-install-v1/
  receipt.json
  receipt.json.tmp
```

receipt format 是 `radishlex-product-install-receipt-v1`。data root/状态目录要求 `0700`，receipt 要求 `0600`、单 link 普通文件。未知对象、临时 receipt、owner/mode/identity 漂移均失败关闭。

产品身份只保存 release、bundle ID 与 ProductManifest/bundle tree/canonical code identity evidence 的 SHA-256，不保存绝对路径、Team ID 明文、签名输出或用户数据。

## 验证

```bash
cargo test -p radishlex-ime-product-install --all-targets
cargo clippy -p radishlex-ime-product-install --all-targets -- -D warnings
./scripts/check-product-install-core.sh
```

测试只使用合成私有临时目录，不访问真实 Application Support、`~/Applications`、`~/Library/Input Methods`、系统设置、Keychain 或签名凭据。
