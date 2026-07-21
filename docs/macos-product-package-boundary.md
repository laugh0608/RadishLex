# macOS 产品包边界

本文定义 RadishLex M4 macOS 产品发布候选的组件、版本、数据、签名和验证边界，读者是产品构建、InputMethodKit、Manager 与发布门禁的维护者。本文不记录具体构建流水、Apple 凭据、真实安装操作或历史验收结果；可重复构建与复验步骤见 [macOS 产品装配 Runbook](runbooks/macos-product-assembly.md)。

## 目标与范围

M4 macOS 产品包必须把以下已有能力组织成同一版本、可验证且可升级的产品：

- `RadishLexInputMethod.app`：InputMethodKit 输入法薄壳；
- `radishlex_manager.app`：本地数据、隐私和诊断管理界面；
- `libradishlex_ime_ffi.dylib`：两端调用的 Rust ABI；
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
- InputMethod 额外携带 `librime`、RimeData 与对应许可证；
- Manager 只携带其真实调用所需的 native dependency，不为目录对称复制 `librime`。

M4-P01 的稳定装配产物是版本化产品目录及其 manifest。面向用户的 `.pkg`、`.dmg` 或安装器应用属于后续安装批次；在安装位置、权限和回滚语义完成实证前，不能把装配目录称为普通用户安装包。

### 产品 manifest

产品装配必须生成 `ProductManifest.json`，至少绑定：

- manifest format、产品 ID、产品版本、build number 和最低 macOS；
- Manager 与 InputMethod 的 bundle ID、版本和 build；
- FFI ABI、userdb schema、Rime schema ID、RimeData manifest 和 native libraries manifest 版本；
- 两个 bundle 内所有普通文件的相对路径、大小和 SHA-256，以及安全内部 symlink 的相对目标；
- 仓库许可证文件的相对路径、大小和 SHA-256。

manifest 不包含构建机绝对路径、签名身份、Team ID、Apple 凭据、用户目录、时间戳或输入数据。相同输入 bundle 必须生成字节一致的 manifest；签名和公证证据另行记录。

manifest 是产品完整性与兼容性证据，不替代 Apple code signature、notarization ticket 或 Gatekeeper 验证。

### RimeData 来源与许可证

`packaging/rime/product-rime-data.json` 是首个候选的 RimeData 来源锁。产品构建只从锁定的 committed 文件离线装配，不在构建时联网，不读取用户、Squirrel 或其他输入法的数据目录。锁必须绑定每个资产的仓库来源、完整 commit、源路径、运行时路径、SHA-256 和许可证映射。

首个候选固定使用 RadishLex 维护的 `radishlex_pinyin` schema 和 Apache-2.0 `pinyin_simp.dict.yaml`。产品 schema 保留简体全拼、用户词典、常用中西文标点和候选翻页，明确移除 upstream `stroke` reverse lookup 及 `prelude` preset 导入；因此不携带 LGPL `rime-stroke`、`rime-prelude`、`luna_pinyin` 或 `essay`。若未来增加笔画反查或扩展符号表，必须重新完成行为设计、来源与逐包许可证评审，不能向现有锁静默追加文件。

装配后的 `RimeData/` 必须携带与锁字节一致的 `SourceManifest.json` 和 `Licenses/<component>/LICENSE|AUTHORS`。RimeData manifest v2 对所有数据和许可证文件计算 hash，并单独绑定来源 manifest 与许可证集合；额外文件、hash 漂移、空许可证或 symlink 均失败关闭。

## 单一版本真相源

`packaging/macos/product.json` 是 M4 macOS 产品元数据真相源。其他文件可以保留语言或构建系统要求的版本字段，但门禁必须验证它们与真相源一致。

当前首个 M4 候选固定：

