# macOS 产品包边界

本文定义 RadishLex M4 macOS 产品发布候选的组件、版本、数据、安装、签名和验证边界，读者是产品构建、InputMethodKit、Manager、Installer 与发布门禁的维护者。本文不记录具体构建流水、Apple 凭据、真实安装操作或历史验收结果；可重复构建与复验步骤见 [macOS 产品装配 Runbook](runbooks/macos-product-assembly.md)，安装载体决策见 [ADR 0008](adr/0008-macos-installation-carrier.md)。

## 目标与范围

M4 macOS 产品包必须把以下已有能力组织成同一版本、可验证且可升级的产品：

- `RadishLexInputMethod.app`：InputMethodKit 输入法薄壳；
- `radishlex_manager.app`：本地数据、隐私和诊断管理界面；
- `libradishlex_ime_ffi.dylib`：两端调用的 Rust ABI；
- 两端各自的 `Contents/Helpers/RadishLexUpgradeValidationHost`：使用本 bundle native library 验证固定 migration candidate 或最终 userdb；
- `librime` 及其非系统传递依赖；
- 合法来源的 Rime schema/data、RadishLex 许可证和第三方许可证；
- 产品 manifest、安装升级 runbook 和发布验证证据。

本阶段不开放真实用户同步，不引入第二平台，也不把 InputMethodKit 的输入热路径迁入 Manager。发布产品不能依赖 Homebrew 绝对路径、开发者 shell 环境变量、仓库 `target/` 路径或静默 fixture。

## 稳定产品组成

### 双 bundle 保持独立

InputMethod 与 Manager 是同一产品版本下的两个独立 bundle，不互相嵌套：

- InputMethod 由系统输入法生命周期启动，不能依赖 Manager 正在运行；
- Manager 可以检查 InputMethod 安装和兼容性，但不是输入热路径守护进程；
- 两端各自携带匹配的 Rust native library，不能从另一个可移动 bundle 加载 dylib；
- 两端各自携带同名但职责不同的 upgrade validation helper，不能互换或由单一通用 helper 代替；
- InputMethod 额外携带 `librime`、RimeData 与对应许可证；
- Manager 只携带其真实调用所需的 native dependency，不为目录对称复制 `librime`。

M4-P01 的稳定装配产物是版本化产品目录及其 manifest。M4-P03 首发选择社区 ad-hoc DMG 中的独立用户域 Installer app；该载体需要人工放行，不具备 Apple 发布者认证或公证资格。产品装配目录与 InstallPayload 仍不能单独称为用户安装包。

只读 `RadishLexUpgradePreflightHost` 是 Installer/协调器调用的平台宿主，固定嵌入 target Manager 并进入产品 manifest，但不参与 Manager 普通业务。Installer 必须通过 manifest-bound adapter 调用它，不能临时改成脚本、UI 进程检查或调用方自报容量。平台宿主的固定输入与只读边界见 [macOS 产品升级宿主说明](../platforms/macos-product/README.md)。

### 产品 manifest

产品装配必须生成 `ProductManifest.json`，至少绑定：

- manifest format、产品 ID、产品版本、build number、最低 macOS 和 distribution identity；
- Manager 与 InputMethod 的 bundle ID、版本和 build；
- FFI ABI、userdb schema、Rime schema ID、RimeData manifest 和 native libraries manifest 版本；
- 两个 bundle 内所有普通文件的相对路径、大小和 SHA-256，以及安全内部 symlink 的相对目标；
- 仓库许可证文件的相对路径、大小和 SHA-256。

manifest 不包含证书、私钥、designated requirement、Apple 凭据、构建机绝对路径、用户目录、时间戳或输入数据。format v3 只声明 `distribution_identity=community-adhoc-v1`；相同输入 bundle 必须生成字节一致的 manifest，具体 ad-hoc requirement 由 sealed release identity 另行记录。

manifest 是产品完整性与兼容性证据，不构成 Apple code signature、notarization ticket 或 Gatekeeper 验证。

### InstallPayload 与历史升级源

