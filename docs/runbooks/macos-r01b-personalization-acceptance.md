# macOS R01B 本地个人化验收

本文档用于在冻结的 macOS 开发 bundle 上复验 R01B 学习、重启、删除/恢复和隐私阻断。R01A 基础输入、发布、迁移、同步与日常卸载不在本文；通用步骤见 [开发 runbook](macos-inputmethodkit-development.md)，稳定边界见 [InputMethodKit](../macos-inputmethodkit-boundary.md)。

## 退出结论

R01B 只有同时取得以下证据才可退出：

- TextEdit 的真实选择写入本轮新建 userdb，并在全新隔离 Rime user-data 上把固定候选从 engine index `1` 重排到 display index `0`。
- 精确停止输入法进程、由系统重新拉起后，同一逻辑状态和真实选择效果保持。
- delete 后旧选择与再次选择都不能复活词条；只有 explicit restore 产生更新版本，后续选择才重新建立 ranker weight。
- 隐私模式、unknown host、固定 P0 host 均不产生逻辑写入；secure field 按实际系统路由如实归档。
- 隐私键、TIS、bundle、Rime、进程、本轮 userdb family 与预存父目录权限全部恢复到安装前基线。

真实 UI 顺序会同时受 librime 自身 user-data 影响，因此只看候选窗升序不能证明 RadishLex ranker 生效。排序归因必须使用同一产品 RimeData、全新隔离 Rime user-data 和正式 userdb 的 `rime snapshot` 结果。

## 固定输入与构建身份

本轮 case 版本固定为 `r01b-shi-time-v1`：

| 字段 | 固定值 |
| --- | --- |
| 产品构建 | `CFBundleVersion=34` |
| schema | `pinyin_simp` |
| input code | `shi` |
| target text | `时` |
| reading | absent，不传 `--reading` |
| 正常 context | `editor` |
| 产品候选页 | `5` |
| 初始 engine/display index | `1 / 1` |
| 初始五项 | `是`、`时`、`事`、`使`、`市` |

冻结输入中的 schema、dictionary 与数据许可证 SHA-256 分别为：

- `609b33e9548b06c809aa102ad1ee9639d6be1fa15722059206fa8ba7a86c2154`
- `e341598343a0f0f2035bb1aafc34a7f3bb7887deeecb3f60796262aaa2983e6b`
- `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`

开始真实动作前，还要记录 build 34 的 `Info.plist`、主程序、FFI dylib、Rime data manifest 与 native libraries manifest 五项 SHA-256。任一源码、构建号、工具链、数据清单、签名输入或上述 case 输出漂移，都取消当前冻结；不得在安装现场改 case、重建或退回 build 33。

## 授权边界

验收使用两个互不替代的授权：

- 授权 A：同一冻结输入的 Apple Development 重建与签名、用户级复制、系统设置添加/移除、真实应用和两个验证 host 的启动、人工输入、精确进程停止与系统重拉起、隐私键临时启用/恢复、CLI delete/restore，以及保留 userdb 的普通 bundle/Rime 清理。
- 授权 B：仅在授权 A 的普通清理、隐私恢复、数据库关闭和归属复核完成后，删除本轮四个固定 SQLite 文件，并把预存父目录恢复为空和 mode `0755`。

授权 A 不允许删除 userdb family；授权 B 不允许删除父目录、Rime、bundle 或修改系统设置。代码、构建身份、安装域、基线 receipt 或目标路径变化时，已有授权失效。注销、重新登录和网络断开若确有需要，仍分别列明，不能从授权 A 自动推导。

开发者只负责聚焦、手动切换 source 和实体输入/候选操作；执行者负责构建、签名、复制、只读来源记录、CLI 检查和回滚。不得调用 `TISSelectInputSource`、注入按键或自动点击候选代替人工证据。

每个人工组必须在目标宿主内闭合后再回 Codex：

