# Manager 同步入口边界

本文面向 RadishLex Manager、Rust bridge 与同步功能维护者，固定 Manager 在 M3 真实同步实现前后的职责、敏感材料边界和产品停止线。本文不定义 C ABI 命令草案、审批流程、部署证据包格式、同步协议或密码学细节；协议和密钥语义分别以 `docs/privacy-sync.md`、`docs/sync-key-management.md`、`docs/production-recovery-flow.md` 和 `docs/sync-server-api-storage.md` 为准。

## 当前结论

- 当前 Manager 只展示同步配置草案、本地 readiness、连接健康摘要、恢复与设备流程的不可用原因。
- 真实远端同步、恢复码生成与输入、设备加入授权、设备撤销和密钥轮换没有产品执行入口。
- `ManagerBridge` 当前不提供上述同步命令；缺少能力本身就是产品关闭证据，不使用 future command preview 或审批状态机模拟接口。
- 即使 endpoint、平台 backend、部署证据和 readiness 摘要均显示 ready，用户同步入口仍保持关闭，直到 M3 退出条件满足。
- manager Release native library 可以包含普通 DPK 与 Secure Enclave P-256 backend 及只返回固定 flags 的独立产品 validation ABI；这些 ABI 不属于 `ManagerBridge` 同步命令，不进入 Dart binding，也不能解锁按钮。

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
- 不新增 manager 同步命令 C ABI、Dart native binding 或远端写入调用。
- 本地 Docker、localhost、fixture、readiness ready 和合成 smoke 只能用于开发验证，不能解锁产品入口。
- 服务端继续被视为不可信，输入热路径不得依赖网络。

进入真实实现前应重新基于当时的 Rust sync / crypto API、服务端协议和平台密钥 backend 设计命令接口，不恢复本次归档的 review-only DTO 或审批目录。

## 当前验证归属

- 关闭态与隐私优先级：`test/models/manager_sync_entry_gate_test.dart`、`test/models/manager_sync_transient_secret_interaction_test.dart`
- settings 不保存敏感材料：`test/ffi_manager_bridge_test.dart`、`test/models/manager_sync_transient_secret_interaction_test.dart`
- diagnostics 脱敏与错误可见性：`test/screens/settings_diagnostics_test.dart`、`test/models/manager_sync_transient_secret_interaction_test.dart`
- UI 禁用入口：`test/screens/sync_test.dart`、`test/screens/settings_test.dart`

这些测试证明当前能力安全关闭，不代表 M3 同步产品已实现。
