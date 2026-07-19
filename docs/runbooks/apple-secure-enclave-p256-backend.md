# Apple Secure Enclave P-256 Backend Runbook

本文档定义 `apple-secure-enclave-p256-v1` 的仓库验证、macOS manager 产品构建、受控实机 smoke、失败矩阵和清理要求。读者是实现或复验 Apple Secure Enclave 设备签名 backend 的开发者。本文不开放真实同步，不操作输入法安装，不定义 Flutter 同步页面；稳定决策见 ADR 0007。

## 当前阶段

repository implementation、自动测试、Release build 与受支持 macOS 产品资格链均已完成。当前允许声明运行时、hardware-backed 与产品资格；user sync、user presence 与 backup migration 继续独立关闭，unsupported 在合适环境可得时补测。

任何以下动作都必须单独授权：

- 由 manager 产品进程创建、加载、签名或删除 Secure Enclave/Keychain item。
- 使用 Apple Development/provisioning 资格构建入口，因其会使用本机 signing identity 并可能联系 Apple Developer 服务。
- 锁定/解锁 Keychain、改变 search list、entitlement、App Sandbox、access-control policy 或其他系统状态。

## 稳定配置

```text
backend_id = apple-secure-enclave-p256-v1
algorithm = ecdsa-p256-sha256-v1
service = org.radishlex.sync.signing.secure-enclave.p256
label = RadishLex Secure Enclave P-256 Device Signing Key
token = kSecAttrTokenIDSecureEnclave
accessible = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
access_control = kSecAccessControlPrivateKeyUsage
```

application tag 由固定 service 与 RadishLex opaque `signing_key_id` 组成。禁止加入用户名、设备真实名称、host name、本机路径、用户词、input code 或上下文。

## Repository 门禁

安全的默认门禁不得访问 Keychain：

```text
cargo test -p radishlex-ime-crypto --features apple-keychain
cargo test -p radishlex-ime-ffi --features apple-keychain
./scripts/check-manager-product.sh
./scripts/check-repo.sh
```

要求：

- Secure Enclave store status 在 macOS feature build 报告 `compiled/available/can_create/can_sign/hardware_backed=true`。
- `product_qualified=true` 只表达已经完成的受支持 macOS 主路径评审；`user_sync_enabled=false`，不得由 backend 资格自动开放。
- `exportable=false`、`user_presence_required=false`、`backup_migratable=false`。
- C header、Rust ABI、Swift struct 与 Objective-C/C layout contract 一致。
- manager product dylib 包含独立 status/smoke symbol，Dart 目录中没有对应 binding。
- 默认/feature 单测中的 Secure Enclave integration smoke 保持 `ignored` 且有环境门。

## 产品资格构建

Secure Enclave 与 data protection keychain 使用同一 host identity/Keychain access group 资格构建边界。冻结 bundle 前运行现有专用入口：

```text
RADISHLEX_MANAGER_DEVELOPMENT_TEAM=<10-character Team ID> \
RADISHLEX_MANAGER_CODESIGN_IDENTITY=<Apple Development identity> \
  ./scripts/build-manager-macos-dpk-qualified-product.sh \
  --authorized-apple-development-provisioning-build
```

运行前必须另行授权。构建成功后记录 bundle id、Team ID、application identifier、access group、profile UUID/expiry、strict codesign 和冻结产物 hash。该构建本身不启动产品、不访问 Secure Enclave item，也不证明 runtime 可用。

## 产品进程 Gated Smoke

入口必须使用显式授权参数：

```text
./scripts/run-manager-apple-secure-enclave-p256-product-smoke.sh \
  --authorized-secure-enclave-product-smoke
```

正常生命周期必须在 native 内完成：

1. 创建合成 Secure Enclave P-256 key。
2. 从独立 store 重新加载同一 public key。
3. 对合成 canonical bytes 签名并完成 Rust/Go 验签。
4. 尝试 private external representation，要求失败；任何成功返回都使资格失败并立即释放 bytes。
5. 精确删除 item，确认内存撤销后不能签名，fresh store 返回 missing。
6. 无论中途成功或失败都执行固定合成 key cleanup，并只输出固定摘要。

private/public key bytes、canonical bytes、signature bytes、CFError 文本和 query dictionary不得进入 Swift/Dart/stdout/stderr。Go verifier 子进程输出关闭。

## 失败矩阵

独立场景：

```text
--authorized-secure-enclave-denied-probe
--authorized-secure-enclave-locked-prepare
--authorized-secure-enclave-locked-probe
--authorized-secure-enclave-locked-cleanup
--authorized-secure-enclave-unsupported-probe
```

- denied：无合格 entitlement/host identity 或受限环境下创建必须失败关闭，不得留下 item。
- locked：同一合格 identity 先创建固定合成 key；probe 入口提供 20 秒倒计时，由开发者手动锁屏但不休眠或合盖，倒计时后只探测 sign/load；解锁后精确清理。仓库脚本不得自行锁屏、锁定/解锁 Keychain 或改写 search list。
- unsupported：无 Secure Enclave 的设备、虚拟机或 CI 必须返回 backend unavailable/unsupported 固定分类，不回退普通 DPK。
- user presence：本 backend 未启用 user-presence flag。若系统仍要求交互，记录结构化 blocker，不把它改写为 `user_presence_required=true`；先复核平台行为与 backend/version 决策。

经典 `security lock-keychain` 只锁定登录 Keychain，不等价于 Data Protection Keychain / Secure Enclave 的设备锁定态。2026-07-18 实测该状态下签名仍成功，返回 `expected_failure_not_observed`；该结果不算 locked 失败关闭证据，也不算 backend 故障。后续不得再以登录 Keychain 锁定替代手动锁屏。

修正后的产品 probe 通过 20 秒倒计时配合开发者手动锁屏。实测签名返回 `PrivateKeyLocked`、OSStatus `-25308`，`fail_closed=1/expected_failure_confirmed=1`；解锁后的精确 cleanup 返回 `deleted=1/missing_confirmed=1/cleanup_required=0`。这组证据满足本机设备锁定态语义；unsupported 真实环境延期补测，user presence 与 backup migration 仍保持独立关闭。

## 资格字段评审

证据必须逐字段评审：

- `runtime_available/can_create/can_sign`：只有合格产品生命周期成功才可开启。
- `exportable=false`：官方约束加实测 private external representation 失败。
- `hardware_backed=true`：只有 token 配置、产品创建/签名与不可导出证据共同支持才可开启。
- `user_presence_required`：本版本目标为 false；必须确认正常签名不出现额外认证交互。
- `backup_migratable=false`：Secure Enclave key 设备绑定；迁移/恢复仍需独立实测，不能宣称备份可迁移。
- `product_qualified`：受支持 macOS 设备的生命周期、不可导出、denied、locked、cleanup、日志脱敏与评审满足后可开启；unsupported 仍需在合适环境补测，但不是个人开发阶段硬门槛。
- `user_sync_enabled`：继续为 false，直到 M3 orchestration、设备授权/恢复/撤销、key epoch 和部署证据全部满足。

## 清理与中止

- smoke 只处理固定 service 下的合成 key id，不枚举或删除其他 Keychain item。
- prepare 场景留下 item 时必须输出 `cleanup_required=1`；后续由同一合格 identity 执行 cleanup。
- 删除失败时报告 locked/denied/unavailable 固定分类并停止，不新建替代身份。
- private external representation 意外成功、Dart 收到敏感 bytes、日志出现 key/signature/canonical 内容、或任一路径 fallback 到普通 DPK/test memory 时，立即停止资格评审并回退实现。
