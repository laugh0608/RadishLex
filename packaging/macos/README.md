# macOS 产品与安装元数据

本目录保存 macOS ABI/schema、distribution identity 和安装目标的 committed 真相源，面向产品装配、Installer 和发布门禁维护者。产品版本与 Flutter build number 的唯一人工真相源是仓库根 `version.json`；门禁把该值同步校验到本目录、Flutter 与 Xcode，不允许各构建系统独立维护版本。

- `product.json`：format v3 的 macOS 产品镜像，固定版本/build、最低系统、bundle ID、FFI ABI、userdb schema、RimeData/native manifest、`application-support-v1` 和 `community-adhoc-v1`。
- `install-layout.json`：format v1，固定 DMG + 独立 Installer、当前用户安装域、两个组件映射、Application Support、外层状态目录和默认保留数据移除语义。

当前 identity 不保存发布者 Team ID：strict ad-hoc 必须为 `TeamIdentifier=not set`。本目录也不保存证书、私钥、Apple 凭据、notary submission、绝对用户路径、receipt 或真实安装状态。未来 Developer ID 路径必须使用新的 distribution identity 和版本治理，不能改写 `community-adhoc-v1` 的语义。

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
./scripts/check-macos-installer.sh
./scripts/check-macos-release-carrier.sh
```

产品与发布各层证据固定为：

- `ProductManifest.json` format v3：绑定产品 metadata、双 bundle 完整 tree、许可证和 distribution identity；
- `InstallPayloadManifest.json` format v2：绑定 committed layout、target product 与显式历史 `UpgradeSources`；
- `ReleaseIdentity.json` format v2：绑定 target/历史 source 的双 component strict ad-hoc requirement 集合；
- `CommunityReleaseEvidence.json` format v1：绑定 DMG 文件名、大小、SHA-256 与 release identity SHA-256。

下列入口只写 ignored `target/`，不会安装程序、修改系统输入源或访问真实 Application Support：

```bash
./scripts/build-macos-product.sh
./scripts/build-macos-install-payload.sh
./scripts/build-macos-release-installer.sh
./scripts/build-macos-release-dmg.sh
```

发布入口按嵌套顺序重新生成 strict ad-hoc identity、manifest、sealed release identity 与 DMG evidence。目标目录已存在时拒绝覆盖；需要改变任何产品内容时，应提升根 `version.json` 的 build 并重新生成全部受影响证据，不能手工修改外层 JSON。
