# Rime 隐私存储隔离回归

本文面向维护者，说明如何用产品 schema、合成输入和独立 native 进程验证单一学习存储及有效配置检查。它不替代真实系统输入法验收；当前结果与剩余条件见 [产品审阅](../remediation/product-review-2026-09.md)。历史诊断 v1 保留于 `4c3fe0f`，其观察及证据不改写成修复结果。

## 范围与前置条件

- 使用已安装的 librime、`rime_dict_manager`、Python 3 和已缓存 Rust 依赖；不安装或下载依赖。
- 仓库锁定产品数据通过 `product_data.py assemble` 离线装配，只输入固定合成串 `shi`，选择当时第二候选；不接受真实数据路径或外部输入文本。
- 父测试在系统临时目录创建新的私有根，每个场景独立 user、log 与 SQLite。历史根不覆盖、不清理。
- 旧库对照只用 `rime_dict_manager --import` 生成一条高频合成词；输入前冻结其文件字节，部署、提交及重启均须保持不变。导出只打开关闭后的合成副本。
- 不启动系统输入法、Manager、GUI 或 guest，不调用系统按键，不接触冻结的 P04/R01B/L6 数据。

## 执行

macOS 既有 Homebrew 环境示例；其他环境使用已安装库的实际 include/lib 路径：

```bash
export RIME_INCLUDE_DIR=/opt/homebrew/opt/librime/include
export RIME_LIB_DIR=/opt/homebrew/opt/librime/lib
cargo test --offline --locked -p radishlex-ime-ffi --features native-rime \
  --test native_rime_privacy_probe rime_privacy_storage_probe \
  -- --ignored --exact --nocapture
cargo test --offline --locked -p radishlex-ime-ffi --features native-rime \
  --test native_rime_learning_guard rime_learning_guard_native_regression \
  -- --ignored --exact --nocapture
```

只运行精确父测试名，内部 worker 由父进程驱动。默认测试仍为 ignored；缺少前置条件必须失败，不把 skip 写成 native 通过。

## 存储与重启观察

每场景运行四个独立 OS 进程：baseline 初始化与部署、cancel 输入后 reset、commit 输入并选择、reopen 用 unknown 上下文再次输入观察并 reset。commit 阶段的上下文切换发生在 composition 形成后，仍须完成匹配提交。

标准场景为 normal、privacy、unknown、secure、sensitive、normal↔privacy、normal↔unknown；加上预置旧库的 normal、privacy、unknown，共 12 场景 / 48 个子进程。每阶段释放 session、shutdown 并退出后才检查文件。断言：

- 只有全程 normal 产生一条 RadishLex selection；受限和切换场景均为零。
- 空库不创建 Rime userdb；预置旧库逐文件字节不变，没有新增学习词条。
- 提交前与重启后的 engine-only 候选相同；预置旧库的候选也与空库一致。
- 指定 log 目录未匹配本次合成选择的 UTF-8 文本。

`report.json` 保留候选、学习 disposition、事件数量、导出行和文件变化；任一隐私断言失败时测试必须失败，不再沿用诊断 v1 的“观察到缺口也成功”语义。

## 有效配置与生命周期负向验证

第二个父测试运行六场景 / 12 个子进程，先用安全产品配置部署，再修改该测试自己的文件：

- 已编译配置启用 user_dict、关闭项缺失、namespaced translator；
- 默认列表中未被调用方选中的不安全 schema，并设置为上次选中项；
- 用户 custom patch 在部署时重新启用 user_dict；
- runtime 存续期间替换当前/第二 schema 与 default 文件，覆盖双 session、切 schema 和零 session 间隙。

前五项须在 native session 创建前失败，不能出现 Rime userdb。最后一项须继续使用已检查配置；shutdown 后重新初始化读取修改后的文件并失败。此测试检查公开 config handle 的实际缓存生命周期，不仅靠 stub 调用计数。

## 解释限制

- secure/sensitive 是直接设置 FFI 上下文，不证明真实密码输入进入引擎，也不替代两平台路由验收。
- 只覆盖固定合成输入、产品 pipeline 与本机 librime，不声称所有版本、schema、插件或故障路径已通过。
- 日志只检查指定目录普通文件，跳过便利 symlink；UTF-8 未命中不等于无编码日志、内存残留或全系统无敏感字段。
- 旧库验证证明本次没有读取收益或文件变化，不代表旧数据已擦除、迁移，或旧版进程也会停止学习。
- 基础输入、分段、翻页、重排/删除恢复另跑 [native smoke](rime-native-smoke.md)；这些有限回归不量化长句、自学词与新词召回损失。

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
