# RadishLex Manager

RadishLex Manager 是萝卜词核的 Flutter 本地管理端。macOS 正常构建默认进入 `product` mode：从 app bundle 的 `Contents/Frameworks` 加载匹配 ABI 的 `libradishlex_ime_ffi.dylib`，通过 macOS 原生 bridge 解析固定 Application Support 路径，并与 InputMethodKit 薄壳共享同一 `userdb.sqlite3`。native library、路径、权限或 userdb 初始化失败会显示结构化启动错误，不会退回 fixture。

合成 fixture 只允许通过编译期 `RADISHLEX_MANAGER_MODE=demo` 显式启用，运行期间持续显示“合成演示数据”横幅。当前本地产品能力包括：

- 本地 userdb 词条管理视图，包含搜索过滤、空态、词条审计详情、导入历史筛选 / 排序 / 批次联动审计、删除确认和操作失败分类提示。
- 本地学习状态聚合摘要，包含 P1 原始事件不展示说明。
- `rank explain` 非敏感摘要，包含筛选和候选贡献项详情。
- `sync preflight` 状态、对象分类、设备门禁和生产不可用原因。
- 同步服务连接健康摘要，包含 endpoint 状态、access token 存在性、transport 分类、server state 摘要、只读探测来源和错误分类。
- 恢复码与设备授权只读准备态，包含恢复码保存确认、恢复记录、join request、授权包前置条件、设备撤销、丢失设备风险提示和 key epoch 状态。
- 恢复码 setup / restore、设备 join / revocation 的 readiness 聚合摘要，以及 `SyncInteractionEntryPlan` 派生的非执行操作进入计划。
- recovery setup / restore 的可见层状态码，包含一次性展示占位、保存确认、恢复输入占位、恢复记录查询、失败限速和设备登记状态；这些状态只用于 UI / diagnostics 说明，不显示、输入、保存或传递真实 secret。
- loopback HTTPS 合成同步资格 run，包含一次性 token/CA、运行阶段、取消、脱敏错误和清理摘要；它不读取真实 userdb，不触发平台 key 操作，也不打开真实用户同步。
- 脱敏诊断摘要预览 / 导出，预览支持字段分组、筛选和复制脱敏文本。
- 自部署服务端配置草案、sync gate 草案预览和状态来源。

真实远端同步、恢复码和设备授权产品成功路径尚未开放；平台私钥只通过脱敏产品状态与独立 gated validation 取证，不由普通 Manager UI 执行。相关停止线见仓库根 `docs/manager-ui-boundary.md`；本地验收口径见 `docs/manager-local-acceptance.md`；settings JSON 与诊断报告字段见 `docs/manager-settings-diagnostics.md`。
当前 FFI bridge 覆盖 active / suppressed 词条与 deleted tombstone 查询、delete、explicit restore、用户词库 inspect / import / export、import batches、learning status 摘要、rank explain 摘要、sync preflight 摘要、隔离资格 run、设置持久化和脱敏诊断报告导出。`rank explain` 区域通过专用 `ime-ffi` ABI 读取单候选贡献项，Flutter 只展示复制后的非敏感摘要，并支持筛选和候选详情；学习页只展示聚合计数和贡献信号，不展示 P1 原始选择事件、原始输入历史或应用窗口信息。词库页会显示导入检查、词条 key / source / status / import batch / tombstone / sync 分类审计详情、导入历史筛选 / 排序 / 批次联动审计、本地 sync preflight 影响摘要、导入 / 导出结果摘要、删除确认、独立恢复确认和操作失败分类提示；同步页会显示 sync gate 状态来源、本地 P2 对象分类、local-only 事件计数、服务连接健康、设备 backend capability、production gate 阻断原因、恢复码准备清单、设备授权准备清单和只读操作进入计划，真实同步按钮继续禁用；设置页会显示配置来源诊断和同步门禁草案预览，可记录 allowlist 形式的部署证据来源标签，可回填净化后的 `sync_connection_health.v1` 摘要，并可按诊断字段索引分组预览、筛选、复制和导出包含 gate source / stop line 但不含用户词、文件路径、token、恢复码、签名、wrapped material 或 payload bytes 的诊断摘要。macOS 隐私模式通过 InputMethodKit 使用的 CFPreferences 域写入并读回，不以 settings JSON 镜像作为真相源。bridge 失败按操作和分类展示结构化错误码，不把 native 错误明细透传给 widget 层。sync gate 状态由设置草案、隐私模式、平台私钥 backend gate、部署证据来源草案、连接健康摘要和恢复 / 授权只读 readiness 共同派生；真实远端同步、恢复码生成 / 输入、join request 创建、设备授权成功和设备撤销 UI 继续关闭。

