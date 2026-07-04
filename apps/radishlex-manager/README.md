# RadishLex Manager

RadishLex Manager 是萝卜词核的 Flutter 管理端起步工程。

当前工程通过受控 `ManagerBridge` contract 接入管理数据源。默认仍使用合成 fixture；显式配置本地 userdb 和 `ime-ffi` native library 后，可切到第一批真实 Dart FFI bridge，用于验证 Phase 4 第一批本地管理能力：

- 本地 userdb 词条管理视图。
- 本地学习状态摘要。
- `rank explain` 非敏感摘要。
- `sync preflight` 状态和生产不可用原因。
- 脱敏诊断摘要预览 / 导出。
- 自部署服务端配置草案和 sync gate 状态来源。

真实远端同步、恢复码、设备授权和平台私钥 backend 成功路径尚未开放。相关停止线见仓库根 `docs/manager-ui-boundary.md`。
当前 FFI bridge 覆盖本地 userdb 词条 list / delete、用户词库 inspect / import / export、import batches、learning status 摘要、rank explain 摘要、sync preflight 摘要、设置草案持久化和脱敏诊断报告导出。`rank explain` 区域通过专用 `ime-ffi` ABI 读取单候选贡献项，Flutter 只展示复制后的非敏感摘要。设置页会显示配置来源诊断，并可预览 / 导出不含用户词、文件路径、token 或 payload bytes 的诊断摘要；bridge 失败按操作和分类展示结构化错误码，不把 native 错误明细透传给 widget 层。sync gate 状态由设置草案、隐私模式、平台私钥 backend gate 和部署证据草案共同派生；真实远端同步、恢复码和设备授权 UI 继续关闭。

## 代码结构

- `lib/src/screens/manager_home_screen.dart` 保留 snapshot 加载、导航壳层、跨页 bridge 操作调度和统一错误提示。
- `lib/src/screens/dictionary_view.dart`、`learning_view.dart`、`sync_view.dart` 和 `settings_view.dart` 分别承载词库、学习、同步和设置页面。
- `lib/src/screens/manager_widgets.dart` 收纳页面共享的 section、metric、key-value row 和状态 badge 组件。

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

可选 `RADISHLEX_MANAGER_SYNC_SERVER` 会作为首次启动的服务端草案；可选 `RADISHLEX_MANAGER_SETTINGS_FILE` 指向本地 settings JSON，用于保存非 secret 的设置草案。两者都不会启用真实上传、恢复码或设备授权。

## 验证

```bash
../../scripts/check-manager.sh
../../scripts/check-manager-ffi-smoke.sh
```

`check-manager-ffi-smoke.sh` 会构建 `radishlex-ime-ffi` 动态库，在仓库外临时目录创建 SQLite userdb、settings JSON、导入 TSV、读取 import batches 与 rank explain、保存设置草案、删除词条、导出词库并导出脱敏诊断报告，用真实 Dart FFI bridge 复验本地管理链路。该 smoke 只使用合成词条，不连接真实同步后端，也不读取真实输入法目录。

如需在 macOS 桌面运行 fixture 版本：

```bash
flutter run -d macos
```
