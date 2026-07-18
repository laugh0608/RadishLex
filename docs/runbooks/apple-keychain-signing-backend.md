# Apple Keychain Signing Backend Runbook

本文档定义 `apple-keychain-v1` 与普通软件 DPK `apple-keychain-p256-v1` 的平台验证边界，以及 macOS manager 产品进程 gated smoke。Secure Enclave 使用独立 ADR 0007 与 runbook，不在本 backend 内切换。本文不包含真实同步命令、App Sandbox 迁移、输入法安装流程或 Flutter 同步页面；平台私钥抽象见 ADR 0004，算法 profile 见 ADR 0006。

## 当前结论

- `apple-keychain-v1` 保留为历史 Ed25519 候选；真实创建已证明当前路径不支持 `ed25519-v1`，其 production gate 继续关闭。
- ADR 0006 已新增 `ecdsa-p256-sha256-v1` 和独立 `apple-keychain-p256-v1`；该普通 DPK 软件 backend 的运行时已验证，但不是非导出生产 backend。
- 默认 Rust workspace 继续只启用 `test-memory-v1` 和 `unavailable` backend，不访问 Keychain；macOS backend 只在显式 `apple-keychain` feature 下编译。
- `apple-keychain-v1` 已完成 feature-gated 接线和 ignored smoke 测试骨架；真实 Keychain smoke 已执行但未通过，不能视为平台验证通过。
- 2026-06-30 smoke 在沙盒和提权真实环境均阻塞于 Ed25519 Keychain key 创建阶段，错误为 `UnsupportedSignatureAlgorithm { algorithm: "ed25519-v1" }`；测试未进入签名成功或用户可用同步路径。
- `apple-keychain-v1` 的 `backend_status` 在平台策略未解决前必须阻断生产签名；普通 feature 测试只验证编译和状态门禁，不创建 Keychain item。
- `apple-keychain-p256-v1` 已进入 manager Release native library 并通过 provisioning-backed manager 产品进程 DPK 生命周期。Apple 的能力边界与实测参数审计确认普通 DPK 软件 key 可由平台 API 导出，因此运行时字段开放，`exportable=true`、`product_qualified=false`。
- 独立 `apple-secure-enclave-p256-v1` 已完成仓库与 manager 产品构建接线，但本 runbook 的普通 DPK 证据不得复用；实际 Secure Enclave 访问见独立 runbook，当前 runtime/hardware/product 字段保持关闭。
- 当前策略保留 `ed25519-v1` 设备签名协议，不把 Ed25519 seed 作为 generic password / data item 存入 Keychain 后取回 Rust 签名，也不把该软件保护方案伪装成 `apple-keychain-v1`。
- `apple-keychain-v1` 用于真实远端对象前必须通过本 runbook 的创建、加载、签名、删除 / 撤销、锁屏 / 权限、备份迁移和日志脱敏验证。
- 未验证 Secure Enclave 前，不承诺 `hardware_backed = true`。
- 未验证 user presence 前，不承诺 `user_presence_required = true`。
- 未验证 iCloud Keychain / 设备迁移语义前，不承诺 `backup_migratable = true`。
- 真实用户同步仍不得因为 macOS Keychain 可用就绕过外部 TLS、认证、备份恢复演练或客户端合并写回边界。

## Backend 标识与职责

稳定 backend id：

```text
apple-keychain-v1
apple-keychain-p256-v1
```

职责：

- `apple-keychain-v1` 只表达 Ed25519 原生 SecKey 路径；`apple-keychain-p256-v1` 只表达 P-256 原生 SecKey 路径。
- 平台 backend 创建永久 signing key 并保存到 Apple Keychain；RadishLex 不请求 private external representation。
- 平台 bridge 返回 `DeviceSigningPublicKey`、`DeviceSigningKeyHandle` metadata 和签名结果。
- Rust core 只接收 canonical bytes、public key、handle metadata 和 signature bytes。
- FFI / CLI / Flutter / 平台壳不得获得私钥 bytes、seed、Keychain item secret 或可导出 key backup。

当前不做：

