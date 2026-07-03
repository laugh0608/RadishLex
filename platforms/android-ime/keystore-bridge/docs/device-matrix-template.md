# Android Keystore Device Matrix Template

本文档是 `android-keystore-v1` 设备 / API / provider 行为矩阵记录模板。读者是执行 Android Keystore 诊断的开发者和审阅同步私钥边界的维护者。本文不包含真实设备结果；执行前必须复制为一次性记录，并避免写入完整 Keystore alias、canonical bytes、signature bytes、私钥、token、真实账号、联系人、手机号、设备真实名称或用户输入内容。

## 执行前确认

- 执行者：
- 日期（Asia/Shanghai）：
- 已获批准触碰测试设备 Android Keystore：是 / 否
- 测试分支 / commit：
- 测试命令：

```text
./gradlew connectedAndroidTest -Pradishlex.runAndroidKeystoreDiagnostics=true
```

## 设备与工具链

- Android version：
- API level：
- security patch level：
- device manufacturer：
- device model：
- AVD / physical device：
- Gradle Wrapper：
- Android Gradle Plugin：
- Kotlin plugin：

## Provider 与算法工厂

- `AndroidKeyStore` provider available：
- provider name：
- `Signature.getInstance("Ed25519")`：
- `KeyPairGenerator.getInstance("Ed25519", "AndroidKeyStore")`：

## Provider 直接生成结果

- key pair generate：
- generated public key algorithm：
- generated public key format：
- generated public key encoded length：
- generated public key encoded head：
- generated private key algorithm：
- `KeyInfo.insideSecureHardware`：
- `KeyInfo.securityLevel`：
- `KeyInfo.isUserAuthenticationRequired`：
- direct `Ed25519` sign with generated key：

## Bridge 映射结果

- bridge create：
- bridge load：
- observed bridge error code：
- 是否出现 fallback 到 seed / app storage / `test-memory-v1`：否 / 是
- 是否解除 `android-keystore-v1` production status 门禁：否 / 是

## Cleanup 与日志边界

- provider diagnostics key cleanup：
- bridge diagnostics key cleanup：
- 是否出现完整 alias、canonical bytes、signature bytes、private material 或 provider exception 原文泄漏：否 / 是
- 诊断输出前缀：

```text
radishlex.android_keystore.diagnostics
```

## 结论

- 当前设备是否可支持非导出 Ed25519 signing key：
- 若不能支持，阻塞原因：
- 后续动作：
