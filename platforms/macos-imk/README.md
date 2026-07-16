# macOS InputMethodKit 薄壳

本目录实现 M1 第一平台的 InputMethodKit 薄壳、R01B 本地个人化 runtime 接线、开发 bundle build 与不安装系统输入法的 contract smoke。它不包含同步、manager、发布签名、公证或复杂候选 UI，也不提供任何自动安装、注册或输入法服务重启动作。

## 结构

- `Sources/RadishLexBridge.*`：复制 Rust-owned key result、snapshot 与 candidate view，固定 owner-thread 和错误边界。
- `Sources/RadishLexInputController.*`：映射 `NSEvent`，更新 marked text，以唯一 display index 驱动候选视觉和 Rust selection。
- `Sources/RadishLexLearningContext.*`：按固定 Bundle ID 生成 sensitive/context-known/context-kind，不接触正文或 userdb。
- `Sources/RadishLexCandidatePanel.*`：进程级非激活 AppKit 候选面板，负责 owner 生命周期、焦点隔离、全局定位、多屏限制、鼠标/辅助功能 index 回调和原生视觉；不承载 engine 或候选排序。
- `Sources/RadishLexRuntime.*`：创建独立 Rime session；进程退出时先释放全部 session，再调用 `radishlex_rime_runtime_shutdown`。
- `Resources/Rime/default.yaml.in`：产品自有 Rime 默认配置模板，固定 schema list 与 5 项候选页；不读取或继承用户 Rime 配置。
- `build-bundle.sh`：构建 contract 或显式 native-rime 开发 bundle，不安装 bundle。
- `Tests/contract_smoke.m`：使用合成 demo engine 复验 ABI v4、完整按键映射、display/engine index、个人化/学习状态、Unicode cursor、候选选择结果和生命周期，不读取 Rime 目录。
- `Tests/candidate_panel_contract.m`：创建真实 AppKit panel/control，复验视觉与 accessibility selection、appearance、anchor fallback、owner 接管和完整隐藏。
- `Tests/input_controller_contract.m`：使用正式 controller、panel 和 Rust demo session，贯通方向 keyDown/keyUp、Space、鼠标、accessibility press、Enter、Escape、宿主快捷键和双 client 生命周期。
- `Tests/cleanup_user_install_contract.sh`：在隔离仓库、`HOME` 和工作目录中，以参数级命令 stub、双槽假 TIS 与假进程状态动态复验路径状态、清理前后 TIS 迁移和数据保留边界，不查询真实 TIS、不终止真实进程。
- `Tests/privacy_mode_contract.sh` 与 `Tests/r01b_test_userdb_cleanup_*_contract.sh`：复验 privacy absent/false 精确恢复和本轮 userdb receipt、固定文件、关闭证明、失败恢复边界。
- `Tools/tis_source_status.m`：使用公开 TIS API 按精确 Bundle ID 查询 parent/mode 状态，或以通知和 CFRunLoop 实时输出精确 current source；不启用、停用或选择输入源。
- `ValidationHost/`：构建固定 unknown/P0 两个普通/secure 文本宿主；默认门禁只构建和检查，不启动 GUI。
- `cleanup-user-install.sh`：在系统设置已人工移除且取得授权后，清理正式开发 bundle、`Rime` 运行目录和精确进程，并要求 TIS 零残留；默认保留 `userdb.sqlite3`。
- `privacy-mode.sh` 与 `cleanup-r01b-test-userdb.sh`：只服务 R01B 授权验收，前者精确保存/恢复产品布尔键，后者按 receipt 删除本轮固定 userdb family，不提供通用数据删除。
- `ReferenceProbe/`：隔离验证原生候选事件路由与单 mode 输入源 metadata，不链接 Rime 或正式 FFI，也不替代产品薄壳。

## 候选窗定位与选择

Rust snapshot 的 cursor 是允许落在 composition 末尾的插入位置，但 `attributesForCharacterIndex:lineHeightRectangle:` 接收 inline session 内的现存字符索引。正式 panel 因此先读取 client marked range：存在 marked text 时使用 `min(cursor, length - 1)`，没有 inline session 时使用 `0`。若该行矩形不可用，才以 marked range 的文档绝对末尾位置调用 `firstRectForCharacterRange:actualRange:`；两种 range 不能混用。

