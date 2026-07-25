# ADR 0008: macOS 用户域安装载体与程序升级事务

本文固定 RadishLex M4-P03 的 macOS 分发载体、安装域、程序目标、升级事务和移除边界，读者是 Installer、产品装配、升级协调器和发布门禁维护者。本文不包含 Apple 凭据、真实系统安装步骤、界面稿或历史验证流水；数据 migration 仍以 [macOS 数据升级协调器](../macos-data-upgrade-coordinator.md) 为准。

## 状态

Accepted

## 背景

RadishLex 不是单一可拖入 `/Applications` 的普通 app。首个候选同时包含 Manager 与 InputMethod，并要求：

- InputMethod、Manager 和 Application Support 都在当前用户上下文工作；
- 升级时先阻止两端启动，再持续证明进程和数据库静止；
- source/target 产品代码分别验证数据，失败时同时恢复程序与原数据库；
- 用户手动完成输入源切换、添加和移除，安装器只读确认公开 TIS 状态；
- 默认移除程序时保留用户数据。

Apple 当前直接分发支持 ZIP、DMG 和 Installer package；其中 DMG 与 flat PKG 都可签名、公证。PKG 适合把多个组件复制到固定位置，也支持用户 home 安装域，但标准 Installer 的 payload/script 生命周期不能直接替代 RadishLex 所需的当前用户交互、外层程序事务和 M4-P02 数据协调器。

## 决策

### 分发载体

首个发布候选使用：

```text
Developer ID Application 签名的 RadishLex Installer.app
  inside
签名并公证、已 staple ticket 的 UDIF DMG
```

`RadishLex Installer.app` 是独立产品宿主，不是 Manager 的普通页面，也不进入输入热路径。它可以从只读 DMG 运行；完成或失败后不把自身安装到持久产品位置。

当前不使用以下载体作为主路径：

- 纯拖拽 DMG：无法把两个独立 bundle 安装到两个固定目录，也不能协调升级和回滚；
- ZIP：容器本身不可签名，且不能提供受控双目标安装；
- `.pkg`：保留给未来系统域或企业分发评估，不承担首个候选的用户数据协调与程序事务；
- Manager 自安装：会混淆正常业务启动、startup gate 和安装恢复职责。

### 固定用户域

所有程序和状态都以当前用户 home 为根，安装器不提供自定义目标：

| 对象 | 固定相对路径 |
| --- | --- |
| Manager | `Applications/RadishLex Manager.app` |
| InputMethod | `Library/Input Methods/RadishLexInputMethod.app` |
| 产品数据 | `Library/Application Support/RadishLex` |
| 安装事务状态 | `Library/Application Support/RadishLex/.radishlex-install-v1` |

路径以结构化相对值保存在 `packaging/macos/install-layout.json`，不能使用 `~`、绝对路径、`..`、环境变量或调用方覆盖。Installer 只从系统 user-domain API 取得 authoritative home，再拼接固定目标。

选择用户域的原因：

- 已有 InputMethodKit 实机证据全部绑定 `~/Library/Input Methods`；
- Manager、InputMethod 和 Application Support 保持同一用户所有权，不需要管理员授权；
- 安装器不以 root 身份读取、迁移或删除用户明文数据；
- 默认移除和独立数据删除授权可以保持清晰。

### 安装 payload

Installer bundle 的资源中固定携带：

```text
Contents/Resources/InstallPayload/
  InstallLayout.json
  InstallPayloadManifest.json
  Product/
    Components/radishlex_manager.app
    Components/RadishLexInputMethod.app
    LICENSE
    ProductManifest.json
```

`ProductManifest.json` 绑定两个产品 bundle、native dependency、RimeData 和许可证。`InstallPayloadManifest.json` 再绑定产品 manifest、安装 layout、版本/build、Installer bundle ID 和目标路径。发布装配顺序必须是签名嵌套 Mach-O 与两个产品 bundle、冻结 product/payload manifest、签名 Installer bundle、创建并签名 DMG；任何后续修改都要求重新生成受影响的外层 manifest、重新签名并重新公证。

### 外层安装事务

M4-P02 的数据 receipt 不能单独证明两个程序 bundle 已完成切换。M4-P03 必须新增 `.radishlex-install-v1` 外层 receipt/guard，并让 Manager/InputMethod startup gate 在业务初始化前同时检查它。

升级顺序固定为：

