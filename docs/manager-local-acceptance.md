# Flutter manager 本地验收口径

本文档定义 Flutter manager 在 M2 的本地产品验收口径，并汇总自动化证据与尚待实机执行的证据。读者是维护 `apps/radishlex-manager`、审阅本地个人化能力和决定 M2 是否退出的协作者。本文不定义真实远端同步、恢复码或设备授权成功路径，也不替代 macOS 产品实机验收 runbook。

## 当前结论

截至 2026-07-18，manager 本地产品运行态的实现和自动门禁已经完成：正常启动默认进入 `product` mode，Release app bundle 携带匹配 ABI 的 native library，平台侧解析固定 userdb/settings 路径并收紧权限，真实 tombstone 与 suppressed 状态可查询和显式恢复，隐私设置通过 macOS `CFPreferences` 写入并读回，启动失败不会静默回退 fixture。`demo` mode 只能由编译期显式开关启用，并持续显示“合成演示数据”。

临时 SQLite 上的双连接测试已经覆盖 migration 所有权、WAL 可见性、输入侧选择、manager 删除、输入侧 tombstone 观察和 manager 显式恢复；Release 产品 bundle 也已在不启动 GUI 的条件下完成签名结构、依赖、ABI、符号和真实 Dart FFI smoke。

这些证据证明实现已具备进入真实产品验收的条件，但尚不单独构成 M2 退出：仍需按 [macOS manager 产品验收 runbook](runbooks/macos-m2-manager-product-acceptance.md)，在单独授权下从正常 Release app 启动，无 shell 环境变量地复验固定平台路径、重启持久化、隐私键真实读回，以及输入法与 manager 同库运行时的双端可见性。真实同步继续保持关闭。

## 验收范围

纳入 M2 本地验收：

- 默认 `product` 与显式 `demo` 启动模式，以及产品失败可见性。
- app bundle 内 native library 的 ABI contract、必需符号、架构、依赖、签名嵌套和加载路径。
- 固定 userdb/settings 路径、目录 `0700`、文件 `0600`、symlink 拒绝和结构化错误。
- active、suppressed、deleted 三种词条状态；delete 产生 tombstone，恢复只能由明确用户动作触发。
- 本地词库查看、搜索、审计、导入检查、导入、导出和批次历史。
- 学习聚合摘要与 `rank explain`；不展示 P1 原始事件。
- manager 与输入 runtime 共享 Rust/userdb 真相源时的 WAL、busy、migration 和重启行为。
- macOS 隐私模式的系统偏好写入、读回、失败回滚和输入侧零学习增量。
- settings draft、脱敏诊断、同步 readiness、关闭态和不可用原因解释。

不纳入 M2 验收：

- 真实远端同步上传或下载。
- 恢复码生成、轮换、撤销、恢复加入或设备授权成功路径。
- 平台私钥 backend 的 M3 生产可用性。
- App Store sandbox、App Group 迁移、公证、安装升级和最终发布包；这些属于 M4。
- 第二真实平台。

## 退出标准映射

| M2 能力 | 当前证据 | 证据入口 |
| --- | --- | --- |
| 产品启动不伪装成功 | 默认 product；native/path/userdb/ABI 失败进入 `UnavailableManagerBridge`；fixture 仅由显式 demo 构建启用并显示常驻标识。 | `manager_bridge_factory.dart`、`unavailable_manager_bridge.dart`、`widget_test.dart`、`./scripts/check-manager.sh` |
| 产品 bundle 可加载真实 Rust 能力 | Xcode 构建阶段嵌入 native library，校验 ABI v5、owner-thread、panic boundary、架构、必需符号和依赖；Release bundle smoke 使用其中的 dylib。 | `embed-manager-native-library.sh`、`check-manager-product.sh`、`ffi_dynamic_native_binding.dart` |
| 用户能管理真实本地词条 | UI 和 FFI 覆盖 active/suppressed/deleted、删除、tombstone 查询、明确恢复、导入导出、按本地 batch id 关联导入审计与重启后的状态保持。 | `dictionary_test.dart`、`ffi_manager_bridge_test.dart`、`ffi_bridge_smoke.dart` |
| 输入法与 manager 共享数据语义 | 独立 `UserDb` 连接覆盖八路并发 schema 初始化、短时初始化锁等待、WAL 可见性、选择、删除、防复活和恢复；busy timeout 在任何 schema/integrity SQL 前生效，首次 WAL 协商只在固定预算内重试锁竞争。 | `ime-userdb` store tests、`cargo test -p radishlex-ime-userdb` |
| 隐私设置作用于输入 runtime | Flutter 通过 MethodChannel 调用 macOS `CFPreferences` 的 CurrentUser/AnyHost 层，保存后读回；settings/权限失败会回滚。 | `MainFlutterWindow.swift`、`method_channel_manager_platform_control.dart`、对应 Flutter tests |
| P1 与诊断边界不扩张 | 学习页只展示聚合；诊断不包含用户词、真实路径、token、恢复码、密钥或 payload bytes。 | `settings_diagnostics_test.dart`、`ffi_manager_bridge_test.dart`、产品 smoke |
| 真实用户同步保持关闭 | 设置和同步页只保存本地草案、展示 readiness 与阻塞原因；上传主操作保持禁用。 | `sync_test.dart`、`settings_test.dart`、`docs/manager-ui-boundary.md` |

## 自动验证入口

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
./scripts/check-manager-product.sh
./scripts/build-manager-macos-product.sh
git diff --check
./scripts/check-repo.sh
```

- `check-manager.sh` 执行 Dart format、Flutter analyze 和 unit/widget tests。
- `check-manager-ffi-smoke.sh` 用临时合成数据复验开发期真实 FFI bridge。
- `check-manager-product.sh` 构建 Release app，检查非 sandbox M2 profile、签名、bundle native library、ABI/符号/架构/依赖，并直接对 bundle dylib 执行产品 smoke；不启动 GUI。
- `build-manager-macos-product.sh` 形成正常 Release app 并复核签名和 native 依赖，供后续授权实机验收使用。
- `check-repo.sh` 覆盖仓库文本、文档、Rust、Go 与产品运行态 contract。

自动测试只使用合成词、临时 SQLite、临时 settings JSON 和虚构状态，不得读取真实 P1 原始行或数据库正文。

## 尚待真实产品验收

M2 当前只剩产品实机证据，不再以继续添加 fixture 或复制业务逻辑替代：

- 从正常 Release app 直接启动，确认没有 demo 标识、无需 shell 环境变量，并使用固定平台路径。
- 在 GUI 完成合成词条导入、删除、deleted/suppressed 显式恢复和 app 重启持久化。
- 通过 GUI 切换隐私模式，复核 `CFPreferences` 实际读回与输入侧零学习增量。
- 输入法与 manager 同时连接同一测试 userdb，复核双端状态可见、无非预期 `SQLITE_BUSY`，并在进程重启后保持一致。
- 按 runbook 恢复 TIS、bundle、Rime、进程、隐私键、测试 userdb、settings 和目录权限基线，全程不读取 P1 原始行或数据库正文。

实机证据全部通过后才能关闭 M2 并进入 M3；若任一项失败，应回到对应 Rust、FFI、Flutter 或平台层修复，并重跑匹配门禁。

## M3 停止线

- `apple-keychain-v1`、Android Keystore、恢复码、设备授权、设备撤销和 key epoch 的生产证据仍按 M3 专题推进。
- 发布级目标部署和真实 sync client 尚未进入用户产品链路。
- 安全证据齐备前，不打开真实用户同步开关，也不把 `preflight_ready` 表述为可同步。
