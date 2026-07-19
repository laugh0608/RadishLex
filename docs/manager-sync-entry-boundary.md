# Manager 同步入口边界

本文面向 RadishLex Manager、Rust bridge 与同步功能维护者，固定 Manager 在 M3 真实同步实现前后的职责、敏感材料边界和产品停止线。本文不定义 C ABI 命令草案、审批流程、部署证据包格式、同步状态机、同步协议或密码学细节；Rust 编排以 `docs/sync-orchestration.md` 为准，协议和密钥语义分别以 `docs/privacy-sync.md`、`docs/sync-key-management.md`、`docs/production-recovery-flow.md` 和 `docs/sync-server-api-storage.md` 为准。

## 当前结论

- 当前 Manager 只展示同步配置草案、本地 readiness、连接健康摘要、恢复与设备流程的不可用原因。
- 真实远端同步、恢复码生成与输入、设备加入授权、设备撤销和密钥轮换没有产品执行入口。
- `ManagerBridge` 当前不提供上述同步命令；缺少能力本身就是产品关闭证据，不使用 future command preview 或审批状态机模拟接口。
- 即使 endpoint、平台 backend、部署证据和 readiness 摘要均显示 ready，用户同步入口仍保持关闭，直到 M3 退出条件满足。
- manager Release native library 可以包含普通 DPK 与 Secure Enclave P-256 backend 及只返回固定 flags 的独立产品 validation ABI；这些底层 validation ABI 不直接进入 Dart binding。Manager 只通过下述独立 status-only 业务摘要读取脱敏结果，不能据此解锁按钮。

## Status-only 产品摘要

M3 允许一条不带入参、只读且不访问系统密钥条目的 `radishlex_manager_sync_product_status` C ABI。它随现有 `loadSnapshot` 进入 Manager，不增加 `ManagerBridge` 命令方法，也不创建、读取、签名、解封或删除 Keychain / Secure Enclave 项目。

该摘要只包含固定版本与数值枚举：signing backend / algorithm、编译与静态运行时 capability、`can_create` / `can_sign`、`exportable` / `hardware_backed`、signing 产品资格、独立 key-agreement backend 编译与实机资格、组合 `product_qualified`、`user_sync_enabled` 和首个稳定 blocker。它不得包含 device id、key id、公钥、路径、OSStatus、平台错误正文、signature、wrapped material、master key、shared secret、nonce、HTTP 内容或任意自由文本。

规则固定如下：

- signing 状态可以复用不触发系统调用的 backend metadata；key-agreement 在未完成独立实机资格前只能报告 compiled，不能继承 signing backend 的运行时或 hardware-backed 证据。
- `product_qualified` 只有 signing 与 key-agreement 两条独立资格同时成立时才可为 `1`；当前必须为 `0`。`user_sync_enabled` 还受阶段停止线约束，当前固定为 `0`。
- blocker 按 signing 未编译、signing runtime 不可用、signing 未获产品资格、key-agreement 未编译、key-agreement 实机资格缺失、key-agreement 产品资格缺失、当前阶段仍关闭的顺序选择首项；`none` 只允许全部条件成立时出现。
- C ABI 使用标准 `RadishLexStatusCode` 与受控 `error_out`；空输出指针返回 `InvalidArgument`。Dart 对未知 schema、enum、非 `0/1` flag 或自相矛盾组合必须降级为 `native_sync_product_status_invalid`、`backendId = unavailable`、`productionGate = blocked`，保持 Manager 其余本地管理能力可用。
- Dart 只把 allowlist 后的 backend 与 blocker 映射为 `DeviceSecuritySummary`，不持久化原始结构；settings、readiness JSON、diagnostics 和 widget 继续只看到稳定状态码。

## UI 职责

Manager 可以：

- 编辑并持久化非敏感同步配置，例如 endpoint、隐私模式、是否保留配置和“凭据是否已配置”。
- 展示 Rust / bridge 提供的结构化状态码、阻塞原因、错误类别和脱敏连接健康摘要。
- 展示恢复码与设备授权流程当前为何不可用，以及进入后续产品阶段所需的真实条件。
- 导出不含用户词、文件路径、凭据、恢复材料或传输正文的诊断摘要。

Manager 不可以：

- 成为同步状态、设备身份、密钥、恢复记录或 tombstone 的真相源。
- 在 Dart 模型中复制 Rust 同步、加密、授权或冲突合并逻辑。
- 用 fixture、readiness、local smoke 或 deployment label 打开真实同步按钮。
- 在 FFI 加载失败时静默回退为 fixture 产品模式。
- 用不可执行 request / result shape、approval、migration review、fake replay 或 evidence bundle 代替真实实现。

