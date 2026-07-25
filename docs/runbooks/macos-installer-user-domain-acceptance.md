# macOS Installer 真实用户域验收 Runbook

本文指导维护者验收 M4-P03 独立 Installer 的真实 current-user bootstrap、人工输入源交互、进程静止、安装事务与回退边界。读者是执行发布前实机验收的开发者和协作者。本文不授权 Developer ID 凭据、公证、DMG 上传、自动选择输入源、真实用户同步、数据删除或历史事务材料清理。

## 验收分层

验收必须绑定同一冻结产物，并按以下层次推进：

1. ad-hoc 开发构建只证明当前用户域解析、嵌入 payload 和发布身份缺失时失败关闭；
2. Developer ID 构建才允许证明首次安装、修复、程序移除和升级事务；
3. 公证、staple、Gatekeeper 与 DMG 下载隔离证据进入后续发布切面。

ad-hoc、Apple Development 或缺少 `ReleaseIdentity.json` 的 Installer 必须显示：

```text
phase=blocked action=refresh error=product_identity_unavailable state=none
```

此结果是成功的负向验收，不得通过临时 Team ID、`HOME`、`CFFIXED_USER_HOME`、调用方路径或放宽 `codesign` requirement 继续安装。

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

- `security find-identity -v -p codesigning` 没有本轮要求的发布身份，却准备执行正向安装；
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
3. 记录 Installer、Manager、InputMethod 主 bundle 的 hash、版本和签名状态；发布验收要求所有对象绑定同一 Team ID 与冻结 designated requirement。
4. 运行 `./scripts/cleanup-macos-imk.sh --status`，只读记录 TIS matches/enabled/selected、已安装 bundle、runtime data、userdb family 和 InputMethod 进程。
5. 按精确进程名只读确认 Manager 与 InputMethod 均停止。沙盒无法读取进程表时必须在获准的真实用户会话复验，不能把 `unavailable` 当作 `stopped`。
6. 对四个固定对象及其父目录执行 `lstat`/`stat`。不要在基线阶段创建目录、chmod、删除对象或打开数据库。

## B. ad-hoc bootstrap 负向验收

本阶段允许启动冻结的 Installer，不允许执行安装 mutation：

1. 确认 Installer 为 `Signature=adhoc`、`TeamIdentifier=not set`，且资源中不存在 `ReleaseIdentity.json`。
2. 启动 Installer；稳定诊断必须为 `product_identity_unavailable`，唯一可用 action 是 `refresh`。
3. 刷新后结果必须不变。退出 Installer。
4. 重新执行 A 中的固定路径、TIS 和进程检查；双 bundle、外层 state、userdb/settings/Rime 不得因启动或刷新而出现或改变。

若出现 `driver_unavailable`，说明当前用户域/发布身份 bootstrap 尚未进入预期路径；若出现 ready 或 mutation action，说明 ad-hoc 身份被错误接受，必须阻断发布。

## C. Developer ID 正向验收前置

只有后续发布身份切面同时满足下列条件，才进入正向验收：

- Installer、payload 内嵌套 executable、Manager 与 InputMethod 均通过 strict Developer ID/Hardened Runtime 验证；
- `ReleaseIdentity.json` 为 Installer 签名资源，format v1、Team ID 和双 component designated requirement 与冻结产物逐字节一致；
- bridge 复验嵌入 payload manifest、完整 bundle tree 和 exact code identity 后才产生 ready snapshot；
- 真实 preflight 通过公开 API 证明 Manager/InputMethod 不运行、固定 data handle 未打开；
- 当前输入源由开发者手动切换，Installer 只读确认，不调用 TIS mutation API。

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

- HEAD 基线进入本切面时为 `262876c`，工作区干净。
- `security find-identity -v -p codesigning` 返回 `0 valid identities found`；正向发布安装不具备前置条件。
- 真实用户会话只读状态为 TIS `matches=0 enabled=0 selected=0`、InputMethod bundle absent、runtime/userdb absent、InputMethod/Manager 精确进程 stopped。
- 两个固定程序目标均 absent；Application Support/RadishLex 为 empty directory、mode `0755`。该目录不满足事务核心要求的 `0700`，本轮没有静默 chmod 或删除。
- 重建的 Installer 为 `Signature=adhoc`、`TeamIdentifier=not set`，嵌入完整 InstallPayload，且不含 `ReleaseIdentity.json`；原生/UI 门禁稳定投影 `product_identity_unavailable`。

因此本轮只关闭真实用户域 bootstrap 的负向验收和回退方案，不记录首次安装、升级、修复、移除、Developer ID、公证、Gatekeeper 或 DMG 成功证据。
