# macOS InputMethodKit 平台边界

本文档定义 RadishLex 第一真实平台的稳定实现边界，读者是实现 `platforms/macos-imk/`、Rust input runtime、C ABI 和 macOS 平台验证的开发者。本文不包含系统输入法安装命令、签名与公证步骤、视觉稿、Apple 私钥 backend、远端同步或逐日开发状态；安装与移除另写 runbook，实时批次见 `docs/status/current.md`。

## 产品范围

macOS InputMethodKit 是 M1 离线输入 Alpha 的第一真实平台。M1 必须在真实应用输入框中闭合：

```text
system key event
  -> platform normalization
  -> Rust key result
  -> composition and candidates
  -> candidate selection or engine commit
  -> text commit
```

M1 不要求远端同步、设备授权、恢复码、完整 manager 产品包、最终签名安装包或第二平台。M2 再接真实学习与本地 manager，M4 再闭合普通用户分发。

## 职责分工

InputMethodKit 薄壳负责：

- 系统输入法生命周期和输入 client 切换；
- macOS key event 到 `RadishLexKeyEvent` 的规范化；
- 调用 Rust FFI 并处理 status、`consumed`、commit 和 snapshot；
- marked text、候选展示、候选选择和文本提交；
- 把平台可判断的 secure input、隐私模式和应用类别传给 Rust runtime；
- 将结构化错误送入脱敏诊断，不吞错、不伪装成功。

薄壳不负责：

- 拼音切分、候选生成或候选重排；
- userdb schema、学习事务、删除语义或同步合并；
- 加密、设备授权、恢复码或远端 transport；
- 把平台对象、窗口句柄或 InputMethodKit 生命周期注入 `ime-core`；
- 在按键热路径读取 manager 状态或发起网络请求。

## Runtime 与 session 生命周期

`librime` setup、initialize、notification 和 finalize 必须由 Rust 进程级 runtime 统一管理：

- 同一输入法进程只初始化一次；多个输入 session 复用同一 runtime。
- 单个 Rust input session 只拥有对应 engine session、composition 与候选状态。
- 每个活动输入 context 使用独立 session；不得让两个输入 client 共享可变 composition。
- 所有 Rime session 创建、按键、候选选择、reset、释放和 runtime shutdown 使用同一个串行 owner thread；不能让不同 client 各自在任意线程直接调用 librime。
- client 切换、输入法停用、异常取消和进程退出必须有明确 reset/drop 路径。
- 任意 session drop 只释放自己的 engine session，不触发 finalize；进程 teardown 时先释放全部 session，再调用 `radishlex_rime_runtime_shutdown`，由 runtime 复核零活动 session 后执行 finalize。

runtime 初始化失败必须返回结构化错误。平台不得静默切换 demo engine，也不得把未初始化状态显示为可用输入法。

## 按键处理契约

平台主入口使用 `docs/ffi-boundary.md` 定义的版本化 key result。处理顺序固定为：

1. 只把当前支持的 key、modifier 和 phase 规范化为稳定 ABI；未知组合明确交还宿主或返回可诊断错误。Control、Option 或 Command 修饰的字符不得退化成无修饰字母送入 Rime，必须保持未消费并交还宿主；Shift 仍可参与普通字母输入。
2. 在 session owner thread 调用 Rust 按键入口。
3. status 失败时不读取部分结果，并按错误类别执行安全 reset、保留原按键或提示诊断。
4. `consumed = 0` 时返回未处理，让宿主应用继续接收按键。
5. 有即时 commit 时提交对应文本，不从 composition 或下一次 snapshot 猜测。
6. 使用同一 key result 的 snapshot 更新 marked text 与候选。
7. 复制所有 borrowed view 后及时释放 key result；平台状态不得持有 Rust 裸指针。

`Backspace`、`Escape`、`Enter`、`Space`、方向键、翻页键、数字选择、modifier-only、key release 和未知键必须有明确行为表与黑盒测试。未消费按键不能被候选窗或错误处理意外吞掉。

## Composition、候选与提交

