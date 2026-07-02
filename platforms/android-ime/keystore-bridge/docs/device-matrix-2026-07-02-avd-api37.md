# Android Keystore Device Matrix - 2026-07-02 AVD API 37

本文档记录一次 `android-keystore-v1` 设备 / API / provider 行为诊断。读者是后续实现 Android Kotlin bridge、NDK / JNI 调用层、`ime-crypto` backend 接线和审阅同步隐私边界的开发者。本文不包含完整 alias、canonical bytes、signature bytes、私钥、token、真实账号、联系人、手机号、设备真实名称或用户输入内容。

## 执行前确认

- 执行者：Codex；开发者已启动 Pixel 10 Pro API 37 AVD，并要求继续执行诊断。
- 日期（Asia/Shanghai）：2026-07-02
- 已获批准触碰测试设备 Android Keystore：是
- 测试分支 / commit：`dev` @ `e34c5cb`，加本次未提交 API 37 device matrix / 文档记录。
- 测试命令：

```text
JAVA_HOME=<Android Studio bundled JBR> ./gradlew connectedAndroidTest -Pradishlex.runAndroidKeystoreDiagnostics=true
```

## 设备与工具链

- Android version：17
- API level：37
- security patch level：2026-06-05
- device manufacturer：Google
- device model：`sdk_gphone16k_arm64`
- AVD / physical device：Pixel 10 Pro API 37 AVD
- Gradle Wrapper：9.0.0
- Android Gradle Plugin：8.7.3
- Kotlin plugin：2.0.21

## Provider 与算法工厂

- `AndroidKeyStore` provider available：true
- provider name：`AndroidKeyStore`
- `Signature.getInstance("Ed25519")`：success；algorithm `Ed25519`；provider `AndroidOpenSSL`
- `KeyPairGenerator.getInstance("Ed25519", "AndroidKeyStore")`：success；algorithm `Ed25519`；provider `AndroidKeyStore`

## Provider 直接生成结果

- key pair generate：success
- generated public key algorithm：`EC`
- generated public key format：`X.509`
- generated public key encoded length：91
- generated public key encoded head：`3059301306072a8648ce3d020106082a8648ce3d`
- generated private key algorithm：`EC`
- `KeyInfo.insideSecureHardware`：false
- `KeyInfo.securityLevel`：0
- `KeyInfo.isUserAuthenticationRequired`：false
- direct `Ed25519` sign with generated key：`error:InvalidKeyException`

## Bridge 映射结果

- bridge create：`error:unsupported_signature_algorithm`
- bridge load：`error:unsupported_signature_algorithm`
- observed bridge error code：`unsupported_signature_algorithm`
- 是否出现 fallback 到 seed / app storage / `test-memory-v1`：否
- 是否解除 `android-keystore-v1` production status 门禁：否

## Cleanup 与日志边界

- provider diagnostics key cleanup：deleted
- bridge diagnostics key cleanup：deleted
- 是否出现完整 alias、canonical bytes、signature bytes、private material 或 provider exception 原文泄漏：否
- 诊断输出前缀：

```text
radishlex.android_keystore.diagnostics
```

## 结论

- 当前设备是否可支持非导出 Ed25519 signing key：否。
- 阻塞原因：Pixel 10 Pro API 37 AVD 上，`Signature.getInstance("Ed25519")` 和 `KeyPairGenerator.getInstance("Ed25519", "AndroidKeyStore")` 均能取得对象，但 `AndroidKeyStore` 实际生成的 key pair 暴露为 `EC`，直接 Ed25519 签名失败，bridge 因此正确收敛为 `unsupported_signature_algorithm`。
- 后续动作：保持 `android-keystore-v1` production status 门禁关闭；继续用同一 diagnostics harness 扩展真实设备 / API / provider 矩阵，尤其是真机和不同 OEM。若考虑 P-256 或其他算法，必须先补签名算法 ADR、迁移计划和 Go / Rust verifier 变更。
