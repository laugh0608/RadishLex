# Android IME Platform Boundary

本文档说明 `platforms/android-ime/` 当前用途。读者是后续实现 Android Kotlin bridge、NDK / JNI 调用层、Android 输入法薄壳和同步设备管理入口的开发者。本文不包含完整键盘 UI、`InputMethodService` 主线、系统输入法安装流程或完整设备矩阵。

## 当前状态

- 当前只落地 `keystore-bridge/` 仓库内接线准备和 Android instrumented smoke harness。
- 该目录只服务于 `android-keystore-v1` 设备签名 backend。
- 当前没有完整 Android app、IME service、Flutter manager 或 Android target build 记录；Rust raw JNI glue 已在 `crates/ime-crypto` 接到 Kotlin facade。
- 2026-07-02 已在 Pixel 9 Pro API 35 AVD 上执行 gated smoke；`AndroidKeyStore` 返回 `EC` 公钥，bridge 结果为 `unsupported_signature_algorithm`，不声明 `android-keystore-v1` 可生产签名。
- 默认开发、仓库检查和 Android Gradle 编译不创建真实 Android Keystore key；只有显式传入 `radishlex.runAndroidKeystoreSmoke=true` 的 gated smoke 才允许触碰测试设备 Keystore。

## 边界

允许：

- 固定 Kotlin / JNI bridge contract。
- 提供独立 Android Gradle library harness 和 gated instrumented smoke。
- 使用 `AndroidKeyStore` provider 和 `Ed25519` 算法表达创建、加载、公钥读取、签名和删除操作。
- 将 Android public key 的 X.509 SubjectPublicKeyInfo 编码转换为 Rust contract 需要的 32-byte raw Ed25519 public key。
- 将平台错误收敛到 `ime-crypto` 已定义的 bridge error code。

禁止：

- 在 Android 输入热路径中做同步签名。
- 返回 seed、private key bytes、完整 alias、canonical bytes、signature bytes 或 provider exception 原文。
- fallback 到 `test-memory-v1`、软件 seed、SharedPreferences、SQLite、文件存储或 P-256。
- 在真实设备 smoke 通过前把 `android-keystore-v1` 标记为生产可用。

长期边界见 `docs/runbooks/android-keystore-signing-backend.md`。
