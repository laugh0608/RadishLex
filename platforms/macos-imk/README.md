# macOS InputMethodKit 薄壳

本目录实现 M1 第一平台的 InputMethodKit 薄壳、开发 bundle build 与不安装系统输入法的 contract smoke。它不包含 ranker/userdb 学习、同步、manager、发布签名、公证或复杂候选 UI，也不提供任何自动安装、注册或输入法服务重启动作。

## 结构

- `Sources/RadishLexBridge.*`：复制 Rust-owned key result、snapshot 与 candidate view，固定 owner-thread 和错误边界。
- `Sources/RadishLexInputController.*`：映射 `NSEvent`，更新 marked text，以唯一 display index 驱动候选视觉和 Rust selection。
- `Sources/RadishLexCandidatePanel.*`：进程级非激活 AppKit 候选面板，负责 owner 生命周期、焦点隔离、全局定位、多屏限制、鼠标/辅助功能 index 回调和原生视觉；不承载 engine 或候选排序。
- `Sources/RadishLexRuntime.*`：创建独立 Rime session；进程退出时先释放全部 session，再调用 `radishlex_rime_runtime_shutdown`。
- `build-bundle.sh`：构建 contract 或显式 native-rime 开发 bundle，不安装 bundle。
- `Tests/contract_smoke.m`：使用合成 demo engine 复验 ABI v3、完整按键映射、Unicode cursor、候选选择结果和生命周期，不读取 Rime 目录。
- `Tests/candidate_panel_contract.m`：创建真实 AppKit panel/control，复验视觉与 accessibility selection、appearance、anchor fallback、owner 接管和完整隐藏。
- `Tests/input_controller_contract.m`：使用正式 controller、panel 和 Rust demo session，贯通方向 keyDown/keyUp、Space、鼠标、accessibility press、Enter、Escape、宿主快捷键和双 client 生命周期。
- `ReferenceProbe/`：隔离验证原生候选事件路由与单 mode 输入源 metadata，不链接 Rime 或正式 FFI，也不替代产品薄壳。

## 不安装验证

```bash
./scripts/check-macos-imk.sh
```

该入口会构建 `target/macos-imk/contract/RadishLexInputMethod.app`，执行 Objective-C wrapper、真实 AppKit candidate panel 和 controller-to-commit contract，并检查 bundle、动态库加载路径、完整 ad-hoc 开发签名和关键 FFI symbol。contract-only initializer/inspection API 不进入 native 产品。contract bundle 只用于编译与契约复验，不能安装或作为真实输入证据。

候选事件与单 mode metadata 的隔离 probe 使用独立入口：

```bash
./scripts/check-macos-imk-reference-probe.sh
```

该入口只构建并静态验证 `target/macos-imk/reference-probe-mode/RadishLexIMKModeReferenceProbe.app`，不会安装、注册、选择或启动输入法；完整停止线与清理顺序见 `ReferenceProbe/README.md`。

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

`RADISHLEX_RIME_DEPLOY_ON_START` 可显式设为 `0` 或 `1`，默认 `1`。构建产物位于 `target/macos-imk/native/RadishLexInputMethod.app`；bundle 同时保存 copied shared data、数据许可证和哈希清单，并拒绝 shared data symlink。native build 从显式 `RIME_LIB_DIR` 解析依赖，但运行产物会递归复制全部非系统 dylib 到 `Contents/Frameworks`、重写为 bundle 内 `@rpath`，并保存逐库许可证和签名后哈希清单；门禁拒绝残留外部绝对依赖。脚本对每个 dylib、主程序和完整 bundle 依次签名与严格复验，可通过 `RADISHLEX_CODESIGN_IDENTITY` 显式提供 Apple Development identity。

bundle metadata 固定正式 Bundle ID `org.radishlex.inputmethod.macos` 与单一 `org.radishlex.inputmethod.macos.Pinyin` 模式，包含简体中文 script/repertoire、图标、本地化标签和 `LSUIElement`。正式 bundle 文件名固定为 `RadishLexInputMethod.app`；开发期不再复用已被 macOS 26 TIS 负缓存的旧 ID 或 `RadishLex.app` 路径。contract/native 门禁会验证 mode id 的 reverse-DNS 字符范围，避免把允许下划线的 `pinyin_simp` schema id 直接用作 TIS mode id。构建脚本不启动或安装 bundle；普通用户分发、Developer ID、公证和发布级供应链门禁仍属于 M4。

安装、启用、真实应用输入和移除会修改本机状态，必须另行取得授权后按独立 runbook 执行。
