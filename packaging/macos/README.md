# macOS 产品与安装元数据

本目录保存 macOS 产品和安装目标的 committed 真相源，面向产品装配、Installer 和发布门禁维护者。它不保存签名身份、Apple 凭据、notary submission、绝对用户路径或真实安装状态。

- `product.json`：产品版本、build、bundle ID、FFI ABI、userdb schema、RimeData/native manifest 和数据布局。
- `install-layout.json`：M4-P03 分发容器、Installer 类型、当前用户安装域、两个组件映射、Application Support 和默认移除语义。

固定安装目标以当前用户 home 为基准：

```text
Applications/RadishLex Manager.app
Library/Input Methods/RadishLexInputMethod.app
Library/Application Support/RadishLex
```

校验入口：

```bash
./scripts/check-macos-product-metadata.sh
./scripts/check-macos-install-layout.sh
```

InstallPayload 装配只写 `target/`，不会安装程序或访问真实 Application Support：

```bash
./scripts/build-macos-install-payload.sh
```

该命令要求当前版本的 `target/macos-product/<version>-<build>/` 已由产品装配入口生成并通过 `ProductManifest.json` 复验。
