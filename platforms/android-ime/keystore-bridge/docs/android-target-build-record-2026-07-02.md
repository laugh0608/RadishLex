# Android Target Build Record - 2026-07-02

本文档记录一次 Android Rust target build 验证。读者是后续维护 Android Kotlin bridge、Rust raw JNI glue 和 `ime-crypto` Android Keystore backend 的开发者。本文不包含本机 SDK 绝对路径、完整 Keystore alias、canonical bytes、signature bytes、私钥、token、真实账号、联系人、手机号、设备真实名称或用户输入内容。

## 执行前确认

- 执行者：Codex
- 日期（Asia/Shanghai）：2026-07-02
- 目标：验证 `radishlex-ime-crypto` 的 `android-keystore` feature 可面向 `aarch64-linux-android` 编译。
- 是否触碰 Android Keystore：否
- 是否启动模拟器 / 设备 smoke：否

## 环境与工具链

- Rust target：`aarch64-linux-android`
- Android NDK：28.2.13676358 / r28c
- Android API level for clang：35
- Rust package：`radishlex-ime-crypto`
- Cargo feature：`android-keystore`

## 执行命令

首次 preflight 发现 Rust Android target 未安装：

```text
./scripts/check-android-target.sh --preflight-only
```

授权安装 target 后执行：

```text
rustup target add aarch64-linux-android
./scripts/check-android-target.sh
```

`./scripts/check-android-target.sh` 实际执行：

```text
cargo check -p radishlex-ime-crypto --features android-keystore --target aarch64-linux-android
```

## 结果

- Android SDK / NDK / API 35 clang preflight：pass
- Rust target availability：pass
- `cargo check` Android target build：pass
- Android target warning：初次检查发现非 Android fallback bridge 在 Android target 下 dead code；已通过 `cfg(not(target_os = "android"))` 收紧编译范围，复验后无 warning。

## 结论

- Rust raw JNI glue、Android Keystore bridge wrapper 和 `ime-crypto` Android feature 已具备 Android target 编译证据。
- 该验证不代表 Android Keystore 可生产签名；Pixel 9 Pro API 35 AVD smoke 仍为 `unsupported_signature_algorithm`。
- `android-keystore-v1` production status 门禁继续保持关闭；后续应按真实设备 / API / provider 矩阵调查原生非导出 Ed25519 支持。