1. 读完步骤后重新点击指定宿主和输入框，再手动选择 RadishLex；不沿用 Codex 焦点或旧 source。
2. 正常时在宿主提交固定 case 后切回系统拼音或 U.S.；异常时先按 `Esc`、切回中立 source，并关闭验证 host。
3. 来源监视确认离开 RadishLex 后才能在 Codex 报告；禁止保留 composition/source 跨窗口报告。
4. 每组前后比较全库聚合和固定目标。任何非固定 case 或无法解释的增量都触发停止线，不读取 P1 原始行追查正文。

## 安装前原子基线

先完成不安装门禁和 build 34 ad-hoc native 冻结，再在没有中间系统状态变更的窗口内执行：

```bash
./scripts/cleanup-macos-imk.sh --status
./scripts/manage-macos-imk-privacy-mode.sh --status
./scripts/manage-macos-imk-privacy-mode.sh --capture-baseline
./scripts/cleanup-macos-imk-r01b-test-userdb.sh --capture-baseline
```

最后一条命令重新取得完整状态，只接受 TIS zero、bundle/Rime/userdb/sidecar absent、祖先 safe、当前用户普通空父目录 `0755` 和进程 stopped；不可覆盖的 `0600` receipt 绑定父目录身份。privacy 工具固定当前用户 `0700` state dir 与排他 `0600` 单链接 receipt，先持久化身份和 baseline，再修改 CurrentUser/AnyHost 设置，并区分键 absent 与显式 false。receipt、路径、身份、权限或设置漂移时立即停止，不覆盖现场。

原子基线通过后才申请授权 A。Apple Development 产物必须由同一源码和输入重建，复制前再核对五项哈希、完整签名和上述状态；安装目标只允许 `~/Library/Input Methods/RadishLexInputMethod.app`。

## 非选择快照与 case 状态

每次排序快照都在无 symlink 祖先的 `/private/tmp` 下为 `--user-data` 创建一个新的当前系统账户所有、mode `0700` 的普通空目录，并使用冻结 build 34 内的 `Contents/Resources/RimeData`。CLI 从系统账户记录取得权威 home，不信任 `HOME`；它会拒绝用户 Rime、Squirrel、Input Methods、`RadishLex/Rime` 及其后代，并把复核后的 canonical 目录实际传给 Rime。

```bash
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime snapshot \
  --schema pinyin_simp \
  --shared-data <frozen-build-34>/Contents/Resources/RimeData \
  --user-data <fresh-empty-private-tmp-directory> \
  --deploy-on-start 1 \
  --rank-db "$HOME/Library/Application Support/RadishLex/userdb.sqlite3" \
  --context editor \
  shi

cargo run -p radishlex-ime-cli -- \
  learn case-status \
  --db "$HOME/Library/Application Support/RadishLex/userdb.sqlite3" \
  --input shi --text 时 --context editor
```

`rime snapshot` 只接受小写 ASCII 拼音和撇号，拒绝数字、候选 index 与 `--key`；不调用选择入口，engine 自行 commit 时失败。带 `--rank-db` 会执行产品 migration、PRAGMA 和权限检查，只能操作本批已取得归属的数据库，不能称为文件系统只读。`case-status` 在同一 Deferred 读事务中返回聚合、目标 term、指定 context weight 和 tombstone；必须输出 `p1_rows: omitted`，不得输出 P1 原始行或 session ID。

快照目录只含本轮隔离的 Rime 部署数据。记录其固定前缀和最终清理结果，不把临时绝对路径、整段候选输出或数据库 dump 写入正式文档。

## 动作与预期状态

所有增减量都相对于动作前一次 `case-status`。本轮数据库安装前不存在，因此出现无关 term、import batch 或无法解释的聚合增量时，停止并保留数据库。

