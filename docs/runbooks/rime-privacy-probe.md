# Rime 隐私存储隔离诊断

本文面向维护者，说明如何用产品 schema、合成输入和独立 native 进程观察两套学习存储。它是诊断入口，不是系统输入法验收或隐私修复门禁；待决策方案与当前结果见 [产品审阅](../remediation/product-review-2026-09.md)。

## 范围与前置条件

- 使用已安装的 librime、`rime_dict_manager`、Python 3 和已缓存 Rust 依赖；本入口不安装或下载任何依赖。
- 使用仓库锁定的公开产品词典，只输入固定合成串 `shi`，选择当时第二候选。不接受真实用户数据路径或外部输入文本。
- 父测试自动在系统临时目录创建新的 `radishlex-rime-privacy-*` 私有根，每个场景有独立 user、log 与 RadishLex SQLite。历史根不覆盖、不清理。
- 产品数据通过现有 `product_data.py assemble` 校验和离线装配；第二份临时 A/B schema 只把 `enable_user_dict: true` 改为 `false`，不修改 committed schema 或来源锁。A/B 结果不称为现行产品行为。
- 不启动系统输入法、Manager、GUI 或 guest，不调用系统按键，不接触已冻结的 P04/R01B/L6 数据。

## 执行

macOS 既有 Homebrew librime 环境示例；其他环境应显式使用已安装库的实际 include/lib 路径：

```bash
RIME_INCLUDE_DIR=/opt/homebrew/opt/librime/include \
RIME_LIB_DIR=/opt/homebrew/opt/librime/lib \
cargo test --offline --locked -p radishlex-ime-ffi --features native-rime \
  --test native_rime_privacy_probe rime_privacy_storage_probe \
  -- --ignored --exact --nocapture
```

必须使用精确父测试名，不运行内部 `rime_privacy_probe_worker`。默认测试不执行上述两个 ignored 测试；缺少前置条件时明确失败，不把 skip 写成 native 通过。

## 观察方法

每个场景串行运行四个独立 OS 子进程：

1. `baseline`：初始化与部署，不输入，记录元数据基线。
2. `cancel`：输入固定合成串、读取候选后 reset，不提交，作为输入无学习对照。
3. `commit`：输入并选择第二候选，回读 commit 与 learning disposition；切换场景在 composition 已形成后、提交前更改 LearningContext。
4. `reopen`：新进程以 unknown 上下文读取同一输入的候选并 reset。此时 RadishLex ranker 受策略阻断，可观察 Rime 自身的持久化排序影响。

每阶段显式释放 session、shutdown librime，子进程退出成功后，父进程才检查文件和导出。`rime_dict_manager --export` 会打开 LevelDB，因此只对关闭后的合成数据库副本执行，不影响下一阶段原库。导出非注释行用于区分真实词条学习与 installation、manifest、LOG 等初始化或维护变化。

标准场景为 normal、privacy、unknown、secure、sensitive，以及 normal↔privacy、normal↔unknown。另有禁用用户词典的 normal、privacy、unknown 三项 A/B 对照，共 12 个场景、48 个独立子进程。

`report.json` 包含候选、合成选择、SQLite selection_events、Rime 导出词条、用户目录新增/变化/移除文件及日志中选择文本的 UTF-8 命中。诊断可在发现隐私缺口时返回成功：这只表示流程完成、对照成立，必须检查 `sqlite_skipped_but_rime_persisted` 等观察结果。

## 解释限制

- secure/sensitive 是直接设置 FFI 上下文，不证明真实 macOS secure 输入会进入引擎，也不替代 Fcitx sensitive 路由验收。
- 观察覆盖一组固定合成输入、当前 schema 和本机 librime；不代表所有 schema、引擎版本、平台、输入方式或故障路径。
- 日志检查只扫描本次指定 log 目录里的普通文件并跳过便利 symlink；“未匹配合成选择文本”不能扩展为无编码日志、无任何敏感字段或全系统无日志。
- A/B 使用全新用户词典，不证明旧 Rime 学习数据已经擦除、迁移或停止读取，也不量化长句、自学词、新词召回的损失。
- 观察模式切换后仍提交既有 composition，不自动决定修复时应清空输入还是保留输入并禁止学习；不得为了阻止学习丢掉用户待提交文本。

## SQLite 运行时身份

以下 opt-in 入口只创建内存数据库，返回测试产物实际链接的 SQLite 版本与 source id；不代表部署服务或冻结 bundle 已执行验证：

```bash
cargo test --offline --locked -p radishlex-ime-userdb \
  --test sqlite_library_identity -- --ignored --nocapture
```

在 `server/sync-server` 执行：

```bash
GOPROXY=off GOTOOLCHAIN=local RADISHLEX_SQLITE_IDENTITY_PROBE=1 \
go test ./internal/storage -run '^TestSQLiteLibraryIdentity$' -count=1 -v
```

升级关闭条件仍须覆盖产物版本、多连接 WAL/checkpoint、迁移、备份恢复和适用平台门禁；版本打印不是数据安全验收。
