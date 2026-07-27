# macOS 社区 ad-hoc DMG Runbook

本文指导发布维护者构建、复验和分发 `community-adhoc-v1` macOS DMG，读者是没有加入 Apple Developer Program 的项目维护者与安装用户。本文不宣称 Apple 开发者身份、notarization、Gatekeeper 自动放行或恶意软件扫描，也不授权上传 Release、修改系统输入源或清理历史安装事务材料。

## 发布口径

- 产品版本由仓库根 `version.json` 唯一确定；当前修复候选为 `26.7.1 (38)`，标准 tag 为 `v26.7.1-release`。`26.7.1 (35)` 把 quarantine 传播到最终程序，`26.7.1 (36)` 又无法清除只读 dylib 的 quarantine；二者均失效，不得作为首发 assembly。build 37 已完成真实安装证据，但因 Installer 生命周期代码变化不再作为最终发布载体。
- `ProductManifest.json` format v3 固定 `distribution_identity=community-adhoc-v1`。
- Installer、Manager 与 InputMethod 使用严格 ad-hoc code signature。该签名用于检测包内意外变化和绑定事务 identity，不提供 Apple 认可的发布者认证。
- DMG 不签名、不提交公证、不含 ticket。`notarize-macos-release-dmg.sh` 在当前模式必须稳定失败关闭。
- 对外必须同时发布 DMG 与 `CommunityReleaseEvidence.json`；后者精确绑定版本、文件名、大小、DMG SHA-256 和 sealed release identity SHA-256。
- 当前候选 `RadishLex-26.7.1-38.dmg` 大小为 `32196093` bytes，SHA-256 为 `f171e74bdc0a429655a84b30429481bce3926b17076d09298feed77d9ce4ce4e`；远端 draft 仅含匹配的 DMG、checksum 和 evidence，仍未发布且没有正式 Git tag。Chrome 独立下载副本已同时匹配大小与 SHA-256，逐字节比较一致，并带有真实 quarantine 和 GitHub Release 来源元数据；该副本的标准 Application 菜单、`⌘Q`、关闭最后窗口终止进程、`prepared` 重启续跑、首次安装、固定路径 Manager 启动、公开合成输入与 repair 已通过人工复验。双 bundle 在安装与 repair 后均精确匹配 manifest/release identity 且无 quarantine，repair 保持 Application Support、Rime 与 userdb inode；默认程序 remove 尚未闭合，不能称为正式冻结发布。

## 构建

先完成产品装配门禁，再构建社区 Installer 与 DMG：

```bash
./scripts/build-macos-product.sh
./scripts/build-macos-release-installer.sh
./scripts/build-macos-release-dmg.sh
```

首发输出固定在：

```text
target/macos-release/26.7.1-38/
├── RadishLex Installer.app
├── RadishLex-26.7.1-38.dmg
├── CommunityReleaseEvidence.json
├── InstallPayload/
└── Product/
```

构建脚本不读取 `RADISHLEX_DEVELOPER_ID_APPLICATION`，不访问 Keychain、timestamp 或 notary 服务。若目标目录已存在，脚本拒绝覆盖；需要保留既有产物并使用新的 build number 重新构建。

## 发布前复验

```bash
./scripts/check-macos-release-carrier.sh

python3 scripts/macos-product/community_release.py verify \
  --carrier "$PWD/target/macos-release/26.7.1-38/RadishLex-26.7.1-38.dmg" \
  --identity "$PWD/target/macos-release/26.7.1-38/RadishLex Installer.app/Contents/Resources/ReleaseIdentity.json" \
  --evidence "$PWD/target/macos-release/26.7.1-38/CommunityReleaseEvidence.json"
```

发布页必须明确写明“未使用 Apple Developer ID、未公证，需要用户手动批准”，并直接列出 DMG SHA-256。不能使用“已签名”“Apple 已验证”“通过 Gatekeeper”或等价表述。

## 用户安装

用户应先对下载文件执行：