- 不把 Apple Keychain 调用直接散落到同步、userdb、ranker 或平台壳。
- 不在任一 Apple backend 内静默降级为 Keychain 保存 seed、Rust 取出 seed 签名的软件保护路径，也不在 P-256 失败后尝试 Ed25519 或 `test-memory-v1`。
- 不通过 `security` 命令行工具实现生产 backend。
- 不在默认 `cargo test` 中访问用户真实 Keychain。
- 不把 Keychain account、access group 或 label 设计成包含系统用户名、设备真实名称、词库内容、input code 或本机绝对路径。

## 建议桥接边界

平台层可以使用 Apple Security framework 的 Keychain / SecKey API：

- `SecKeyCreateRandomKey`：创建设备签名私钥。
- `SecKeyCopyPublicKey`：读取公钥。
- `SecKeyCreateSignature`：对 canonical bytes 签名。
- `SecItemCopyMatching`：按 opaque key id 加载 key item。
- `SecItemDelete`：删除或撤销本机 key item。

Rust core 仍只看到抽象方法：

```text
create_signing_key(device_id, signing_key_id, created_at_ms) -> DeviceSigningPublicKey
handle(device_id, signing_key_id) -> DeviceSigningKeyHandle
public_key(handle) -> DeviceSigningPublicKey
sign(handle, canonical_bytes) -> DeviceSignature
delete_or_revoke(handle, revoked_at_ms)
backend_status() -> DevicePrivateKeyStoreStatus
```

桥接规则：

- `sign` 只能接收 canonical bytes，不接收 plaintext payload、SQLite row、HTTP request body 或 userdb JSON。
- `handle` 中的 `signing_key_id` 必须是 RadishLex 生成的 opaque id，不是 Keychain 可读 label。
- Keychain item lookup 所需 account / application tag / access group 是本地实现细节，不进入 Go server metadata。
- 如果平台 API 需要用户交互，应返回 `private_key_user_presence_required` 或 `private_key_access_denied`，不得回退到 `test-memory-v1`。
- 如果 Keychain 被锁定、权限缺失或 item 损坏，应返回明确错误，不创建新 key 顶替旧设备身份。

## 能力声明

能力必须按证据分层。当前 macOS `apple-keychain-p256-v1` feature build 的声明为：

```text
storage_backend = apple-keychain-p256-v1
signature_algorithm = ecdsa-p256-sha256-v1
compiled = true
available = true
can_create_signing_keys = true
can_sign = true
product_qualified = false
user_sync_enabled = false
exportable = true
hardware_backed = false
user_presence_required = false
backup_migratable = false
```

上述运行时字段由 2026-07-18 合格 manager 产品生命周期支持。未启用 feature、非 macOS target 与阻塞中的 Ed25519 path 继续失败关闭；运行时可用不代表生产可用。

规则：

- `exportable = true` 表示普通 DPK 软件私钥可由平台 API 导出；RadishLex 仍不提供任何导出 ABI/UI，但该限制不能被描述成平台不可导出保护。
- `hardware_backed = true` 只允许在 Secure Enclave 路径有实际设备验证后启用。
- `user_presence_required = true` 只允许在本机验证签名确实需要用户确认或生物识别后启用。
- `backup_migratable = true` 只允许在 iCloud Keychain、Time Machine、设备迁移或 app container 迁移语义有实测记录后启用。
- iOS Keyboard Extension 与 containing app 的 Keychain access group 必须单独验证；未验证前不能假定键盘扩展可访问 manager 创建的 key。

## Key ID 与 Keychain Item

### macOS storage domain 与 host identity

