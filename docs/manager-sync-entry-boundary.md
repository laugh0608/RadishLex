# Manager 同步入口边界

本文面向 RadishLex Manager、Rust bridge 与同步功能维护者，固定 Manager 在 M3 真实同步实现前后的职责、敏感材料边界和产品停止线。本文不定义 C ABI 命令草案、审批流程、部署证据包格式、同步状态机、同步协议或密码学细节；Rust 编排以 `docs/sync-orchestration.md` 为准，协议和密钥语义分别以 `docs/privacy-sync.md`、`docs/sync-key-management.md`、`docs/production-recovery-flow.md` 和 `docs/sync-server-api-storage.md` 为准。

## 当前结论

- 当前 Manager 只展示同步配置草案、本地 readiness、连接健康摘要、恢复与设备流程的不可用原因。
- 真实远端同步、恢复码生成与输入、设备加入授权、设备撤销和密钥轮换没有产品执行入口。
- `ManagerBridge` 当前不提供真实用户同步、恢复、授权、撤销或轮换命令；唯一新增的执行能力是下述 loopback HTTPS 合成资格 run，不能解锁任何产品入口。
- 即使 endpoint、平台 backend、部署证据和 readiness 摘要均显示 ready，用户同步入口仍保持关闭，直到 M3 退出条件满足。
- manager Release native library 可以包含普通 DPK、Secure Enclave signing 与独立 Secure Enclave key-agreement backend，以及只返回固定 flags 的相互独立产品 validation ABI；这些底层 validation ABI 不直接进入 Dart binding。Manager 只通过下述独立 status-only 业务摘要读取脱敏结果，不能据此解锁按钮。

## Status-only 产品摘要

M3 允许一条不带入参、只读且不访问系统密钥条目的 `radishlex_manager_sync_product_status` C ABI。它随现有 `loadSnapshot` 进入 Manager，不增加 `ManagerBridge` 命令方法，也不创建、读取、签名、解封或删除 Keychain / Secure Enclave 项目。

该摘要只包含固定版本与数值枚举：signing backend / algorithm、编译与静态运行时 capability、`can_create` / `can_sign`、`exportable` / `hardware_backed`、signing 产品资格、独立 key-agreement backend 编译与实机资格、组合 `product_qualified`、`user_sync_enabled` 和首个稳定 blocker。它不得包含 device id、key id、公钥、路径、OSStatus、平台错误正文、signature、wrapped material、master key、shared secret、nonce、HTTP 内容或任意自由文本。

规则固定如下：

- signing 状态可以复用不触发系统调用的 backend metadata；key-agreement 在未完成独立实机资格前只能报告 compiled，不能继承 signing backend 的运行时或 hardware-backed 证据。
- `product_qualified` 只有 signing 与 key-agreement 两条独立资格同时成立时才可为 `1`；当前 macOS 产品构建为 `1`，默认/非 macOS 构建仍为 `0`。`user_sync_enabled` 还受阶段停止线约束，所有构建当前固定为 `0`。
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

当前 Manager 没有恢复码、短码、私钥、签名或 wrapped material 的输入字段与执行方法；设置文件只保存 `access_token_configured` 布尔值，不保存 token 文本。资格 modal 可接受一次性 bearer token 与可选 CA DER 路径，但 controller、Dart buffer、FFI copy 和 Rust transport copy 均限制在单次 run，并在提交、取消、失败或完成后清除；路径和 bytes 不进入结果或诊断。

Apple P-256 产品进程 gated smoke 只由五个显式命令行场景之一与环境门触发：DPK 正常生命周期、预期 denied 创建、locked 前置、locked 签名探测、解锁后清理。正常生命周期和前置场景在 native 内使用 synthetic canonical/signature 并完成 Rust 验签；正常生命周期额外调用短生命周期 Go verifier。smoke schema v4 返回 Swift 的只有固定 scenario、result、error category/detail、数值 OSStatus 和布尔摘要，不含 CFError 文本。private key、public key、canonical bytes、signature bytes 不得进入 Dart、Flutter method channel、settings 或 diagnostics。普通 manager 启动不访问该 Keychain 路径；脚本不锁定、解锁或改写 Keychain 搜索列表；InputMethodKit 不参与同步密钥或签名。

Secure Enclave P-256 使用独立环境门、native symbol 与六个显式场景，额外保留 unsupported create，并在正常生命周期内要求 private external representation 失败。它复用同一固定 26-word 脱敏摘要布局，但拥有独立 schema version；qualification lifecycle、ad-hoc denied、真实设备锁屏 locked 与 cleanup 已通过，按受支持 macOS 主路径评审开放运行时、hardware-backed 与产品资格字段。unsupported 延期补测，用户同步 gate 继续关闭。Dart、普通 manager 启动和 InputMethodKit 同样不接触该路径。

Secure Enclave key-agreement 再使用一组独立 status/smoke symbol、环境门和固定摘要。其 lifecycle 已完成 fresh store 公钥一致、ECDH、合成 wrapped epoch 解封、精确删除和 fresh missing，denied、locked 与 cleanup 也有独立实机证据；unsupported 场景保留并延期补测。普通 Manager 只读取 metadata-only status，不能触发 smoke；当前报告 runtime/hardware-backed/product qualified，但组合用户同步 gate 仍保持关闭。

