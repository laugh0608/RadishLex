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

`MacOsPreparationHasher` 为源库准备核心提供完整文件流 SHA-256，复用本 adapter 已有依赖，读取错误直接失败。`tests/source_preparation.rs` 以新合成目录、真实 SQLite 与该 hasher 验证准备快照、维护意图、源库准备、异常/退出重载及学习/tombstone 保留；授权/静止 port 使用合成实现，不能当作真实 outer guard、程序资格或 Installer 接线通过。新增 journal 测试使用仓库冻结的纯合成 SQLite family，在持久维护意图之后组合出中断状态，验证恢复身份先落盘、三处进程退出重载与漂移拒绝；它不代表真实 WAL 模式转换的连续崩溃现场。真实 spill/重放边界的独立进程测试位于 userdb。准备流程及历史接续分别在 `source_prepared` / `previous_archived` 保留启动阻断；核心后续 handoff 与终态释放已纳入下述合成测试，真实产品接线按[已批准设计](../../../docs/remediation/macos-wal-source-preparation-design.md)继续。

验证入口：

```bash
./scripts/check-macos-upgrade-coordinator.sh
./scripts/check-macos-upgrade-product-coordination.sh
```

前者验证 adapter contract 和 feature 编译边界；后者重新装配真实双端产品，只在私有合成 user home/Application Support 中执行 manifest-bound 协调资格，不读取真实用户目录，也不安装或启动 GUI 产品。

`tests/preparation_history.rs` 及子模块使用真实 SQLite、SHA-256 与私有文件验证旧终态 inventory、逐槽保留接续、新 v1 handoff、三类终态封存/释放、已释放前驱的连续准备及中断/漂移拒绝。旧 v1 receipt/产品身份及静止 port 为合成材料；测试不启动真实 Manager/InputMethod，也不证明 outer guard、Installer 接线或冻结 source/target 副本资格。核心 inventory 的初始化 I/O/进程退出矩阵另在升级核心 crate 中维护。

`tests/preparation_history/release_chain.rs` 覆盖多代已释放祖先的缺失、内容/身份/权限/链接漂移、错误前驱和回调内篡改，要求只读查询、下一次准备与释放写入都拒绝继续；正常后续学习/WAL 仍允许历史查询。所有材料均为临时合成 fixture。

`tests/source_preparation/cancellation.rs` 与源库收尾子模块验证独立取消请求、维护前的 family 保持、维护后的完整等价、受控 journal 恢复和持久进度重载。历史套件同时验证取消源库收尾不搬动或重写旧材料；原准备/v1 与 startup 入口仍阻断。这些测试使用合成取消/源库授权 port；本 adapter 尚未实现对应真实 outer 接线、旧程序取消资格或 UI 入口，`source_ready` 不能解释为取消完成。

`tests/preparation_history/cancellation_archive*.rs` 与已释放前驱回归验证取消材料封存、精确位置重载、outer 原件保留及兼容副本发布，覆盖维护前后、三类旧终态、部分归档、无 data 前驱、已释放 v1 历史、身份漂移/冲突及授权失效/真实子进程退出。结果为 `preserved`，活动请求与封存记录仍阻断启动，产品 authority 为合成。双 guard 与严格外层回执的组合测试位于 `InstallCoordinatorAdapter/tests/cancellation_archive.rs`；它也不代表真实程序/Installer 资格。