`InstallPayloadManifest.json` format v2 固定 target `Product/` 与 `UpgradeSources/`。历史 source 必须作为构建时显式输入复制进 payload；每项绑定精确 product version/build、规范化 `UpgradeSources/<version>-<build>` 路径和自己的 ProductManifest。ProductManifest 再绑定许可证、Manager/InputMethod 完整文件树与 upgrade validation host；运行时还必须命中 sealed release identity 中对应 component 的严格 ad-hoc designated requirement。

source build 必须唯一、严格早于 target，不能使用 target 副本、qualification fixture、调用方路径或只改版本号的当前代码替代。source 只供旧版本 validation/rollback host 执行，不作为 target staging 内容。首发没有历史正式发布时，`UpgradeSources` 保持空集合；这会让 production upgrade 在外层 receipt 或程序 mutation 前稳定阻断，而不是降级到未绑定 helper。

### RimeData 来源与许可证

`packaging/rime/product-rime-data.json` 是首个候选的 RimeData 来源锁。产品构建只从锁定的 committed 文件离线装配，不在构建时联网，不读取用户、Squirrel 或其他输入法的数据目录。锁必须绑定每个资产的仓库来源、完整 commit、源路径、运行时路径、SHA-256 和许可证映射。

首个候选固定使用 RadishLex 维护的 `radishlex_pinyin` schema 和 Apache-2.0 `pinyin_simp.dict.yaml`。产品 schema 保留简体全拼、用户词典、常用中西文标点和候选翻页，明确移除 upstream `stroke` reverse lookup 及 `prelude` preset 导入；因此不携带 LGPL `rime-stroke`、`rime-prelude`、`luna_pinyin` 或 `essay`。若未来增加笔画反查或扩展符号表，必须重新完成行为设计、来源与逐包许可证评审，不能向现有锁静默追加文件。

装配后的 `RimeData/` 必须携带与锁字节一致的 `SourceManifest.json` 和 `Licenses/<component>/LICENSE|AUTHORS`。RimeData manifest v2 对所有数据和许可证文件计算 hash，并单独绑定来源 manifest 与许可证集合；额外文件、hash 漂移、空许可证或 symlink 均失败关闭。

## 单一版本真相源

仓库根 `version.json` 是产品版本与 Flutter build number 的唯一人工真相源；`packaging/macos/product.json` 是 macOS ABI、schema、布局和 distribution identity 真相源。门禁必须验证两者及各构建系统镜像一致。

当前首个 M4 候选固定：

| 字段 | 值 | 约束 |
| --- | --- | --- |
| product ID | `radishlex-macos` | manifest 稳定标识 |
| ProductManifest format | `3` | v3 使用显式 distribution identity；旧结构失败关闭 |
| product version | `26.7.1` | Radish `YY.M.RELEASE`，Manager 与 InputMethod 相同 |
| build number | `35` | 正整数且两个 bundle 相同 |
| minimum macOS | `13.0` | 取两端真实支持范围的交集 |
| FFI ABI | `9` | 保留数据 startup/validation contract，增加独立外层 install startup gate |
| userdb schema | `9` | 不允许旧产品打开未来 schema |
| RimeData manifest | `2` | 绑定来源锁、多许可证与完整数据 hash |
| data layout | `application-support-v1` | 首版继续使用已验证布局 |
| distribution identity | `community-adhoc-v1` | 未使用 Apple 发布者认证或公证 |

build number 只描述产品构建，不替代 schema 或 ABI。任何 ABI、数据库、RimeData 或 native manifest 格式变化都必须独立递增对应版本，并更新兼容测试。

Cargo workspace 中的 `0.1.0` 是未独立发布 crates 的内部包版本，不是 RadishLex 产品版本或 macOS bundle 版本；它不进入用户发布文件名、ProductManifest、receipt release 或 tag。

## 兼容性与启动失败语义

### 构建期

产品装配在复制产物前必须验证：