最终 panel frame 使用目标 `NSScreen.visibleFrame` 在锚点下方或上方放置并限制在屏幕内；contract 环境没有有效 screen frame 时围绕有效行矩形构造计算区域，不回退到 `(0,0)`。controller 的唯一 display index 同时驱动视觉高亮、Space、数字键、鼠标和 accessibility selection；Rust runtime 固化并执行 display index 到 engine index 的映射。

native 产品 session 通过 ABI v4 的 personalized Rime 构造入口持有独立 userdb connection，数据库固定为 `~/Library/Application Support/RadishLex/userdb.sqlite3`。Objective-C 只创建并收紧 `RadishLex` 父目录到 `0700`，SQLite migration、WAL、busy timeout、文件权限、排序、selection 事务和失败回退都由 Rust 负责。每个事件前平台只传 secure input、隐私模式、上下文可信度与粗粒度类别；当前 Alpha 只把 TextEdit 和 Codex 识别为允许学习的普通上下文，其他未知应用默认 engine-only，敏感应用与 secure input 同样不读取、不写入个人化数据。

当前分类把 TextEdit 映射为 `editor`、Codex 映射为 `code`；Passwords、Keychain Access、1Password 8/7 与固定 P0 验证宿主标记为敏感，其余应用（包括固定 unknown 验证宿主）映射为未知 `other`。隐私模式由输入法 `NSUserDefaults` 中的 `RadishLexPrivacyMode` 布尔值控制，缺省关闭；开启后仍可使用既有本地排序摘要，但不会记录当前 selection 或更新 user term/ranker weight。设置或前台/secure 状态变化时，controller 会先刷新 Rust learning context 和候选 snapshot，再接受 display index 选择。

`userdb.sqlite3` 不是临时 Rime 数据，卸载或普通开发清理不得删除。`--status` 分别报告 bundle、固定删除路径祖先安全性、RadishLex 父目录的存在性/类型/权限、Rime 目录、userdb、已知 SQLite sidecar 和进程；悬空 symlink 也视为存在或不安全，但这些 metadata 不自动证明数据归属。R01B 在复制前必须捕获 privacy 与空父目录 receipts；授权 A 完成普通清理和隐私恢复后，授权 B 才能通过专用 helper 精确删除本轮四个 SQLite 文件并恢复预存父目录 `0755`。当前父目录不得删除，完整 case、增减量和停止线见 `docs/runbooks/macos-r01b-personalization-acceptance.md`。

## 不安装验证

```bash
./scripts/check-macos-imk.sh
```

该入口会构建 `target/macos-imk/contract/RadishLexInputMethod.app` 和 unknown/P0 双验证 host，执行 Objective-C wrapper、真实 AppKit candidate panel、controller-to-commit、分类、privacy 与安全清理 contract，并检查 bundle、动态库加载路径、完整 ad-hoc 开发签名和关键 FFI symbol。它不启动 host；contract-only initializer/inspection API 不进入 native 产品。contract bundle 只用于编译与契约复验，不能安装或作为真实输入证据。

候选事件与单 mode metadata 的隔离 probe 使用独立入口：

```bash
./scripts/check-macos-imk-reference-probe.sh
```

该入口只构建并静态验证 `target/macos-imk/reference-probe-mode/RadishLexIMKModeReferenceProbe.app`，不会安装、注册、选择或启动输入法；完整停止线与清理顺序见 `ReferenceProbe/README.md`。

## native-rime 开发 bundle

native build 不查找用户已有输入法目录，也不下载 schema。调用方必须显式提供 `librime` include/lib、一份隔离的 shared data 及其许可证文件；shared data 必须包含 `<schema-id>.schema.yaml` 及其声明的依赖。产品不会采用输入目录中的 `default.yaml`，而是从仓库模板生成 schema list 与 `menu.page_size: 5`，避免 engine 页大小与 5×1 平台展示契约分叉：

