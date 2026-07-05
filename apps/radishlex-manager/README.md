# RadishLex Manager

RadishLex Manager 是萝卜词核的 Flutter 管理端起步工程。

当前工程通过受控 `ManagerBridge` contract 接入管理数据源。默认仍使用合成 fixture；显式配置本地 userdb 和 `ime-ffi` native library 后，可切到第一批真实 Dart FFI bridge，用于验证 Phase 4 第一批本地管理能力：

- 本地 userdb 词条管理视图，包含搜索过滤、空态、词条审计详情、导入历史筛选 / 排序 / 批次联动审计、删除确认和操作失败分类提示。
- 本地学习状态聚合摘要，包含 P1 原始事件不展示说明。
- `rank explain` 非敏感摘要，包含筛选和候选贡献项详情。
- `sync preflight` 状态、对象分类、设备门禁和生产不可用原因。
- 脱敏诊断摘要预览 / 导出，预览支持字段分组、筛选和复制脱敏文本。
- 自部署服务端配置草案、sync gate 草案预览和状态来源。

真实远端同步、恢复码、设备授权和平台私钥 backend 成功路径尚未开放。相关停止线见仓库根 `docs/manager-ui-boundary.md`；settings JSON 与诊断报告字段见 `docs/manager-settings-diagnostics.md`。
当前 FFI bridge 覆盖本地 userdb 词条 list / delete、用户词库 inspect / import / export、import batches、learning status 摘要、rank explain 摘要、sync preflight 摘要、设置草案持久化和脱敏诊断报告导出。`rank explain` 区域通过专用 `ime-ffi` ABI 读取单候选贡献项，Flutter 只展示复制后的非敏感摘要，并支持筛选和候选详情；学习页只展示聚合计数和贡献信号，不展示 P1 原始选择事件、原始输入历史或应用窗口信息。词库页会显示导入检查、词条 key / source / import batch / tombstone / sync 分类审计详情、导入历史筛选 / 排序 / 批次联动审计、本地 sync preflight 影响摘要、导入 / 导出结果摘要、删除确认和操作失败分类提示；同步页会显示 sync gate 状态来源、本地 P2 对象分类、local-only 事件计数、设备 backend capability 和 production gate 阻断原因，真实同步按钮继续禁用；设置页会显示配置来源诊断和同步门禁草案预览，可记录 allowlist 形式的部署证据来源标签，并可按诊断字段索引分组预览、筛选、复制和导出包含 gate source / stop line 但不含用户词、文件路径、token 或 payload bytes 的诊断摘要。bridge 失败按操作和分类展示结构化错误码，不把 native 错误明细透传给 widget 层。sync gate 状态由设置草案、隐私模式、平台私钥 backend gate 和部署证据来源草案共同派生；真实远端同步、恢复码和设备授权 UI 继续关闭。

## 页面与操作说明

当前 manager 页面围绕本地可审计管理能力组织，不进入输入热路径：

- 词库页用于查看本地 userdb 词条、按输入码 / 文本 / 来源筛选、查看词条审计详情、执行删除确认、预览导入检查、导入用户词库、导出用户词库，并查看 import batches 与本地 sync preflight 影响摘要。导入和导出都必须由用户显式触发；导入错误、删除失败和文件操作失败按结构化分类显示。
- 学习页只展示聚合学习状态和 `rank explain` 非敏感贡献信号。页面不得展示 P1 原始 selection event、原始输入历史、窗口标题或应用上下文明细。
- 同步页只展示本地 sync preflight、P2 对象分类、local-only 事件计数、设备 backend capability 和 production gate 阻断原因。`preflight_ready` 只表示本地草案可解释，不代表真实远端同步已开放。
- 设置页保存非 secret settings draft，包含自部署服务端地址草案、隐私模式、诊断导出草案、保留同步配置草案和部署证据来源标签。保存草案会重新派生 sync gate 状态，但不会连接真实远端。
- 设置页诊断摘要预览只展示脱敏字段索引和完整脱敏文本。section 筛选和关键字筛选只影响对话框里的字段列表；复制按钮始终复制完整 `manager.diagnostics.v1` 脱敏文本，导出也必须保持同一份脱敏摘要语义。

