# macOS 产品升级协调适配器

本文说明 `radishlex-macos-upgrade-coordinator` 的调用边界，面向产品升级协调入口和安装载体维护者。它不提供命令行工具，不决定安装位置，也不执行真实用户升级。

adapter 实现 `UpgradeCoordinatorPort`，把平台无关协调核心绑定到固定 macOS 产品装配：

- `load(source, target)` 只接收两代产品根，不接收 helper、数据库或 settings 路径；
- 两代 release、schema、component 和 helper 均来自严格的 `ProductManifest.json`；
- target preflight 用于每个静止 checkpoint；
- target 双端 validation host 用于 candidate 与最终固定路径；
- source 双端 validation host 用于回滚后的旧版本兼容证明；
- 每次执行前重新校验 helper 的普通文件身份、长度和 SHA-256；
- host 输出、绝对路径和底层错误不进入 receipt。

调用方应先执行 `inspect_preflight()` 取得容量，再把同一 adapter 交给 `UpgradeReceiptStore::resume_userdb_upgrade`。M4-P03 安装载体还必须在调用前证明产品根的固定来源与 code signature；本 crate 不把 manifest hash 当作发布者身份。

验证入口：

```bash
./scripts/check-macos-upgrade-coordinator.sh
```
