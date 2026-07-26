# macOS Installer 真实用户域验收 Runbook

本文指导维护者验收 M4-P03 社区 ad-hoc DMG 内独立 Installer 的真实 current-user bootstrap、人工输入源交互、进程静止、安装事务与回退边界。读者是执行发布前实机验收的开发者和协作者。本文不授权 DMG 上传、自动选择输入源、真实用户同步、数据删除或历史事务材料清理。

## 验收分层

验收必须绑定同一冻结产物，并按以下层次推进：

1. 普通开发构建只证明当前用户域解析、嵌入 payload 和 sealed release identity 缺失时失败关闭；
2. `community-adhoc-v1` 冻结产物必须先通过 ProductManifest、InstallPayload、ReleaseIdentity、DMG 和 SHA-256 evidence 自动门禁；
3. 同一 DMG 的独立下载副本完成 SHA-256 核对和用户人工放行后，才允许证明首次安装、修复、程序移除与启动门禁；真实跨发布升级还必须另有上一正式版本 assembly。

缺少 `ReleaseIdentity.json`、identity/payload 漂移或非 `community-adhoc-v1` 的 Installer 必须显示：

```text
phase=blocked action=refresh error=product_identity_unavailable state=none
```

此结果是成功的负向验收，不得通过临时 Team ID、`HOME`、`CFFIXED_USER_HOME`、调用方路径或放宽 strict ad-hoc requirement 继续安装。

## 固定对象

验收只允许以下用户域对象：

```text
~/Applications/RadishLex Manager.app
~/Library/Input Methods/RadishLexInputMethod.app
~/Library/Application Support/RadishLex
~/Library/Application Support/RadishLex/.radishlex-install-v1
```

Installer 自身只从当前 executable 反推 `Contents/Resources/InstallPayload`，从有效用户数据库 API 取得 effective uid 对应 home。UI、settings、shell `HOME`、`~`、命令行和测试 fixed-home 都不能覆盖这些值。

## 停止线

出现以下任一情况立即停止并保留现场：

- DMG SHA-256 与发布 evidence/发布说明不一致，或 Installer、payload、sealed identity 不是同一冻结产物；
- 固定路径或任一父目录是 symlink、owner 异常，或 Application Support 已存在但不是可归属的私有目录；
- TIS、bundle、进程、receipt、staging、backup 或 userdb 现场无法归属；
- Manager/InputMethod 仍运行，或开发者尚未手动切换到中立输入源；
- Installer snapshot 不是已知 ABI/枚举，或日志出现路径、PID、operation ID、签名正文和底层命令输出；
- 任一步要求程序化选择/注册/停用输入源、编辑 TIS 私有数据库、删除历史 operation 材料或查看用户数据正文。

## A. 只读基线与产物冻结

1. 确认 `dev` 工作区干净，记录 HEAD。
2. 运行 `./scripts/check-macos-installer.sh`，确认 App 内同时存在：
   - `InstallLayout.json`；
   - 完整 `InstallPayload/InstallPayloadManifest.json`；
   - 完整 `InstallPayload/Product/`。
3. 记录 DMG/evidence、Installer、Manager、InputMethod 的 hash、版本和签名状态；发布验收要求三端均为 strict ad-hoc，双产品 requirement 命中 sealed 有界集合，`TeamIdentifier=not set`。
4. 运行 `./scripts/cleanup-macos-imk.sh --status`，只读记录 TIS matches/enabled/selected、已安装 bundle、runtime data、userdb family 和 InputMethod 进程。
5. 按精确进程名只读确认 Manager 与 InputMethod 均停止。沙盒无法读取进程表时必须在获准的真实用户会话复验，不能把 `unavailable` 当作 `stopped`。
6. 对四个固定对象及其父目录执行 `lstat`/`stat`。不要在基线阶段创建目录、chmod、删除对象或打开数据库。

## B. 开发 bootstrap 负向验收

本阶段允许启动冻结的 Installer，不允许执行安装 mutation：

1. 确认普通开发 Installer 为 `Signature=adhoc`、`TeamIdentifier=not set`，且资源中不存在 `ReleaseIdentity.json`。
2. 启动 Installer；稳定诊断必须为 `product_identity_unavailable`，唯一可用 action 是 `refresh`。
3. 刷新后结果必须不变。退出 Installer。
4. 重新执行 A 中的固定路径、TIS 和进程检查；双 bundle、外层 state、userdb/settings/Rime 不得因启动或刷新而出现或改变。

若出现 `driver_unavailable`，说明当前用户域/发布身份 bootstrap 尚未进入预期路径；若出现 ready 或 mutation action，说明缺少 sealed identity 的开发构建被错误接受，必须阻断发布。

## C. 社区发布正向验收前置

只有同一冻结产物同时满足下列条件，才进入正向验收：