## 代码结构

- `lib/src/screens/manager_home_screen.dart` 保留 snapshot 加载、导航壳层、页面选择和统一提示入口。
- `lib/src/screens/manager/manager_home_actions.dart` 收纳跨页 bridge 操作编排、对话框调用和结构化结果文案。
- `lib/src/screens/dictionary_view.dart`、`learning_view.dart`、`sync_view.dart` 和 `settings_view.dart` 分别承载词库、学习、同步和设置页面。
- `lib/src/screens/dictionary/` 收纳词库页导入历史区域、删除确认、导入检查和导出对话框，避免继续扩大 `dictionary_view.dart`。
- `lib/src/screens/learning/` 收纳学习页聚合摘要、空态、rank explain 筛选列表和候选详情，避免继续扩大 `learning_view.dart`。
- `lib/src/screens/sync/` 收纳同步页预检、设备签名和同步空态组件，避免继续扩大 `sync_view.dart`。
- `lib/src/screens/settings/` 收纳设置页诊断报告预览和诊断导出对话框，避免继续扩大 `settings_view.dart`。
- `lib/src/screens/manager_widgets.dart` 收纳页面共享的 section、metric、key-value row 和状态 badge 组件。
- `lib/src/models/manager_models.dart` 作为模型 barrel 入口；具体模型按 `dictionary`、`learning`、`sync`、`settings`、`diagnostics` 和 `snapshot` 拆分到同目录文件。
- `lib/src/bridge/ffi_dynamic_native_binding.dart` 是真实 Dart FFI bridge 主实现；动态符号加载、status/error 调用规则、ABI struct types 和 Rust view 复制分别拆在同目录的 `ffi_dynamic_native_*` 文件中。
- `lib/src/bridge/ffi_manager_bridge.dart` 保留 `ManagerBridge` 编排；snapshot、dictionary、learning、sync 和 runtime diagnostics 映射拆在 `ffi_manager_*_mapper.dart` helper 中。
- `lib/src/bridge/ffi_manager_native_models.dart` 作为 native DTO barrel 入口；contract、dictionary、learning、sync 和 rank DTO 已按能力拆分。
- `test/screens/` 按页面拆分 dictionary、learning、settings、sync 和 settings diagnostics widget 覆盖；`test/widget_test.dart` 只保留 app shell 级加载失败覆盖。

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

可选 `RADISHLEX_MANAGER_SYNC_SERVER` 会作为首次启动的服务端草案；可选 `RADISHLEX_MANAGER_SETTINGS_FILE` 指向本地 settings JSON，用于保存非 secret 的设置草案。settings JSON 当前写入 `format_version: 1`，部署证据只保存 `local_smoke`、`external_tls`、`backup_restore` 或 `upgrade_rollback` 这类来源标签，不保存日志、证书、token、恢复码、payload bytes 或运行输出。两者都不会启用真实上传、恢复码或设备授权。

## 验证

```bash
../../scripts/check-manager.sh
../../scripts/check-manager-ffi-smoke.sh
```

`check-manager-ffi-smoke.sh` 会构建 `radishlex-ime-ffi` 动态库，在仓库外临时目录创建 SQLite userdb、settings JSON、导入 TSV、读取 import batches 与 rank explain、保存包含部署证据来源标签的设置草案、删除词条、导出词库并导出脱敏诊断报告，用真实 Dart FFI bridge 复验本地管理链路。该 smoke 只使用合成词条，不连接真实同步后端，也不读取真实输入法目录。

如需在 macOS 桌面运行 fixture 版本：

```bash
flutter run -d macos
```
