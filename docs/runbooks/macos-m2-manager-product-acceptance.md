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

开发者负责目标宿主聚焦、输入源手动切换、实体键盘输入和候选框观察。取得对应授权后，AI 负责 manager GUI、系统设置可见操作、只读状态监视和最终回滚；不得程序化选择输入源或合成按键冒充输入验收。

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
4. 只有现场为空，或既有对象能由有效 receipt 和明确基线证明归属时，才执行 `./scripts/cleanup-macos-m2-manager-test-data.sh --capture-baseline`。该入口只接受固定动作，生成不可覆盖的 `target/macos-manager/m2/m2-manager-test-data-baseline.receipt`，并要求 receipt 为当前用户普通 `0600` 文件。
5. 测试词、App 标识和输入文本必须是公开合成数据；不要导入真实个人词库。

## 授权 A：产品启动与本地管理

授权 A 应精确覆盖本轮 manager app 的签名/复制/启动，以及固定 Application Support 测试数据的创建与 GUI 操作；不自动包含输入法安装、隐私偏好变更或最终删除。

1. 从冻结 Release app 直接启动，不注入 `RADISHLEX_MANAGER_*` 环境变量。
2. 确认没有“合成演示数据”标识；运行诊断显示 bundle native library 与平台 Application Support userdb，且不泄露真实绝对路径。
3. 确认 `~/Library/Application Support/RadishLex` 为 `0700`，settings、可能存在的原子写临时文件和已创建的 userdb 为 `0600`，并确认都不是 symlink；manager 进程必须从启动起使用 `0077` umask，不能依赖保存完成后的补改权限。
4. 通过 manager 导入固定合成 TSV，检查 active 词条、学习聚合和 rank explain；词条审计必须按持久化 batch id 指向本轮导入批次，不得按 term source 与 batch source name 的显示字符串猜测关联；诊断不展示 P1 原始行或导入正文。
5. 删除固定词条，确认 active 消失且 deleted tombstone 出现；普通刷新、导入或重启不得复活。
6. 通过独立确认动作 explicit restore，确认 tombstone 消失、词条恢复；对 suppressed 固定条目执行同样的明确恢复，不允许其他操作隐式恢复。
7. 完全退出并重新启动 manager，复核词条状态、settings 和排序/筛选保持一致。

任一启动、读取、删除、恢复或重启错误必须以结构化用户可见状态出现；不得临时切换 demo 或直接编辑 SQLite 绕过。

### 本轮 Authorization A 证据（2026-07-18）

- clean HEAD `b2acd0a` 的 repository、manager product 与 macOS InputMethodKit 门禁通过；冻结 Release 主程序、bundle FFI 与 `Info.plist` SHA-256 分别为 `8ef53ec82d48d7148983ac0e00e9234d1cb1b6cf5b5575d085df0d57038b8c35`、`4785dd662a918188ad506c0ec1b543a67ef477bafbd479de81e407623a794f0f`、`780d2ecbf8d4fd6c615eb7b5cd40e03227fde2280956d005d573bc0087451ce2`；严格验签通过，签名为 ad-hoc。
- 冻结 app 未注入环境变量直接启动，显示 `product` / `local_only`，没有 demo 标识；真实 v3 测试 userdb 成功迁移到 v4。迁移故障诊断只读取 schema 元数据和本轮合成副本，不读取表数据、数据库正文或 P1 原始行。
- 固定合成 TSV 建立三条导入审计记录。active 词条关联 `#2 / manager-import / 2/2`，suppressed 条目经第三次导入更新后关联 `#3 / manager-import / 1/2`；导入历史在重启后仍按最新批次优先排序。
- 删除 active 后出现 tombstone；再次导入结果为更新 1、跳过 1，deleted 计数 1，普通导入没有复活 tombstone。随后分别通过独立确认动作恢复 deleted 与 suppressed，二者均变为 active，tombstone 消失。
- 完整退出并确认进程停止后重新启动，词库保持 2 个 active、0 suppressed、0 deleted；学习摘要为 user terms 2、selection events 0，两个 rank explain 的 user contribution 均为 `1.000`，suppressed/deleted contribution 均为零。settings 仍为 absent，符合 Authorization A 未进入隐私设置的边界。
- 退出后只读复验为 TIS `matches=0 enabled=0 selected=0`、bundle/Rime/runtime absent、InputMethodKit 与 manager 进程 stopped、隐私键 absent；Application Support 父目录为 `0700`，仅 userdb 为普通 `0600` 文件，sidecars/settings absent，M2 receipt 与固定合成 TSV 均为普通 `0600` 文件，未发现 symlink。