- `apple-keychain-p256-v1` 在 macOS 只使用 data protection keychain，创建、查询、签名前加载和删除必须一致带 `kSecUseDataProtectionKeychain=true`。
- 不查询 legacy file-based keychain，不把 legacy item 当作 DPK item，也不在 DPK missing/denied 时创建 fallback identity。
- `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` 与默认 access group 均依赖 host main executable 的签名身份；in-process dylib 不能获得超出 manager executable 的 Keychain 权限。
- ad-hoc Release bundle 没有 Team ID 或受 provisioning profile 授权的 application identifier，只能用于编译、symbol/status 和预期 denied 路径；正常生命周期资格证据必须绑定到明确的 Apple Development/后续发布身份、bundle id、Team ID、entitlements 和 provisioning profile 摘要。
- `scripts/build-manager-macos-dpk-qualified-product.sh` 是 Apple Development 资格构建入口。它要求显式授权参数、Team ID 和 identity，允许 Xcode automatic signing 创建或下载 profile，并验证 app/native Team ID、`com.apple.application-identifier`、embedded profile 与默认 access group 一致；该脚本不启动 app 或执行 Keychain item 操作。运行它会使用本机签名 identity，并可能联系 Apple Developer 服务创建/更新 profile 或 App ID，因此每次都需单独授权。
- 当前没有真实用户同步 key，旧 smoke item 已删除，因此本次从默认 `SecItem` domain 切换到 DPK 不执行迁移。未来 identity/access group/storage domain 变化必须单独设计迁移或重新授权，不能静默新建同一设备身份。

建议映射：

```text
device_id: RadishLex opaque device id
signing_key_id: RadishLex opaque signing key id
keychain_service: org.radishlex.sync.signing
keychain_account_or_tag: signing_key_id
keychain_label: RadishLex Device Signing Key

P-256 keychain_service: org.radishlex.sync.signing.p256
P-256 keychain_label: RadishLex P-256 Device Signing Key
```

限制：

- `keychain_account_or_tag` 不包含系统用户名、设备名、host name、本机路径或用户输入内容。
- `keychain_label` 只使用固定产品字符串，不拼接用户词或设备可识别名称。
- 如果需要 Apple access group，必须用 bundle / team 配置固定，不写入用户数据。
- committed fixture 只能使用合成 `device_id` 和 `signing_key_id`。

## 创建与加载验证

必须验证：

- Ed25519 路径创建成功时返回 32-byte public key；不支持时返回明确 `unsupported_signature_algorithm`。
- P-256 路径创建成功时返回 65-byte `0x04 || X || Y` SEC1 uncompressed public key，不接受 compressed point、SPKI 或平台对象序列化。
- `DeviceSigningKeyHandle` metadata 使用与算法一一对应的 backend id 和 `signature_algorithm`。
- handle Debug 不包含私钥、seed、Keychain query dictionary、account secret 或 access token。
- 重新启动进程后能通过 `device_id + signing_key_id` 加载同一 public key。
- 尝试加载不存在的 key 返回 `private_key_not_found` / `private_key_unavailable` 等明确错误。
- 不支持 Ed25519 的系统返回 `unsupported_signature_algorithm` 或 `unsupported_storage_backend`，不得创建 fallback test key。

## 签名验证

必须验证：

- Ed25519 handle 只生成 64-byte Ed25519 signature；P-256 handle 使用 SHA-256/X9.62 签名，并在 backend 内把严格 DER 转成 64-byte P1363 `r || s`。
- `DeviceSignature::verify_at` 可用返回的 public key 验证签名。
- P-256 signature 还必须由 Go verifier 对同一 canonical bytes 验证；server/API 不接受 DER signature。
- 篡改 canonical bytes、signing key id 或 signer device id 后验签失败。
- revoked key 后续签名失败。
- access denied、user presence required、Keychain locked 和 item corrupted 的错误可区分。
- 日志和错误不包含 canonical bytes 内容、signature bytes、private key material 或 Keychain query 细节。

签名输入限制：

- 只签 `radishlex-signature-v1` canonical bytes。
- 不签任意 HTTP request body。
- 不签 plaintext userdb payload。
- 不签 P1 原始选择事件或负反馈明细。

## 删除与撤销验证

撤销设备时：

1. 客户端先生成 signed device revocation。
2. 服务端接受撤销后拒绝该设备后续写入。
3. 本机 backend 执行 `delete_or_revoke(handle, revoked_at_ms)`。
4. 后续 `sign(handle, canonical_bytes)` 必须失败。

验证要求：

- `SecItemDelete` 成功后，加载 key 返回 not found。
- 如果平台删除失败，backend 返回 locked / denied / unavailable 等明确错误，不能宣称本机 item 已删除；跨设备设备状态仍由已经提交的 signed revocation 失败关闭，不能依赖进程内集合替代服务端撤销真相。
- 删除 userdb 不等于删除设备私钥；管理 UI 后续必须把两者分开。

