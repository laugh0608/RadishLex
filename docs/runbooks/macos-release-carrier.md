# macOS 社区 ad-hoc DMG Runbook

本文指导发布维护者构建、复验和分发 `community-adhoc-v1` macOS DMG，读者是没有加入 Apple Developer Program 的项目维护者与安装用户。本文不宣称 Apple 开发者身份、notarization、Gatekeeper 自动放行或恶意软件扫描，也不授权上传 Release、修改系统输入源或清理历史安装事务材料。

## 发布口径

- 产品版本由仓库根 `version.json` 唯一确定；首发基线为 `26.7.1 (35)`，标准 tag 为 `v26.7.1-release`。
- `ProductManifest.json` format v3 固定 `distribution_identity=community-adhoc-v1`。
- Installer、Manager 与 InputMethod 使用严格 ad-hoc code signature。该签名用于检测包内意外变化和绑定事务 identity，不提供 Apple 认可的发布者认证。
- DMG 不签名、不提交公证、不含 ticket。`notarize-macos-release-dmg.sh` 在当前模式必须稳定失败关闭。
- 对外必须同时发布 DMG 与 `CommunityReleaseEvidence.json`；后者精确绑定版本、文件名、大小、DMG SHA-256 和 sealed release identity SHA-256。

## 构建

先完成产品装配门禁，再构建社区 Installer 与 DMG：

```bash
./scripts/build-macos-product.sh
./scripts/build-macos-release-installer.sh
./scripts/build-macos-release-dmg.sh
```

首发输出固定在：

```text
target/macos-release/26.7.1-35/
├── RadishLex Installer.app
├── RadishLex-26.7.1-35.dmg
├── CommunityReleaseEvidence.json
├── InstallPayload/
└── Product/
```

构建脚本不读取 `RADISHLEX_DEVELOPER_ID_APPLICATION`，不访问 Keychain、timestamp 或 notary 服务。若目标目录已存在，脚本拒绝覆盖；需要保留既有产物并使用新的 build number 重新构建。

## 发布前复验

```bash
./scripts/check-macos-release-carrier.sh

python3 scripts/macos-product/community_release.py verify \
  --carrier "$PWD/target/macos-release/26.7.1-35/RadishLex-26.7.1-35.dmg" \
  --identity "$PWD/target/macos-release/26.7.1-35/RadishLex Installer.app/Contents/Resources/ReleaseIdentity.json" \
  --evidence "$PWD/target/macos-release/26.7.1-35/CommunityReleaseEvidence.json"
```

发布页必须明确写明“未使用 Apple Developer ID、未公证，需要用户手动批准”，并直接列出 DMG SHA-256。不能使用“已签名”“Apple 已验证”“通过 Gatekeeper”或等价表述。

## 用户安装

用户应先对下载文件执行：

```bash
shasum -a 256 "$HOME/Downloads/RadishLex-26.7.1-35.dmg"
```

结果必须与发布页及 `CommunityReleaseEvidence.json` 的 `carrier_sha256` 完全一致。随后打开 DMG 并尝试启动 `RadishLex Installer.app`。macOS 阻止启动时，首选系统支持的人工路径：

1. 打开“系统设置 → 隐私与安全性”；
2. 在安全性区域确认刚才被阻止的是 `RadishLex Installer.app`；
3. 选择“仍要打开”，再次确认。

只有在 SHA-256 已核对、且用户理解该构建没有 Apple 发布者认证时，才使用终端 fallback。先把 Installer 从只读 DMG 复制到当前用户目录，再只移除这一个精确 bundle 的 quarantine：

```bash
mkdir -p "$HOME/Applications"
ditto "/Volumes/RadishLex Installer/RadishLex Installer.app" \
  "$HOME/Applications/RadishLex Installer.app"
xattr -dr com.apple.quarantine \
  "$HOME/Applications/RadishLex Installer.app"
open "$HOME/Applications/RadishLex Installer.app"
```

该用户域路径不需要 `sudo`。不得对 `/Applications`、`$HOME/Applications`、下载目录或磁盘根执行宽泛递归 `xattr`；不得把移除 quarantine 描述为签名或公证替代品。

## 升级与失败关闭

`ReleaseIdentity.json` format v2 记录 target 与全部显式历史 source 的 Manager/InputMethod ad-hoc designated requirement 集合。集合必须有界、排序、无重复，且两端不能重叠；运行时仍逐项复验 manifest、完整 tree、bundle ID、strict ad-hoc identity 和 release 顺序。

首发 `UpgradeSources` 为空。未来升级必须显式提供真实上一发布 assembly，不得用当前二进制改版本号伪造历史 source。identity、manifest、载体 evidence、receipt 或 startup gate 任一漂移都失败关闭，且不自动清理 staging、backup 或历史 operation。

## 未来 Developer ID 路径

仓库保留 Developer ID 解析和 notarization 证据模块作为未来可选能力，但它们不是当前 `community-adhoc-v1` 发布路径。未来若加入 Apple Developer Program，必须以新的 distribution identity/manifest 版本重新评审，不得静默把社区包升级为 Developer ID 包。

Apple 对未通过 App Store 分发的软件建议使用 Developer ID 与 notarization；对被阻止 App 的人工放行说明见：

- [Safely open apps on your Mac](https://support.apple.com/guide/mac-help/mh40616/mac)
- [Distributing software on macOS](https://developer.apple.com/documentation/xcode/distributing-your-app-for-beta-testing-and-releases)
