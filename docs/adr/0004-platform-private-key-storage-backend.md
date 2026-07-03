# ADR 0004: 平台私钥存储 Backend 边界

本文档用于固定 RadishLex 真实远端同步前的平台私钥存储 backend 边界，读者是后续实现 `ime-crypto`、平台 bridge、Go sync server 验签接线、管理 UI 设备页面和审阅隐私边界的开发者。本文不包含平台 SDK 调用代码、FFI 导出接口、Flutter 页面、系统输入法壳接入或安装权限流程；`apple-keychain-v1` 平台验证边界见 `docs/runbooks/apple-keychain-signing-backend.md`，Android Keystore 验证边界见 `docs/runbooks/android-keystore-signing-backend.md`，Apple 签名策略见 `docs/adr/0005-apple-platform-signing-strategy.md`。

## 状态

Accepted

## 背景

`docs/adr/0003-device-signing-key-storage.md` 已固定设备签名对象、canonical bytes、Ed25519 签名模型、`DevicePrivateKeyStore` 抽象和错误语义。当前 Rust 实现已提供合成 `test-memory-v1` signing key store、`unavailable` 明确失败 store、backend capability metadata 和生产签名门禁测试，用于测试 signed sync object manifest、signed recovery record、signed device authorization 和 signed device revocation；`apple-keychain-v1` 平台 runbook 已固定，macOS backend 已在 `apple-keychain` feature 下接线，真实 Keychain smoke 已执行但阻塞于 `ed25519-v1` 创建。ADR 0005 已决定保留 `ed25519-v1` 协议，不把 Keychain seed 存储 fallback 混入 `apple-keychain-v1`，并让该 backend status 在 smoke 通过前阻断生产签名。`android-keystore-v1` 已补平台 runbook、`android-keystore` feature、不可用状态门禁、Rust bridge wrapper、bridge contract、合成 bridge 单测、ignored smoke 入口、仓库内 Kotlin / Gradle harness、`@JvmStatic` facade、Rust raw JNI glue、gated instrumented smoke、provider diagnostics、smoke 记录模板和设备矩阵记录；当前 Android target build 已通过 `./scripts/check-android-target.sh`；Android Gradle harness 已在 Pixel 9 Pro API 35 AVD 上执行真实 smoke 和 provider diagnostics，并在 Pixel 10 Pro API 37 AVD 上执行 provider diagnostics，结果均为 `unsupported_signature_algorithm`，不解除生产签名门禁。

进入真实同步前，还需要明确生产私钥如何落到系统安全存储。否则后续平台壳、Flutter manager 或 FFI 可能为了接线方便直接持有私钥 bytes，破坏设备身份和撤销边界。

## 决策

RadishLex 采用平台 backend 插拔策略：

- Rust core 只依赖 `DevicePrivateKeyStore` 抽象和 `DeviceSigningKeyHandle` metadata。
- 生产设备签名私钥必须由平台 backend 创建和保存，不允许通过 FFI、CLI 或管理 UI 导出私钥 bytes。
- 默认 workspace 继续只启用 `test-memory-v1` 和 `unavailable` backend，不链接平台 SDK，不访问系统 Keychain / Keystore。
- 平台 backend 必须显式声明 `storage_backend`、`exportable`、`hardware_backed`、`user_presence_required`、`backup_migratable` 和 `created_at_ms` 等属性。
- 如果某个平台无法提供非导出私钥存储，可以先以软件保护 backend 进入设计，但必须标记 `exportable = true` 或等价风险属性，并在管理 UI / runbook 中说明保护级别。
- 真实远端同步默认要求生产 backend 可用；backend unavailable 时，本地输入和本地学习仍可用，但同步对象上传、设备授权、撤销和恢复记录轮换必须返回明确错误。

## Backend 标识

稳定 backend id 初稿：

```text
test-memory-v1
unavailable
apple-keychain-v1
android-keystore-v1
windows-cng-v1
linux-secret-service-v1
```

规则：

- `test-memory-v1` 只能用于单元测试、integration test 和合成 fixture。
- `unavailable` 用于默认构建或平台能力缺失时的明确失败，不允许静默回退到 test memory。
- `apple-keychain-v1` 和 `android-keystore-v1` 已补平台 runbook；`windows-cng-v1`、`linux-secret-service-v1` 仍只是能力边界标识，进入实现前必须分别补平台 runbook 或 spike 记录。
- backend id 是协议和日志可见 metadata，不得包含系统用户名、设备真实名称、本机路径或用户输入内容。

## Key Handle Metadata

生产 `DeviceSigningKeyHandle` 至少应记录：

```text
device_id
signing_key_id
signature_algorithm
storage_backend
public_key
exportable
hardware_backed
user_presence_required
backup_migratable
created_at_ms
last_used_at_ms
revoked_at_ms
```

规则：

