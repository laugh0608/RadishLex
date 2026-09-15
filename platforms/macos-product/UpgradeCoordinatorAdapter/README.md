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

调用方应先执行 `inspect_preflight()` 取得容量，再把同一 adapter 交给 `UpgradeReceiptStore::resume_userdb_upgrade`。M4-P03 安装载体还必须在调用前证明产品根的固定来源、版本化 distribution identity 与 strict ad-hoc code identity；本 crate 不把 manifest hash 当作发布 identity。

`MacOsPreparationHasher` 为源库准备核心提供完整文件流 SHA-256，复用本 adapter 已有依赖，读取错误直接失败。`tests/source_preparation.rs` 以新合成目录、真实 SQLite 与该 hasher 验证准备快照、维护意图、源库准备、异常/退出重载及学习/tombstone 保留；授权/静止 port 使用合成实现，不能当作真实 outer guard、程序资格或 Installer 接线通过。准备流程在 `source_prepared` 保留启动阻断，后续产品接线和旧事务接续按[已批准设计](../../../docs/remediation/macos-wal-source-preparation-design.md)继续。

验证入口：

```bash
./scripts/check-macos-upgrade-coordinator.sh
./scripts/check-macos-upgrade-product-coordination.sh
```

前者验证 adapter contract 和 feature 编译边界；后者重新装配真实双端产品，只在私有合成 user home/Application Support 中执行 manifest-bound 协调资格，不读取真实用户目录，也不安装或启动 GUI 产品。