- 两个 bundle 的 `CFBundleShortVersionString` 和 `CFBundleVersion` 完全一致；
- bundle ID 与产品元数据一致；
- `LSMinimumSystemVersion` 不低于产品最低版本；
- Rust contract、Dart expected ABI 与 manifest 声明一致；
- userdb 当前 schema 与 manifest 声明一致；
- InputMethod 的 RimeData 和 native library manifests 存在且可复算；
- 所有非系统 Mach-O dependency 都解析到本 bundle 的 `Frameworks`；
- schema/data、native libraries 和许可证无 symlink；bundle 只允许目标仍位于同一 bundle 内的框架结构 symlink；
- bundle 中不存在构建目录或 Homebrew 绝对依赖。

任一条件失败时不得生成最终产品目录。

### 运行期

Manager 与 InputMethod 都必须先执行 ABI v9 外层 install gate，再执行数据 upgrade gate，并对下列情况失败关闭并返回稳定错误：

- FFI ABI 不匹配或必需 symbol 缺失；
- startup gate 返回阻止/失败结果，或 active guard、非终态/损坏 receipt、中断 artifact、未知对象与身份漂移无法排除；
- userdb schema 高于当前支持版本；
- 数据路径不是预期目录/普通文件，或权限无法收紧；
- RimeData、native dependency 或 manifest 缺失/损坏；
- 输入法 bundle 与 Manager 产品版本不兼容。

不能通过加载旧 dylib、创建空数据库、删除升级状态、改用 fixture 或忽略 manifest 来掩盖错误。Manager 的两层门禁必须早于 Flutter/settings/userdb，InputMethod 的两层门禁必须早于 `IMKServer`/Rime runtime；外层运行身份只能由当前 executable 所在固定用户域 bundle 的 Info.plist、完整 tree 与 code identity 形成。只有两层 gate 都返回已知允许结果才能继续。

## 数据布局与所有权

### 首个发布候选保持 Application Support v1

首个 M4 候选继续使用已通过 M1/M2 实机验证的用户级布局：

```text
~/Library/Application Support/RadishLex/
  userdb.sqlite3
  userdb.sqlite3-wal
  userdb.sqlite3-shm
  manager-settings.json
  Rime/
```

本批不迁入 App Group，理由如下：

- Manager 与 InputMethod 当前均为非 App Sandbox 产品形态；
- 现有目录已完成权限、并发、migration、删除恢复和真实双端复验；
- App Group 会同时改变 entitlement、签名、容器 URL、迁移和回滚条件；
- 当前没有必须依赖 App Group 才能满足的产品需求。

若未来进入 App Sandbox、Mac App Store 或出现明确的共享容器要求，必须新增 ADR，先证明 entitlement 与两端进程访问能力，再设计原子迁移。不能在普通升级中静默切换容器。

### 数据职责

- `ime-userdb` 是 SQLite schema、事务 migration 和数据一致性真相源；
- InputMethod 与 Manager 都只能通过 Rust userdb 边界访问数据库；
- Manager host 负责解析固定平台路径和收紧目录/文件权限，不解释业务数据；
- Rime runtime 负责 `Rime/` 用户数据，不能与 P2 userdb 混为同一同步对象；
- 安装器只安装程序组件，不创建、迁移或删除真实用户内容；
- 升级协调器负责进程静止、备份 receipt、兼容性预检、切换与回滚，不解析明文词条。

协调器状态、SQLite 一致快照、receipt、中断恢复和双端验证的详细 contract 见 [macOS 数据升级协调器边界](macos-data-upgrade-coordinator.md)。

### 升级与回滚原则

M4 数据升级必须覆盖当前布局内的 schema 演进与程序版本切换：

1. 升级前确认 InputMethod 与 Manager 不再持有写连接；
2. 记录固定路径、原产品版本、schema、文件身份和权限，不记录明文内容；
3. 使用 `ime-userdb` 在隔离副本上执行 migration 与完整性检查；
4. 只有副本通过新版本双端打开验证后才切换；
5. 失败时保留原 bundle、原数据库和原 settings；
6. 回滚不得让旧版本打开其不支持的未来 schema；
7. 删除旧备份必须是有版本、receipt 与明确目标的后续动作。

不能逐个复制 SQLite WAL/SHM 后假设得到一致快照，也不能通过删除数据库解决 migration 失败。

## 安装与移除边界

