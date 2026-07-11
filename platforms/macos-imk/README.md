# macOS InputMethodKit 薄壳

本目录实现 M1 第一平台的 InputMethodKit 薄壳、开发 bundle build 与不安装系统输入法的 contract smoke。它不包含 ranker/userdb 学习、同步、manager、发布签名、公证或复杂候选 UI，也不提供任何自动安装、注册或输入法服务重启动作。

## 结构

- `Sources/RadishLexBridge.*`：复制 Rust-owned key result、snapshot 与 candidate view，固定 owner-thread 和错误边界。
- `Sources/RadishLexInputController.*`：映射 `NSEvent`，更新 marked text，使用 `IMKCandidates` 展示原生候选并按稳定 index 提交。
- `Sources/RadishLexRuntime.*`：创建独立 Rime session；进程退出时先释放全部 session，再调用 `radishlex_rime_runtime_shutdown`。
- `build-bundle.sh`：构建 contract 或显式 native-rime 开发 bundle，不安装 bundle。
- `Tests/contract_smoke.m`：使用合成 demo engine 复验 ABI v2、完整按键映射、Unicode cursor、候选索引和生命周期，不读取 Rime 目录。

## 不安装验证

```bash
./scripts/check-macos-imk.sh
```

该入口会构建 `target/macos-imk/contract/RadishLex.app`，执行 Objective-C wrapper contract smoke，并检查 bundle、动态库加载路径、完整 ad-hoc 开发签名和关键 FFI symbol。contract bundle 只用于编译与契约复验，不能安装或作为真实输入证据。

## native-rime 开发 bundle

native build 不查找用户已有输入法目录，也不下载 schema。调用方必须显式提供 `librime` include/lib、一份隔离的 shared data 及其许可证文件；shared data 必须包含 `default.yaml` 和 `<schema-id>.schema.yaml`：

```bash
RIME_INCLUDE_DIR=<include> \
RIME_LIB_DIR=<lib> \
RADISHLEX_RIME_SHARED_DATA=<isolated-shared-data> \
RADISHLEX_RIME_SCHEMA=<schema-id> \
RADISHLEX_RIME_DATA_LICENSE=<license-file> \
./scripts/check-macos-imk-native.sh
```

`RADISHLEX_RIME_DEPLOY_ON_START` 可显式设为 `0` 或 `1`，默认 `1`。构建产物位于 `target/macos-imk/native/RadishLex.app`；bundle 同时保存全部 copied shared data 和许可证的 SHA-256 清单，并拒绝 shared data symlink。检查入口验证架构、plist、rpath、Rust symbol、`librime` 及其直接传递依赖，并默认使用 `codesign --sign -` 对嵌套 dylib、主程序和整个开发 bundle 依次签名与严格复验。可通过 `RADISHLEX_CODESIGN_IDENTITY` 显式提供 Apple Development identity；脚本不启动 bundle。它只复制调用方提供的数据到生成目录，不会写系统输入法目录、启动服务或修改系统配置。当前开发 bundle 仍从显式 `RIME_LIB_DIR` 对应的开发环境加载依赖，完整依赖封装、发布签名与普通用户分发属于 M4。

安装、启用、真实应用输入和移除会修改本机状态，必须另行取得授权后按独立 runbook 执行。
