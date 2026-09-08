# SQLite 旧版本合成库

本目录只用于 userdb 的跨 SQLite 版本回归，不包含真实用户数据或冻结产品数据库。

`sqlite-3.46.0-schema9.sqlite3` 是 SQLite 3.46.0 写出的 schema 9 数据库，大小 143360 bytes，SHA-256 为 `f0234adef1facc8d3eba8e2640586b89a6f0e48e846b9b5b1dd1170bb0def721`。文件头 last-writer version 为 `3046000`。它只含一次 `synthetic-normal-commit` 对 `shi` / `时` 的合成选择、对应词和频次 1。

来源是提交 `40cd1bd237629e7542bdaa43536584e634b950ba` 的 `native_rime_privacy_probe` 正常场景，证据批 `radishlex-rime-privacy-44873-1788870603674313000`；生成时 Rust 链为 `rusqlite 0.32.1` / `libsqlite3-sys 0.30.1`，SQLite source id 为 `2024-05-23 13:25:27 96c92aba00c8375bc32fafcdf12429c58bd8aabfcadab6683e35bbb9cdebf19e`。所有子进程已结束，没有 WAL/SHM sidecar；文件以原始字节复制，未用新版本 SQLite 重写或降版本伪造。

测试将字节写入独立临时库，验证新库打开、学习、删除、防复活、备份及显式恢复。此 fixture 不复现 WAL-reset 罕见竞争；旧 schema 的 migration 和事务失败回滚由同模块既有 v1/v2/v3 等合成测试覆盖。