- `public_key`、`signing_key_id`、`storage_backend` 可进入服务端设备 metadata。
- `exportable` / `hardware_backed` / `backup_migratable` 是能力声明，不是服务端信任根。
- `last_used_at_ms` 只允许本地保存；如需同步，只能作为非敏感设备状态摘要单独设计。
- Debug 输出不得包含私钥 bytes、seed、系统 item secret、Keychain account secret、Keystore alias secret 或 DPAPI protected blob 明文。

## 抽象接口

Rust 层保持以能力为中心的接口：

```text
DevicePrivateKeyStore
  create_signing_key(device_id, algorithm) -> DeviceSigningPublicKey
  load_signing_key_handle(device_id, signing_key_id) -> DeviceSigningKeyHandle
  sign(handle, canonical_bytes) -> DeviceSignature
  public_key(handle) -> DeviceSigningPublicKey
  delete_or_revoke(handle)
  backend_status() -> DevicePrivateKeyStoreStatus
```

接口规则：

- `create_signing_key` 不返回私钥 bytes。
- `sign` 只接收 canonical bytes，不接收 plaintext payload、SQLite row 或 HTTP request 原文。
- `public_key` 可以返回公开 key 和 key id。
- `delete_or_revoke` 必须让后续 `sign` 明确失败；如果平台只能逻辑撤销，必须记录 backend limitation。
- `backend_status` 用于管理 UI 展示是否可同步、是否需要解锁、是否降级为软件保护。

## 平台边界

### Apple 平台

`apple-keychain-v1` 代表 macOS / iOS Keychain 方向。

边界：

- 私钥创建、加载和签名由 Apple 平台 backend 负责。
- Rust core 不直接调用 Objective-C / Swift API。
- 平台 bridge 只把签名结果、公钥和 handle metadata 传回 Rust。
- 是否使用 Secure Enclave、是否要求 user presence、是否允许 iCloud Keychain 迁移，需要后续平台 spike 固定；未验证前不得在文档或 UI 中承诺硬件保护。

### Android

`android-keystore-v1` 代表 Android Keystore 方向。

边界：

- Kotlin / JNI bridge 负责创建和持有平台 key alias。
- Rust core 只看到 handle metadata、公钥和签名结果。
- 是否硬件支持、是否强制锁屏、是否要求用户认证，由 backend status 明确报告。
- Android full backup / device transfer 对 key 的影响需要实机验证后再写入 runbook。

### Windows

`windows-cng-v1` 代表 Windows CNG / 系统密钥存储方向。

边界：

- Windows 平台 bridge 负责创建、加载、签名和删除 key。
- Rust core 不直接持有私钥材料。
- DPAPI 可用于保护本地辅助 secret，但不能替代设备签名 key usage 分离。
- TSF 壳不得直接参与同步签名；同步签名应由管理 / sync client 层调用明确 backend。

### Linux

`linux-secret-service-v1` 代表 Linux desktop Secret Service / libsecret 方向。

边界：

- Linux backend 可能无法提供与移动平台相同的非导出硬件私钥语义。
- 如果只能存储软件私钥或被包装的私钥材料，必须标记 `exportable` 或 `hardware_backed = false`。
- headless self-host 或 server 环境不得假装有桌面 secret service；应返回 `storage_backend_unavailable`。
- Fcitx5 / IBus 平台壳不持有私钥 bytes。

## FFI 与平台 Bridge 边界

当前不新增 FFI 私钥接口。

后续如果需要跨语言签名，必须满足：

- FFI 不导出私钥 bytes。
- FFI 不导出内部 key handle 指针给长期持有的外部对象。
- FFI 只允许传入 canonical bytes 或受控 manifest fields。
- 释放、线程、错误对象和 panic 边界必须沿用 `docs/ffi-boundary.md` 与 `docs/runbooks/ffi-platform-call-contract.md`。
- 平台 bridge 复制 string / bytes view 后必须立即释放 Rust handle。
- 任何平台 UI 都不能显示私钥、seed 或可导出的 key backup。

## 错误语义

必须稳定区分：

- `storage_backend_unavailable`
- `private_key_not_found`
- `private_key_locked`
- `private_key_access_denied`
- `private_key_user_presence_required`
- `private_key_export_blocked`
- `private_key_revoked`
- `private_key_corrupted`
- `unsupported_signature_algorithm`
- `unsupported_storage_backend`
- `backend_capability_mismatch`

错误处理规则：

- backend unavailable 不得静默创建 test-memory key。
- private key locked 可以提示用户解锁设备或重试。
- access denied 不得自动重试到导出私钥 fallback。
- key corrupted 需要引导用户撤销设备或用恢复码重新加入。
- 错误日志不得包含系统 key alias 中的敏感片段、本机路径、私钥 bytes、seed、同步主密钥、恢复码或 plaintext payload。

## 设备迁移与备份

设备私钥迁移默认不承诺跨设备可用：