## 页面与操作说明

当前 manager 页面围绕本地可审计管理能力组织，不进入输入热路径：

- 词库页用于查看本地 userdb 的 active / suppressed 词条和 deleted tombstone、按输入码 / 文本 / 来源筛选、查看词条审计详情、执行删除与 explicit restore 独立确认、预览导入检查、导入用户词库、导出用户词库，并查看 import batches 与本地 sync preflight 影响摘要。导入和导出都必须由用户显式触发；普通导入不得复活 tombstone；导入错误、删除 / 恢复失败和文件操作失败按结构化分类显示。
- 学习页只展示聚合学习状态和 `rank explain` 非敏感贡献信号。页面不得展示 P1 原始 selection event、原始输入历史、窗口标题或应用上下文明细。
- 同步页展示本地 sync preflight、P2 对象分类、local-only 事件计数、连接健康、设备 backend capability、production gate 阻断原因、恢复码 / 设备授权准备清单、只读操作进入计划和 recovery visible-layer 状态码；另有与“启用同步”分区的 loopback HTTPS 合成资格 run。`preflight_ready` 和资格成功都不代表真实远端同步已开放。
- 设置页保存非 secret settings draft，包含自部署服务端地址草案、诊断导出草案、保留同步配置草案、部署证据来源标签和净化后的连接健康摘要。隐私模式保存时写入 `org.radishlex.inputmethod.macos` 的 `RadishLexPrivacyMode` 偏好并读回确认；settings JSON 中的同名字段只作镜像。保存本地设置会重新派生 sync gate 状态，但不会连接真实远端。
- 设置页诊断摘要预览只展示脱敏字段索引和完整脱敏文本。section 筛选和关键字筛选只影响对话框里的字段列表；复制按钮始终复制完整 `manager.diagnostics.v1` 脱敏文本，导出也必须保持同一份脱敏摘要语义。

## 代码结构

- `lib/src/screens/manager_home_screen.dart` 保留 snapshot 加载、导航壳层、页面选择和统一提示入口。
- `lib/src/screens/manager/manager_home_actions.dart` 收纳跨页 bridge 操作编排、对话框调用和结构化结果文案。
- `lib/src/screens/dictionary_view.dart`、`learning_view.dart`、`sync_view.dart` 和 `settings_view.dart` 分别承载词库、学习、同步和设置页面。
- `lib/src/screens/dictionary/` 收纳词库页导入历史区域、删除确认、导入检查和导出对话框，避免继续扩大 `dictionary_view.dart`。
- `lib/src/screens/learning/` 收纳学习页聚合摘要、空态、rank explain 筛选列表和候选详情，避免继续扩大 `learning_view.dart`。
- `lib/src/screens/sync/` 收纳同步页预检、连接健康、恢复码准备态、设备授权准备态、设备签名、合成资格交互和同步空态组件，避免继续扩大 `sync_view.dart`。
- `lib/src/screens/settings/` 收纳设置页诊断报告预览和诊断导出对话框，避免继续扩大 `settings_view.dart`。
- `lib/src/screens/manager_widgets.dart` 收纳页面共享的 section、metric、key-value row 和状态 badge 组件。
- `lib/src/models/manager_models.dart` 作为模型 barrel 入口；具体模型按 `dictionary`、`learning`、`sync`、`settings`、`diagnostics` 和 `snapshot` 拆分到同目录文件。
- `lib/src/bridge/ffi_dynamic_native_binding.dart` 是真实 Dart FFI bridge 主实现；动态符号加载、status/error 调用规则、ABI struct types 和 Rust view 复制分别拆在同目录的 `ffi_dynamic_native_*` 文件中。
- `lib/src/bridge/ffi_manager_bridge.dart` 保留 `ManagerBridge` 编排；snapshot、dictionary、learning、sync 和 runtime diagnostics 映射拆在 `ffi_manager_*_mapper.dart` helper 中。
- `lib/src/bridge/ffi_manager_sync_readiness_mapper.dart` 固定 `manager_sync_readiness.v1` 非敏感摘要到 Dart readiness model 的准备层映射，不改变 `ManagerBridge` contract 或 C ABI。
- `lib/src/bridge/ffi_manager_sync_qualification_mapper.dart` 校验资格 run 的 state/phase/error/count/cleanup 快照；unknown enum、非 canonical flag 或不完整成功条件必须失败关闭。
- `lib/src/bridge/ffi_manager_native_models.dart` 作为 native DTO barrel 入口；contract、dictionary、learning、sync 和 rank DTO 已按能力拆分。
- `test/screens/` 按页面拆分 dictionary、learning、settings、sync 和 settings diagnostics widget 覆盖；`manager_home_actions_test.dart` 固定跨页 action helper 的结构化结果文案和 failure 分类；`test/widget_test.dart` 只保留 app shell 级加载失败覆盖。

