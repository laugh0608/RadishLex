# Linux Manager 本地验收边界

本文定义 M5-P04 的 Linux Flutter Manager 产品 host、共享 XDG 数据、隐私配置、应用粗分类、同库并发和本地个人化验收边界。读者是维护 `apps/radishlex-manager`、`platforms/linux-fcitx5`、`ime-ffi` 与 Linux 验证入口的协作者。本文不定义发行版安装、系统级路径、package transaction、升级移除、真实用户同步、恢复码、设备授权或新的 Manager 页面设计；M5-P05 见 [Linux 安装维护边界](linux-installation-maintenance-boundary.md)，同步能力仍留在后续开放评审。

## 当前结论

截至 2026-08-05，M5-P03 已完成 Fcitx5 Wayland/X11 输入、常见应用、生命周期、离线与隐私实机验收，M5-P04 已完成 Linux Manager 与同库个人化验收。现有 Flutter 页面、`ManagerBridge`、ABI v9、Rust userdb/ranker、导入导出、删除/tombstone/explicit restore、学习摘要和 rank explain 均直接复用；本批没有重写业务真相源，也没有通过新增平台私有 ABI 复制既有能力。

首个源码子批已建立 Linux Flutter runner、共享 XDG/Manager runtime、bundle `.so` 约束、Linux privacy 配置与同库 native-rime contract。后续子批建立先 watch 后初读的 privacy 感知、失败关闭 runtime、精确 allowlist 粗分类和默认关闭的受控取证模式。UTM Debian 13 ARM64 已完成真实 Flutter 3.44.0 Release bundle、Fcitx5 addon、ELF/`$ORIGIN`、正式 ABI symbol、native-rime 与 Dart smoke；P04 实机时 Fcitx CTest 为七项，当前仓库在 staged/system runtime layout 与两种 startup 编译身份加入后为 10 项。后两项尚未形成新的 ARM64 startup-enabled payload 实机证据。

Firefox 已在 Wayland/X11 两侧取得相同的精确 `firefox-esr` 身份、GTK3 Fcitx frontend、password capability 和 userdb 零增量证据；生产 allowlist 因而只加入 `firefox-esr -> browser`，其他候选与变体继续失败关闭。生产分类后的桌面纵向链已证明 Manager 刷新、privacy 开关、单次恢复学习、删除、既有/新 Fcitx session 防复活、explicit restore 和恢复后重新学习均读取同一 userdb。

同一 guest 与 XDG userdb 上的真实 Manager I/O 已用两条公开合成词闭合：dry run 检查不落库；普通导入后删除“爸”，再次普通导入只更新仍 active 的“巴”并跳过 tombstone；显式导出只含 active P2 词条“巴”和“把”，不含 tombstone 或 P1 明细。Manager 进程重启与完整桌面 X11 session 重启后，新 Manager/Fcitx 进程继续读取同一 schema v9/WAL 数据库，候选排序、rank explain、导入批次和聚合终态一致，Escape 取消未增加 selection。

本结论来自固定 Release bundle、实际 addon/FFI 映射和用户实体输入，没有使用空 `linux/` 目录、fixture mode、手工 native library 路径或单连接 SQLite 测试冒充完成。最终 Manager 聚合为 user terms 2、selection events 5、suppressed 0、deleted 1，另有 import batches 2、negative feedback 2、ranker weights 1；`integrity_check=ok`，没有非预期 `SQLITE_BUSY`、migration 漂移、投影错误、丢失或重复学习。

## 验收范围

M5-P04 纳入：