- 新设备应生成自己的设备签名 key。
- 设备备份恢复后，如果平台 key 不可用，应把该设备视为需要重新授权或恢复。
- 如果平台允许 key 随系统备份迁移，backend 必须标记 `backup_migratable = true`，管理 UI 应提示这对设备撤销语义的影响。
- 同步域恢复依赖恢复码或已有设备授权，不依赖复制旧设备私钥。

## 撤销与删除

撤销设备时：

- 客户端创建 signed device revocation。
- 本地 backend 应调用 `delete_or_revoke(handle)`。
- 如果平台删除失败，客户端仍必须把设备状态视为 revoked，并阻止后续签名。
- 服务端接受撤销后，拒绝该设备签发新对象、授权包或恢复记录。

本地删除学习数据不等于删除设备私钥。管理 UI 后续必须区分：

- 清空本地 userdb。
- 停止同步。
- 撤销当前设备。
- 删除当前设备私钥。

## 实施顺序

1. 已保持当前 `test-memory-v1` signing key store 只用于测试，并通过生产签名门禁阻止其进入生产对象签名。
2. 已在 Rust 模型中补 `DevicePrivateKeyStoreStatus` 和 backend capability metadata 测试。
3. 已为 unavailable backend 固定错误语义，确保真实同步功能在无生产 backend 时明确失败。
4. 已补 `apple-keychain-v1` 平台 runbook，固定创建、加载、签名、删除、锁屏 / 权限、备份迁移、日志脱敏和停止线。
5. 已在 `apple-keychain` feature 下接 macOS Keychain backend 和 ignored gated smoke，默认 workspace 不访问系统 Keychain；真实 Keychain smoke 已执行但返回 `UnsupportedSignatureAlgorithm { algorithm: "ed25519-v1" }`。
6. 已补 ADR 0005，固定 Apple 平台签名策略：保持 `ed25519-v1` 协议，`apple-keychain-v1` 不做 seed 存储 fallback，status 在 smoke 通过前阻断生产签名。
7. 已补 `android-keystore-v1` 平台 runbook、`android-keystore` feature、不可用状态门禁、Rust bridge wrapper、bridge contract、raw JNI glue、合成 bridge 单测、ignored smoke 入口、仓库内 Kotlin / Gradle harness、`@JvmStatic` facade、gated instrumented smoke、provider diagnostics、smoke 记录模板和设备矩阵记录，固定 Android Keystore Ed25519 创建 / 加载 / 签名 / 删除、锁屏 / 权限、备份迁移、IME 生命周期和日志脱敏验证边界；Android target build 已通过 `./scripts/check-android-target.sh` 复验 `radishlex-ime-crypto --features android-keystore --target aarch64-linux-android`；Android Gradle harness 已在 Pixel 9 Pro API 35 AVD 上执行真实 smoke 和 provider diagnostics，并在 Pixel 10 Pro API 37 AVD 上执行 provider diagnostics，结果均为 `unsupported_signature_algorithm`，不解除生产签名门禁。
8. 其他平台仍需先补 backend spike / runbook，再接具体平台 SDK。
9. 平台 backend 通过后，再允许真实远端对象上传下载使用生产签名。
10. 最后才把管理 UI 的设备与恢复页面接入生产 backend。

## 验证口径

进入真实远端同步前必须覆盖：

- 默认构建不会访问系统 Keychain / Keystore / CNG / Secret Service。
- `test-memory-v1` 不会在生产配置中被选为签名 backend。
- unavailable backend 对签名、授权、撤销和恢复记录轮换返回明确错误。
- key handle Debug 不输出私钥、seed、系统 item secret 或同步主密钥。
- `exportable`、`hardware_backed`、`backup_migratable` 能被测试 fixture 覆盖并进入能力判断。
- revoked key 后续不能签名。
- private key locked / access denied / unavailable 的错误可区分。
- FFI 没有私钥 bytes 导出入口。
- 平台 backend runbook 覆盖创建、加载、签名、删除、锁屏 / 解锁、备份迁移和权限拒绝。

## 后果

收益：

- 真实同步前明确设备私钥不会穿过 FFI、CLI 或管理 UI 明文暴露。
- 平台能力差异通过 backend capability 显式表达，不让 Linux desktop 或 headless 环境伪装成硬件保护。
- Go server 仍只验证公钥、签名和设备状态，不获得任何私钥材料。

代价：

- 每个平台都需要单独 spike 和 runbook。
- 管理 UI 需要处理 locked、unavailable、software-protected 等状态。
- 设备备份迁移和撤销解释会更复杂，但这是保护同步域身份边界的必要成本。

## 停止线

- 生产 backend 未通过平台验证前，不做真实远端对象上传下载。
- 私钥 bytes 可能进入 FFI、CLI、日志、崩溃报告或 Go server 时，必须停止并回退该设计。
- backend unavailable 时不得使用 `test-memory-v1` 继续生产同步。
- 平台壳不得持有同步私钥或承担同步签名真相源。