| 动作 | 必须成立的结果 |
| --- | --- |
| 初始快照 | 五项固定；`时` 为 display `1` / engine `1`；term、weight、tombstone 均 absent |
| TextEdit 第一次选 `时` | selection `+1`、active term `+1`、ranker row `+1`、frequency `1`；新鲜快照为 display `0` / engine `1` |
| TextEdit 第二次选 `时` | selection `+1`，active/ranker 行数不变，frequency `2`；候选提交仍是 `时` |
| 进程重启后再选 | 系统重拉起后仍为 display `0` / engine `1`；selection `+1`、frequency `3` |
| `dict delete` | active `-1`、ranker `-1`、tombstone `+1`、negative aggregate `+1`；term 为 deleted/weight `0`；新鲜快照中 `时` 为 display `4` / engine `1`、deleted penalty `10` |
| delete 后再次选择 | selection `+1`；term 仍 deleted，ranker 仍 absent，tombstone/version 不变 |
| `dict restore` | active `+1`、tombstone `-1`、ranker 仍 absent、negative aggregate 保留；source 为 `manual_add`，`restored_at_ms` 与新 version 均晚于 delete；新鲜快照回到 display `0` / engine `1` |
| restore 后再次选择 | selection `+1`、ranker row `+1`、frequency 重新从 `1` 开始；tombstone absent |

真实 TextEdit 组固定先用数字 `2` 选择初始 display `1`，再输入 `shi` 并以当前 display `0` 的 Space 选择；每次提交后只留下固定测试字符。来源监视必须证明输入期间 current source 精确为 `org.radishlex.inputmethod.macos.Pinyin`，并在返回 Codex 前证明已切回中立 source。

重启前由开发者先手动切回中立 source，执行者在授权 A 下运行：

```bash
./scripts/stop-macos-imk-process.sh --authorized-stop-process
```

该入口只终止名称和完整命令行均匹配固定安装 executable 的进程，不查询 TIS、不删除路径。停止后先跑 `case-status` 和新鲜非选择快照；随后由开发者手动重选 RadishLex，让 macOS 重新拉起，不能直接运行 bundle executable。

删除与恢复只在进程已停止时执行：

```bash
cargo run -p radishlex-ime-cli -- \
  dict delete \
  --db "$HOME/Library/Application Support/RadishLex/userdb.sqlite3" \
  --input shi --text 时

cargo run -p radishlex-ime-cli -- \
  dict restore \
  --db "$HOME/Library/Application Support/RadishLex/userdb.sqlite3" \
  --input shi --text 时
```

## 隐私、unknown、P0 与 secure

隐私组先保存 `case-status`，再在授权 A 下启用固定产品键：

```bash
./scripts/manage-macos-imk-privacy-mode.sh --authorized-enable
./scripts/stop-macos-imk-process.sh --authorized-stop-process
```

开发者重新选择 RadishLex，在 TextEdit 输入并选择一次 `r01b-shi-time-v1`。既有本地排序仍可读，但 selection、term version、ranker frequency、tombstone 与所有聚合计数必须零增量。随后恢复并精确复验原值：

```bash
./scripts/manage-macos-imk-privacy-mode.sh --authorized-restore
./scripts/manage-macos-imk-privacy-mode.sh --status
```

unknown 与 P0 组分别启动构建门禁生成的两个 host：

- `target/macos-imk/validation-host/unknown/RadishLexContextValidationHost.app`
- `target/macos-imk/validation-host/p0/RadishLexContextValidationHost.app`

两者的普通框只输入固定 case；允许 engine 提交，但全库聚合必须零增量。P0 Bundle ID 属于生产敏感分类，unknown 保持 `contextKnown=false`。两者都是 engine-only，真实顺序受 librime user-data 影响，前序选择可能已把 `时` 提升到首位；不得预设数字 `2` 或把 UI 变化归因于 RadishLex ranker。只确认目标仍在五项中并按实际 display index 提交、记录该 index。host 不读取、记录或持久化正文，也不调用 secure event API。