本组证据只关闭 Authorization A。InputMethodKit 安装、系统输入源、实体键盘输入、候选框与隐私偏好仍属于 Authorization B；最终测试数据删除仍属于 Authorization C。

## 授权 B：输入法共库与隐私

授权 B 应精确覆盖 InputMethodKit 构建/签名/安装、系统设置添加/移除、开发者手动 source 选择、精确进程控制和隐私键临时变更。若真实输入法尚未安装或现场不是零基线，先按现有 macOS runbook 建立可回滚基线。

1. 确认 manager 和输入 runtime 指向同一个固定 userdb，不复制数据库或建立第二真相源。
2. 保持 manager 打开，由开发者在普通 TextEdit 字段手动选择 RadishLex，用固定合成 case 产生一次选择学习；切回中立输入源后，在 manager 刷新并确认聚合/词条状态可见。
3. 在 manager 删除该固定词条；重新聚焦 TextEdit 后，确认输入侧观察 tombstone/排序抑制且普通选择不能复活。再由 manager explicit restore，确认输入侧新 session 可见恢复状态。
4. 在两个连接都曾存活的条件下复核操作没有非预期 `SQLITE_BUSY`、重复 migration 或损坏降级；精确重启 manager 和输入法进程后再次确认一致。
5. 在 manager 开启隐私模式，确认 macOS CurrentUser/AnyHost 偏好读回为 true。开发者在普通非 P0 字段提交固定合成 case；仅检查全库聚合零增量，不读取 P1 原始行或正文。
6. 在 manager 关闭隐私模式并读回 false/基线值；再做一次受控普通选择，确认学习恢复且只产生预期聚合增量。
7. secure field 只复核 macOS 路由旁路，不把 controller secure 分支记为实机通过；若系统不允许切换 RadishLex，记录来源监视和聚合零增量即可。

### 本轮 Authorization B 证据（2026-07-18）

- 最终冻结于 clean HEAD `a7e385e`。Release `Info.plist`、主程序与 bundle FFI SHA-256 分别为 `780d2ecbf8d4fd6c615eb7b5cd40e03227fde2280956d005d573bc0087451ce2`、`4ae289fe1df7c1653940ea6e869a626c057dc8ce6b8469a0825fc2279e913bf6`、`4785dd662a918188ad506c0ec1b543a67ef477bafbd479de81e407623a794f0f`，ad-hoc 严格验签通过。页头刷新修复的精准 Flutter 测试、manager/FFI/product、InputMethodKit 和完整 repository 门禁均在 clean HEAD 通过。
- 输入侧第一次外部提交使 selection/frequency 各增加 1，manager 通过页头刷新在不重启 app 的情况下读到新聚合。manager delete 后 active `3 -> 2`、ranker `1 -> 0`、tombstone `0 -> 1`；输入侧再次选择只增加 selection，term/version/tombstone/ranker 不变，普通选择没有复活。
- manager explicit restore 后 active `2 -> 3`、tombstone `1 -> 0`，恢复版本保留；后续两次实际提交使 selection `3 -> 5`、frequency `0 -> 2`。manager 与 IMK 精确重启后同库状态保持，再次提交使 selection `5 -> 6`、frequency `2 -> 3`，没有非预期 busy、migration 或损坏错误。
- manager 隐私模式保存并真实读回 true；普通 TextEdit 提交前后全库与固定 case 零增量。关闭隐私并读回 false 后，一次正常提交使 selection `6 -> 7`、frequency `3 -> 4`，证明学习恢复且增量与实际动作一致。
- secure field 显示 Secure Event Input 已启用。secure 聚焦期间顶部输入法菜单不能切换到 RadishLex 或系统拼音；快捷键可以退回系统拼音，但不能进入 RadishLex；解除聚焦后保持系统拼音。来源监视未记录 secure 期间 RadishLex，聚合保持 selection `7`、frequency `4`。该项只记为 macOS secure 路由旁路，不记为 controller `policy_blocked` 实机通过。
- 回滚时系统设置第一次移除发生一次已知输入源回流；完整重启设置后再次移除，parent 变为 disabled。删除 bundle/Rime 后 TIS 暂留 disabled 目录缓存；全新系统设置进程检查现有列表和“添加 -> 简体中文”目录后收敛为 `matches=0 enabled=0 selected=0`。没有修改 TIS 私有状态。隐私键恢复 absent，三个精确 M2 临时目录删除，manager/IMK 停止；userdb/settings/sidecars 与 M2 receipt 按 Authorization C 边界保留。