- Flutter Linux Release/development bundle 从固定相对布局加载 workspace 同版本 `libradishlex_ime_ffi.so`。
- Linux host 通过与 addon 相同的 C++ XDG resolver 返回 userdb、Manager settings 和平台 privacy 路径。
- 正常启动默认进入 `product` mode；路径、权限、native library、ABI 或 userdb 失败以结构化错误失败关闭，不回退 demo。
- Manager 与 Fcitx addon 打开同一 userdb，覆盖 WAL、busy timeout、migration、并发可见性与重启行为。
- Manager 查看、刷新、导入、导出、删除、tombstone 查询、explicit restore、学习摘要和 rank explain 的 Linux 产品复用。
- Linux privacy 配置由 Manager 写入、读回，addon 及时感知；开启后允许读取既有本地摘要，但禁止新学习写入。
- Fcitx capability 与受控程序身份只映射为固定粗粒度 `LearningContext`，不把原始程序名、窗口标题或正文传入 Rust、日志或 userdb。
- 合成词学习改变后续候选，删除不被旧选择或导入复活，显式恢复后新 session 才可重新学习。
- 脱敏诊断不输出用户词、P1 原始事件、真实路径、程序身份、token 或 payload bytes。

M5-P04 不纳入：

- 写 `/usr`、发行版 package、桌面文件安装、Fcitx 自动配置、产品 autostart、升级、repair、remove 或数据保留事务。
- Linux 产品签名、公开载体、正式 Release、tag 或远端上传。
- 真实用户同步、恢复码、设备授权、撤销、key epoch 或生产部署开放。
- 新 Flutter 信息架构、跨平台统一候选窗、Linux 私有 userdb schema 或 Dart 侧排序/删除逻辑。
- 依靠程序名猜测敏感字段、读取窗口标题、正文、剪贴板或原始输入流。

## 职责分层

| 层 | P04 职责 | 禁止承担 |
| --- | --- | --- |
| Flutter UI / Dart bridge | 复用现有页面与 `ManagerBridge`，显示本地真相源和结构化失败 | 直接拼 XDG 路径、解析 SQLite、复制 ranker/privacy 规则 |
| Linux Flutter host | 启动前收紧 umask，解析固定 bundle/XDG 路径，实现 runtime method channel 与平台 privacy 原子读写 | 候选排序、学习、删除、同步或安装事务 |
| Fcitx5 addon | 消费 capability、受控程序粗分类和 privacy 摘要，向 ABI v9 投影 `LearningContext` | 保存原始程序身份、解析 Manager settings、复制 userdb/ranker |
| 共享 Linux platform code | XDG、private file、bundle layout、privacy format、分类与只读 startup binding contract | Flutter widget、Rime 私有状态、package mutation 或远端配置 |
| Rust FFI/runtime/userdb/ranker | 业务真相源、并发 SQLite、学习策略、删除语义、explain 与管理查询 | Fcitx/GTK/Flutter 生命周期或平台文件选择 |

Linux host 和 addon 可以链接同一份浅层 C++ platform source/target；不得把 resolver 复制到 `apps/radishlex-manager/linux` 后独立演化。Flutter generated runner 只保留平台启动和 channel 接线，较长实现继续放在 `platforms/linux-fcitx5` 的共享职责模块中。

Linux Manager 的主题必须显式提供兼顾 Latin、数字和简体中文的字体 fallback，不能依赖 Flutter/GTK 偶然选中单一系统字体。P04 staged product 已使用受控系统字体族验证可读性；M5-P05 进一步固定为发行版 hard dependency：Debian profile 使用 `fonts-dejavu-core` 与 `fonts-noto-cjk`，payload 只保留 Flutter `Icons.*` 所需的固定 Material Icons 图标字形，不携带或注册其他字体。P05A 已复验两族 system fontconfig 解析；该自动结果与 P04 的真实界面可读性证据共同成立，任一侧缺失都不能宣称字体产品边界完成。

## Linux product bootstrap

### 启动顺序

Linux runner 必须先设置进程 `umask(0077)`，再执行共用 Linux product startup gate，只有取得与编译身份一致的 move-only permit 后才可创建 Flutter application/engine、Dart isolate、settings store 或 userdb，随后注册 `dev.radishlex.manager/runtime` method channel。`createDefaultManagerBootstrap()` 仍是 Dart 唯一业务产品入口，依次取得平台路径、加载 native binding 并形成真实 `FfiManagerBridge`。