- composition 为空时清除 marked text；非空时更新 marked text 与 cursor。
- 候选展示使用 macOS 原生 AppKit 机制或 InputMethodKit 兼容机制，不自造跨平台统一浮窗协议。正式实现使用进程级、非激活的 AppKit candidate panel，并以五候选页形成 5×1 横排；它只属于 macOS 平台壳，不进入 Rust core，也不形成其他平台必须复用的窗口协议。
- 平台展示索引必须稳定映射到 RadishLex ranked candidate 与 engine selection index。
- 用户选择候选后通过 Rust selection API 驱动 engine，平台不得直接把展示文本当作 engine 选择结果。
- selection result 必须携带 consumed、optional commit 与选择后的 snapshot。分段拼音候选可能只确定当前音节并继续 composition；只有 commit 存在时平台才向宿主插入文本，否则更新 marked text 与候选。
- engine 即时 commit 与候选选择产生的 commit 使用同一文本提交边界，并清理相应 marked text。
- M1 全拼有候选时，Space 选择当前高亮候选，由 Rime adapter 调用稳定的 engine candidate selection 语义并通过同事件 key result 返回；平台壳不得把 Space 特判成直接提交展示文本，也不得依赖临时 schema 是否携带 `key_binder`。
- 同一输入法进程只创建一个 candidate panel。panel 以弱引用记录当前 owner controller；新 session 展示候选时接管 owner，旧 session 的 deactivate/close 只能隐藏自己仍拥有的 panel，不能破坏后来激活的 session。空 snapshot、取消、错误恢复、失焦和 owner close 必须隐藏 panel。
- 方向事件由当前 `IMKInputController` 接收并更新唯一的 display selection index；panel 的视觉高亮只从该 index 渲染，Space 也只以同一 index 调用 Rust selection API。鼠标点击和辅助功能 press 通过 panel delegate 回传 display index，再进入同一 Rust selection API。平台不得维护一份“视觉 index”和另一份“提交 index”，也不得把候选正文作为选择身份。
- key release、modifier-only 或其他未改变 schema、preedit、cursor 与完整候选内容的 snapshot 必须保留当前 display selection；只有候选 presentation 确实变化时才重置到首项。否则方向 keyDown 后紧随的 keyUp 会把视觉和 Space 选择错误地拉回 index 0。
- candidate panel 使用 `NSPanel` + 标准 AppKit view/control，必须是 `NSWindowStyleMaskNonactivatingPanel`，不能成为 key/main window，不能抢走宿主输入焦点。panel level 使用当前 `IMKTextInput.windowLevel + 1`；锚点优先使用 `attributesForCharacterIndex:lineHeightRectangle:` 的全局行矩形。该 API 的 index 相对 inline session 且必须指向现存字符：marked range 非空时使用 `min(cursor, length - 1)`，无 inline session 时固定使用 `0`，不能把允许等于 composition length 的插入 cursor 原样传入。屏幕原点 `(0,0)` 本身可能合法，不以坐标拒绝启发式判断返回值。`firstRectForCharacterRange:actualRange:` fallback 继续使用文档绝对插入位置，允许 range 落在 marked text 末尾；两套 range 语义不得混淆。最终 frame 按实际 `NSScreen.visibleFrame` 选择下方或上方、限制在目标屏幕内。Spaces、全屏辅助窗口、窗口循环和多屏行为必须使用公开 `NSWindowCollectionBehavior` 表达。
- panel 的候选项必须进入 AppKit accessibility hierarchy，暴露稳定 label、index 与 selected value；选择变化发送公开 accessibility notification。候选 control 显式实现公开 accessibility press，并复用与鼠标相同的 target-action、owner 和 index 检查，不能另建提交路径。VoiceOver、全键盘访问、宿主焦点、多屏、全屏 Space 和 client 切换仍属于经授权实机 smoke，不可由自动 contract 替代。
- 输入源只声明唯一可选择的全拼 mode 及其 `TISInputSourceID`。SDK `TextInputSources.h` 把 bundle input method 与 `ComponentInputModeDict` 中的 input mode 定义为两个层级，因此系统枚举不可选择 parent source 与可选择 mode 是平台模型，不是 `menu` 返回值生成的第二个产品 mode。
- `IMKInputController.menu` 只返回 input-method-specific commands。M1 当前没有这类命令，正式实现固定返回 `nil`；macOS 26 因此渲染的图标空白 command 行记录为系统呈现限制。不得用空 `NSMenu`、菜单标题、重复身份项、disabled placeholder 或 plist fallback 填充该区域。以后只有在真实设置/命令能力存在时才能加入可执行 `NSMenuItem`，且必须通过 `doCommandBySelector:commandDictionary:` 进入明确动作与生命周期。
- 分页、Escape、带 Command 等修饰键和普通未消费按键继续沿既有 key result 边界处理。
- 取消、失焦、client 切换和 schema 切换不能把旧 composition 提交到新 client。

候选窗口视觉、分页快捷键和无障碍细节可以迭代，但不能改变索引映射、所有权或提交语义。

## 候选事件与输入菜单证据矩阵