## 锁屏、权限与用户交互

macOS 验证矩阵：

- 正常登录会话：创建、加载、签名、删除。
- Keychain locked 或无法访问：返回 locked / access denied，不回退。
- App Sandbox / hardened runtime 权限缺失：返回 access denied，并记录非敏感错误分类。
- user presence policy 开启时：未满足交互返回 user presence required 或 access denied。

data protection keychain 的删除不需要解密 item key material，locked 状态下精确 `SecItemDelete` 可能成功。因此 locked 验证不得假设删除也返回 locked，更不能以“读取失败 -> 删除并重建”恢复；本 backend 只在显式撤销流程中执行精确删除。

受控矩阵分为三个互不冒充的证据：

1. ad-hoc manager bundle 访问 DPK 时应失败关闭为 `private_key_access_denied` 或明确 entitlement blocker，不记为正常产品能力。
2. 合格签名身份下，正常创建、重载、签名、Rust/Go 验签、精确删除和 fresh missing 必须重新通过。
3. 正常身份预先创建合成 key 后，在另行授权的 locked 状态只验证 load/sign 返回 locked；解锁后由同一身份清理并重新跑正常生命周期。脚本不得自行锁定登录 Keychain、改变 search list 或改写用户其他 item ACL。

iOS / Keyboard Extension 后续还需验证：

- containing app 创建 key 后，Keyboard Extension 是否可访问同一 access group。
- full access 关闭时，键盘扩展是否应禁止同步签名。
- 锁屏、后台、系统输入法生命周期下签名是否可用。

## 备份与迁移

必须记录实测结论：

- macOS 设备迁移、Time Machine、iCloud Keychain 是否迁移该 key。
- iOS encrypted backup / iCloud Keychain / app reinstall 是否保留该 key。
- 同一 key 迁移到新设备时，设备撤销语义如何告知用户。

默认策略：

- 未验证前 `backup_migratable = false`。
- 新设备应创建新设备签名 key，通过已有设备授权或恢复码加入。
- 如果平台迁移导致旧设备 key 在新设备可用，管理 UI 必须提示用户撤销旧设备或重新授权。

## 日志与数据

允许日志字段：

- backend id
- error category
- device id
- signing key id
- operation name
- result code
- created / revoked timestamp

禁止日志字段：

- private key bytes、seed、SecKey raw data。
- Keychain item secret、完整 query dictionary、access token。
- canonical bytes 原文、signature bytes。
- 同步主密钥、object key、恢复码明文。
- 明文用户词、input code、reading、P1 event、ranker 明细。
- 系统用户名、设备真实名称、本机绝对路径。

## 自动化验证建议

默认 CI：

- 继续只跑 `test-memory-v1` 和 `unavailable`，不访问系统 Keychain。
- 测试两个 Apple backend 的 capability metadata、算法绑定和 Debug 脱敏。
- 测试 DER -> P1363、locked / denied / missing 固定错误映射，以及普通 DPK runtime 因 `exportable=true` 阻断生产签名。

安全的 repository feature 门禁：

```text
cargo test -p radishlex-ime-crypto --features apple-keychain
```

该命令会显示 Keychain integration tests 为 ignored，不会创建系统 key。`./scripts/check-manager-product.sh` 还会构建 Release app、确认 bundle dylib 含 Apple validation ABI，并用独立 C host 检查编译/软件 DPK 运行时字段为 true、`exportable=true`、产品和同步字段为 false；它不启动 app，不访问 Keychain。

macOS 本机手动 / gated smoke：

```text
RADISHLEX_RUN_APPLE_KEYCHAIN_P256_SMOKE=1 \
  cargo test -p radishlex-ime-crypto --features apple-keychain \
  --test apple_keychain_smoke \
  apple_keychain_p256_smoke_creates_reloads_cross_verifies_and_deletes_key \
  -- --ignored --exact --nocapture
```