- Installer、payload 内嵌套 executable、Manager 与 InputMethod 均通过 strict ad-hoc 验证，`TeamIdentifier=not set`、`Signature=adhoc`、CodeDirectory ad-hoc flag 与 primary CDHash 一致；
- `ReleaseIdentity.json` format v2 为 Installer sealed resource，双 component target/历史 source requirement 集合与冻结产物逐字节一致；bridge 先验证 Installer 自身 strict ad-hoc 结构，再读取该资源；
- bridge 复验嵌入 payload manifest、完整 bundle tree 和 exact code identity 后才产生 ready snapshot；
- `CommunityReleaseEvidence.json` 精确绑定版本、DMG 文件名、大小、DMG SHA-256 与 release identity SHA-256；
- 独立下载副本的 SHA-256 与发布说明及 evidence 完全一致，用户已通过“隐私与安全性 → 仍要打开”明确放行；
- 真实 preflight 通过公开 API 证明 Manager/InputMethod 不运行、固定 data handle 未打开；
- 当前输入源由开发者手动切换，Installer 只读确认，不调用 TIS mutation API。

发布产物只允许通过：

```bash
./scripts/build-macos-release-installer.sh
./scripts/build-macos-release-dmg.sh
```

生成。不得把普通开发构建、Apple Development、Developer ID、临时 Team ID 或手工编写的 `ReleaseIdentity.json` 作为当前 community identity 的替代；完整载体复验与人工放行步骤见 [macOS 社区 ad-hoc DMG Runbook](macos-release-carrier.md)。

## D. 首次安装、人工添加与启动

1. 开发者手动切换到中立输入源并关闭 Manager；AI 只读复验。
2. 在 Installer 中显式确认首次安装。`prepared` 必须先持久化并等待人工步骤，不能一次点击越过静止边界。
3. 再次确认后，Installer 重新执行平台 preflight，按 receipt 完成双 bundle staging、逐端 commit、两段终态并到达 `completed`。
4. 开发者在系统设置中手动添加并选择 RadishLex；AI 不点击、不模拟按键、不调用 TIS 注册或选择 API。
5. 开发者用公开合成文本完成最小输入 smoke，再切回中立输入源。
6. 关闭 Manager/InputMethod 后复验双端 install startup gate 和 data startup gate 均允许；日志只保留稳定 decision/error/state。

## E. 升级、故障恢复与回滚

升级必须使用真实上一发布产物作为 source，不得用同代码改版本冒充跨发布证据：

1. 冻结 source/target product、InstallPayload、双 designated requirement 和基线数据身份。
   发布构建必须显式传入真实历史 assembly：

   ```bash
   ./scripts/build-macos-release-installer.sh \
     --upgrade-source-product-root "/absolute/path/to/frozen-source-product"
   ```

   构建必须分别证明 source/target 双 component strict ad-hoc identity，且 payload v2 中 source release/path/ProductManifest 与 sealed requirement 集合精确绑定；不同发布的 ad-hoc CDHash 不要求相同。
2. 人工切回中立输入源并关闭双端；开始 upgrade，确认外层 receipt 先到 `prepared`。
3. 分别在 Manager 已提交、双程序已提交、data switch 后、`final_verified` 后执行受控异常退出；重启 Installer 必须沿同 operation 续跑。
4. candidate 或 post-switch 失败时，必须按 receipt 恢复 source 双 bundle和原数据 inode；旧程序只有在 source-release validation 通过后才允许启动。
5. staging、backup、数据备份和历史 operation 材料全部保留，清理由后续身份绑定切面单独授权。

## F. 程序移除与回退

1. 开发者手动切换到中立输入源，并在系统设置中手动移除 RadishLex 输入源。
2. 关闭双端后，在 Installer 中确认“只移除程序并保留数据”。
3. completed remove 后两个 bundle 必须不存在；Application Support、userdb、settings、Rime、backup 和历史 receipt 保留。
4. 若验收基线原先已有受 receipt 证明的 bundle，只能通过对应 source backup/receipt 恢复；不得从构建目录任意复制冒充回滚。
5. 数据删除不属于本 runbook。需要恢复空基线时，另行取得固定白名单与 receipt 绑定的删除授权。

## 当前实机证据（2026-07-25）

- 普通开发构建已证明缺少 sealed identity 时稳定投影 `product_identity_unavailable`，且没有改变 TIS、双 bundle、Application Support 数据或进程基线。
- HEAD `ad5ce37` 已构建 `26.7.1 (35)` community Installer 与 APFS/UDZO DMG；只读挂载、唯一根对象、Installer/payload/release identity 复验和完整仓库门禁通过。
- 当前本机候选 DMG SHA-256 为 `1137b14f284275723a5d019447c70cd926638937944b73e749cce81c8975b4ec`，evidence 明确记录 `apple_notarized=false`；产物保留在 ignored `target/`，未公开上传。
- 尚未形成独立下载副本、用户人工放行、真实 first install/repair/remove、重启双端 startup gate 或跨发布 upgrade 成功证据。

下一次实机验收必须从 A 重新采集只读基线，并只使用上述同一冻结候选。若候选内容或版本变化，应生成新的 evidence 与验收记录，不能沿用本节 SHA-256。