Apple 的公开 [InputMethodKit 概览](https://developer.apple.com/documentation/inputmethodkit) 明确 `IMKCandidates` 是可选能力；[IMKCandidates](https://developer.apple.com/documentation/inputmethodkit/imkcandidates?language=objc) 负责呈现候选并在用户活动时通知 controller；[candidateSelectionChanged:](https://developer.apple.com/documentation/inputmethodkit/imkinputcontroller/candidateselectionchanged(_:)) 只描述用户在候选窗中的移动通知。macOS 26.5 SDK `IMKCandidates.h` 进一步定义事件优先顺序、identifier 与选择 API，但没有承诺直接调用继承的 `keyDown:` 会驱动内部状态，也没有承诺 `selectCandidateWithIdentifier:` 成功后同步完成视觉重绘。

| 路径 | 事件接收方 | panel 状态 | callback | 视觉 | Space/提交 | 结论 |
| --- | --- | --- | --- | --- | --- | --- |
| 正式 build 28：controller 调用 `selectCandidateWithIdentifier:` | controller | 平台 display index 已移动，API 返回成功 | 不作为提交真相源 | 高亮停在 index 0 | engine/FFI 与目标 index 一致 | 语义正确但视觉失败，退出正式模型 |
| 单 mode probe：`SendServerKeyEventFirst=YES`，controller 返回未处理 | controller 后按 header 转交 `IMKCandidates` | 未观察到迁移 | 未观察到非首项 | 高亮停在 index 0 | 提交 index 0 | 已证伪，不再依赖隐式 fallback |
| 单 mode probe：controller 显式调用 panel `keyDown:` | controller；直接调用继承的 responder 方法 | `selectedCandidate` 前后均为 index 0 | callback index 0 | 高亮停在 index 0 | 提交 index 0 | 已证伪；且公开契约不承诺直接调用等价于系统路由 |
| `SendServerKeyEventFirst=NO` / 默认 candidate-first | `IMKCandidates` 优先 | 未经实机验证 | 未经实机验证 | 未经实机验证 | 未经实机验证 | 顺序虽有公开契约，但仍依赖同一不透明 panel；不生成第三个签名 probe，也不作为正式闭环 |
| macOS AppKit candidate panel | controller 更新唯一 display index | owner-scoped 状态由动态 contract 直接断言 | 鼠标/辅助功能 delegate 回传 index；键盘不依赖 IMK callback | 同一 index 驱动真实 `NSButton` 样式与 accessibility state | Right keyDown、keyUp/modifier 保持、Space 与点击已动态贯通 Rust selection | 正式方向；不安装 contract 已通过，仍需一次集中授权实机 smoke |

[menu](https://developer.apple.com/documentation/inputmethodkit/imkinputcontroller/menu()) 的公开职责是“输入法专用命令”，并允许每次绘制前按当前状态更新。它与系统从 bundle/mode metadata 自动形成的 source 身份区域不是同一层：

| 对象或返回值 | 公开职责 / 实机事实 | 正式处理 |
| --- | --- | --- |
| bundle parent source | SDK 定义的 input method 层；实机为不可选择 parent | 保留稳定 bundle 身份，不伪装成第二个 mode 或命令 |
| Pinyin mode | `ComponentInputModeDict` 定义的可选择 input mode | 唯一产品输入源，稳定 ID、名称和图标 |
| `menu = nil` | 表达没有 input-method-specific command；macOS 26 仍显示空白 command 行 | 保留；将空白行记录为系统限制 |
| 新建空 `NSMenu` | 没有用户能力；实机触发多余 menu/deactivate 生命周期 | 禁止 |
| 稳定标题 `NSMenu` | 标题与系统身份项重复；不是可执行命令 | 禁止 |

SDK `IMKInputSession.h` 明确给自建候选窗提供 `windowLevel`，并说明使用 client level 加一；同一协议还区分用于 inline 字符属性/全局行矩形的 character index，以及接收文档绝对 range 的 `firstRectForCharacterRange:actualRange:`。AppKit 的 [`NSWindowStyleMaskNonactivatingPanel`](https://developer.apple.com/documentation/appkit/nswindow/stylemask-swift.struct/nonactivatingpanel?language=objc)、[`NSWindowCollectionBehavior`](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct?language=objc) 与 [Accessibility for AppKit](https://developer.apple.com/documentation/appkit/accessibility-for-appkit) 构成自建 panel 的公开平台依据。这里的“自建”只表示不使用 `IMKCandidates` 的不透明选择状态，仍必须使用原生 AppKit 窗口、控件、外观和辅助功能接口。

不安装动态 contract 必须创建真实 `NSApplication`、`NSPanel`、`NSButton` 和正式 controller，不只验证 helper 或源码字符串。组件 contract 覆盖非激活窗口、level、Spaces behavior、五候选、视觉/accessibility selection、appearance 重解析、长候选压缩、character index 与 insertion range 分离、anchor fallback、owner 接管、旧 control 拒绝和完整隐藏；fake client 必须记录收到的 character index，并在越界时返回有限高度的 `(0,0)` 矩形，使末尾 cursor、中间 cursor、无 inline session 与最终有效行锚点成为可判伪动态证据。controller contract 覆盖 composition、方向 keyDown 后 keyUp/modifier 保持、候选变化重置、Space、鼠标、accessibility press、Enter、Escape、宿主快捷键与双 client 生命周期。contract-only initializer 和 inspection API 只在 `RADISHLEX_CONTRACT_SMOKE=1` 编译，native 产品门禁必须证明这些 selector 不存在。

正式 `build 30` 的一次实体键盘观察发生在系统开启“自动切换到文稿的输入法”的环境中；事后 TIS 显示 TextEdit 已切回系统拼音，所以候选高亮迁移与 Space 提交不能归属为 RadishLex 通过证据。该轮同时观察到候选窗固定在屏幕左下角，仓库审计确认末尾插入 cursor 被错误当成 inline character index。修正后的产品构建号为 `31`；2026-07-14 已使用 Apple Development identity 重建、通过严格签名和安装副本哈希复核，并在不注销的当前会话进入系统设置现有列表，真实 UI 尚待按人工分组流程判定。

本轮曾用公开 `TISSelectInputSource` 精确选择正式 mode，同会话返回 `property_selected=1` 且 current source ID 精确匹配，但菜单栏仍显示系统拼音，实体输入表现为 RadishLex；开发者手动切到 U.S. 再切回系统拼音后显示与行为恢复一致。这证明菜单栏/SystemUIServer 呈现可能滞后于 TIS 与实际事件归属。该诊断不计作候选功能通过，后续验收禁止自动选择 source：执行者只做部署、系统设置添加、只读来源监视和清理，开发者手动聚焦、切换与实体交互。

## 隐私与本地数据

- 输入热路径完全离线；Go server、manager 和网络不参与按键处理。
- secure text entry、P0 应用、用户隐私模式或无法安全判断的敏感场景不得产生学习事件。
- 日志、崩溃报告、截图和人工 smoke 记录不得包含真实输入历史、联系人、密码、证件或支付信息。
- M1 可以不写学习事件，但必须保留向 M2 runtime 传递 privacy context 的稳定位置。
- M2 输入法与 manager 若共享 userdb，必须固定 App Group 或等价目录、文件权限、WAL/busy 策略、migration 所有权和并发测试。
- 平台壳不得直接打开 SQLite；目录只作为受控配置传给 Rust runtime 或 manager bridge。

## Native 依赖与目录

M1 开发版必须明确并隔离：

- RadishLex native library；
- `librime` 动态库及其加载路径；
- 合法来源的 schema/shared data；
- 输入法专用 user data；
- 可选诊断目录。

开发 smoke 不得读取用户现有 Rime 配置或词库目录，也不得把本机绝对路径写入 committed 文档或 fixture。M1 已使用固定上游 commit、保留 Apache-2.0 许可证和来源记录的 `rime-pinyin-simp` 临时隔离数据复验 native bundle 与真实 FFI 输入链。native bundle 必须递归封装全部非系统 dylib、把加载路径改写到 bundle 内、保存逐库许可证与签名后哈希清单，并拒绝任何外部绝对依赖。该开发期封装不替代 M4 的 Developer ID、公证、升级/移除和发布级供应链门禁。

TIS input source/mode id 与 bundle 文件名属于平台稳定身份，不等同于允许下划线的 Rime schema id。当前正式 Bundle ID 为 `org.radishlex.inputmethod.macos`，单一全拼 mode 为 `org.radishlex.inputmethod.macos.Pinyin`，bundle 文件名为 `RadishLexInputMethod.app`，Rime schema 仍为 `pinyin_simp`；mode metadata 必须同时固定 `LSUIElement`、简体中文 language、script、repertoire、图标、本地化标签和可见顺序。输入法列表 TIFF 使用 16pt 逻辑尺寸，当前 Retina 资产固定为 `32×32 @144dpi` 并保留安全边距，避免系统设置按 64pt 放大后覆盖名称。macOS 26.5.1 已观察到失败身份与安装路径的 TIS 负缓存，开发过程不得复用旧 `org.radishlex.inputmethod` 或 `RadishLex.app`，也不得通过修改 TIS 私有数据库清缓存。系统设置列表更新与公开 TIS 枚举之间还可能短时竞态；清理必须先在系统设置真实移除，再等待或触发公开刷新并复核精确零残留，不能用 `TISDisableInputSource` 或私有配置代替系统设置动作。

## Header、线程与错误

- Swift / Objective-C 只依赖仓库生成或维护并受编译测试约束的 C header/module map。
- ABI version、结构大小、常量值、bool 表达、UTF-8 view、handle 释放和 panic boundary 必须由 host test 固定。
- 平台对象停留在 Swift / Objective-C 层；Rust 不保存 `IMKInputController`、client、event 或 UI object 指针。
- session owner thread 外的调用必须通过明确调度返回 owner thread，不能靠锁绕过未声明的线程约束。
- engine、FFI 或目录错误必须转成结构化、脱敏、可测试的错误；不能 fallback 到 demo engine、空候选或默认成功。

## 开发安装边界

新增、启用或移除系统输入法会修改本机状态，必须在独立 runbook 中说明影响、路径、回滚和 smoke 数据要求，并在执行前获得用户明确授权。自动测试默认只构建 bundle、检查结构和运行 host contract，不自动安装、启用或重启系统输入法服务。授权实机采用固定人机分工：执行者负责部署、系统设置添加、只读 TIS 监视和最终移除清理；开发者负责目标文稿聚焦、当前 source 手动切换及实体键盘、鼠标和辅助功能交互。自动化不得调用 `TISSelectInputSource` 或注入按键代替验收；菜单栏名称仅作辅助观察，来源归属以输入期间只读精确 source 记录为准。注销或重启登录会话必须由开发者另行安排，不能作为自动刷新步骤。

当前开发实现位于 `platforms/macos-imk/`。`./scripts/check-macos-imk.sh` 只构建 contract `.app` bundle、编译 production 条件分支并运行 wrapper、真实 AppKit panel 与 controller 集成 contract；`./scripts/check-macos-imk-native.sh` 必须由调用方显式提供 `RIME_INCLUDE_DIR`、`RIME_LIB_DIR`、隔离 shared data、schema id 和许可证文件，并检查 mode metadata、架构、递归 dependency closure、symbol、逐库许可证、完整 bundle 签名、数据哈希清单与 contract-only API 缺失。默认 ad-hoc 签名只服务无安装门禁；真实安装 smoke 还必须显式提供当前用户有效的 Apple Development identity。两条入口都不查找用户已有 Rime 目录，不执行安装、注册、bundle 启动或服务重启。开发版安装与移除步骤见 `docs/runbooks/macos-inputmethodkit-development.md`。

`platforms/macos-imk/ReferenceProbe/` 是 R01A 的隔离诊断资产，不是第二套产品输入法。它只使用合成候选和独立身份，证明静态事件路由、metadata 与清理停止线；probe 的 TIS 枚举、安装或失败不能单独修改正式输入源身份、Rust/Rime 边界或 M1 退出结论。

## M1 验收证据

自动证据至少覆盖：

- key result 的 `consumed`、即时 commit、snapshot、错误和释放契约；
- C header/module map 与 Swift/Objective-C 最小调用方编译；
- 进程级 runtime 的单次初始化、多 session、异常释放和最终 finalize；
- key normalization、候选索引映射、取消、reset、schema 切换和未消费按键；
- 真实 AppKit panel 的视觉/accessibility selection、appearance、owner 和 focus contract，以及 controller 到 Rust selection/commit 的键盘、鼠标和辅助功能动态链；
- bundle 结构、native library 加载和缺失依赖的明确失败。

经授权的人工 smoke 至少覆盖：

- 在两个真实 macOS 应用输入框中连续输入合成中文；
- composition、首候选、非首候选、翻页、取消、Backspace、Enter 和中英文混输；
- 未消费快捷键或普通按键仍由宿主接收；
- 断网后输入能力不变；
- 切换输入 client、停用再启用和进程重启后状态正确；
- smoke 只使用合成词，不记录真实敏感输入。

M1 只有在开发者可按 runbook 重复构建、安装、输入和移除后退出。CLI、fixture、host smoke 或 manager 页面不能替代真实 InputMethodKit 证据。

## 停止线

- key result、进程级 runtime 和 header 未闭合前，不开始堆叠复杂候选 UI。
- M1 基础输入可在 M2 学习语义完成前推进，但不得用假学习或平台侧词库绕过 Rust runtime。
- 不为 macOS 壳引入远端同步、平台私钥、设备授权或 manager 依赖。
- 第一平台达到可重复日常输入前，不启动第二平台实现。
- 未经授权不执行安装、启用、系统目录写入、服务重启、签名或发布操作。
