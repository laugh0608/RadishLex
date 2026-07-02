# Android Keystore Bridge

本文档说明 Android Keystore bridge 仓库内代码骨架。读者是后续接 Kotlin / JNI、Android instrumented smoke 和 `ime-crypto` Android backend 的开发者。本文不包含完整 Android Gradle 工程、JNI glue、设备矩阵结果或系统输入法 UI。

## 当前交付

- `src/main/kotlin/org/radishlex/android/keystore/RadishLexAndroidKeystoreBridge.kt`
- Kotlin contract 常量与 `ime-crypto` 的 `android-keystore` feature 保持一致：
  - `contract_version = 1`
  - `provider = AndroidKeyStore`
  - `signature_algorithm = Ed25519`
  - operation：`create_signing_key`、`load_public_key`、`sign`、`delete_signing_key`
  - error code 白名单与 Rust `AndroidKeystoreBridgeErrorCode` 对齐

## 停止线

- 该目录当前不是可独立构建的 Android 工程。
- 未接 JNI 前，Rust `AndroidKeystoreDeviceKeyStore::new()` 仍应保持 unavailable。
- 未完成真实 Android API / 设备矩阵 smoke 前，不得解除 `android-keystore-v1` 生产签名门禁。
- 如果 `Ed25519` + `AndroidKeyStore` 无法创建、加载或签名，应返回 `unsupported_signature_algorithm` 或 `unsupported_storage_backend`，不得降级。

## 后续验证

后续进入真实设备 smoke 前，应先补：

- Gradle / Android instrumented test 最小工程。
- JNI 或等价平台调用层。
- 临时 test key 的 alias 生成和删除策略。
- API level、security patch、provider、设备型号和失败错误码记录。
- smoke 后清理步骤和日志脱敏检查。