| 字段 | 值 | 约束 |
| --- | --- | --- |
| product ID | `radishlex-macos` | manifest 稳定标识 |
| product version | `0.1.0` | Manager 与 InputMethod 相同 |
| build number | `35` | 正整数且两个 bundle 相同 |
| minimum macOS | `13.0` | 取两端真实支持范围的交集 |
| FFI ABI | `8` | 增加只读 startup gate 与双端升级 validation contract |
| userdb schema | `9` | 不允许旧产品打开未来 schema |
| RimeData manifest | `2` | 绑定来源锁、多许可证与完整数据 hash |
| data layout | `application-support-v1` | 首版继续使用已验证布局 |

build number 只描述产品构建，不替代 schema 或 ABI。任何 ABI、数据库、RimeData 或 native manifest 格式变化都必须独立递增对应版本，并更新兼容测试。

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

Manager 与 InputMethod 都必须对下列情况失败关闭并返回稳定错误：

- FFI ABI 不匹配或必需 symbol 缺失；
- userdb schema 高于当前支持版本；
- 数据路径不是预期目录/普通文件，或权限无法收紧；
- RimeData、native dependency 或 manifest 缺失/损坏；
- 输入法 bundle 与 Manager 产品版本不兼容。

不能通过加载旧 dylib、创建空数据库、改用 fixture 或忽略 manifest 来掩盖错误。

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

M4-P01 不修改真实系统安装位置。后续安装批次必须在以下约束下选择并验证分发载体：

- Manager 和 InputMethod 的目标位置固定且可审计；
- 用户能在安装前看到将写入的程序路径和所需权限；
- 工具不得程序化选择或模拟切换输入源；
- 升级前后使用公开 API 只读确认输入源状态；
- 默认移除只删除产品 bundle，保留 Application Support 数据；
- “移除程序并删除数据”必须单独授权、检查 receipt、关闭数据库并限制到固定文件集合；
- 安装、升级和回滚脚本不得暴露通用递归删除或调用方自定义删除路径。

真实安装、系统设置、进程停止、签名、公证和数据清理仍遵守仓库授权规则。

## 签名、公证与供应链

开发构建可以使用 ad-hoc 或 Apple Development，但必须明确标记，不能作为发布证据。直接分发的发布候选要求：

- 所有嵌套 Mach-O 先签名，再签主 executable 和外层 bundle；
- 使用 Developer ID Application 身份和 Hardened Runtime；
- 若采用 installer package，使用独立的 Developer ID Installer 身份；
- 使用 Apple 当前支持的 `notarytool` 或 Notary API 提交；
- 验证 notary log，staple ticket，并在隔离环境执行 Gatekeeper 评估；
- 发布证据只记录固定状态、产品 hash、submission ID 和结果，不保存凭据。

Apple 官方边界参考：

- [Creating distribution-signed code for macOS](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac/)
- [Packaging Mac software for distribution](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution)
- [Notarizing macOS software before distribution](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)

签名身份、notary credential、公开上传和正式分发不进入普通仓库验证，需要发布授权。

## M4-P01 实现顺序

1. 固定 `packaging/macos/product.json` 与 source contract 门禁；
2. 对齐 Manager、InputMethod、FFI、userdb 和最低 macOS 版本；
3. 从现有两个构建入口装配版本化产品目录；
4. 生成并复验 `ProductManifest.json`；
5. 把 source contract 单元测试接入仓库门禁；
6. 在可用的隔离 RimeData/native dependency 环境运行完整装配验证；
7. 下一批再实现数据升级协调器和安装/回滚载体。

## 当前退出标准

M4-P01 只有同时满足以下条件才可退出：

- 产品元数据只有一个正式真相源；
- Manager 与 InputMethod 的版本、build、最低 macOS 和 bundle ID 受自动门禁约束；
- FFI ABI、userdb schema 和两个既有 native manifest 版本被产品 manifest 明确绑定；
- 产品目录由稳定入口装配，不引用构建机绝对依赖；
- manifest 对 bundle 篡改、缺失、symlink 和版本错配失败；
- 不触碰真实 Application Support、Input Methods、系统设置或 Keychain；
- 文档、测试和状态入口不把装配产物误称为已公证普通用户安装包。