## 产品启动与升级门禁

macOS 正常 product mode 在 `applicationWillFinishLaunching` 最前先执行 ABI v9 `radishlex_product_install_startup_gate`，再执行兼容保留的数据 `radishlex_product_upgrade_startup_gate`。平台层只从用户域解析固定 `Application Support/RadishLex` 和当前 effective uid；外层运行身份由 FFI 从当前 executable 的固定 bundle 形成。两层检查都发生在 Flutter delegate、settings、userdb 和所有 Manager 业务初始化之前。

data root 不存在、两层状态目录/receipt 不存在，或两层 receipt 均处于允许启动的终态时才可继续。active guard、非终态或损坏 receipt、中断 artifact、未知对象、unsafe root/state、identity drift、completed remove、FFI 失败或未知 result 都直接退出，不能通过创建目录、修改权限、删除 receipt/sidecar 或切换到 demo fixture 绕过。失败日志只包含稳定 decision/error/state 数值，不输出路径或数据内容。

Apple P-256 与 Secure Enclave key-agreement 的显式 gated product smoke 是独立的早退出自检模式，不进入普通 Manager bootstrap；它们不能作为绕过 startup gate 启动产品 UI 的入口。

Manager bundle 另携带 `Contents/Helpers/RadishLexUpgradeValidationHost`。无参数模式固定读取 `.radishlex-upgrade-v1/migration-candidate.sqlite3` 与可选 `source-settings.json`；唯一参数 `--post-switch` 固定读取最终 `userdb.sqlite3` 与 `manager-settings.json`。两种模式都复用 bundle 内 native library 执行 current-schema 管理查询和 settings format v1 兼容检查，不接受调用方路径、不启动 Flutter、不写数据库/settings，也不产生 WAL/SHM/journal；数据库字节变化或 sidecar 残留均失败。

## FFI bridge

macOS 正常 product 构建使用仓库稳定入口：

```bash
../../scripts/build-manager-macos-product.sh
```

Xcode 构建阶段会编译 `radishlex-ime-ffi`；macOS 产品 dylib 显式启用 `apple-keychain` feature，再修正 install name，检查目标架构、依赖与 manager 所需 symbol 集，把库复制到 app bundle 的 `Contents/Frameworks` 后签名。Dart 启动时读取 `radishlex_ffi_contract`，要求 ABI v9、owner-thread policy 和 panic boundary 与 manager 预期一致。普通 DPK 与 Secure Enclave P-256 status/product smoke symbol 只服务原生 gated validation，Dart 不直接绑定；现有 snapshot 只读取 `radishlex_manager_sync_product_status` 的固定脱敏状态。同步页另提供只连接 loopback HTTPS、只使用合成数据的资格 run，不触发系统 key 操作，也不返回 key、canonical、signature、wrapped material、payload 或 HTTP body。

`radishlex_manager_sync_product_status` 把 signing 与 key-agreement 作为两条独立资格链：`product_qualified` 只有在两者都通过时才为真，`user_sync_enabled` 是另一个产品策略开关，不能由资格结果推导或自动打开。`blocker` 按固定优先级返回首个失败原因；Manager 只负责把这些 enum/boolean 映射为状态说明，不执行创建、读取、签名、derive 或删除操作。完整字段与常量见 [Manager 同步产品状态参考](../../docs/manager-sync-product-status.md)。

product mode 不读取 `RADISHLEX_MANAGER_DB`、`RADISHLEX_MANAGER_FFI_LIBRARY`、`RADISHLEX_MANAGER_SETTINGS_FILE` 或其他 shell 路径环境变量。macOS 固定路径为：

- `~/Library/Application Support/RadishLex/userdb.sqlite3`
- `~/Library/Application Support/RadishLex/manager-settings.json`
- `<app>/Contents/Frameworks/libradishlex_ime_ffi.dylib`

M2 本地产品 profile 不启用 App Sandbox，以便 manager 与已经验收的 InputMethodKit 薄壳共享单一 userdb。父目录保持 `0700`，新建 settings 文件保持 `0600`；schema migration 只由 Rust `ime-userdb` 打开流程执行。未来若转 App Group，必须先补迁移、回滚与双端回归证据。

开发合成 UI 时必须显式使用 demo mode：

```bash
flutter run -d macos --dart-define=RADISHLEX_MANAGER_MODE=demo
```