M4-P03 首个候选固定使用未公证的 APFS/UDZO UDIF DMG，内部只提供独立 `RadishLex Installer.app`。Installer 在当前用户会话运行，不请求管理员权限，也不把自身安装为持久产品组件。用户必须先核对 SHA-256，再通过“隐私与安全性 → 仍要打开”或精确用户域 `xattr` fallback 放行。纯拖拽 DMG、ZIP、Manager 自安装和 `.pkg` 均不作为首个候选主路径；取舍与未来 `.pkg` 重新评估条件见 [ADR 0008](adr/0008-macos-installation-carrier.md)。

`packaging/macos/install-layout.json` 是安装目标真相源，路径均相对 authoritative current-user home：

```text
Applications/RadishLex Manager.app
Library/Input Methods/RadishLexInputMethod.app
Library/Application Support/RadishLex
Library/Application Support/RadishLex/.radishlex-install-v1
```

安装与移除继续遵守：

- Installer 不接受自定义目标，用户能在变更前看到两个程序路径和保留数据语义；
- 用户只需人工放行已核对摘要的 Installer；安装事务使用 `ditto --noqtn` 阻止下载 quarantine 传播，随后对固定 staging tree 逐节点审计无 quarantine，并重复 manifest/tree/strict ad-hoc identity 复验；最终 Manager/InputMethod 不得被隔离或从 App Translocation 启动；
- 工具不得程序化选择或模拟切换输入源；
- 升级前后使用公开 API 只读确认输入源状态；
- 默认移除只删除产品 bundle，保留 Application Support 数据；
- “移除程序并删除数据”必须单独授权、检查 receipt、关闭数据库并限制到固定文件集合；
- 安装、升级和回滚脚本不得暴露通用递归删除或调用方自定义删除路径。

InstallPayload 固定包含 committed layout、外层 payload manifest 和完整 Product 目录。payload manifest 绑定产品 manifest hash、layout hash、版本/build、Installer bundle ID、两个 component-to-target 映射和移除语义；产品 manifest 继续绑定 bundle 内全部文件。`./scripts/build-macos-install-payload.sh` 只写 `target/`，不执行安装。

程序切换还需要 `.radishlex-install-v1` 外层 receipt/guard。两端 startup gate 必须在业务初始化前拒绝非终态、损坏或身份漂移的程序事务；数据 receipt 终态不能绕过尚未完成的程序切换。真实安装、系统设置、进程停止、签名、公证和数据清理仍遵守仓库授权规则。

独立 Installer 的 UI/驱动 contract 已固定。UI 只消费版本化 snapshot，展示 verified operation、receipt 进度、固定目标、稳定错误和默认保留数据语义；未知 snapshot 失败关闭。所有 mutation action 都重新验证当前 snapshot 并要求显式确认，remove 额外确认保留 Application Support，手动静止确认不能替代公开平台 API 的重新取证。详细边界见 [macOS Installer App UI 与驱动边界](macos-installer-app-boundary.md)。

## 社区身份与发布供应链

首发 `community-adhoc-v1` 不要求付费 Apple Developer Program：

- Installer、Manager 与 InputMethod 仅使用严格 ad-hoc code signature；它用于结构和事务 identity 复验，不提供发布者认证；
- `ReleaseIdentity.json` format v2 绑定 target 与全部历史 source 的双 component requirement 有界集合，不自绑定 Installer `cdhash`，避免签名内容循环；
- DMG 不签名、不公证、不 staple；`notarize-macos-release-dmg.sh` 在社区模式稳定拒绝执行；
- `CommunityReleaseEvidence.json` 绑定版本、DMG 文件名、大小、SHA-256 和 release identity SHA-256；
- 发布说明必须明确提示未签名/未公证、人工放行和 SHA-256 核对，不得暗示 Apple 已验证。

`./scripts/build-macos-release-installer.sh` 不读取 Developer ID 环境变量，按嵌套顺序重新生成 strict ad-hoc identity、ProductManifest、InstallPayloadManifest 与 sealed release identity。`./scripts/build-macos-release-dmg.sh` 生成根目录精确只有 Installer 的 APFS/UDZO DMG，挂载复验后写出社区发布证据。详细操作见 [macOS 社区 ad-hoc DMG Runbook](runbooks/macos-release-carrier.md)。