```bash
shasum -a 256 "$HOME/Downloads/RadishLex-26.7.1-38.dmg"
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
xattr -drs com.apple.quarantine \
  "$HOME/Applications/RadishLex Installer.app"
open "$HOME/Applications/RadishLex Installer.app"
```

该用户域路径不需要 `sudo`。不得对 `/Applications`、`$HOME/Applications`、下载目录或磁盘根执行宽泛递归 `xattr`；不得把移除 quarantine 描述为签名或公证替代品。

用户人工放行只针对已核对摘要的 Installer。Installer 使用 `ditto --noqtn` 排除下载 quarantine，同时保留其他 xattr；固定 staging bundle 还必须通过 manifest、完整 tree、strict ad-hoc identity 和逐节点无 quarantine 审计。它不会修改文件权限或全局 Gatekeeper 设置。安装完成后若系统仍要求单独放行 Manager/InputMethod，或日志显示程序从 App Translocation 启动，应立即停止该候选，不要通过逐个“仍要打开”或宽泛 `xattr` 绕过。

## Installer 操作与故障处理

Installer 只管理以下当前用户对象，不写 `/Applications` 或系统级目录：

```text
~/Applications/RadishLex Manager.app
~/Library/Input Methods/RadishLexInputMethod.app
~/Library/Application Support/RadishLex
```

- 首次安装先持久化 `prepared`，提示用户手动切换到中立输入源并关闭 Manager；用户再次确认后才执行双 bundle 切换。安装完成后，用户仍需在系统设置中手动添加并选择 RadishLex 输入源。
- repair 只接受已安装的同一 release，重新复验并替换程序 bundle，不把它当作升级，也不删除 Application Support。
- upgrade 只在发布 payload 显式携带当前已安装 release 的历史 assembly 时可用。首发 payload 没有伪造的上一版本；缺少精确 source 时会在写 receipt 或切换程序前阻断。
- “移除程序”默认只移除 Manager 和 InputMethod，保留 userdb、settings、Rime 数据、receipt、staging、backup 与历史 operation。用户应先在系统设置中手动移除输入源；删除个人数据不是该动作的一部分。
- Installer 被关闭或异常退出后，应重新打开同一冻结 artifact，按显示的“继续”或“重试”从已持久化状态恢复。不要手工删除 receipt、`.radishlex-install-*`、staging、backup 或 Application Support 来绕过门禁。

`product_identity_unavailable` 表示当前 Installer/payload/sealed identity 缺失或漂移，应重新核对 DMG SHA-256 和发布来源；`driver_unavailable` 常见于请求升级但 payload 没有匹配的历史 source。active guard、非终态/损坏 receipt、未知对象、身份漂移或 completed remove 都会失败关闭。日志只输出稳定 `phase/action/error/state`，不会提供可安全复制执行的底层路径修复命令。

## 升级与失败关闭

`ReleaseIdentity.json` format v2 记录 target 与全部显式历史 source 的 Manager/InputMethod ad-hoc designated requirement 集合。集合必须有界、排序、无重复，且两端不能重叠；运行时仍逐项复验 manifest、完整 tree、bundle ID、strict ad-hoc identity 和 release 顺序。

首发 `UpgradeSources` 为空。未来升级必须显式提供真实上一发布 assembly，不得用当前二进制改版本号伪造历史 source。identity、manifest、载体 evidence、receipt 或 startup gate 任一漂移都失败关闭，且不自动清理 staging、backup 或历史 operation。

## 未来 Developer ID 路径

仓库保留 Developer ID 解析和 notarization 证据模块作为未来可选能力，但它们不是当前 `community-adhoc-v1` 发布路径。未来若加入 Apple Developer Program，必须以新的 distribution identity/manifest 版本重新评审，不得静默把社区包升级为 Developer ID 包。

Apple 对未通过 App Store 分发的软件建议使用 Developer ID 与 notarization；对被阻止 App 的人工放行说明见：

- [Safely open apps on your Mac](https://support.apple.com/guide/mac-help/mh40616/mac)
- [Distributing software on macOS](https://developer.apple.com/documentation/xcode/distributing-your-app-for-beta-testing-and-releases)