该 smoke 有 ignored 与环境变量双门，使用合成 `device_id` / `signing_key_id`，在 DPK 创建临时 P-256 item，跨独立 store 重载 public key，完成 Rust/Go 验签后删除 item 并复核 missing。测试带失败路径 cleanup guard，会在异常返回时尝试删除同一合成 item。失败时必须输出阻塞原因和可复验命令，不得把 skip 写成通过。运行前必须明确告知会触碰本机 macOS Keychain，并获得开发者批准。旧实机记录发生在增加 `kSecUseDataProtectionKeychain` 之前，只是 legacy storage domain 历史证据，不可复用为本命令当前实现的通过结论。

正常 smoke 不证明 locked / denied。若要锁定 Keychain、改变 app 权限、sandbox/entitlement 或 user-presence policy，必须单独列出系统状态变化和恢复步骤并再次获得授权。

macOS manager 产品进程 gated smoke：

```text
./scripts/run-manager-apple-keychain-p256-product-smoke.sh \
  --authorized-product-keychain-smoke
```

执行前必须先通过 `./scripts/check-manager-product.sh` 冻结同一 Release bundle，并单独取得“启动 manager 产品进程 + 访问本机 Keychain”的授权。入口使用显式参数、产品进程环境门和 native FFI 内部环境门三重约束；普通 GUI 启动不会调用它。当前默认 Release bundle 是 ad-hoc 身份，正常生命周期不用于预期 denied 取证；应先运行专用 denied 场景：

```text
./scripts/run-manager-apple-keychain-p256-product-smoke.sh \
  --authorized-product-keychain-denied-probe
```

只有冻结了合格的 Apple Development 或后续发布 host identity，并记录 bundle id、Team ID、application identifier、Keychain access group、entitlements 与 provisioning 摘要后，才运行正常生命周期。成功只支持软件 DPK 运行时能力，不覆盖 `exportable=true` 的产品资格阻塞。

资格构建示例：

```text
RADISHLEX_MANAGER_DEVELOPMENT_TEAM=<10-character Team ID> \
RADISHLEX_MANAGER_CODESIGN_IDENTITY=<Apple Development identity> \
  ./scripts/build-manager-macos-dpk-qualified-product.sh \
  --authorized-apple-development-provisioning-build
```

不得把 ad-hoc 签名后手工注入未经 profile 授权的 entitlement 当作资格构建。若本机没有 profile，只有在明确授权 Xcode 联网创建/下载 App ID/profile 后才能继续。

产品 smoke 在 manager bundle executable 进程中加载 bundle dylib。正常生命周期在 native 内完成创建、跨 store 重载、签名、Rust 验签、短生命周期 Go verifier、删除、内存撤销后失败、fresh store missing 与 cleanup guard。smoke schema v4 返回固定 scenario、result、error category/detail、数值 OSStatus 和布尔摘要；不返回 CFError 文本、private key、public key、canonical bytes 或 signature bytes。Dart/Flutter method channel 不绑定该 ABI，Go 子进程 stdout/stderr 关闭。

locked 矩阵使用固定合成 item，必须由同一合格 host identity 分三段执行：

```text
./scripts/run-manager-apple-keychain-p256-product-smoke.sh \
  --authorized-product-keychain-locked-prepare

# 此处只能由开发者在另行授权的外部步骤锁定登录 Keychain。

./scripts/run-manager-apple-keychain-p256-product-smoke.sh \
  --authorized-product-keychain-locked-probe

# 开发者解锁后必须执行清理。

./scripts/run-manager-apple-keychain-p256-product-smoke.sh \
  --authorized-product-keychain-locked-cleanup
```

prepare 会明确报告 `cleanup_required=1` 并保留一个合成 DPK item；probe 只尝试签名，预期得到固定 locked 类别，不在锁定状态用删除结果代替可访问性证据；cleanup 在解锁后删除并确认 missing。仓库脚本不得调用 `security lock-keychain`、`security unlock-keychain` 或改写 search list。

locked 矩阵仍可用于验证错误语义，但不能把 `exportable=true` 的普通 DPK backend 变成生产 backend。该入口不证明 Secure Enclave、hardware-backed、user presence、backup migration 或真实用户同步可用。

## 2026-07-18 legacy storage domain 历史证据

