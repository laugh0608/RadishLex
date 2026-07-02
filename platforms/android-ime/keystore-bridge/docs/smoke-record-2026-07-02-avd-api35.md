# Android Keystore Smoke Record - 2026-07-02 AVD API 35

本文档记录一次 `android-keystore-v1` 真实 AVD smoke。读者是后续实现 Android Kotlin bridge、NDK / JNI 调用层、`ime-crypto` backend 接线和审阅同步隐私边界的开发者。本文不包含完整 alias、canonical bytes、signature bytes、私钥、token、真实账号、联系人、手机号、设备真实名称或用户输入内容。

## 执行前确认

- 执行者：Codex；开发者已启动 Android Studio 和 AVD，并要求执行 smoke。
- 日期（Asia/Shanghai）：2026-07-02
- 已获批准触碰测试设备 Android Keystore：是
- 测试分支 / commit：`dev` @ `766776f`，加本次未提交 Android bridge / Gradle / 文档修正。
- 测试命令：

```text
JAVA_HOME=<Android Studio bundled JBR> ./gradlew connectedAndroidTest -Pradishlex.runAndroidKeystoreSmoke=true
```

## 设备与工具链

- Android version：15
- API level：35
- security patch level：2024-09-05
- device model：`sdk_gphone16k_arm64`
- AVD：Pixel 9 Pro API 35
- Gradle Wrapper：9.0.0
- Android Gradle Plugin：8.7.3
- Kotlin plugin：2.0.21
- installed build tools：34.0.0、35.0.0、36.1.0、37.0.0
- Keystore provider：`AndroidKeyStore`
- `Signature.getInstance("Ed25519")` 结果：未进入 sign 阶段，未单独确认。
- `KeyPairGenerator.getInstance("Ed25519", "AndroidKeyStore")` 结果：调用未抛出 unsupported exception，但生成后的 certificate public key 为 `EC` / `X.509` / 91 bytes，不是 32-byte raw Ed25519 public key。

## Smoke 结果

- create signing key：unsupported
- returned public key length：other；bridge 未返回 public key，诊断到 Keystore certificate public key encoded length 为 91 bytes。
- returned public key metadata：algorithm `EC`，format `X.509`，DER header `3059301306072a8648ce3d020106082a8648ce3d`
- load public key after create：not reached
- sign canonical bytes：not reached
- signature length：not reached
- verify signature with public key：not reached
- delete signing key：cleanup attempted by test `finally`
- load public key after delete：not reached
- temporary key cleanup confirmed：未独立复验；测试失败路径已执行 `finally` 删除逻辑，下一次 smoke 开头也会先删除同一合成 alias。

## Error Mapping

- observed bridge error code：`unsupported_signature_algorithm`
- mapped Rust error：`UnsupportedSignatureAlgorithm { algorithm: "ed25519-v1" }`
- 是否出现 fallback 到 seed / app storage / `test-memory-v1`：否
- 是否出现 alias、canonical bytes、signature bytes、private material 或 provider exception 原文泄漏：否

## Capability Metadata

- `available` 可否声明 true：否
- `hardware_backed` 证据：无
- `user_presence_required` 证据：无
- `backup_migratable` 证据：无

## 结论

- 是否解除 `android-keystore-v1` production status 门禁：否
- 阻塞原因：Pixel 9 Pro API 35 AVD 上，`AndroidKeyStore` 在当前 Kotlin bridge 路径中没有产出 Ed25519 signing key；生成后的 certificate public key 为 `EC`，因此不能满足 RadishLex `ed25519-v1` 设备签名协议。
- 后续动作：保持 `android-keystore-v1` 不可生产签名；如继续调查 Android 原生非导出 Ed25519，应扩展真实设备 / API / provider 矩阵；如考虑 P-256，必须先补签名算法 ADR、迁移计划和 Go / Rust verifier 变更。