startup gate 直接复用 bundle FFI 的 additive request/result v1，在 C++ host 中从当前 executable 取得 canonical component path；调用前以 `dladdr` 与 canonical path 证明 startup/error symbols 确实来自 executable 的精确 sibling FFI，拒绝 loader interposition。输入 session/key ABI 仍为 v9。`development-staged` 只在 system state/receipt 缺失且 package 严格未安装时允许，`debian-system-product` 只接受 terminal receipt、Installed package、严格 staging proof、完整 dependency/version relationship、Manager bundle tree、manifest/同侧 FFI 与双 FFI equivalence。active advisory guard、receipt/stage tmp、nonterminal、半配置、completed remove、缺 receipt 或身份/dependency 漂移都在 Flutter 初始化前退出；gate 不调用 `dpkg`，不创建 state，不解析 XDG，也不打开 userdb/settings/privacy/Rime。字体 family/glyph/owner 仍留给 L6，不由 component scope 冒充。

任一前置失败都返回稳定 `ManagerPlatformException` / `ManagerStartupException`；正常 `product` 构建不得读取 `RADISHLEX_MANAGER_MODE` 以外的运行期路径 override，也不得切换 `FixtureManagerBridge`。显式编译期 `demo` 继续只用于测试和合成演示，并保留全程标识。

### bundle 内 native library

P04 staged Flutter bundle 使用固定布局：

```text
<bundle>/radishlex_manager
<bundle>/lib/libradishlex_ime_ffi.so
<bundle>/lib/libapp.so
<bundle>/data/...
```

Linux host 从当前 executable 的 canonical parent 派生 sibling `lib/libradishlex_ime_ffi.so`，不读取工作目录、`LD_LIBRARY_PATH`、仓库路径或调用方参数。目标必须是 bundle 内非 symlink regular file，且不得 group/other writable；Dart binding 继续验证 ABI contract version 与 Manager 所需 symbol 集。P04 证明 staged product bundle 与 workspace native library 一致；P05A 又以全新 committed-source ARM64 bundle 通过 root-owned 目标布局、product manifest、ELF/closure、双 FFI 和构建路径强门禁。P05B 已形成 actual `.deb` relationship、恢复型事务、production mutable port/system executor、Flutter 前只读 startup decision，以及 source `55351f2` revision 1/target `e5b6da1` revision 2 的真实双 clean-root ARM64 pair；L6 format v1 已固定，但真实进程/dpkg 与 Manager package startup 证据仍未形成。

Dart `ManagerProductPaths` 必须接受平台明确返回的固定 `.dylib` 或 `.so` basename，拒绝其他文件名；错误文案改为平台中立。该调整不得削弱 macOS `Contents/Frameworks/libradishlex_ime_ffi.dylib` 的既有测试和产品门禁。

### XDG 数据路径

Linux host 必须直接调用 `resolveProductionXdgPaths()` 与 private path helper；不得在 Dart、GTK runner 或 shell 中再次拼接 `$HOME`。稳定路径为：

```text
${XDG_DATA_HOME:-$HOME/.local/share}/radishlex/userdb.sqlite3
${XDG_DATA_HOME:-$HOME/.local/share}/radishlex/rime
${XDG_CONFIG_HOME:-$HOME/.config}/radishlex/settings.json
${XDG_CONFIG_HOME:-$HOME/.config}/radishlex/privacy-mode.json
${XDG_STATE_HOME:-$HOME/.local/state}/radishlex
${XDG_CACHE_HOME:-$HOME/.cache}/radishlex
```

production home 仍来自 `geteuid` + `getpwuid_r`。四个 XDG 环境变量只接受既有 resolver 允许的绝对无 dot-component 路径；测试注入只在编译期开启。产品目录为 `0700`，userdb、settings、privacy 与原子临时文件从创建时即为 `0600`；symlink、错误 owner、非 regular file 和 group/other 权限均失败关闭。

## Privacy 配置真相源

### 独立文件而非 Manager settings

Linux privacy 真相源固定为 `privacy-mode.json`，不直接让 Fcitx addon 解析 `settings.json`。Manager settings 是会持续演进的 UI/sync 草案；将其引入输入热路径会扩大解析面、并发面和隐私风险。独立文件与 macOS CFPreferences 的既有职责一致：`settings.json` 中的 `privacy_mode` 只是保存时的 UI 投影，snapshot 加载必须始终由平台 privacy 真相源覆盖。