- 环境：arm64 macOS 26.5.2（25F84），仓库分支 `dev`，本轮基线提交 `c8457e9`。
- 经开发者单独授权执行上述唯一 P-256 gated smoke；结果为 `1 passed`，内嵌 Go verifier `TestExternalP256SignatureSmoke` 同步通过。
- 证据覆盖当时默认 file-based Keychain 的 key 创建、独立 store 重载同一 public key、非导出签名、Rust/Go 验签、删除后当前 store 撤销，以及 fresh store 返回 missing。测试使用合成标识，成功路径删除 item，失败路径 cleanup guard 保持启用。
- 本轮未锁定 Keychain、修改权限、sandbox/entitlement、user-presence policy 或系统输入法设置，也未访问真实同步、userdb 或用户输入数据。
- 后续复核确认该实现没有显式设置 `kSecUseDataProtectionKeychain`，因此 `WhenUnlockedThisDeviceOnly` 没有形成目标 DPK 证据。该记录不证明当前 DPK 实现的运行时能力、locked/denied、产品 bundle 访问、Secure Enclave、hardware-backed、user presence、backup migration 或 production-ready。

## 2026-07-18 DPK 产品接线与评审证据

- manager Release dylib 已显式启用 `ime-ffi/apple-keychain`，导出只读 status 与 gated product smoke validation ABI；ABI v5 的既有 Dart 管理接口未改变。
- `./scripts/check-manager-product.sh` 已通过 Release build、严格签名、native symbol、C status host 和既有 Dart FFI smoke；不启动产品、不访问 Keychain。最终 status 为 `compiled/runtime_available/can_create/can_sign=true`、`exportable=true`、`product_qualified/user_sync_enabled=false`。
- validation smoke schema v4 固定五个场景、脱敏错误分类和数值 OSStatus；native 生命周期材料不进入 Dart，manager 普通启动和 InputMethodKit 不接触同步签名 key。
- 经单独授权，当前 ad-hoc Release bundle 的 DPK denied 场景固定摘要通过：`result=0`、`scenario=1`、`error_category=3`、`created=0`、`fail_closed=1`、`expected_failure=1`、`cleanup_required=0`、`cleanup_attempted=1`。该证据只证明无合格身份时失败关闭，不开放运行时能力或产品资格。
- qualification build 使用 Team ID `WF9UUN335P`、bundle id `dev.radishlex.radishlexManager` 和 application/default access group `WF9UUN335P.dev.radishlex.radishlexManager`；embedded profile UUID 为 `7ab763be-90d8-4f19-90ac-69a5681323af`，有效期至 2026-07-25。app、native dylib、profile 与 entitlement 校验及真实环境 strict codesign 均通过。
- 首次 DPK 产品创建返回 `errSecNoSuchAttr (-25303)`。SDK key-class 支持表与 Apple DTS 资料确认 legacy shim 曾忽略的 `kSecAttrComment` / `kSecAttrIsExtractable` 不属于普通 DPK key 属性，而且标准 DPK 私钥可导出。实现删除这些不受支持属性并把 capability 改为 `exportable=true`，未切换 Secure Enclave。
- 修正后经新授权运行同一 qualification 产品包，固定摘要为 `result=0 scenario=0 error_category=0 error_detail=0 platform_status=0 created=1 reloaded=1 rust_verified=1 go_verified=1 deleted=1 missing=1 fail_closed=1 cleanup_attempted=1`。合成 item 已删除；未锁定、解锁或重配置 Keychain。

## 停止线

- `apple-keychain-p256-v1` 的普通 DPK 软件运行时字段可以开放，但 `exportable=true` 已拒绝产品资格；不用于真实远端对象上传，也不通过补 locked 证据规避该条件。
- Secure Enclave 必须使用独立 backend id、capability 与产品证据；unsupported 设备失败关闭，不回退普通 DPK、legacy Keychain 或 `test-memory-v1`。
- 如果需要导出私钥 bytes 才能完成签名，应停止并回退设计。
- 如果 backend unavailable 时回退到 `test-memory-v1`，必须停止并回退实现。
- 如果 Keychain label / account / 日志包含真实用户名、设备名称、本机路径或输入内容，必须停止并修正。
- 如果 iOS Keyboard Extension 无法可靠访问同一 key，不能把 iOS 同步签名接入用户可用路径。