secure 组只使用 host 的 `NSSecureTextField` 和固定 case。若状态显示 Secure Event Input 已启用，但系统未向第三方输入法路由，只记录“系统 secure 路由旁路，controller secure 分支未由本组实机执行”，并附 source 与 userdb 零增量；不得写成 `policy_blocked` 实机通过，也不得注入按键、强制 source 或调用 secure API。自动证据由分类合同和 native FFI policy 测试承担。

## 停止后的同产物补验

同一签名产物若已完成排序、重启、删除/恢复和隐私组，仅因 context host 人工交接污染而停止，可保留污染前逐步冻结的证据；污染后状态不得继续使用。只补 unknown/P0/secure 还必须满足：

- 上一轮已完成隐私恢复、授权 A 普通清理、授权 B 精确 userdb 删除和全部基线复验；
- 产品源码无变化，Apple Development 五项哈希、Team ID、签名和固定输入逐项一致；
- 重新取得原子基线与 receipts；全新 userdb 先做一次 TextEdit 固定选择，确认 active/ranker/selection 和 frequency 均为 `1`；
- 后续每个 host 都按本 runbook 的人工交接规则闭合，并在进入下一 host 前确认全库聚合零增量。

产品输入、签名、五项哈希、前序证据或清理状态任一不满足时，不得使用补验路径，必须重新执行完整 R01B 序列。

## 停止线与记录边界

出现以下任一情况立即停止后续动作，保留 userdb 和 receipts，先完成可安全执行的授权 A 平台回滚：

- source 归属、五项 case、display/engine mapping、个人化状态或聚合增量不符合预期；
- `storage_unavailable`、`read_failed`、`rank_failed`、学习失败或 SQLite busy/损坏；
- 状态输出缺失、重复、不可观测，隐私值漂移，父目录 inode/mode 漂移，receipt 异常；
- secure 路由不明确，unknown/P0 host 出现内容持久化或真实数据；
- lsof 无法证明数据库已关闭，或清理目录出现四个固定 userdb 文件以外的条目。

正式记录只保留：case ID、build/五项哈希、固定数据哈希、source 归属、逻辑增减量、term/ranker/tombstone 状态、display/engine index、隐私 baseline 类型和通过/失败。不得记录真实输入、原始 P1 行、session ID、真实 App 内容、窗口标题、数据库 dump、secure field 截图或含敏感内容的屏幕图。

## 回滚与授权 B

无论验收是否通过，开发者先把所有测试文稿切回系统 source，执行者恢复隐私 baseline，并在系统设置真实移除 RadishLex。停止监视后，在授权 A 下执行普通清理：

```bash
./scripts/cleanup-macos-imk.sh --authorized-after-settings-removal
./scripts/cleanup-macos-imk.sh --status
```

只有 TIS zero、bundle/Rime absent、进程 stopped、隐私值精确恢复、所有 CLI/host 已退出、父目录与 receipt 仍匹配，才可申请授权 B。取得授权 B 后唯一允许的数据命令是：

```bash
./scripts/cleanup-macos-imk-r01b-test-userdb.sh \
  --authorized-delete-r01b-test-userdb
```

入口先复核完整状态，并用 `lsof` 对存在的四个固定文件逐一证明无打开句柄；helper 再通过已打开的同一父目录 fd 复核 receipt、device/inode/uid、mode `0700`、主库存在、所有条目均为当前用户拥有的普通 `0600` 文件，随后只以 `unlinkat` 删除 `userdb.sqlite3`、`-wal`、`-shm`、`-journal`。它不使用通配符，不删除父目录；成功后要求父目录为空、恢复 `0755` 并移除 receipt。

归属不清、`lsof` 不可用、未知条目、symlink、权限或 inode 漂移时一律保留数据。helper 在 sidecar/main 删除、父目录权限恢复或 receipt 收尾中途失败时，会保留同一 baseline 约束下的可恢复状态；不得手工 `rm` 补做，只能记录失败类别，并在同一授权 B 仍有效且完整状态复核再次通过时重跑专用命令。最终基线必须与安装前逐项一致，隔离快照目录也只按本轮记录的精确路径清理。
