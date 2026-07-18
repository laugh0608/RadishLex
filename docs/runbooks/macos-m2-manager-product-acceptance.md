# macOS M2 manager 产品验收 runbook

本文档指导维护者在 macOS 上验收 manager 的真实本地产品运行态，并在验收后恢复系统与数据基线。读者是执行 M2 退出验收的开发者和协作者。本文不包含 M3 真实同步、恢复码/设备授权、M4 公证安装包，也不授权任何系统修改、进程启动或测试数据删除。

## 验收目标

使用同一 clean-HEAD Release 产物证明：

1. manager 无需 shell 环境变量即可从正常 app 启动，默认是 `product` mode，失败不会回退 fixture。
2. app bundle 内 native library、固定 userdb/settings 路径和权限真实生效。
3. active、suppressed、deleted、delete 和 explicit restore 在 GUI 与 Rust 真相源之间一致，重启后不漂移。
4. manager 隐私开关写入输入 runtime 使用的 macOS 偏好层，读回一致，隐私期间输入侧零学习增量。
5. 输入法与 manager 同时访问同一测试 userdb 时，状态双向可见、migration 所有权唯一、无非预期锁错误。
6. 验收数据和系统状态可按精确归属恢复，全程不读取 P1 原始行或数据库正文。

## 权限与停止线

本 runbook 只描述动作，不构成授权。执行时必须把以下动作分别放在动作时授权范围内：

- 构建、签名、复制或启动本地 manager app。
- 若需要输入侧同库验收，构建、签名、安装、添加/选择和启动 RadishLex InputMethodKit bundle。
- 写入或恢复 `org.radishlex.inputmethod.macos` 的 `RadishLexPrivacyMode`。
- 终止精确产品进程、删除测试 bundle/Rime/settings/userdb/sidecars、恢复目录权限。

开发者负责 GUI 聚焦、输入源手动切换、实体键盘输入和 manager 可见操作。AI 不程序化选择输入源、不合成按键冒充验收、不自动点击系统设置。

出现以下任一情况立即停止并保留现场：

- clean HEAD、构建哈希、签名或 bundle native contract 不一致。
- 基线中存在无法归属的 RadishLex TIS、bundle、Rime、进程、隐私键、settings 或 userdb。
- 路径为 symlink、父目录身份/权限异常、receipt 不匹配或待删除对象超出本轮精确清单。
- 产品启动出现 demo 标识、静默 fixture、ABI/路径错误未展示或数据路径与固定契约不同。
- 任一操作需要查看 P1 原始事件、数据库正文或用户真实词条才能判断。
- secure field、系统输入路由或隐私行为与当前稳定边界冲突。

## 产物冻结与只读基线

1. 确认 `dev` 工作区干净，记录 HEAD；在 clean HEAD 依次运行 manager、FFI、产品 bundle 和仓库门禁。
2. 用 `./scripts/build-manager-macos-product.sh` 生成 Release app，记录 app、主程序和 bundle native library 哈希、架构、签名与依赖。
3. 只读确认当前 TIS matches/enabled/selected、安装 bundle、Rime、RadishLex 进程、隐私键、固定 Application Support 目录、userdb/settings/sidecars 和既有 receipts。
4. 只有现场为空，或既有对象能由有效 receipt 和明确基线证明归属时，才形成不可覆盖的本轮基线 receipt。
5. 测试词、App 标识和输入文本必须是公开合成数据；不要导入真实个人词库。

## 授权 A：产品启动与本地管理

授权 A 应精确覆盖本轮 manager app 的签名/复制/启动，以及固定 Application Support 测试数据的创建与 GUI 操作；不自动包含输入法安装、隐私偏好变更或最终删除。

