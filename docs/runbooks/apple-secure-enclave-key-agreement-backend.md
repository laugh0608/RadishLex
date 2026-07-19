# Apple Secure Enclave Key Agreement Backend Runbook

本文档定义 macOS `apple-secure-enclave-p256-v1` key-agreement backend 的独立产品资格门禁、wrapped epoch 往返判据、失败矩阵和清理责任。读者是实现或复验 M3 产品材料装载边界的开发者。本文不开放真实用户同步，不定义 Flutter 命令入口，也不把设备签名 backend 的证据视为 key-agreement 证据。

## 当前停止线

仓库可以实现并自动验证 status-only ABI、默认关闭的 smoke、固定脱敏摘要、产品 symbol 和静态门禁。普通 Manager 启动只能读取 metadata-only 状态，不得创建、读取、派生或删除 Keychain / Secure Enclave item。

以下动作必须先取得明确授权，并且每个场景使用对应的显式授权参数：

- 创建、重新加载、执行 ECDH 或删除真实 key-agreement item；
- 在受限 host 下验证 denied，在设备锁定态验证 locked，或在不支持 Secure Enclave 的环境验证 unsupported；
- 改变 Keychain、锁屏、entitlement、签名身份或其他系统状态。

在独立实机证据完成前，`runtime_qualified/product_qualified` 必须保持 `false`；不能继承签名 backend 的 lifecycle、hardware-backed 或错误矩阵结论。当前受支持 macOS 主路径证据已经完成，因此两项资格为 `true`；`user_sync_enabled` 仍独立保持 `false`。

## 稳定配置

```text
backend_id = apple-secure-enclave-p256-v1
algorithm = p256-ecdh-hkdf-sha256-xchacha20poly1305-v1
service = local.radishlex.sync.key-agreement.secure-enclave.p256.v1
label = RadishLex Secure Enclave P-256 Key Agreement Key
token = kSecAttrTokenIDSecureEnclave
accessible = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
access_control = kSecAccessControlPrivateKeyUsage
```

key-agreement key 使用独立 device/key id、application tag 和平台 handle。不得复用设备签名 key，也不得把用户名、设备真实名称、host name、本机路径或输入内容写入 identity。

## Repository 门禁

安全的默认验证不得访问系统密钥条目：

```text
cargo test -p radishlex-ime-crypto --features apple-keychain
cargo test -p radishlex-ime-ffi --features apple-keychain
./scripts/check-manager-product.sh
./scripts/check-repo.sh
```

要求：

- status-only ABI 只报告固定版本和布尔字段，不调用 Security.framework item API；
- macOS feature build 只能报告 `compiled=true`，独立实机资格和产品总 gate 保持关闭；
- smoke 同时受独立环境变量和显式产品场景门控，普通 Manager、Dart binding 和 InputMethodKit 不声明命令入口；
- C/Rust/Swift 固定摘要布局一致，输出不含 device/key id、公钥、shared secret、wrapping key、master key、nonce、ciphertext、CFError 文本或 query dictionary；
- 自动门禁只验证 gate-disabled、结构、符号和合成 crypto，不运行 gate-enabled smoke。

## 六个受控场景

专用脚本提供六个互斥入口；脚本启动前必须再次获得真实系统条目操作授权：

```text
--authorized-key-agreement-product-smoke
--authorized-key-agreement-denied-probe
--authorized-key-agreement-locked-prepare
--authorized-key-agreement-locked-probe
--authorized-key-agreement-locked-cleanup
--authorized-key-agreement-unsupported-probe
```

1. lifecycle：创建固定合成 identity 的独立 key，fresh store 重新取得同一 public key，执行 ECDH，并用 shared secret 解封内存中的合成 wrapped epoch；必须核对 descriptor 与 32-byte master material，随后精确删除并确认 fresh handle 返回 missing。
2. denied：在无合格权限的产品 host 中创建必须稳定映射为 access denied，且不得遗留 item。
3. locked prepare：创建固定合成 key，并在未锁定时以公开 P-256 测试点执行一次 ECDH；输出 `cleanup_required=true`。
4. locked probe：开发者在脚本倒计时内手动锁屏，产品只对固定 handle 执行 ECDH，必须稳定映射为 locked；脚本不得自行锁屏或操作 Keychain 状态。
5. locked cleanup：解锁后精确删除 prepare item，并确认 fresh handle missing。
6. unsupported：无 Secure Enclave 的真实设备、虚拟机或 CI 中创建必须返回 backend unavailable/unsupported，不得回退普通 DPK。