## 授权 C：清理与最终复验

清理授权必须在所有数据库连接关闭后单独确认，并只删除有效 receipt 归属的本轮对象。若归属、路径或对象清单不一致，保留现场，不猜测删除。

1. 开发者切回中立输入源并从系统设置移除测试输入法；精确停止 manager 与 RadishLex 进程。
2. 用对应 receipt 恢复隐私键；删除本轮固定 manager/InputMethodKit bundle、Rime 测试数据和临时构建快照。
3. 在单独取得本阶段删除授权后，只执行 `./scripts/cleanup-macos-m2-manager-test-data.sh --authorized-delete-m2-manager-test-data`，精确删除 receipt 归属的 `manager-settings.json`、原子写临时文件、userdb 及 `-wal`/`-shm`/rollback journal，并持久移除该 receipt；不得传入路径、扫描或读取数据库正文决定归属。
4. 若父目录在基线为空，恢复其基线 mode；否则保留基线前已有对象，不进行目录级清空。
5. 最终只读复验：TIS `matches=0 enabled=0 selected=0`，测试 bundle/Rime absent，RadishLex 进程 stopped，隐私键恢复基线，测试 userdb/settings/sidecars 与 `target/macos-manager/m2/m2-manager-test-data-baseline.receipt` absent，父目录内容与 mode 回到基线，无本轮 `/private/tmp` 快照目录。

### 本轮 Authorization C 与 M2 关闭证据（2026-07-18）

- C 前固定现场为 TIS zero、bundle/Rime absent、IMK/manager stopped、privacy absent；Application Support 父目录 `0700`，userdb、WAL、SHM、manager settings 与 M2 receipt 均为当前用户普通 `0600` 文件，tmp/journal absent，所有固定文件已关闭。
- 首次执行固定 C 入口在删除前报告 receipt 不匹配并安全停止。诊断只读取固定 metadata 与 receipt 身份字段，未读取数据库正文或 P1：receipt 和父目录 inode 均与 baseline 一致，但二者记录的 `st_dev=16777234` 在当前跨登录现场同时变为 `16777230`。
- 共享 helper 改为先要求 receipt/parent 的 inode、owner、mode、link、固定路径与目录白名单精确匹配；设备号要么分别精确一致，要么必须在旧记录中同设备、当前现场也同设备。成对 device drift contract 在 M2/R01B profile 通过，只漂移 parent 或只漂移 receipt 均继续拒绝；wrapper orchestration contract 与文本检查通过。
- 重跑同一已授权固定入口后，helper 报告 manager test data deleted、父目录 empty/`0755`、receipt removed。最终复验为 TIS `0/0/0`，bundle/Rime/userdb/sidecars/settings/receipts/M2 临时目录 absent，IMK/manager stopped，privacy absent，父目录 empty/`0755`。

## M2 退出判定

只有以下证据全部成立，才能更新 `docs/status/current.md`、本地验收文档和当周 devlog，正式关闭 M2：

- clean-HEAD 自动门禁与冻结产物一致。
- product 无环境变量启动、固定路径/权限、GUI 删除/恢复和重启持久化通过。
- 输入法/manager 同库双端可见、隐私读回与零学习增量通过。
- 产品失败没有静默 fixture，P1/诊断边界没有扩张。
- 系统、进程、偏好和数据最终回到可证明基线。

若全部通过，下一开发批次进入 M3 的同步成功路径设计与安全证据，不启动第二真实平台，也不提前宣称 M4 发布包完成。

上述退出证据已于 2026-07-18 全部成立，M2 本地个人化 MVP 正式关闭。M3 第一主批转向设备签名算法 profile 与 macOS 生产私钥 backend；真实用户同步继续保持关闭。
