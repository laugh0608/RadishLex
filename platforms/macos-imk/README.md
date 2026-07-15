# macOS InputMethodKit 薄壳

本目录实现 M1 第一平台的 InputMethodKit 薄壳、R01B 本地个人化 runtime 接线、开发 bundle build 与不安装系统输入法的 contract smoke。它不包含同步、manager、发布签名、公证或复杂候选 UI，也不提供任何自动安装、注册或输入法服务重启动作。

## 结构

- `Sources/RadishLexBridge.*`：复制 Rust-owned key result、snapshot 与 candidate view，固定 owner-thread 和错误边界。
- `Sources/RadishLexInputController.*`：映射 `NSEvent`，更新 marked text，以唯一 display index 驱动候选视觉和 Rust selection。
- `Sources/RadishLexCandidatePanel.*`：进程级非激活 AppKit 候选面板，负责 owner 生命周期、焦点隔离、全局定位、多屏限制、鼠标/辅助功能 index 回调和原生视觉；不承载 engine 或候选排序。
- `Sources/RadishLexRuntime.*`：创建独立 Rime session；进程退出时先释放全部 session，再调用 `radishlex_rime_runtime_shutdown`。
- `Resources/Rime/default.yaml.in`：产品自有 Rime 默认配置模板，固定 schema list 与 5 项候选页；不读取或继承用户 Rime 配置。
- `build-bundle.sh`：构建 contract 或显式 native-rime 开发 bundle，不安装 bundle。
- `Tests/contract_smoke.m`：使用合成 demo engine 复验 ABI v4、完整按键映射、display/engine index、个人化/学习状态、Unicode cursor、候选选择结果和生命周期，不读取 Rime 目录。
- `Tests/candidate_panel_contract.m`：创建真实 AppKit panel/control，复验视觉与 accessibility selection、appearance、anchor fallback、owner 接管和完整隐藏。
- `Tests/input_controller_contract.m`：使用正式 controller、panel 和 Rust demo session，贯通方向 keyDown/keyUp、Space、鼠标、accessibility press、Enter、Escape、宿主快捷键和双 client 生命周期。
- `Tools/tis_source_status.m`：使用公开 TIS API 按精确 Bundle ID 查询 parent/mode 状态，或以通知和 CFRunLoop 实时输出精确 current source；不启用、停用或选择输入源。
- `cleanup-user-install.sh`：在系统设置已人工移除且取得授权后，清理正式开发 bundle、`Rime` 运行目录和精确进程，并要求 TIS 零残留；默认保留 `userdb.sqlite3`。
- `ReferenceProbe/`：隔离验证原生候选事件路由与单 mode 输入源 metadata，不链接 Rime 或正式 FFI，也不替代产品薄壳。

## 候选窗定位与选择

Rust snapshot 的 cursor 是允许落在 composition 末尾的插入位置，但 `attributesForCharacterIndex:lineHeightRectangle:` 接收 inline session 内的现存字符索引。正式 panel 因此先读取 client marked range：存在 marked text 时使用 `min(cursor, length - 1)`，没有 inline session 时使用 `0`。若该行矩形不可用，才以 marked range 的文档绝对末尾位置调用 `firstRectForCharacterRange:actualRange:`；两种 range 不能混用。

最终 panel frame 使用目标 `NSScreen.visibleFrame` 在锚点下方或上方放置并限制在屏幕内；contract 环境没有有效 screen frame 时围绕有效行矩形构造计算区域，不回退到 `(0,0)`。controller 的唯一 display index 同时驱动视觉高亮、Space、数字键、鼠标和 accessibility selection；Rust runtime 固化并执行 display index 到 engine index 的映射。

native 产品 session 通过 ABI v4 的 personalized Rime 构造入口持有独立 userdb connection，数据库固定为 `~/Library/Application Support/RadishLex/userdb.sqlite3`。Objective-C 只创建并收紧 `RadishLex` 父目录到 `0700`，SQLite migration、WAL、busy timeout、文件权限、排序、selection 事务和失败回退都由 Rust 负责。每个事件前平台只传 secure input、隐私模式、上下文可信度与粗粒度类别；当前 Alpha 只把 TextEdit 和 Codex 识别为允许学习的普通上下文，其他未知应用默认 engine-only，敏感应用与 secure input 同样不读取、不写入个人化数据。

当前分类把 TextEdit 映射为 `editor`、Codex 映射为 `code`；Passwords、Keychain Access、1Password 8/7 的固定 Bundle ID 标记为敏感，其余应用映射为未知 `other`。隐私模式由输入法 `NSUserDefaults` 中的 `RadishLexPrivacyMode` 布尔值控制，缺省关闭；开启后仍可使用既有本地排序摘要，但不会记录当前 selection 或更新 user term/ranker weight。设置或前台/secure 状态变化时，controller 会先刷新 Rust learning context 和候选 snapshot，再接受 display index 选择。

`userdb.sqlite3` 不是临时 Rime 数据，卸载或普通开发清理不得删除。当前 `--status` 只报告 bundle、Rime 目录和进程，尚不能单独证明 R01B 测试数据库零残留；实机前必须补齐父目录和 userdb 只读状态。只有安装前已证明 userdb/Rime 不存在、对应内容全由本轮合成测试生成且另有明确授权时，才能精确删除本轮创建的数据；预存空父目录必须恢复为空并保留。

## 不安装验证

```bash
./scripts/check-macos-imk.sh
```

该入口会构建 `target/macos-imk/contract/RadishLexInputMethod.app`，执行 Objective-C wrapper、真实 AppKit candidate panel 和 controller-to-commit contract，并检查 bundle、动态库加载路径、完整 ad-hoc 开发签名和关键 FFI symbol。contract-only initializer/inspection API 不进入 native 产品。contract bundle 只用于编译与契约复验，不能安装或作为真实输入证据。

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
./scripts/cleanup-macos-imk.sh --authorized-after-settings-removal
```

`--status` 输出正式 bundle family 的 TIS source 属性与 `matches/enabled/selected` 汇总，并报告安装路径、隔离运行数据和精确进程。`--monitor` 先输出 `event=initial`，随后在公开 selected-source 通知到达时输出 `event=changed` 与最新 current source；一次切换可能出现多条相同 source 记录，`is_radishlex_pinyin=1` 只表示精确匹配正式 Pinyin mode。两者不记录输入正文，监视以 `Ctrl-C` 结束。

第三条命令只能在系统设置已移除 RadishLex、当前输入源已切回系统输入法且本次授权明确覆盖清理时执行。这些入口不选择、启用或停用 source，不修改 `com.apple.HIToolbox` 或 TIS 私有数据库；完整集中验收与回滚顺序见 `docs/runbooks/macos-inputmethodkit-development.md`。
