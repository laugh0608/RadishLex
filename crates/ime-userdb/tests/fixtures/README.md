# SQLite 合成数据库与回滚日志

本目录用于 userdb 的跨 SQLite 版本回归和源库准备恢复合同，不包含真实用户数据或冻结产品数据库。

## 旧 SQLite 版本

`sqlite-3.46.0-schema9.sqlite3` 是 SQLite 3.46.0 写出的 schema 9 数据库，大小 143360 bytes，SHA-256 为 `f0234adef1facc8d3eba8e2640586b89a6f0e48e846b9b5b1dd1170bb0def721`。文件头 last-writer version 为 `3046000`。它只含一次 `synthetic-normal-commit` 对 `shi` / `时` 的合成选择、对应词和频次 1。

来源是提交 `40cd1bd237629e7542bdaa43536584e634b950ba` 的 `native_rime_privacy_probe` 正常场景，证据批 `radishlex-rime-privacy-44873-1788870603674313000`；生成时 Rust 链为 `rusqlite 0.32.1` / `libsqlite3-sys 0.30.1`，SQLite source id 为 `2024-05-23 13:25:27 96c92aba00c8375bc32fafcdf12429c58bd8aabfcadab6683e35bbb9cdebf19e`。所有子进程已结束，没有 WAL/SHM sidecar；文件以原始字节复制，未用新版本 SQLite 重写或降版本伪造。

测试将字节写入独立临时库，验证新库打开、学习、删除、防复活、备份及显式恢复。此 fixture 不复现 WAL-reset 罕见竞争；旧 schema 的 migration 和事务失败回滚由同模块既有 v1/v2/v3 等合成测试覆盖。

## 当前 SQLite 的单库 hot rollback journal

以下三份文件来自 2026-09-15 当前锁定的 `libsqlite3-sys 0.37.0` / bundled SQLite 3.51.3，schema 9。独立子进程在 DELETE 模式执行未提交的真实页 spill 后立即退出，保留主库及 hot journal；没有改写上面的旧版本 fixture。

| 文件 | bytes | SHA-256 |
| --- | --- | --- |
| `journal-before.sqlite3` | 143360 | `2fd5dab464cb9b6bdd8b57a94df2c8dae875fedfba1d600a5f8d6af89a179386` |
| `journal-spilled.sqlite3` | 167936 | `011554ddaadc3e6fe6c60838d838eb5cb659872c03a816111a7a5e965d8ee702` |
| `journal-spilled.sqlite3-journal` | 13832 | `83815abbf9e13f662fed5502cc033feeebf17d2ae15c64ab01a2fec86ae7ffb4` |

原始逻辑内容仅含 `synthetic-recovery` 下 `shi` / `时`、`shan` / `删` 的合成选择，后者已删除并保留 tombstone。未提交事务把 `user_terms.text` 追加 8000 位数字以触发 spill。journal 首段声明 35 个原始页、512 bytes sector、4096 bytes page、2 个记录；主库与 journal 必须成对使用。

生成入口为 [maintenance_recovery_tests.rs](../../src/store/maintenance_recovery_tests.rs) 的 `journal_recovery_child`。重新生成时仅使用 `std::env::temp_dir()` canonical 根下新建的空私有目录，名字以 `radishlex-journal-recovery-` 开头，设置 `RADISHLEX_JOURNAL_RECOVERY_ROOT` 为该绝对路径、`RADISHLEX_JOURNAL_RECOVERY_POINT=export`，离线运行：

```bash
cargo test -p radishlex-ime-userdb --lib store::maintenance::recovery::tests::journal_recovery_child -- --ignored --exact
```

入口独占创建上述三个文件，不覆盖已有输出；时间及 SQLite journal nonce 会使重新生成的字节摘要变化。更新 fixture 时须重新核对、记录摘要并完成恢复测试。两个 `.sqlite3` 文件是明确跟踪的测试资产，受通用忽略规则影响，新增时需精确强制暂存。

macOS adapter 测试在新合成目录内先建立持久维护意图，再按此 family 组合中断状态，仅证明恢复身份与编排重载。userdb 测试另行实时生成 spill 并执行恢复边界退出。两者均不证明 WAL→DELETE 内部真实断电、pager 逐页恢复中途断电或完整产品连续升级；WAL 页一恢复测试是单独注明的人工头部变体。