lifecycle 的通过条件是 `created/reloaded/public_key_matched/shared_secret_derived/wrapped_epoch_verified/deleted/missing_confirmed` 全部为真且 `cleanup_required=false`。任何中途失败仍必须由 native cleanup guard 尝试删除本场景的固定合成 item；cleanup 失败必须保留为显式失败结果，不能被原始错误覆盖或静默吞掉。

## 错误与日志语义

平台错误只映射到固定 category/detail 和数值 OSStatus：missing、revoked、locked、user-presence-required、access-denied、backend-unavailable、corrupted、other。错误设备/domain/key id、错误 epoch、AAD 或 ciphertext 篡改属于 Rust wrapped-material 认证失败，不能包装为平台可用性错误。

产品 stdout/stderr 只允许输出固定数值摘要和场景名。private key 永不离开 Secure Enclave；ECDH shared secret、HKDF wrapping key、解封后的 master key 只在 Rust 本次调用中短暂存在并按类型析构清零，不进入 Swift、Dart、SQLite、settings、日志、panic 或 FFI 返回值。

## 资格结论

- `compiled` 只说明目标包含 backend。
- `runtime_qualified` 需要受支持设备上的 lifecycle、denied、locked 和 cleanup 独立真实证据；unsupported 无环境时允许延期补测。
- `hardware_backed` 只有 Secure Enclave token 配置与独立 ECDH 实机证据共同成立后才可评审；不得从名称或签名证据推导。
- `exportable/user_presence_required/backup_migratable` 当前固定为 false；任何变化都需要新 schema、设计评审和实机证据。
- `product_qualified` 还要求 wrapped epoch 往返、错误矩阵、日志脱敏和清理审计全部通过。
- `user_sync_enabled` 继续为 false；该 runbook 通过也不接 Manager 成功入口。

## 当前实机证据

2026-07-19 已在 ad-hoc Manager bundle 执行 denied create：固定摘要返回 missing-entitlement `-34018`，`fail_closed/expected_failure/cleanup_attempted=true`，没有创建合成 item。随后使用 Team `WF9UUN335P`、application/access group `WF9UUN335P.dev.radishlex.radishlexManager`、profile `7ab763be-90d8-4f19-90ac-69a5681323af`（有效至 2026-07-25）恢复资格 bundle；构建入口的 strict codesign 通过。证书名称括号中的 `ZF6QRGH28J` 是持有人标识，证书 OU 才是 Team ID，不得混淆。

同一冻结 bundle 的 lifecycle 固定摘要为 `result=0 created=1 reloaded=1 public_key_matched=1 shared_secret_derived=1 wrapped_epoch_verified=1 deleted=1 missing=1 cleanup_required=0 cleanup_attempted=1`。locked prepare 在解锁态完成 ECDH并保留固定 item；开发者手动锁屏后 probe 返回 `PrivateKeyLocked/-25308`、`fail_closed/expected_failure=true`；解锁 cleanup 返回 `deleted=1 missing=1 cleanup_required=0`。本轮合成 item 已零残留。

冻结 hash：`Info.plist=780d2ecb...451ce2`、主程序 `d80e3ff2...b4b5b`、FFI dylib `ddea6306...d5fe`、profile `e816e592...922b`。完整值记录于本周周志。

锁屏链结束后，受限执行环境内的只读命令一度返回 `CSSMERR_TP_NOT_TRUSTED`、Authority unavailable 和 `0 valid identities`。真实登录会话中复验同一未改写产物，`security find-identity` 返回 `1 matching/1 valid identity`，`codesign --verify --deep --strict` 验证 app、嵌套 framework 与 FFI dylib 全部通过，四项冻结 SHA-256 均未变化。该现象已确定为执行隔离造成的信任评估假象，不要求重签、重建证书或修改 Keychain；以后资格验签若在沙盒内失败，必须先在获准的真实登录会话只读复验。按受支持 macOS 单设备主路径口径，`runtime_qualified/hardware_backed/product_qualified=true`；unsupported 延期补测，`user_sync_enabled=false`。
