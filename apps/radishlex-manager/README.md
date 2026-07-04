# RadishLex Manager

RadishLex Manager 是萝卜词核的 Flutter 管理端起步工程。

当前工程通过受控 `ManagerBridge` contract 接入管理数据源。默认仍使用合成 fixture；显式配置本地 userdb 和 `ime-ffi` native library 后，可切到第一批真实 Dart FFI bridge，用于验证 Phase 4 第一批本地管理能力：

- 本地 userdb 词条管理视图。
- 本地学习状态摘要。
- `rank explain` 非敏感摘要。
- `sync preflight` 状态和生产不可用原因。
- 自部署服务端配置草案。

真实远端同步、恢复码、设备授权和平台私钥 backend 成功路径尚未开放。相关停止线见仓库根 `docs/manager-ui-boundary.md`。
当前 FFI bridge 覆盖本地 userdb 词条 list / delete、用户词库 inspect / import / export、import batches、learning status 摘要、rank explain 摘要和 sync preflight 摘要。`rank explain` 区域通过专用 `ime-ffi` ABI 读取单候选贡献项，Flutter 只展示复制后的非敏感摘要。设置页会显示配置来源诊断；bridge 失败只展示结构化错误码，不把 native 错误明细透传给 widget 层。真实远端同步、恢复码和设备授权 UI 继续关闭。

## FFI bridge

默认启动不加载 native library：

```bash
flutter run -d macos
```

显式接入本地 userdb 时，需要先构建 `radishlex-ime-ffi` 的动态库，然后传入 SQLite 路径和库路径：

```bash
cargo build -p radishlex-ime-ffi
RADISHLEX_MANAGER_DB=/tmp/radishlex-userdb.sqlite \
RADISHLEX_MANAGER_FFI_LIBRARY=../../target/debug/libradishlex_ime_ffi.dylib \
flutter run -d macos
```

可选 `RADISHLEX_MANAGER_SYNC_SERVER` 只用于显示自部署端点草案，不会启用真实上传、恢复码或设备授权。

## 验证

```bash
../../scripts/check-manager.sh
../../scripts/check-manager-ffi-smoke.sh
```

`check-manager-ffi-smoke.sh` 会构建 `radishlex-ime-ffi` 动态库，在仓库外临时目录创建 SQLite userdb、导入 TSV、读取 import batches 与 rank explain、删除词条并导出词库，用真实 Dart FFI bridge 复验本地管理链路。该 smoke 只使用合成词条，不连接真实同步后端，也不读取真实输入法目录。

如需在 macOS 桌面运行 fixture 版本：

```bash
flutter run -d macos
```
