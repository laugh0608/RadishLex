# Android Keystore Smoke Record Template

本文档是 `android-keystore-v1` 真实设备 smoke 的记录模板。读者是执行 Android instrumented smoke 的开发者和审阅同步隐私边界的维护者。本文不包含真实设备结果；执行 smoke 前必须复制为一次性记录，并避免写入真实账号、联系人、手机号、设备真实名称、完整 Keystore alias、canonical bytes、signature bytes、私钥、token 或用户输入内容。

## 执行前确认

- 执行者：
- 日期（Asia/Shanghai）：
- 已获批准触碰测试设备 Android Keystore：是 / 否
- 测试分支 / commit：
- 测试命令：

```text
./gradlew connectedAndroidTest -Pradishlex.runAndroidKeystoreSmoke=true
```

## 设备与工具链

- Android version：
- API level：
- security patch level：
- build tools：
- Android Gradle Plugin：
- Kotlin plugin：
- Keystore provider：
- `Signature.getInstance("Ed25519")` 结果：
- `KeyPairGenerator.getInstance("Ed25519", "AndroidKeyStore")` 结果：

## Smoke 结果

- create signing key：pass / fail / unsupported
- returned public key length：32 / other
- load public key after create：pass / fail
- sign canonical bytes：pass / fail
- signature length：64 / other
- verify signature with public key：pass / fail
- delete signing key：pass / fail
- load public key after delete：`private_key_unavailable` / other
- temporary key cleanup confirmed：yes / no

## Error Mapping

- observed bridge error code：
- mapped Rust error：
- 是否出现 fallback 到 seed / app storage / `test-memory-v1`：否 / 是
- 是否出现 alias、canonical bytes、signature bytes、private material 或 provider exception 原文泄漏：否 / 是

## Capability Metadata

- `available` 可否声明 true：
- `hardware_backed` 证据：
- `user_presence_required` 证据：
- `backup_migratable` 证据：

## 结论

- 是否解除 `android-keystore-v1` production status 门禁：
- 若不能解除，阻塞原因：
- 后续动作：