1. 验证 Installer code signature、payload manifest、target product manifest 和已安装 source product evidence；
2. 获取外层 guard，并在任何程序或数据变化前持久化严格 receipt；
3. 提示用户切到中立输入源并关闭 Manager，只用公开 API 和固定 preflight 复验静止；
4. 在两个目标各自文件系统的受控私有 staging 中准备 target，并保存可验证的 source 程序副本；
5. 按 receipt 逐端原子切换程序 bundle；任何部分完成状态都必须可复验和回滚；
6. 以保存的 source product 与 payload target product 驱动 M4-P02 数据协调器；
7. 数据完成后复验已安装 target 的签名、manifest、版本与双端 startup gate，再推进外层 `completed`；
8. 数据失败或程序复验失败时，先按 receipt 恢复 source 程序，再要求数据处于 `aborted_preserved` 或 `rolled_back`，最后推进外层 `rolled_back`。

外层 receipt 非终态、损坏、身份漂移或存在未知 staging/backup 时，两端都失败关闭。不能因 M4-P02 receipt 已终态就忽略尚未完成的程序事务，也不能在安装窗口中允许 source 程序打开 target schema。

### 安装、修复与移除

- 首次安装、升级、程序修复和默认移除是不同 operation kind，不通过缺文件猜测动作。
- 默认移除只删除两个固定程序 bundle；Application Support、数据升级备份和用户词库全部保留。
- InputMethod 仍在系统设置中启用或为当前 source 时，Installer 只能提示用户手动切换/移除并等待公开状态复验，不能程序化选择、停用或改写 TIS 私有数据库。
- “移除程序并删除数据”不进入默认 Installer 动作，继续使用独立授权、固定白名单、数据库关闭证明和 receipt。
- Installer 不接受调用方自定义删除路径，不递归扫描 home，也不把 `rm -rf`、`ditto` 叠加覆盖或 Finder 拖拽当作产品事务。

## 发布身份与验证

发布候选要求两个产品 bundle、Installer app 和全部 executable 使用 Developer ID Application、Hardened Runtime 与 trusted timestamp。DMG 单独签名并提交 Apple notary service；使用 `notarytool` 或 Notary API，检查 notary log，staple ticket，并在隔离下载环境执行 Gatekeeper 评估。

当前 payload/layout 门禁只证明确定性内容和目标映射，不证明 Developer ID、notarization、stapling、Gatekeeper、真实安装或 TIS 会话刷新。签名身份、凭据、上传和真实系统动作仍须另行授权。

Apple 官方依据：

- [Packaging Mac software for distribution](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution)
- [Notarizing macOS software before distribution](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [Developer ID certificates](https://developer.apple.com/help/account/certificates/create-developer-id-certificates)

## 后果

- 首个候选无需管理员权限，且不会让 root installer script 接触用户明文数据。
- 仓库需要新增独立 Installer app、外层 receipt/guard、双端 startup gate 扩展和程序 bundle 切换恢复测试。
- `.pkg` 不再阻塞首个候选，但未来系统域/企业分发需要新的 ADR 和独立证据。
- DMG 只是分发容器；在 Installer、外层事务、签名公证和真实安装门禁完成前，payload 仍不能称为普通用户安装包。

## M4-P03 实现顺序

1. 固定本 ADR、`install-layout.json`、payload manifest 和跨平台 contract；
2. 建立只复制到 `target/` 的确定性 InstallPayload 装配与复验；
3. 设计并实现外层 receipt/guard、程序 artifact identity 与 startup gate；
4. 实现双 bundle staging、切换、逐边界故障注入和 source 程序恢复；
5. 把 M4-P02 数据协调器纳入外层事务，覆盖成功、数据失败、程序失败和重启恢复；
6. 已实现独立 Installer app 的状态 UI/驱动 contract、手动输入源提示、默认程序移除授权和脱敏诊断；隔离写 executor 继续按同一边界接入；
7. 完成 Developer ID/Hardened Runtime、DMG、公证、stapling、Gatekeeper 与授权真实安装/升级/移除验收。

## M4-P03 退出标准

- 分发容器、Installer、两个程序 bundle 和安装目标只有一个稳定真相源；
- 首次安装、升级、部分程序切换、数据失败、程序回滚和重启恢复均有确定性终态；
- 外层非终态事务能在两端业务初始化前失败关闭；
- source/target 程序与数据版本始终配对，不存在旧程序打开未来 schema 的窗口；
- 默认移除保留 Application Support，数据删除只能走独立授权；
- Developer ID、Hardened Runtime、notarization、stapling 和 Gatekeeper 证据绑定同一冻结产物；
- 授权实机完成安装、人工添加/切换、升级、程序回滚和移除，且不使用私有 TIS 数据库或自动输入冒充验收。
