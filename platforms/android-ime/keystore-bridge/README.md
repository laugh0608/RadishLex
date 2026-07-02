# Android Keystore Bridge

本文档说明 Android Keystore bridge 仓库内代码骨架。读者是后续接 Android instrumented smoke、设备矩阵诊断和 `ime-crypto` Android backend 的开发者。本文不包含完整 Android app 或系统输入法 UI；Rust raw JNI glue 位于 `crates/ime-crypto`。

## 当前交付

- `build.gradle.kts`：独立 Android library harness，用于后续 Kotlin 编译和 instrumented smoke。
- `src/main/kotlin/org/radishlex/android/keystore/RadishLexAndroidKeystoreBridge.kt`
- `src/main/kotlin/org/radishlex/android/keystore/RadishLexAndroidKeystoreJniBridge.kt`
- `src/androidTest/kotlin/org/radishlex/android/keystore/RadishLexAndroidKeystoreBridgeInstrumentedTest.kt`
- `src/androidTest/kotlin/org/radishlex/android/keystore/RadishLexAndroidKeystoreDiagnosticsInstrumentedTest.kt`
- `docs/smoke-record-template.md`
- `docs/smoke-record-2026-07-02-avd-api35.md`：Pixel 9 Pro API 35 AVD 真实 smoke 记录，结果为 `unsupported_signature_algorithm`。
- `docs/device-matrix-template.md`
- `docs/device-matrix-2026-07-02-avd-api35.md`：Pixel 9 Pro API 35 AVD provider / API 诊断记录，确认 factory 表面可用但生成 key 为 `EC`，Ed25519 签名失败。
- `docs/device-matrix-2026-07-02-avd-api37.md`：Pixel 10 Pro API 37 AVD provider / API 诊断记录，确认 factory 表面可用但仍生成 key 为 `EC`，Ed25519 签名失败。
- `docs/android-target-build-record-2026-07-02.md`：Android Rust target build 记录，确认 `ime-crypto` Android Keystore bridge wrapper / raw JNI glue 可面向 `aarch64-linux-android` 编译。
- Kotlin contract 常量与 `ime-crypto` 的 `android-keystore` feature 保持一致：
  - `contract_version = 1`
  - `provider = AndroidKeyStore`
  - `signature_algorithm = Ed25519`
  - operation：`create_signing_key`、`load_public_key`、`sign`、`delete_signing_key`
  - error code 白名单与 Rust `AndroidKeystoreBridgeErrorCode` 对齐

## 停止线

- 当前 Gradle harness 不代表完整 Android app 或 IME service。
- Rust raw JNI glue 已接到 Kotlin facade，并已通过 Android target build；在设备矩阵证明 Android Keystore 可用前，Rust `AndroidKeystoreDeviceKeyStore::backend_status()` 仍应阻断 production signing。
- Pixel 9 Pro API 35 AVD 的真实 smoke 与 provider diagnostics 已执行但未通过；Pixel 10 Pro API 37 AVD 的 provider diagnostics 也未通过。两者均表现为 `AndroidKeyStore` 返回 `EC` public key，直接 Ed25519 签名失败；不得解除 `android-keystore-v1` 生产签名门禁。
- 如果 `Ed25519` + `AndroidKeyStore` 无法创建、加载或签名，应返回 `unsupported_signature_algorithm` 或 `unsupported_storage_backend`，不得降级。

## 结果解读

`Signature.getInstance("Ed25519")` 或 `KeyPairGenerator.getInstance("Ed25519", "AndroidKeyStore")` 返回 success 只说明 JCA factory 能创建对象，不代表 `AndroidKeyStore` 已能保存非导出 Ed25519 signing key。判断 backend 是否可用必须同时看生成 key 的 algorithm / format / length、`KeyInfo`、直接签名结果和 bridge error code。

当前矩阵结论：

| 环境 | `Signature` provider | `KeyPairGenerator` provider | 生成 key | bridge 结果 |
| --- | --- | --- | --- | --- |
| Pixel 9 Pro API 35 AVD | `AndroidKeyStoreBCWorkaround` | `AndroidKeyStore` | `EC` / `X.509` / 91 bytes | `unsupported_signature_algorithm` |
| Pixel 10 Pro API 37 AVD | `AndroidOpenSSL` | `AndroidKeyStore` | `EC` / `X.509` / 91 bytes | `unsupported_signature_algorithm` |

这两条记录只证明对应 AVD 环境不可用；后续仍需扩展真机、不同 OEM、不同 system image 和不同 security patch 的设备矩阵。

## 本机验证

不触碰真实 Android Keystore 的仓库默认验证：

```text
cargo test -p radishlex-ime-crypto --features android-keystore
./scripts/check-android-target.sh
./scripts/check-repo.sh
```

Android SDK、Gradle 和依赖可用时，可在本目录执行 Kotlin 编译或安装测试包。真实设备 smoke 必须显式传入参数：

```text
./gradlew connectedAndroidTest -Pradishlex.runAndroidKeystoreSmoke=true
```

不传该参数时，instrumented smoke 会跳过，不创建 Android Keystore item。

设备 / API / provider diagnostics 必须显式传入参数：

```text
./gradlew connectedAndroidTest -Pradishlex.runAndroidKeystoreDiagnostics=true
```

诊断只输出 `radishlex.android_keystore.diagnostics` 前缀的非敏感字段，并在 `finally` 中删除合成诊断 key。记录结果时使用 `docs/device-matrix-template.md` 派生一次性记录。

如果 Android Studio bundled JBR 不在全局 `PATH`，按本机实际安装路径设置 `JAVA_HOME`。例如 macOS 用户级 Android Studio 常见路径：

```text
JAVA_HOME=~/Applications/Android Studio.app/Contents/jbr/Contents/Home
```

## 后续验证

后续进入真实设备 smoke 前，还应补或确认：

- Android target Rust build 与 Android Gradle build 结果。
- API level、security patch、provider、设备型号、KeyInfo、直接签名结果和失败错误码记录。
- smoke 后清理步骤和日志脱敏检查。
- 将 smoke 结果复制到 `docs/smoke-record-template.md` 派生的记录中，将 provider diagnostics 结果复制到 `docs/device-matrix-template.md` 派生的记录中。