unknown distribution identity、非 ad-hoc TeamIdentifier/Signature/CodeDirectory、requirement 集合乱序/重复/重叠、manifest/tree/bundle ID 漂移、DMG/evidence 漂移都失败关闭。未来 Developer ID 路径必须使用新的 distribution identity 和版本治理重新开放，不能静默改变社区包语义。

Apple 官方边界参考：

- [Creating distribution-signed code for macOS](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac/)
- [Packaging Mac software for distribution](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution)
- [Notarizing macOS software before distribution](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)

公开上传和正式分发不进入普通仓库验证，需要发布授权。

## M4 已执行顺序

1. 固定 `packaging/macos/product.json` 与 source contract 门禁；
2. 对齐 Manager、InputMethod、FFI、userdb 和最低 macOS 版本；
3. 从现有两个构建入口装配版本化产品目录；
4. 生成并复验 `ProductManifest.json`；
5. 把 source contract 单元测试接入仓库门禁；
6. 在可用的隔离 RimeData/native dependency 环境运行完整装配验证；
7. M4-P01 退出后完成 M4-P02 数据升级协调器、manifest-bound macOS adapter 与隔离真实产品协调资格；
8. M4-P03 已固定 DMG + 独立用户域 Installer、两个目标路径、InstallPayload manifest、外层 receipt/guard、双 bundle 程序切换恢复和 macOS manifest/code-signature adapter；
9. M4-P03 已用独立组合层绑定双 receipt/guard、数据协调结果、installed target 持续复验和 source 程序一致回滚；
10. 已完成外层两段产品终态动作、upgrade data receipt/双 guard 最终绑定、ABI v9 外层只读 gate 与双端最前置接线；
11. 已建立隔离双 bundle + 合成 Application Support 的端到端恢复门禁，覆盖程序部分提交、数据失败回滚、两段终态中断与双端启动决策；
12. 已固定 Installer UI/驱动 contract、可重启 operation 展示、稳定错误、显式用户授权与独立 AppKit contract shell；
13. 已把 authorized intent 接入隔离 restartable executor 与版本化原生 bridge；
14. 已接入 authoritative current-user bootstrap、完整内嵌 InstallPayload、strict ad-hoc release identity 和可回退实机验收 runbook；普通开发构建因缺失 sealed identity 失败关闭；
15. 已固定 community ad-hoc 发布构建、双 component sealed release identity 与 DMG SHA-256 evidence，并开放 first install/repair/default remove 的 production mutation port；
16. 已将历史 source assembly 纳入 payload v2，按外层 receipt 精确选源并开放 production upgrade port；
17. `26.7.1 (35)` 未公证 APFS/UDZO DMG 已完成独立下载核验和 Installer 人工放行，但真实首次安装证明其把 quarantine 传播给双 bundle，Manager 因 App Translocation 被固定路径 startup gate 正确拒绝；该候选已失效，不作为首发 assembly。
18. `26.7.1 (36)` 在独立下载首次安装中证明“复制后递归移除”无法处理 `0444` 第三方 dylib；事务在 InputMethod staged evidence 前失败关闭，双程序均未提交，该候选继续失效。
19. 安装事务改为 `ditto --noqtn` 源头排除传播，并逐节点只读审计无 quarantine；`26.7.1 (37)` 新 DMG/evidence 已通过完整仓库门禁，下一步从独立下载与空用户域基线证明首次安装、启动、修复与移除。

## M4-P01 退出标准

M4-P01 只有同时满足以下条件才可退出：

- 产品元数据只有一个正式真相源；
- Manager 与 InputMethod 的版本、build、最低 macOS 和 bundle ID 受自动门禁约束；
- FFI ABI、userdb schema 和两个既有 native manifest 版本被产品 manifest 明确绑定；
- 产品目录由稳定入口装配，不引用构建机绝对依赖；
- manifest 对 bundle 篡改、缺失、symlink 和版本错配失败；
- 不触碰真实 Application Support、Input Methods、系统设置或 Keychain；
- 文档、测试和状态入口不把装配产物误称为已公证普通用户安装包。