首版格式只接受：

```json
{
  "format_version": 1,
  "privacy_mode": false
}
```

缺少文件表示 `present=false, enabled=false`，用于首次本地启动。存在文件时必须是 UTF-8、对象根、`format_version == 1` 且 `privacy_mode` 为 bool；类型、版本、大小、权限、owner、symlink 或 IO 异常不得猜测为关闭。Manager 返回结构化 privacy 错误并阻止产品 snapshot；addon 将异常状态投影为 `privacy_mode=true`，同时只记录稳定错误分类，不记录路径或文件正文。

### 写入、回滚与变更感知

Linux host 的 `writePrivacyMode` / `restorePrivacyModeState` 必须在 config product 目录内完成受限临时文件写入、文件 `fsync`、同目录原子 rename、目录 `fsync` 和严格读回。不能先覆盖正式文件后再补权限，也不能让 Dart `File.writeAsStringSync` 成为 privacy 真相源写入者。

现有 `FfiManagerBridge.saveSettingsDraft()` 的顺序保持：保存前读取平台 privacy 状态，必要时先写平台真相源，再保存 settings 草案并复核本地权限；后续失败时恢复先前 privacy 状态。进程在任一步崩溃后，以平台 privacy 文件为准，下一次 Manager snapshot 会覆盖 settings 中可能滞后的投影。

addon 必须在启动时读取 privacy，并在同目录原子替换后使新状态作用于下一次可能产生学习的操作；状态变更期间不能短暂按旧的“允许写入”状态学习。实现可以使用 Fcitx/event-loop 可控的文件通知或等价串行机制，但不得在每个按键同步解析 JSON，也不得增加网络或 Manager 进程依赖。切换状态时清除未提交选择意图并刷新当前 session 的 `LearningContext`。

当前实现先建立 config 目录 inotify watch，再读取初始 snapshot，避免 read/watch 启动窗口漏事件；event-loop callback 与每次 FFI 操作前的非阻塞 drain 串行消费目标文件替换，未发生事件时不读文件、不解析 JSON。目标文件替换/删除/权限变化触发严格重读；非法文件或 watch/队列异常保持本次 addon 进程失败关闭。有效 privacy 位变化会同步现存 session 的学习上下文并移除旧候选面板，Rust context-change contract 清除待学习选择。

## 受控应用粗分类

Fcitx `InputContext::program()` 是平台可用但可能为空的程序身份。P04 只允许在 C++ 平台层短暂使用它做固定 allowlist 粗分类；原始值不得跨 FFI、写 userdb/settings、进入诊断或日志。分类输出只允许 `general`、`browser`、`chat`、`code`、`editor`、`office`、`terminal`、`other`。

策略优先级固定：

1. `Password` 或 `Sensitive` capability 设置对应安全位，不读写 userdb。
2. `Terminal` capability 映射 `terminal`，保持不学习。
3. privacy 开启时保留已知粗分类供本地排序，但禁止新写入。
4. 程序身份只有在 Wayland/X11 实机确认值稳定、普通字段可输入，并完成该应用敏感字段传播评审后，才能加入固定 allowlist 并设置 `context_known=1`。
5. 空值、未知值、大小写/包装器变体未显式列入或 frontend 证据不足时，一律 `context_known=0, context_kind=other`。

首批 allowlist 不能凭进程名猜测。实现批次先用合成 classifier contract 覆盖允许、未知、空值和 capability 优先级，再在 guest 只读记录程序身份摘要；只有取得 GTK/Qt/浏览器对应 frontend 与 sensitive 行为证据后，才把所需固定项写入生产表和测试。GTK4 `PRIVATE` 在当前 Debian frontend 未传播 `Sensitive` 的事实继续保留，不能因应用进入 allowlist 就宣称该字段安全。

合成 classifier contract 已实现并通过：精确、大小写敏感的规则只输出固定粗类别，empty、unknown、大小写和包装器变体均失败关闭；Password、Sensitive 和 Terminal 在分类前返回，addon 因而不会读取这些上下文的原始 `program()`。Terminal 使用 `context_kind=terminal, context_known=0`，保持 EngineOnly。

