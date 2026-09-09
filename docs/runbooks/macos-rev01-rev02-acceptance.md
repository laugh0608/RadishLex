# macOS REV-01 / REV-02 候选与验收

本文面向维护者与实机验收者，绑定本轮隐私和 SQLite 修复候选、仓库验证及待执行矩阵。它不授予系统安装、进程控制、输入源切换或真实数据操作权限，也不替代历史 M4 验收记录。

## 候选身份：2026-09-08

- 产品 `26.7.1 (39)`，产品源码提交 `5e9b0a8`，包含隐私修复 `40cd1bd` 与 SQLite 升级 `a03c69b`。包内 smoke 提交为 `1ec8859`；该测试和后续文档不进入本次产品二进制。
- 装配：`target/macos-product/26.7.1-39/`；最终本地载体：`target/macos-release/26.7.1-39/`，含 `Product/`、`InstallPayload/` 与 `RadishLex Installer.app`。
- 本机 native FFI 为 `arm64`，最低系统声明 macOS 13.0；不据 Manager 外壳的其他架构切片宣称 Intel 产品已验收。
- 构建工具：Rust 1.96.0、Xcode 26.6 / 17F113、Flutter 3.44.0 / Dart 3.12.0；librime 1.17.0。使用既有离线依赖，Cargo/pub 锁未变化；根 Rust 1.80 声明仍未验证，见 REV-06。
- `community-adhoc-v1`，未公证、未制作新 DMG、未发布。Installer 及双组件签名、双份 payload、ProductManifest、ReleaseIdentity、RimeData 与 native library manifest 均通过验证。
- `UpgradeSources` 为空，没有把历史 build 38 声明为升级源。此载体准备首次安装验收；不能用于直接证明或执行 build 38 → 39 升级。

以下 SHA-256 对应最终载体；完整 manifest 同时绑定其余资源、依赖与 helpers：

| 文件 | SHA-256 |
| --- | --- |
| `Product/ProductManifest.json` | `de1bf1ea937f742242975ad05a1d639e1d3899454e9acfb41eb28e089fe8362f` |
| Installer 内 `ReleaseIdentity.json` | `dfe8855fcaf4c7f15e27672bc01abfe982d173dd3c1e191febc3f503da2d3dde` |
| InputMethod FFI | `8a815ac6382c97d3f5f056fc8a3d19c8de967dc1f1af2a7cd7787cfd44f7b3cd` |
| Manager FFI | `791a76b345d4d9bd24201a138e7b5c535e1c0c25adca3601ccaf42ec8edd3d1f` |
| RimeData `SourceManifest.json` | `3a224aabcf0f7ca1e4c03042fd163a76b25a944bab88b652e412708be9c25583` |
| `radishlex_pinyin.schema.yaml` | `1b94cc5bd763b34dc86e782bdcfb46b9b14b1f8a9973af9e7864bd4754335bf1` |