1. 从冻结 Release app 直接启动，不注入 `RADISHLEX_MANAGER_*` 环境变量。
2. 确认没有“合成演示数据”标识；运行诊断显示 bundle native library 与平台 Application Support userdb，且不泄露真实绝对路径。
3. 确认 `~/Library/Application Support/RadishLex` 为 `0700`，settings 和已创建的 userdb 为 `0600`，并确认都不是 symlink。
4. 通过 manager 导入固定合成 TSV，检查 active 词条、学习聚合和 rank explain；诊断不展示 P1 原始行或导入正文。
5. 删除固定词条，确认 active 消失且 deleted tombstone 出现；普通刷新、导入或重启不得复活。
6. 通过独立确认动作 explicit restore，确认 tombstone 消失、词条恢复；对 suppressed 固定条目执行同样的明确恢复，不允许其他操作隐式恢复。
7. 完全退出并重新启动 manager，复核词条状态、settings 和排序/筛选保持一致。

任一启动、读取、删除、恢复或重启错误必须以结构化用户可见状态出现；不得临时切换 demo 或直接编辑 SQLite 绕过。

## 授权 B：输入法共库与隐私

授权 B 应精确覆盖 InputMethodKit 构建/签名/安装、系统设置添加/移除、开发者手动 source 选择、精确进程控制和隐私键临时变更。若真实输入法尚未安装或现场不是零基线，先按现有 macOS runbook 建立可回滚基线。

1. 确认 manager 和输入 runtime 指向同一个固定 userdb，不复制数据库或建立第二真相源。
2. 保持 manager 打开，由开发者在普通 TextEdit 字段手动选择 RadishLex，用固定合成 case 产生一次选择学习；切回中立输入源后，在 manager 刷新并确认聚合/词条状态可见。
3. 在 manager 删除该固定词条；重新聚焦 TextEdit 后，确认输入侧观察 tombstone/排序抑制且普通选择不能复活。再由 manager explicit restore，确认输入侧新 session 可见恢复状态。
4. 在两个连接都曾存活的条件下复核操作没有非预期 `SQLITE_BUSY`、重复 migration 或损坏降级；精确重启 manager 和输入法进程后再次确认一致。
5. 在 manager 开启隐私模式，确认 macOS CurrentUser/AnyHost 偏好读回为 true。开发者在普通非 P0 字段提交固定合成 case；仅检查全库聚合零增量，不读取 P1 原始行或正文。
6. 在 manager 关闭隐私模式并读回 false/基线值；再做一次受控普通选择，确认学习恢复且只产生预期聚合增量。
7. secure field 只复核 macOS 路由旁路，不把 controller secure 分支记为实机通过；若系统不允许切换 RadishLex，记录来源监视和聚合零增量即可。

## 授权 C：清理与最终复验

清理授权必须在所有数据库连接关闭后单独确认，并只删除有效 receipt 归属的本轮对象。若归属、路径或对象清单不一致，保留现场，不猜测删除。

1. 开发者切回中立输入源并从系统设置移除测试输入法；精确停止 manager 与 RadishLex 进程。
2. 用对应 receipt 恢复隐私键；删除本轮固定 manager/InputMethodKit bundle、Rime 测试数据和临时构建快照。
3. 精确删除本轮 settings、userdb 及 `-wal`/`-shm` sidecars 和 receipts；不得扫描或读取数据库正文决定归属。
4. 若父目录在基线为空，恢复其基线 mode；否则保留基线前已有对象，不进行目录级清空。
5. 最终只读复验：TIS `matches=0 enabled=0 selected=0`，测试 bundle/Rime absent，RadishLex 进程 stopped，隐私键恢复基线，测试 userdb/settings/sidecars/receipts absent，父目录内容与 mode 回到基线，无本轮 `/private/tmp` 快照目录。

## M2 退出判定

只有以下证据全部成立，才能更新 `docs/status/current.md`、本地验收文档和当周 devlog，正式关闭 M2：

- clean-HEAD 自动门禁与冻结产物一致。
- product 无环境变量启动、固定路径/权限、GUI 删除/恢复和重启持久化通过。
- 输入法/manager 同库双端可见、隐私读回与零学习增量通过。
- 产品失败没有静默 fixture，P1/诊断边界没有扩张。
- 系统、进程、偏好和数据最终回到可证明基线。

若全部通过，下一开发批次进入 M3 的同步成功路径设计与安全证据，不启动第二真实平台，也不提前宣称 M4 发布包完成。