真实桌面评审使用默认关闭的 `RADISHLEX_APPLICATION_EVIDENCE`。它只把源码内固定候选的精确匹配转换为 opaque token，未命中统一输出 `unmatched`；capability 只输出稳定 on/off 变化。默认产品 addon 的强门禁禁止出现这些日志字符串，该模式不修改生产 allowlist，也不记录原始程序身份、窗口标题或正文。

Wayland Firefox 已得到精确对应 `firefox-esr` 的 `browser_candidate_01`，会话和库映射支持原生 Wayland/GTK3 Fcitx frontend。X11 对照得到同一 token，Firefox 为 X11 client，加载 GTK3 `im-fcitx5.so` 与 `libFcitx5GClient`，且未设置 Wayland display。两侧密码字段均产生 `password_on`/`password_off`，普通字段显示候选、密码字段只显示遮罩且无候选，前后 userdb 七项聚合全零。因此生产表只加入精确 `firefox-esr -> browser`；路径、大小写、wrapper、其他 Firefox 候选、unknown 与 GTK4 `PRIVATE` 仍失败关闭。

## 同库并发与个人化语义

Manager 和 addon 必须把同一 `XdgPaths.userdb_path` 交给现有 Rust FFI，各自建立独立连接。两端不共享 SQLite handle、不通过 socket 协调、不复制数据库，也不由 Flutter 执行 migration SQL。既有 Rust 打开流程继续负责：

- 在任何 schema/integrity SQL 前设置 busy timeout；
- 多连接 migration ownership 与固定预算锁等待；
- WAL 可见性、checkpoint 和 sidecar 生命周期；
- tombstone 防复活、explicit restore 新稳定版本；
- import batch、learning summary 与 rank explain 一致性。

产品验收不使用主库/WAL 文件哈希推断语义。Manager 通过页头刷新重新调用 `loadSnapshot()`；addon 使用新 session 或刷新后的同次 Rust snapshot 观察变化。测试只使用合成词、虚构程序身份和临时 userdb，不读取开发者真实数据库正文。

Manager rank explain 不得固定按 `general` 解释所有词条。当前 Dart bridge 复用现有 ABI，对最多六个可见词条依次查询 `general`、`browser`、`chat`、`code`、`editor`、`office`；存在 frequency、recency、context 或 negative 信号的粗类别分别保留，无上下文信号时只保留 `general` 回退。UI 的筛选、行 identity 与详情都必须包含 context，确保 Firefox `browser` 排序可以和同类别 explain 对照。该过程不新增 ABI、不读取 P1 事件行、不输出上下文分布计数或原始程序身份，learning status 继续保持 `context_stats=false`；`terminal` 与 `other` 不属于允许学习的查询类别。

## 验收矩阵

### 自动门禁

| 编号 | 证据 | 退出要求 |
| --- | --- | --- |
| A1 | 共享 XDG contract | data/config/state/cache、privacy path、owner、权限、symlink、invalid XDG 与 production/test 隔离通过 |
| A2 | privacy store contract | absent、true/false、非法 JSON/版本/类型/大小、权限、原子替换、读回和 rollback 通过 |
| A3 | classifier/evidence contract | Password/Sensitive/Terminal 优先级、privacy、allowlist、unknown/empty、opaque token、默认关闭和产品二进制日志排除通过 |
| A4 | Dart platform bridge | macOS `.dylib`、Linux `.so`、错误码、privacy read/write/restore 与 product/demo 隔离通过 |
| A5 | Linux bundle smoke | Release bundle 含固定 `.so`，无仓库/构建路径回退，ABI v9、required symbols、ELF closure 与启动 snapshot 通过 |
| A6 | 同库双端 contract | 独立 addon/runtime 与 Manager 查询连接覆盖 WAL、选择、刷新、删除、防复活、恢复、并发初始化和重启通过 |
| A7 | 既有回归 | Flutter analyze/test、相关 Rust tests、Linux Fcitx contract、macOS Manager 产品门禁与仓库检查通过 |

