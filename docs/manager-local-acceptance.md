# Flutter manager 本地验收口径

本文档定义 Flutter manager 在 M2 的本地产品验收口径，并汇总自动化证据与尚待实机执行的证据。读者是维护 `apps/radishlex-manager`、审阅本地个人化能力和决定 M2 是否退出的协作者。本文不定义真实远端同步、恢复码或设备授权成功路径，也不替代 macOS 产品实机验收 runbook。

## 当前结论

截至 2026-07-18，manager 本地产品运行态的实现、自动门禁和 macOS 产品实机验收均已完成，M2 本地个人化 MVP 正式关闭。正常启动默认进入 `product` mode，Release app bundle 携带匹配 ABI 的 native library，平台侧解析固定 userdb/settings 路径并收紧权限，真实 tombstone 与 suppressed 状态可查询和显式恢复，隐私设置通过 macOS `CFPreferences` 写入并读回，启动失败不会静默回退 fixture。`demo` mode 只能由编译期显式开关启用，并持续显示“合成演示数据”。

临时 SQLite 上的双连接测试已经覆盖 migration 所有权、WAL 可见性、输入侧选择、manager 删除、输入侧 tombstone 观察和 manager 显式恢复；Release 产品 bundle 也已在不启动 GUI 的条件下完成签名结构、依赖、ABI、符号和真实 Dart FFI smoke。

clean HEAD `a7e385e` 的正常 Release app 已按 [macOS manager 产品验收 runbook](runbooks/macos-m2-manager-product-acceptance.md) 完成无环境变量启动、固定平台路径、GUI 导入/删除/恢复、重启持久化、输入法与 manager 同库双端可见、页头外部刷新、隐私真实读回与零学习增量、secure 系统路由和最终系统/数据回滚。真实同步继续保持关闭，后续改动不得削弱本页已经关闭的 M2 产品契约。

## 验收范围

纳入 M2 本地验收：

- 默认 `product` 与显式 `demo` 启动模式，以及产品失败可见性。
- app bundle 内 native library 的 ABI contract、必需符号、架构、依赖、签名嵌套和加载路径。
- 固定 userdb/settings 路径、目录 `0700`、文件 `0600`、symlink 拒绝和结构化错误。
- active、suppressed、deleted 三种词条状态；delete 产生 tombstone，恢复只能由明确用户动作触发。
- 本地词库查看、搜索、审计、导入检查、导入、导出和批次历史。
- 学习聚合摘要与 `rank explain`；不展示 P1 原始事件。
- manager 与输入 runtime 共享 Rust/userdb 真相源时的 WAL、busy、migration 和重启行为。
- manager 保持运行时，通过页头刷新重新加载 bridge snapshot，并观察输入 runtime 写入的最新聚合状态。
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
| 产品 bundle 可加载真实 Rust 能力 | Xcode 构建阶段嵌入 native library，校验 ABI v6、owner-thread、panic boundary、架构、必需符号和依赖；Release bundle smoke 使用其中的 dylib。 | `embed-manager-native-library.sh`、`check-manager-product.sh`、`ffi_dynamic_native_binding.dart` |
| 用户能管理真实本地词条 | UI 和 FFI 覆盖 active/suppressed/deleted、删除、tombstone 查询、明确恢复、导入导出、按本地 batch id 关联导入审计与重启后的状态保持。 | `dictionary_test.dart`、`ffi_manager_bridge_test.dart`、`ffi_bridge_smoke.dart` |
| 输入法与 manager 共享数据语义 | 独立 `UserDb` 连接覆盖八路并发 schema 初始化、短时初始化锁等待、WAL 可见性、选择、删除、防复活和恢复；busy timeout 在任何 schema/integrity SQL 前生效，首次 WAL 协商只在固定预算内重试锁竞争。页头刷新重新调用 bridge snapshot，widget 回归覆盖输入侧外部更新后的可见性。 | `ime-userdb` store tests、`widget_test.dart`、`cargo test -p radishlex-ime-userdb` |
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

## M2 实机证据与 M3 交接

- 正常 Release app 直接显示 `product` / `local_only`，没有 demo 标识；真实 v3 userdb 迁移到 v4，固定合成导入审计、deleted/suppressed 显式恢复和完整重启均保持一致。
- 输入法外部提交后，manager 页头刷新无需重启即可观察聚合；manager delete 后输入侧普通选择不能复活 tombstone，explicit restore 后新 session 可重新学习，manager/IMK 重启未产生非预期 `SQLITE_BUSY` 或 migration 漂移。
- 隐私模式真实读回 true 时普通 TextEdit 提交全库零增量；恢复正常模式后一次提交只增加一次 selection/frequency。secure 期间 macOS 不向 RadishLex 路由，来源监视无 RadishLex，数据库零增量；该项不冒充 controller `policy_blocked` 实机通过。
- Authorization B/C 后，TIS、bundle、Rime、进程、隐私键、测试 userdb、settings、sidecars、receipts 和 M2 临时目录全部恢复基线，父目录 empty/`0755`。全程未读取 P1 原始行或数据库正文。
- cleanup receipt 的跨登录成对 `st_dev` 漂移已由共享 helper 的精确兼容规则和 M2/R01B contract 关闭；单侧 device drift、inode/权限/白名单漂移仍失败关闭。

M3 可以依赖上述本地产品能力，但不能在 Flutter 层复制同步、加密、设备授权或密钥真相源。下一批先关闭设备签名算法 profile 与 macOS 生产私钥 backend，再进入真实产品 sync orchestration 和 `ManagerBridge` 命令。

## M3 停止线

- `apple-keychain-v1`、Android Keystore、恢复码、设备授权、设备撤销和 key epoch 的生产证据仍按 M3 专题推进。
- 发布级目标部署和真实 sync client 尚未进入用户产品链路。
- 安全证据齐备前，不打开真实用户同步开关，也不把 `preflight_ready` 表述为可同步。
