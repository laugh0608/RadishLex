# Android Keystore Bridge

本文档说明 Android Keystore bridge 仓库内代码骨架。读者是后续接 Kotlin / JNI、Android instrumented smoke 和 `ime-crypto` Android backend 的开发者。本文不包含完整 Android app、Rust native JNI glue、设备矩阵结果或系统输入法 UI。

## 当前交付

- `build.gradle.kts`：独立 Android library harness，用于后续 Kotlin 编译和 instrumented smoke。
- `src/main/kotlin/org/radishlex/android/keystore/RadishLexAndroidKeystoreBridge.kt`
- `src/main/kotlin/org/radishlex/android/keystore/RadishLexAndroidKeystoreJniBridge.kt`
- `src/androidTest/kotlin/org/radishlex/android/keystore/RadishLexAndroidKeystoreBridgeInstrumentedTest.kt`
- `docs/smoke-record-template.md`
- Kotlin contract 常量与 `ime-crypto` 的 `android-keystore` feature 保持一致：
  - `contract_version = 1`
  - `provider = AndroidKeyStore`
  - `signature_algorithm = Ed25519`
  - operation：`create_signing_key`、`load_public_key`、`sign`、`delete_signing_key`
  - error code 白名单与 Rust `AndroidKeystoreBridgeErrorCode` 对齐

## 停止线

- 当前 Gradle harness 不代表完整 Android app 或 IME service。
- 未接 Rust native JNI 前，Rust `AndroidKeystoreDeviceKeyStore::new()` 仍应保持 unavailable。
- 未完成真实 Android API / 设备矩阵 smoke 前，不得解除 `android-keystore-v1` 生产签名门禁。
- 如果 `Ed25519` + `AndroidKeyStore` 无法创建、加载或签名，应返回 `unsupported_signature_algorithm` 或 `unsupported_storage_backend`，不得降级。

## 本机验证

不触碰真实 Android Keystore 的仓库默认验证：

```text
cargo test -p radishlex-ime-crypto --features android-keystore
./scripts/check-repo.sh
```

Android SDK、Gradle 和依赖可用时，可在本目录执行 Kotlin 编译或安装测试包。真实设备 smoke 必须显式传入参数：

```text
gradle connectedAndroidTest -Pradishlex.runAndroidKeystoreSmoke=true
```

不传该参数时，instrumented smoke 会跳过，不创建 Android Keystore item。

## 后续验证

后续进入真实设备 smoke 前，还应补：

- Rust native JNI glue 或等价平台调用层。
- API level、security patch、provider、设备型号和失败错误码记录。
- smoke 后清理步骤和日志脱敏检查。
- 将一次性结果复制到 `docs/smoke-record-template.md` 派生的记录中。