demo mode 只使用合成 fixture，并持续显示“合成演示数据”横幅。settings JSON 当前写入 `format_version: 1`，部署证据只保存 `local_smoke`、`external_tls`、`backup_restore` 或 `upgrade_rollback` 这类来源标签，不保存日志、证书、token、恢复码、payload bytes 或运行输出。任何本地设置都不会启用真实上传、恢复码或设备授权。

本地同步服务启动后，可用仓库根 `scripts/check-sync-server-connection-health.sh` 生成 `sync_connection_health.v1` 摘要，再在设置页回填。Manager 只保存净化后的 `sync_connection_health_summary` 子对象，不保存原始 JSON、完整 endpoint、token、请求 / 响应体、证书或 payload bytes。

当前 dynamic library smoke 还会确认 `radishlex-ime-ffi` 未导出已退役的 review-only sync command symbol。隔离资格 `start/poll/cancel/free` executor 最初由 ABI v7 引入，当前 ABI v9 兼容保留，并增加产品 startup/validation contract；真实用户数据同步、恢复码生成 / 输入、join request 创建、授权成功和设备撤销路径仍保持关闭。被拒绝的旧符号名只作为 smoke denylist 保留，不是待实现 API。资格 request、状态机、错误与清理契约见 [ime-sync-runtime 组件说明](../../crates/ime-sync-runtime/README.md)；产品升级 FFI 调用顺序见 [FFI 平台调用契约](../../docs/runbooks/ffi-platform-call-contract.md)。历史预演材料统一归档在仓库根 `docs/archive/review-only-manager-sync/`，不作为当前设计或实现契约。

## 验证

```bash
../../scripts/check-manager.sh
../../scripts/check-manager-ffi-smoke.sh
../../scripts/check-manager-product.sh
```

M2 本地能力验收范围、退出标准映射和隐私检查见仓库根 `docs/manager-local-acceptance.md`。

`check-manager-ffi-smoke.sh` 会构建开发态 `radishlex-ime-ffi`，在仓库外临时目录创建 SQLite userdb、settings JSON、导入 TSV、读取 import batches 与 rank explain、执行删除 / 重启 / explicit restore / 再重启、导出词库并导出脱敏诊断报告，用真实 Dart FFI bridge 复验本地管理链路；同时确认当前动态库未导出已退役的 review-only sync command symbol。`check-manager-product.sh` 额外构建正常 macOS app bundle，验证非 sandbox entitlements、嵌入 dylib、签名、startup gate 与 Manager upgrade validation symbol/helper、两类 Apple P-256 validation symbol、只读 native status host、资格 run 必需 symbol、不可达 loopback 失败/清理路径和 bundle 内真实 Dart FFI smoke，全程不启动 GUI、不执行 upgrade helper、不访问真实 Application Support 或 Keychain/Secure Enclave。

实际产品进程 DPK 验证只能在单独授权后使用仓库根 `scripts/run-manager-apple-keychain-p256-product-smoke.sh`。脚本接受正常生命周期、预期 denied、locked 前置、locked 探测和解锁后清理五个授权参数；它不会自行锁定、解锁或改写 Keychain 搜索列表。locked 矩阵必须按 runbook 分阶段执行，并在解锁后完成清理。

DPK 正常生命周期必须先使用 `scripts/build-manager-macos-dpk-qualified-product.sh` 生成 provisioning-backed Apple Development bundle。该入口要求显式授权、Team ID 和 signing identity，并核对 embedded profile、application identifier 和默认 access group；它可能联系 Apple Developer 服务，但不会启动 app 或创建 Keychain item。默认 `check-manager-product.sh` 仍只生成 ad-hoc bundle，用于仓库门禁和预期 denied。

当前普通 DPK P-256 软件 key 已通过 qualification 产品生命周期，status 报告编译/运行时可用且 `exportable=true`；因此 `product_qualified` 与用户同步 gate 保持 false。下一生产候选为独立 Secure Enclave backend，不允许从普通 DPK 静默 fallback 或原地升级。

Secure Enclave qualification 产品 lifecycle、不可导出、hardware-backed、ad-hoc denied、真实设备锁屏 locked 与 cleanup 已通过，并按受支持 macOS 主路径完成产品资格评审；`scripts/run-manager-apple-secure-enclave-p256-product-smoke.sh` 继续提供 locked 三段和 unsupported 场景，locked probe 启动前保留 20 秒手动锁屏窗口。任何执行都会启动产品并访问 Secure Enclave/Keychain，必须单独授权；unsupported 尚需真实不支持 Secure Enclave 的目标环境并延期补测，用户同步 gate 仍保持关闭。