```bash
RIME_INCLUDE_DIR=<include> \
RIME_LIB_DIR=<lib> \
RADISHLEX_RIME_SHARED_DATA=<isolated-shared-data> \
RADISHLEX_RIME_SCHEMA=<schema-id> \
RADISHLEX_RIME_DATA_LICENSE=<license-file> \
./scripts/check-macos-imk-native.sh
```

`RADISHLEX_RIME_DEPLOY_ON_START` 可显式设为 `0` 或 `1`，默认 `1`。构建产物位于 `target/macos-imk/native/RadishLexInputMethod.app`；bundle 同时保存 copied schema data、产品生成的 `default.yaml`、数据许可证和哈希清单，并拒绝 shared data symlink。native 门禁逐字节复核产品配置模板，并以临时隔离 user data 运行真实 FFI smoke，要求 snapshot 候选页恰好为 5 项。native build 从显式 `RIME_LIB_DIR` 解析依赖，但运行产物会递归复制全部非系统 dylib 到 `Contents/Frameworks`、重写为 bundle 内 `@rpath`，并保存逐库许可证和签名后哈希清单；门禁拒绝残留外部绝对依赖。脚本对每个 dylib、主程序和完整 bundle 依次签名与严格复验，可通过 `RADISHLEX_CODESIGN_IDENTITY` 显式提供 Apple Development identity。

bundle metadata 固定正式 Bundle ID `org.radishlex.inputmethod.macos` 与单一 `org.radishlex.inputmethod.macos.Pinyin` 模式，包含简体中文 script/repertoire、图标、本地化标签和 `LSUIElement`。正式 bundle 文件名固定为 `RadishLexInputMethod.app`；开发期不再复用已被 macOS 26 TIS 负缓存的旧 ID 或 `RadishLex.app` 路径。contract/native 门禁会验证 mode id 的 reverse-DNS 字符范围，避免把允许下划线的 `pinyin_simp` schema id 直接用作 TIS mode id。构建脚本不启动或安装 bundle；普通用户分发、Developer ID、公证和发布级供应链门禁仍属于 M4。

安装、启用、真实应用输入和移除会修改本机状态，必须另行取得授权后按独立 runbook 执行。

## 只读状态、实时来源与清理

正式开发输入源的入口为：

```bash
./scripts/cleanup-macos-imk.sh --status
./scripts/cleanup-macos-imk.sh --monitor
./scripts/stop-macos-imk-process.sh --authorized-stop-process
./scripts/cleanup-macos-imk.sh --authorized-after-settings-removal
```

`--status` 输出正式 bundle family 的 TIS source 属性与 `matches/enabled/selected` 汇总，并报告 `installed_bundle`、`cleanup_path_ancestors=safe|unsafe`、`application_support_parent`、父目录 kind/mode、`runtime_data`、`userdb`、`userdb_sidecars` 和 `process=stopped|running_verified|running_unverified|unavailable`。sidecar 聚合覆盖 WAL、SHM 与 rollback journal；输出只观察固定路径 metadata，不打开数据库、不记录目录条目名，也不推断测试所有权。授权清理在 RadishLex 父目录或从 `HOME` 到两个删除目标之间的祖先为 symlink/非目录、父目录不可读、同名进程不匹配固定 bundle executable，或进程状态不可观测时直接拒绝；只有 `running_verified` 才会先终止并确认停止，再次复核祖先安全后删除 bundle/Rime。`--monitor` 先输出 `event=initial`，随后在公开 selected-source 通知到达时输出 `event=changed` 与最新 current source；一次切换可能出现多条相同 source 记录，`is_radishlex_pinyin=1` 只表示精确匹配正式 Pinyin mode。两者不记录输入正文，监视以 `Ctrl-C` 结束。

进程停止入口只处理命令行精确匹配固定安装 executable 的进程，不查询 TIS 或删除路径；普通清理命令只能在系统设置已移除 RadishLex、当前输入源已切回系统输入法且本次授权明确覆盖时执行。这些入口不选择、启用或停用 source，不修改 `com.apple.HIToolbox` 或 TIS 私有数据库；通用构建/回滚见 `docs/runbooks/macos-inputmethodkit-development.md`，R01B 动作见专用验收 runbook。