双 FFI 均内嵌 SQLite 3.51.3 source id `2026-03-13 10:38:09 737ae4a34738ffa0c3ff7f9bb18df914dd1cad163f28fd6b6e114a344fe6d618`，未发现旧 3.46.0 source id，Mach-O 未链接系统 SQLite。这里是包内二进制静态身份核对；实际 SQLite 版本查询、WAL 和旧库回归见 [REV-02](../remediation/product-review-2026-09.md#rev-02sqlite-wal-reset-修复版本)，不把静态扫描表述为应用内 SQL 查询。

## 已完成的候选检查

| 检查 | 结果与边界 |
| --- | --- |
| 完整仓库门禁、macOS/Linux 版本元数据与冻结 L6 合同 | 通过；当前 build 39 被旧 L6 target 资格正确拒绝 |
| 双组件与 Installer 构建、签名、资源/依赖/载荷身份 | 通过；只构建和读回文件，未启动 app |
| 包内 InputMethod FFI：普通、隐私、unknown、secure、sensitive、隐私→普通、普通→隐私→普通 | 7 个独立合成进程通过；普通场景 1 条事件/词条，其余 0；commit 保留、候选页 5 项、无 Rime 自有 userdb、SQLite 完整性通过 |
| 包内 Manager FFI smoke | 合成临时库 smoke 通过；装配与最终载体的 FFI hash 相同，未调用真实 Keychain |
| Manager analyze / test | `--no-pub`：无问题、99 项测试通过 |
| 旧 build 38 与冻结输入 | 104 项文件树 mode/hash/link 一致；Cargo.lock、pubspec.lock、L6 pair 文件及旧 manifest hash 不变 |

本轮 C probe 直接链接最终 `Product/Components/RadishLexInputMethod.app` 中的 dylib，使用同包 RimeData。上下文由测试显式注入，只验证 FFI/runtime 行为；不证明 macOS secure input、应用识别或控制器路由已复验。分段、重启、旧 Rime 合成库及删除恢复的较宽 native 覆盖来自上一修复批，也不替代本候选实机矩阵。

可重复的包内隐私检查（只创建新的私有临时目录，失败也保留证据）：

```bash
python3 scripts/macos-imk/check_bundled_privacy.py \
  --product-root target/macos-release/26.7.1-39/Product
```

入口先校验 ProductManifest，编译 [C probe](../../platforms/macos-imk/Tests/bundled_privacy_smoke.c)，分别运行七个场景，再只读核对刚生成的合成 SQLite。它不重建产品，不接受真实 userdb 作为输入，也不操作系统输入源。结果目录中的 `result.json` 绑定产品 manifest、FFI 与 probe 源码 hash。

## 实机前置条件与停止条件

项目所有者已于 2026-09-09 授权并完成首次安装及 Manager 启动，结果见下方实机记录；原当日计划保留在[周志](../devlogs/2026-W37.md#2026-09-09明日事项)。下面的前置检查应在对应操作前重新满足，不能把历史空路径与静止观察当作当前仍成立。

1. 先重新只读盘点本机现有 `luobo` 账户：固定双 bundle 路径、现有 build、外层 receipt/guard、相关进程与输入源状态；不读取真实 P1 明文记录，也不从旧授权推断可停进程或覆盖安装。
2. 以[产品包边界](../macos-product-package-boundary.md)和[Installer 边界](../macos-installer-app-boundary.md)为准，确认安装资格。有效 completed remove 可以再次 first install，不要求更换系统账户；已有数据、receipt 或身份差异必须有明确处置范围，不能靠删记录或忽略检查伪装为空基线。需要升级则另行准备经过资格验证的历史 source 载体。
3. 确定合格的验收账户与合成数据范围后，单独批准 Installer GUI、实际安装及双组件启动。固定目标为该用户的 `Applications/RadishLex Manager.app`、`Library/Input Methods/RadishLexInputMethod.app` 和 `Library/Application Support/RadishLex`；默认保留数据，禁止绕过 Installer 手工复制 app。
4. 系统设置、输入源切换、按键输入、退出/重启各按明确动作授权；任何自动点击或合成按键同样在此范围内。真实密码、联系人、证件等不得用作测试材料。
5. 安装后重新交叉核验 receipt 终态、双组件身份、固定路径与实际运行程序，再开始输入矩阵。发现身份漂移、事务未知、越界学习、commit 丢失或异常持久化，立即停止当前场景并保留现场，不自动 repair、retry 或 cleanup。

## 当前账户只读盘点：2026-09-08

- 当前 console 与进程用户均为 `luobo` / uid 501。固定双 bundle 不存在；沙盒外 TIS 状态为 `matches=0 enabled=0 selected=0`，精确产品进程查询无 InputMethod、Manager 或 Installer。
- Application Support 仍为当前用户 `0700` 目录，保留 Rime、SQLite、WAL/SHM 与 `.radishlex-install-v1/receipt.json`。仅读取路径元数据和安装 receipt，未打开数据库或读取 P1 正文。
- receipt 为 build 38 的 `remove_programs/completed`，operation `262d275f187ed2b46abd547527398ec1`，无记录的 failure 或 manual recovery 标记。receipt SHA-256 为 `d463175c3cf1ad0cbed0b7eb83d0724a28d5c65531b8c94b5ee67b0bbf6b97de`；这些字段不表示其当前身份校验已通过。
- receipt 的 data-root device id 为 `16777230`，实际目录为 `16777234`，已在沙盒外交叉确认；inode `18234715`、uid 501 与 `0700` 一致。安装核心要求完整 root identity 相等，当前差异不能静默忽略；本轮未启动 Installer 取得 driver snapshot，也未确定设备号变化原因。
- 本次盘点未把该保留数据现场作为新候选可写目标，未改写 receipt、删除旧库或执行安装。当时建议独立测试账户；项目所有者随后选择继续使用本机现有账户，当前方向以下节为准。
- 新证据根为 `/private/tmp/radishlex-build39-host-inventory-lgqvz4r8/`。`inventory.json` 保留初始沙盒观察；其中 TIS XPC 错误和 `process=unavailable` 不作为通过依据，沙盒外确认结果另存 `confirmed-observations.json`。冻结历史材料未补写。

## 本机保留式整理：2026-09-08

项目所有者明确倾向本机测试，并允许清理或整理之前的记录。本批据此准备把旧现场整体归档，保留原始数据库与证据，不创建宿主账户或虚拟机，也不调整生产安装器的身份比较规则。独立账户不是安装器的必要条件，之前的建议不再作为当前前置要求。

- 精确范围：当前用户 `Library/Application Support/RadishLex` 一项，以及 `Applications`、`Library/Input Methods` 下各八项 `.radishlex-install-<operation>`，共 17 个目录根、305857438 bytes 文件内容；目录只含已盘点的原数据、空事务目录或 build 35/37/38 的 `source-backup.app`。
- 八个 operation 为 `1f7d24f7f47e4aec432efe77d625251e`、`262d275f187ed2b46abd547527398ec1`、`412ec3c9c986e62e27d03ba4f2b1682d`、`63f3fb75783962623c0fe98c7d1173b8`、`7dcbddb367633069b365ef6662dd16c1`、`819d8d7f2f14b30fe4777271c216b8de`、`99696cf4e293c18313f10aa83bde0485`、`f88e9b979b32845bfdd9712e88157619`。命名或版本归属不替代完整历史事务资格；本批只保管原对象，不将其声明为新升级源。
- 目标为当前用户 `Library/Application Support/RadishLex-Archives/20260908-before-build39/original-home/`，保留各对象的原 home-relative 路径结构；归档父目录私有 `0700`。采用同文件系统 rename，保留文件内容、inode、mode 与内部相对 symlink，不永久删除、不重写 receipt、不单独移动 SQLite 主文件而遗漏 WAL/SHM。
- 准备材料为 `/private/tmp/radishlex-macos-history-archive-20260908/` 的 `archive.py`、`plan.json`、`prepare.log`；计划 SHA-256 为 `1b1bd39276f4a17fac662041bf2760aa1b3eb145cc89d060aea011cdfbb13c6a`。只读 prepare 绑定精确父目录身份、每个节点元数据/hash/link、已观察 receipt 与脚本 hash。
- 执行前再次核验全部树、输入源归零、产品进程停止、无打开句柄和 guard。目标碰撞、跨设备、未知条目、源漂移或检查不可用均停止；每个 rename 前后核验并持久记录 journal，中断保留部分归档，禁止自动重试或回滚。
- 执行状态：按上述计划取得精确系统写入授权后，17 项已全部归档，未执行安装。合成测试覆盖同设备 rename 保持内容/inode/mode/link、目标存在与源漂移时零 mutation、外部 symlink 拒绝；真实执行逐项记录于归档根的 `moves.jsonl`，`completed.json` 为 `archived`，永久删除数为 0。
- 独立读回确认原 17 项路径全部 absent，归档内 545 个节点的 device/inode/owner/group/mode/mtime、文件大小/hash 与内部链接一致；receipt 原 hash 保留。执行前后产品进程与打开句柄均为零，TIS 为 `0/0/0`；最后独立 `--status` 确认正式 data root、Rime、userdb/sidecars 和 InputMethod bundle 均 absent、进程 stopped。build 39 最终 ProductManifest 再次验证通过。

09-08 归档完成时形成了新的空安装路径；09-09 的 build 39 安装和 Manager 启动已创建新的测试数据根与空 userdb，未导入旧学习库。归档属于此前明确整理范围，build 38 候选制品、历史验收文档及 Linux 冻结资产不在移动范围。设备号变化的触发原因仍未证实，生产长期身份与重新挂载兼容问题另行评估。归档不是安装事务恢复或生产身份修复；不要把旧资料直接覆盖回新的运行目录，恢复/再利用必须另行确定范围并检查目标状态。

## 本机首次安装与 Manager 启动：2026-09-09

- 安装前在现有 `luobo` / uid 501 账户重新核验：ProductManifest、Installer 内外 payload 和 strict ad-hoc release identity 全部通过，候选摘要与上方冻结身份相同，历史升级源为空；正式双程序、数据/事务根和残留 operation 目录均 absent。输入源为 `0/0/0`，三个产品进程均未运行。沙盒内 TIS XPC 错误及进程查询不可用单独保留，使用沙盒外获准的只读查询确认，不将沙盒失败当作通过。
- 项目所有者明确授权启动本地 build 39 Installer、通过界面首次安装及静止确认、核验终态并启动 Manager。Installer 先显示 `begin_first_install`；首次确认后持久化 `prepared`，此时复核输入源为零、仅 Installer 运行，再确认推进。operation `514bd2af74b57ee131b66104b1333962` 到达 `first_install/completed`，无 failure 或 manual recovery；UI 随后投影为已安装后的 `begin_repair`，本轮未点击 repair/remove。
- 已安装 Manager 53 个节点、InputMethod 48 个节点均完成递归 xattr 只读检查，无 quarantine；双程序 ProductManifest/tree、ReleaseIdentity/strict ad-hoc 及版本 `26.7.1 (39)` 验证通过。新 data root 为 uid 501 / `0700`，device `16777234`、inode `32912651` 与 receipt 完全一致；外层 state 仅有 receipt，无残留 guard。安装完成、Manager 启动前无 userdb/settings/Rime。
- Manager 经固定 `Applications/RadishLex Manager.app` 启动，真实进程路径不在 App Translocation；界面进入空词库，显示 `local_only`、无可显示词条、无 deleted tombstone、导入历史零批次。结合普通启动路径在 Flutter 前强制执行两层 gate，此结果证明 Manager 启动门禁允许；没有采集或编造成功 gate 数值。启动后创建 uid 501 / `0600` 的 `userdb.sqlite3`，receipt 字节保持不变，未读取库正文或导入旧资料。
- 本批结束时 Installer 和 Manager 继续运行；InputMethod 进程未运行，尚未手动添加/选择输入源，也未执行输入、隐私、删除恢复、重启或旧合成库矩阵。本次不复验 DMG 下载/Gatekeeper，不关闭 REV-01/REV-02，不操作 Linux、旧归档、repair/retry/remove 或发布。
- 下一步由项目所有者手动在系统设置添加并选择 RadishLex，再以公开合成文本开始普通输入验收；AI 不程序化注册或切换输入源，按实际结果推进后续矩阵。

## 待执行实机矩阵

首次安装与双端固定路径启动、普通 `shi → 时` 学习及隐私模式完整 composition 零学习已验证；恢复普通后的精确身份恢复学习，但全库额外增量仍待归属。其余覆盖仍待执行。仅对本轮新建验收库记录必要计数/摘要；截图不得含真实输入历史。

| 场景 | 需要的实机证据 |
| --- | --- |
| 首次安装和固定路径启动 | 09-09：首次安装 completed、双 bundle 身份/无 quarantine、固定路径 Manager 和 InputMethod 正常启动；Manager UI 与真实输入路径分别证明前置 gate 允许，未采集成功 gate 数值 |
| 普通拼音、选候选、翻页与中英切换 | 09-09：TextEdit `shi → 时` 数字 2 选择、再次候选 1 空格提交及同库学习增量通过；翻页、产品内中英切换与重开仍待执行 |
| 隐私模式与 composition 中双向切换 | 09-09：全程隐私 `shi → 时` 正确提交且零学习，普通恢复后目标频次递增；恢复区间额外全库增量待归属，未提交 composition 内双向切换仍待执行 |
| secure input / 敏感应用 / unknown 路由 | 记录系统实际是否把事件交给 IME；系统绕过与控制器收到受限上下文分别判定，不用 C 注入结果填充实机通过 |
| 分段候选和自动 commit | 完整文本、分段归属与最严格隐私状态跨段保持，无隐私尾段补学习 |
| Manager 与输入法共享状态 | 同一合成库的条目、删除、显式恢复及候选变化一致；删除后重启/迟到选择不复活 |
| 正常退出与重启 | 新普通学习保留；隐私输入无新增事件或 Rime 学习库；不使用 kill 模拟正常退出 |
| 旧合成库打开、备份恢复 | 3.46/schema 9 合成 fixture 在独立目标继续学习、保留删除语义，恢复后完整性通过；禁止覆盖真实库或冻结证据 |
| 输入质量对照 | 固定合成词集记录候选与排序变化；未有独立新词召回的行为如实记录，不宣称 Rime 自有学习收益仍然存在 |

本候选准备不关闭 REV-01/REV-02，也不改变 Linux M5 系统操作顺位；Linux 新 pair 与新制品、真实平台回归、输入质量和 MSRV 仍需各自闭合。原 build 38、P04、M4 历史数据与 L6 冻结资产不得用作本批可写测试目标。

## 普通输入与同会话学习：2026-09-09

- 项目所有者手动添加并选择输入源后，只读状态为 `org.radishlex.inputmethod.macos.Pinyin` selected=1，TIS `matches/enabled/selected=2/2/1`；InputMethod 为固定安装路径 `running_verified`，运行数据与 sidecars 已存在。首次安装 receipt 仍为相同 operation 的 `completed`，data root 与 userdb 身份保持一致。
- 固定用例 `build39-textedit-shi-time-v1`：在 TextEdit 空白文稿输入 `shi`，第一次按实际候选数字 `2` 选择“时”；第二次“时”位于候选 `1`，按空格选择。两次候选编号、实际提交“时”和未感到卡顿均来自项目所有者人工反馈，不是自动抓取的候选/commit 或延迟测量。两轮反馈后只读 TIS 均为 selected=0；只证明检查时已切离 RadishLex，没有连续输入源监视证据。
- 第一轮前全库 user_terms/selection_events/ranker_weights 均为 `2`，不是空库；仅读取计数，不追溯两条既有记录正文。精确 `shi → 时` 身份当时无 term/ranker/tombstone。第一次之后三项总数均为 `3`，精确身份出现 `engine_selection/active` 词条和 `editor` frequency=1 的排序摘要。第二次之后总数为 `3/4/3`，精确 frequency=2，未产生重复词条/排序行。
- 三次只读快照均为 schema 9，deleted_terms、negative_feedback、import_batches 为零；Rime `*.userdb*` 条目计数为零。查询仅用 SQLite `mode=ro`、`query_only=ON` 和同一读事务读取聚合与固定合成身份，未调用可能维护权限/迁移的 CLI，也未读取 P1 行。
- 本组验证真实普通选词、学习持久化及同会话候选变化；没有 fresh-engine 独立排序对照、重启、压力延迟或广泛输入质量结论。本组结束时隐私键为 absent，后续设置与隐私用例见下节，不能据本组关闭 REV-01/REV-02。

## 隐私模式与恢复普通学习：2026-09-09

- 项目所有者先在 Manager 打开隐私开关，首轮只读前置检查仍为 `privacy_mode=absent`，设置文件不存在；未进行测试输入。CUA 观察到编辑草案的开关为开，保存按钮在下方，滚动后由项目所有者点击“保存草案”并反馈保存成功。此后沙盒外平台 API 和 `manager-settings.json` 均为 true。草案状态不代替系统生效；没有将最初的“已开启”解释为已验证成功，原前置观察与后续消歧分别保留。
- 完整 composition 隐私用例 `build39-textedit-private-shi-time-v1`：开启保存后，从新的 TextEdit `shi` composition 选择“时”一次，项目所有者反馈候选 1、提交“时”、未感到卡顿。前后平台隐私状态均为 true；term/selection/ranker 总数保持 `3/4/3`，全部六类聚合零增量，精确 `shi → 时` 的 term、频次 2、last-used/updated 时间戳及其余采集字段逐项不变，Rime `*.userdb*` 仍为零。此用例的正常提交与零学习通过。
- 项目所有者按步骤关闭隐私开关并保存，再开始新的 `shi → 时`；反馈候选 1、提交正确、未感到卡顿。事后平台与 Manager 持久设置均为 false，精确 `editor` frequency 从 2 增至 3；固定合成身份 selection 总数为 3，证明该身份恢复学习。
- 恢复区间的全库 term/selection/ranker 从 `3/4/3` 变为 `5/7/5`，即事件增 3 而非单次输入预期的 1；两个额外事件及新增词条/摘要尚未归属。已仅询问期间是否另有 RadishLex 输入，不索取或读取内容；在澄清或补充干净用例前，不将该区间写成单次全库精确恢复通过，也不能据此推断隐私阶段泄漏。
- 两次提交后只读 TIS 均为 `2/2/0`，InputMethod 固定路径继续运行，数据根、userdb 与 completed receipt 身份不变。仅查询聚合和固定合成身份，无 P1 行。隐私键从本轮最初 absent 经用户 GUI 保存变为 true，再变为显式 false；当前不是键 absent，也没有执行脚本式原值恢复。
- 本组尚不覆盖未提交 composition 中途切换策略、secure/sensitive/unknown、分段/自动 commit、重启或删除恢复。不能通过切到 Manager 后输入新 composition 冒充同一 composition 的策略往返；平台 deactivate 会取消未提交 composition，后续用例需要保持被测输入上下文。

## 证据位置

- 09-09 普通输入：`/private/tmp/radishlex-build39-input-20260909-_drd2sd7/`，含 `before-shi-time.json`、`after-first-shi-time.json`、`after-second-shi-time.json`；人工反馈与只读聚合分别注明来源。
- 同目录隐私记录：`before-private-shi-time.json`、`privacy-enable-precheck-not-qualified.json` 保留未生效的初次前置与消歧；有效基线为 `before-private-shi-time-saved.json`，后续为 `after-private-shi-time.json`、`after-restored-normal-shi-time.json` 和 `privacy-roundtrip-status-and-discrepancy.json`。额外全库增量的归属仍待补证，禁止改写原快照。

- 09-09 安装前：`/private/tmp/radishlex-build39-preinstall-20260909-oyfvjeis/`，含 `inventory.json` 与沙盒外 `confirmed-observations.json`；初始沙盒失败日志保留。
- 09-09 首次安装：`/private/tmp/radishlex-build39-first-install-20260909-y8ikrpku/`，含 `prepared-receipt.json`、`installed-receipt.json`、`postinstall.json` 与 `manager-startup.json`；UI 观察与真实进程查询分别注明来源。
- 身份与冻结输入核对：`/private/tmp/radishlex-build39-preflight/candidate-identity.json`；前置快照在同目录。
- 装配、Installer、完整门禁与 artifact verify：`/private/tmp/radishlex-build39-{assembly-escalated,installer,check-repo-escalated,artifact-verify}.log`。
- 最终七场景：系统临时目录下 `radishlex-bundled-privacy-jw_2a6y3/result.json`，入口输出保存于 `/private/tmp/radishlex-build39-bundled-privacy-final.log`。
- Manager 包内 smoke：`/private/tmp/radishlex-build39-candidate-fe6xq27f/manager-smoke-escalated.log`；Flutter 日志为 `/private/tmp/radishlex-build39-flutter-{analyze,test}-escalated.log`。
- 沙盒失败日志一并保留：Flutter 缓存权限、仓库 Unix socket guard 和 Manager 本地同步资格初始化均按最小范围提权后通过；没有跳过检查或下载依赖。