## 产品停止线

在 M3 的协议、安全和本地产品资格证据全部满足前：

- `启用同步` 保持禁用。
- 不新增恢复码生成 / 输入、join request、授权成功、撤销或轮换的可执行按钮。
- 下一批只允许新增 localhost、合成 P2、单次调用内存参数的受控资格命令 C ABI；普通用户远端写入、后台同步与真实设备流程仍不得新增。上述 status-only ABI 不属于同步命令。
- 本地 Docker、localhost、fixture、readiness ready 和合成 smoke 只能用于开发验证，不能解锁产品入口。
- 服务端继续被视为不可信，输入热路径不得依赖网络。

Rust orchestration、严格 HTTPS transport 与生产 backend 主路径资格已稳定，可以在 Manager 普通入口继续禁用时设计受控资格命令接口。该接口必须直接复用现有 Rust sync / crypto API、只接受 transient 参数并证明取消/重启/脱敏；不恢复已归档的 review-only DTO 或审批目录。

跨模块产品组合固定进入 `ime-sync-runtime`：`ime-sync` 保持协议、transport 与 orchestration 真相源，`ime-userdb` 保持 SQLite repository 真相源，`ime-ffi` 只包装 run handle。输入用 `ime-runtime` 继续不依赖远端同步；不得为减少一个 crate 把网络执行塞进输入热路径或 ABI 文件。

## 已完成批次：Manager 本地 HTTPS 同步资格执行

该批次不是连接真实用户数据的同步开关，而是由真实 Manager bridge 发起、Rust 完整执行的本地 HTTPS 合成资格流程。runner、跨进程单运行与重启清理、ABI v7、Dart bridge、Manager 交互、Release bundle 和真实短生命周期 Caddy 正向链均已落地；资格摘要仍不能作为 readiness/deployment evidence 或用户同步开放依据。

执行边界固定如下：

- 请求只接受版本化参数：`https://localhost` 或明确 loopback endpoint、一次调用 bearer token、可选本地 CA DER 和受限 timeout。禁止 HTTP、非 loopback host、URL credential/query/fragment，以及调用方提供 userdb 路径、device/domain/key id、payload、master key、wrapped material 或 signature。
- Rust 为每次运行生成唯一合成 domain/device/object 身份和固定 P2 数据，使用隔离临时 userdb/服务状态执行现有 discovery、验签、解密、merge、outbox、上传、冲突处理和第二轮收敛。合成 crypto provider 必须以资格专用身份显式标记，不能冒充生产 backend，也不能改变产品资格或用户同步 gate。
- 资格运行不得隐式创建、读取、签名、派生或删除 Keychain/Secure Enclave 项目。生产 backend 只读取已评审的 metadata status；任何真实系统 key 操作仍使用独立 gated validation 和当次明确授权。
- bearer token 在 Dart、FFI 和 Rust 中只属于本次调用；native 必须复制到受控内存并在完成、取消、超时和失败后清除。CA 是公开信任材料，但同样不写 settings、diagnostics、日志或长期状态；文件选择路径不得跨入 Rust 结果或诊断。
- 执行采用 Rust-owned opaque run handle，固定 created/running/cancelling/completed/failed/cancelled 状态。start、poll、cancel、free 的线程和释放规则必须明确；同一 Manager bridge 同时只允许一个运行，重复 cancel 必须幂等，free 不能留下 worker、临时文件或悬空回调。
- Dart bridge 只复制固定 phase/result/error enum、受限计数和布尔清理结果，不接收 HTTP body、对象/device/domain/key id、token、CA bytes、路径、payload、用户词或平台错误正文。未知版本、状态跳变或非 canonical flag 必须失败关闭。
- Manager UI 必须明确标识“本地合成同步资格测试”，与“启用同步”分区。token 输入只存在于当前 modal/controller，提交、取消、导航、超时或错误后清空；运行期间提供取消和稳定阶段状态，结果进入内存摘要但不成为 readiness/deployment evidence，也不解锁恢复、授权、撤销或同步按钮。

验证必须覆盖：真实短生命周期 Caddy HTTPS 正向链、错误 token、非 loopback/HTTP、非可信 CA、主机名不匹配、超时、网络中断、并发 start、各阶段取消、结果 handle 重复读取/释放、Manager 进程重启、临时 userdb/worker 清理、settings/diagnostics/log 脱敏，以及 C11/Objective-C header、Dart mapper、Flutter widget、Release bundle 和仓库总门禁。

## 当前验证归属

- 关闭态与隐私优先级：`test/models/manager_sync_entry_gate_test.dart`、`test/models/manager_sync_transient_secret_interaction_test.dart`
- settings 不保存敏感材料：`test/ffi_manager_bridge_test.dart`、`test/models/manager_sync_transient_secret_interaction_test.dart`
- diagnostics 脱敏与错误可见性：`test/screens/settings_diagnostics_test.dart`、`test/models/manager_sync_transient_secret_interaction_test.dart`
- UI 禁用入口：`test/screens/sync_test.dart`、`test/screens/settings_test.dart`
- native 产品摘要 contract 与 fail-closed 映射：`crates/ime-ffi/tests/input_header_contract.rs`、`test/ffi_manager_bridge_test.dart`

这些测试证明当前能力安全关闭，不代表 M3 同步产品已实现。