A6 必须至少有一条从 personalized runtime 写入、经 Manager bridge 读取，再由另一 runtime session 观察排序/删除的完整链路；单独调用 `UserDb::open` 两次不算产品双端证据。

### Linux 桌面验收

1. 从 staged bundle 无路径 override 启动 product Manager，确认真实 `.so`、XDG 路径与空库/既有库启动；fixture 标识不得出现。
2. addon 与 Manager 的 userdb 路径、owner 和 inode 一致；只记录路径类别与 identity 摘要，不读取或记录真实正文。
3. Manager 导入公开合成词，Fcitx 新 session 可见；导出只包含用户显式请求的 P2 词条视图。
4. 在已通过分类评审的普通应用中选择合成候选，Manager 刷新看到一次聚合变化，后续候选排序与 rank explain 一致。
5. Manager 删除后，既有或新 addon session 都不能通过选择/导入复活；explicit restore 后新 session 才恢复可学习状态。
6. Manager 开启 privacy 后，addon 下一次学习机会保持 selection/frequency 零增量；关闭后一次公开合成选择只产生一次预期增量。
7. 覆盖 Manager 与 Fcitx 并发、Manager 重启、Fcitx 重启和桌面会话重启，不出现非预期 `SQLITE_BUSY`、migration 漂移、丢失或重复学习。
8. password、Qt `Sensitive`、terminal、unknown 和 GTK4 `PRIVATE` 既有失败关闭结论保持；诊断与截图不包含原始输入、程序身份或真实路径。

需要启动 VM、安装依赖、修改输入框架、启用输入法、运行 GUI 或清理 guest 资产时继续逐项获得授权。AI 只读监视并等待用户完成来源切换和实体输入，不合成操作冒充桌面验收。

## 首个实现批次（已完成）

文档门禁通过后的第一个纵向批次只做 Linux host 与共享数据启动链，但必须完整闭合：

1. 生成真实 Flutter Linux runner，并把较长平台逻辑留在共享 Linux platform 模块。
2. runner 在 Flutter engine 前设置 `umask(0077)`，注册现有 runtime channel。
3. 复用 XDG resolver，返回固定 userdb/settings 和 bundle `.so`；Dart 路径模型改为平台中立并保留 macOS 回归。
4. Linux staged Release bundle 携带 workspace native-rime `.so`，完成 ABI/symbol/ELF 与无路径 override smoke。
5. 使用临时合成库形成“runtime 写入—Manager bridge 刷新—另一 runtime 观察”的产品双端自动证据。

该批没有顺带实现 package/安装，也没有以能打开空窗口结束。privacy watcher、classifier、Wayland/X11 身份、精确 Firefox 生产规则、同库学习/解释、privacy 零增量、删除防复活、explicit restore、导入导出、Fcitx/Manager 重启与桌面会话重启均已复用同一 host、XDG 和真实 bridge 闭合。M5-P04 验收矩阵据此退出；现有 staging、backup、userdb、导入导出文件和 transient service 终态继续保留。后续 P05A payload gate 与 P05B actual package relationship、恢复事务、production system port/CLI、共用 startup gate、L6 format v1、compile-isolated checkpoint/evidence controller 和真实 ARM64 release pair 已独立完成；真实 Linux process/dpkg、L6 与安装实机仍需对应授权和证据。

## 当前停止线

- 不把 staged bundle、用户级 addon 复制、环境覆盖或 autostart 写成 M5-P05 安装产品；系统级 Debian 实现必须遵守独立安装边界。
- 不启用真实用户同步，不新增恢复码、设备授权或网络输入热路径。
- 不在 Dart、Flutter UI 或 C++ addon 复制 Rust userdb、ranker、删除和 explain 语义。
- 不为开启学习把 unknown、GTK4 `PRIVATE` 或未验证程序身份默认归为 known。
- 不记录原始程序名、窗口标题、正文、P1 事件明细或真实用户词。
- 不修改 macOS build 38 identity、载体、冻结证据或默认数据保留语义。