## 状态与错误边界

- UI 只消费稳定、可枚举的状态码和安全摘要；未知 native 状态必须降级为通用安全类别。
- 原始 provider exception、HTTP body、请求体、响应体、payload bytes、真实路径和系统密钥错误正文不得进入 UI 或 diagnostics。
- bridge 加载、参数、存储、同步预检和未知内部错误必须保持可区分，不得吞错或伪装为成功。
- 隐私模式优先于 endpoint、readiness 和连接状态；启用后入口状态必须为 `sync_disabled_by_policy`，且 `userSyncEnabled` 为 false。
- 当前关闭态通过 capability 缺席、禁用按钮、`userSyncEnabled == false` 和明确停止线表达。

## Transient secret 生命周期

恢复码、短码、访问令牌、设备私钥、签名、wrapped material 和同等级材料都属于 transient secret 或更高敏感级别。

后续 M3 若增加真实交互，必须同时满足：

- 仅由用户显式操作触发，作用域限制在一次调用或当前可见 modal。
- 提交、取消、导航离开、超时和错误后立即清除。
- 不进入 settings JSON、路由参数、长期 Dart state、剪贴板历史、日志、analytics、crash report、测试 golden 或 fixture。
- diagnostics 只记录“已配置 / 未配置”、状态码和脱敏错误类别，不记录明文或可逆摘要。
- Rust / FFI / Dart 的所有权、复制、释放、线程和 panic 边界必须在真实命令实现时由真实 contract test 验证。

当前 Manager 没有恢复码、短码、私钥、签名或 wrapped material 的输入字段与执行方法；设置文件只保存 `access_token_configured` 布尔值，不保存 token 文本。

Apple P-256 产品进程 gated smoke 只由五个显式命令行场景之一与环境门触发：DPK 正常生命周期、预期 denied 创建、locked 前置、locked 签名探测、解锁后清理。正常生命周期和前置场景在 native 内使用 synthetic canonical/signature 并完成 Rust 验签；正常生命周期额外调用短生命周期 Go verifier。smoke schema v4 返回 Swift 的只有固定 scenario、result、error category/detail、数值 OSStatus 和布尔摘要，不含 CFError 文本。private key、public key、canonical bytes、signature bytes 不得进入 Dart、Flutter method channel、settings 或 diagnostics。普通 manager 启动不访问该 Keychain 路径；脚本不锁定、解锁或改写 Keychain 搜索列表；InputMethodKit 不参与同步密钥或签名。

Secure Enclave P-256 使用独立环境门、native symbol 与六个显式场景，额外覆盖 unsupported create，并在正常生命周期内要求 private external representation 失败。它复用同一固定 26-word 脱敏摘要布局，但拥有独立 schema version；qualification lifecycle、ad-hoc denied 与真实设备锁屏 locked 已通过，当前运行时和 hardware-backed 字段开放，unsupported、产品资格与用户同步 gate 继续关闭。Dart、普通 manager 启动和 InputMethodKit 同样不接触该路径。

## 产品停止线

在 M3 的协议、安全和真实部署证据全部满足前：

- `启用同步` 保持禁用。
- 不新增恢复码生成 / 输入、join request、授权成功、撤销或轮换的可执行按钮。
- 不新增 manager 同步命令 C ABI 或远端写入调用；上述 status-only ABI 不属于同步命令。
- 本地 Docker、localhost、fixture、readiness ready 和合成 smoke 只能用于开发验证，不能解锁产品入口。
- 服务端继续被视为不可信，输入热路径不得依赖网络。

关闭态 Rust orchestration 可以在 Manager 入口继续禁用时独立实现。只有该 service 稳定且生产 backend 资格另行通过后，才重新基于当时的 Rust sync / crypto API、服务端协议和平台密钥 backend 设计窄命令接口；不恢复本次归档的 review-only DTO 或审批目录。

## 当前验证归属

- 关闭态与隐私优先级：`test/models/manager_sync_entry_gate_test.dart`、`test/models/manager_sync_transient_secret_interaction_test.dart`
- settings 不保存敏感材料：`test/ffi_manager_bridge_test.dart`、`test/models/manager_sync_transient_secret_interaction_test.dart`
- diagnostics 脱敏与错误可见性：`test/screens/settings_diagnostics_test.dart`、`test/models/manager_sync_transient_secret_interaction_test.dart`
- UI 禁用入口：`test/screens/sync_test.dart`、`test/screens/settings_test.dart`
- native 产品摘要 contract 与 fail-closed 映射：`crates/ime-ffi/tests/input_header_contract.rs`、`test/ffi_manager_bridge_test.dart`

这些测试证明当前能力安全关闭，不代表 M3 同步产品已实现。
