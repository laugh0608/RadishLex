# macOS InputMethodKit 开发构建与人工 smoke

本文档供执行 RadishLex M1 macOS 开发 bundle 构建、授权后安装、真实应用输入和通用回滚的维护者使用。R01B 的固定 case、状态增减量和双授权清理见 [本地个人化验收](macos-r01b-personalization-acceptance.md)。本文不包含发布签名、公证、普通用户分发、Rime schema 下载、真实用户词库迁移或远端同步；稳定边界见 `docs/macos-inputmethodkit-boundary.md`。

## 无系统改动的默认验证

```bash
./scripts/check-macos-imk.sh
```

该命令只在 `target/` 下构建 contract bundle、smoke executable、只读 TIS 状态工具、隐私/清理合同和 unknown/P0 双验证 host，不写用户级或系统级 `Input Methods` 目录，不注册输入源，也不启动两个 host 或输入法服务。contract bundle 使用合成 demo engine，只证明 wrapper、bundle 和 FFI 调用链，不证明真实 Rime 或 InputMethodKit 可用。

## 只读 TIS 诊断

正式开发输入源使用同一包装入口编译并运行仓库内 TIS 工具：

```bash
./scripts/cleanup-macos-imk.sh --status
./scripts/cleanup-macos-imk.sh --monitor
```

`--status` 输出匹配 source 的属性与 `matches/enabled/selected` 汇总，并分别报告用户级 bundle、`cleanup_path_ancestors=safe|unsafe`、RadishLex 父目录的存在性/类型/权限、`RadishLex/Rime`、`userdb.sqlite3`、已知 SQLite sidecar 和 `process=stopped|running_verified|running_unverified|unavailable`；它不打开数据库，也不因路径存在而推断输入源已添加或数据属于本轮。`userdb_sidecars` 聚合 WAL、SHM 与 rollback journal，悬空 symlink 同样按存在或不安全处理；同名进程的命令行必须匹配固定 bundle executable 才是 `running_verified`，其余非 stopped 状态均禁止清理。`--monitor` 输出 `event=initial|changed source_id=... bundle_id=... is_radishlex_pinyin=0|1`，每次通知后立即刷新 stdout，使用 `Ctrl-C` 结束。同一次切换可能出现重复记录，必须按事件顺序与精确 source ID 判定来源。

两条命令只写仓库 `target/macos-imk/tools/` 下的编译产物，不选择或修改输入源，也不读取输入正文。若沙盒上下文出现 HiServices XPC 连接错误或切换后没有通知，按后文实机分工停止并切换到获准的真实用户上下文，不能换成变异 API。

## native bundle 前提

1. 使用已有、显式指定的 `librime` include/lib；构建脚本不安装依赖。
2. 准备来源与许可证已确认的 shared data/schema，并复制到与任何真实用户输入法目录无关的隔离目录。
3. shared data 必须包含 `<schema-id>.schema.yaml` 及其声明的依赖，许可证文件必须非空并显式传入；bundle 的 `default.yaml` 由仓库产品模板生成，固定 schema list 与 `menu.page_size: 5`，不采用输入目录或用户目录的默认配置。
4. schema id 只允许 ASCII 字母、数字、点、下划线和连字符。
5. smoke 只使用合成词，不记录窗口正文、输入历史、联系人或其他敏感信息。

```bash
RIME_INCLUDE_DIR=<include> \
RIME_LIB_DIR=<lib> \
RADISHLEX_RIME_SHARED_DATA=<isolated-shared-data> \
RADISHLEX_RIME_SCHEMA=<schema-id> \
RADISHLEX_RIME_DATA_LICENSE=<license-file> \
./scripts/check-macos-imk-native.sh
```

`RADISHLEX_RIME_DEPLOY_ON_START` 默认 `1`，只接受 `0` 或 `1`。产物为 `target/macos-imk/native/RadishLexInputMethod.app`。检查入口会验证 plist、单一全拼 mode metadata、产品生成的 5 项候选页配置、当前架构、关键 FFI symbol、完整 bundle 签名，以及 copied shared data/许可证清单；随后以临时隔离 user data 运行真实 FFI smoke，要求 snapshot 恰好返回 5 项候选。构建会递归收集 `librime` 的全部非系统 dylib，重写为 bundle 内 `@rpath`，复制逐库许可证并生成签名后哈希清单；任何残留外部绝对依赖都会使门禁失败。shared data 中的 symlink 同样会被拒绝。默认使用 ad-hoc 开发签名；真实安装 smoke 需要调用方通过 `RADISHLEX_CODESIGN_IDENTITY` 提供当前用户可用的 Apple Development identity。检查不会启动或安装 bundle。

## 授权停止线

以下步骤会使用开发者签名身份或修改本机输入法、产品设置与数据状态，未取得当次明确授权时必须停止：

- 使用 Apple Development identity 签名真实安装产物；
- 写入用户级或系统级 `Input Methods` 目录；
- 使用系统设置启用输入源；
- 启动、终止或重启输入法相关进程；
- 注销、重新登录或重启 macOS 以刷新输入源；
- 在真实应用输入框执行 smoke；
- 临时修改并恢复 `RadishLexPrivacyMode` 等产品设置；
- 让 runtime 收紧预存父目录 mode，或在验收后恢复安装前 mode；
- 使用 CLI 删除或显式恢复合成词状态；
- 删除已安装 bundle、已证明属于本轮的 userdb family / Rime，或修改系统输入源配置。

R01B 固定使用授权 A（平台、合成数据操作与普通清理）和授权 B（本轮 userdb family 精确删除），不能用一次笼统授权合并。动作清单、先后条件、receipt 与失败处置统一见 [R01B 验收 runbook](macos-r01b-personalization-acceptance.md)；当前父目录永不删除。未列入的动作仍须停止，代码、构建号、身份、安装域或清理目标变化时，已有授权立即失效。

取得相应授权后，执行者应先记录准备安装的生成 bundle 路径、安装域和回滚目标。启用与移除优先通过系统设置人工完成，不把注册、服务重启或系统数据库修改写进自动门禁。

### 实机人机分工

- 执行者负责冻结产物、签名、安装、打开系统设置并添加 source、启动只读 TIS 监视，以及测试完成后的系统设置移除、bundle/运行数据/进程清理和零残留复核。
- 开发者负责聚焦目标文稿、通过菜单栏或实体快捷键手动切换当前输入源，并完成实体键盘、鼠标、应用切换与视觉观察；另行安排辅助功能专项时再负责 VoiceOver 操作。输入法进程由 macOS 随用户选择启动，不直接运行 bundle executable 代替该步骤。
- 执行者每次只交付一组短步骤，等待开发者报告后才继续。验收自动化不得调用 `TISSelectInputSource`、注入合成按键或自动操作候选项来替代人工行为；只读 TIS 监视固定使用 `./scripts/cleanup-macos-imk.sh --monitor`，它先输出 `event=initial`，再订阅 `kTISNotifySelectedKeyboardInputSourceChanged` 并通过 CFRunLoop 输出 `event=changed`。每行的 `source_id` 是精确 current source，只有正式 mode 精确匹配时 `is_radishlex_pinyin=1`；同一次切换可能收到多条相同 source 通知，来源判定使用事件顺序和精确 ID，不按通知条数计数。开始产品组前必须先以系统 source 手动切换自检，不能用不处理通知的轮询进程冒充实时记录。
- 若沙盒上下文出现 HiServices XPC 连接错误或手动切换后没有通知，停止该监视进程；只能在获得必要授权后于真实用户上下文运行同一已提交工具并重新执行系统 source 自检，不能改代码、重建产物或改用变异 API 掩盖环境隔离。
- 菜单栏名称和图标只作辅助观察。若它与精确 TIS 或实际输入行为冲突，当前组停止并标记为显示缓存竞态；开发者先手动切到 U.S. 等中立输入源，再切到目标 source 后从头重做该组。
- “自动切换到文稿的输入法”是开发者为避免文稿级 source 占用而主动关闭的测试前提；本轮保持关闭，执行者不得自动开启、关闭或恢复该设置。
- 注销、重新登录或重启不会由刷新失败自动触发。若 source 未进入当前会话目录，执行者必须停下，由开发者保存工作并另行安排登录边界。

## 集中验收冻结点

进入真实动作前必须冻结源码与产物，至少记录：

1. Git 工作区干净，当前提交、分支和相对 `origin/dev` 的领先状态明确；验收过程中不修改源码或重建另一个 build。
2. `CFBundleVersion`、Bundle ID、mode ID、schema id 与安装文件名符合当前文档。R01A 最终证据固定为 `build 32`；R01B 的 build 33 冻结因新增生产上下文分类与受控 host 而失效，当前候选必须使用 `build 34` 重新冻结。历史 build 不得与当前实机证据混用。
3. 申请第一阶段授权前，以默认 ad-hoc 签名完成 native 门禁、完整递归签名复验和 `codesign --verify --deep --strict`，记录生成 bundle 的 `Info.plist`、主程序、FFI dylib、Rime data manifest 与 native libraries manifest 的 SHA-256，冻结源码、构建输入与候选；`Info.plist` 哈希用于把证据绑定到具体 `CFBundleVersion`，此时不得提供 Apple Development identity。
4. 取得第一阶段授权后，只用同一源码和构建输入完成 Apple Development 重建/签名与全部门禁，重新记录上述五项 SHA-256；此时仍不复制安装副本。
5. 复制前最后一个检查点必须重新执行一次完整 `--status`。R01B 还必须由专用入口捕获 privacy 与空父目录 receipts，固定要求与命令见 [R01B 验收 runbook](macos-r01b-personalization-acceptance.md)。检查与复制之间不得插入系统状态变更，任一字段漂移即取消本轮，不覆盖、迁移、复用或删除现场。
6. 安装目标仅为 `~/Library/Input Methods/RadishLexInputMethod.app`，Rime 运行数据仅为 `~/Library/Application Support/RadishLex/Rime`，R01B userdb 固定为 `RadishLex/userdb.sqlite3`；不得读取或复用用户现有 Rime 或 userdb 数据。当前预存空父目录不属于本轮，runtime 收紧权限的变化必须在授权 A 中列明，并在授权 B 恢复为 `0755`，除非另获精确授权保留 `0700`。

冻结后发现源码或产物问题，应取消本次真实动作并回到仓库修复。不得在已登录、已添加或已选择输入法的现场边改边重建。2026-07-15 的 `build 31` 已在精确 source 归属下通过四向屏幕边缘与浅色/深色外观，但长输入实机显示 9 项候选，与 5×1 契约不一致；本轮立即停止后续矩阵并完成系统设置、TIS、bundle、运行数据和进程零残留清理。修复后的 `build 32` 重新取得授权并冻结哈希后，已通过真实 5×1、长候选、全屏/菜单、双 client、进程重启和离线矩阵，R01A 完成；当前单屏环境未覆盖副屏，VoiceOver 仍是非 Alpha 声明范围的已知限制。

上一正式 `build 30` 虽完成 Apple Development 签名、用户级安装、注销/登录、系统设置添加和实体键盘观察，但系统当时开启了“自动切换到文稿的输入法”。实体输入后 TIS 显示 TextEdit 已切回系统拼音，所以候选高亮迁移与 Space 提交不具备 RadishLex 来源归属，不能写成正式通过；该轮观察到的候选窗固定屏幕左下角则形成定位缺陷输入。完整清理后 TIS 为 `matches=0 enabled=0 selected=0`，bundle、隔离运行数据和精确进程均不存在。

## 安装准备与会话刷新

1. 只有第一阶段授权已生效，才确认 Apple Development identity 与完整证书链有效，并用同一冻结输入重建 native bundle；不导出、记录或提交私钥。
2. 对生成 bundle 执行 `codesign --verify --deep --strict`，确认 build number、mode id、图标、本地化资源和 native dependency manifest 都属于同一次构建。
3. 当前批次的目标必须保持 absent；若原子基线后、复制前出现目标，立即取消本轮，不删除或覆盖。基线通过后才复制完整 bundle，并立即逐字节复核安装副本与冻结生成产物；仅未来已证明旧目标属于同一获准批次的重装，才可先完整移除再复制，不能用 `ditto` 或 Finder 叠加覆盖旧签名资源。
4. 正式开发身份固定为 Bundle ID `org.radishlex.inputmethod.macos`、mode ID `org.radishlex.inputmethod.macos.Pinyin` 和 bundle 文件名 `RadishLexInputMethod.app`；同一身份只保留一个待扫描安装副本。
5. macOS 26.5.1 已确认 TIS 会对失败的 Bundle ID/安装路径保留负缓存：旧 `org.radishlex.inputmethod` 与 `RadishLex.app` 在签名和 metadata 修正后仍不重新枚举，而相同产品二进制使用全新 ID 与路径可立即出现。开发与回滚不得继续复用旧身份或旧路径，也不得修改 TIS 私有数据库清缓存。
6. 构建目录、废纸篓、用户级与系统级副本的 LaunchServices 重复记录会干扰诊断，应在安装前注销或移出扫描路径。即时注册成功不能替代 TIS source 枚举证据。
7. 对全新 Bundle ID/安装路径，用户级副本可能已被 TIS 解析却不进入当前登录会话的系统设置可添加目录。注销/重新登录只能是实现与验收矩阵冻结后、开发者主动安排的集中验收边界，不能作为逐 build 日常调试机制。若已取得注销授权，应保持签名、bundle 和路径不变，只执行一次；登录后先只读复核 TIS 与系统设置，不提前启用、选择、注册或重建。若当次不适合打断登录会话或登录后仍不出现，停止并按清理停止线回滚，不能连续更换 ID、路径或修改私有数据库。

登录后先用 TIS 查询或系统设置确认目标 source 确实存在。若第一阶段授权已明确覆盖添加、用户手动选择和 smoke，可在只读复核通过后继续；否则停在动作前重新取得授权。不要直接修改 `com.apple.HIToolbox` defaults，不把自注册逻辑放进输入法进程。

候选事件或 metadata 的 reference probe 使用独立说明与精确清理入口，见 `platforms/macos-imk/ReferenceProbe/README.md`；probe 证据不能替代正式 native bundle smoke。

## 人工 smoke

集中 smoke 使用同一安装产物，按以下顺序推进。前一阶段的硬性条件失败后不继续扩大矩阵，不修改现场 build；只记录脱敏错误类别并进入完整回滚。

### 第一阶段：输入与候选一致性

至少在 TextEdit 中先判定：

1. 执行者先启动只读精确 TIS source 监视，以 U.S. 与系统拼音的手动切换确认通知链能捕获变化，再明确本组唯一按键序列；开发者先聚焦目标 TextEdit 文稿与插入点，再手动选择 RadishLex，不能反序操作。
2. 只读记录必须确认正式 Pinyin mode 在本组输入期间持续为 current/selected source；开发者完成本组前不切换应用。返回 Codex 报告导致的文稿级自动切换只标记本组结束，不反推输入期间来源。
3. 若菜单栏显示系统拼音但精确 source 或实际行为为 RadishLex，当前组不继续；开发者经中立输入源手动重选后从头执行。菜单栏显示不能单独证明或否定来源。
4. 来源归属成立后，判定全拼 composition、首候选、非首候选和连续提交正确。
5. 左右及上下方向移动时，视觉高亮、controller display index 与 Space 最终提交始终指向同一候选；keyUp 与 modifier-only 不把选择拉回首项。
6. 候选 panel 必须跟随当前文字光标，不得固定在屏幕原点、左下角或旧 client；靠近边缘时的上下放置与裁剪留到第二阶段扩大验证。
7. 数字选择、翻页、Backspace、Escape、Enter、取消和重置语义明确；中英文混输与宿主快捷键边界正确。
8. 鼠标点击候选通过当前候选选择语义提交，不产生第二套平台提交路径，且点击后宿主输入焦点保持正确。

视觉索引、选择索引与最终提交任一不一致，或候选窗抢走宿主输入焦点，均直接判定失败。

### 第二阶段：AppKit 平台行为

第一阶段通过后继续判定：

1. 候选 panel 始终为 nonactivating，鼠标选择和候选更新后宿主输入框仍保持正确焦点。
2. 浅色与深色外观下选中态清晰；长候选允许压缩显示，同时保留完整 tooltip 和 accessibility label。
3. 输入框靠近屏幕上下左右边缘时，候选窗选择可见的上下位置并限制在当前屏幕 `visibleFrame` 内。
4. 主副显示器、全屏应用和不同 Space 中定位正确，不残留到错误屏幕或错误 client。
5. 输入菜单只接受系统自动生成的 parent/mode 层级和已记录的 macOS 26 空白 command 行限制；产品不得生成重复标题、空菜单、disabled placeholder 或额外 mode。

实体鼠标、多显示器和全屏行为必须由人工观察；自动 contract 或只截宿主窗口的截图不能替代这些证据。

VoiceOver 当前已知限制是：旁白可导航到第二候选，但视觉 selected state 未同步，accessibility press 也未提交该项。它不属于 R01A/M1 Alpha 退出矩阵，不在剩余集中窗口重复消耗现场；在产品明确支持或宣传 VoiceOver 前，必须另行执行辅助功能专项，届时焦点所在项、视觉高亮、accessibility selected state、press index 与最终 commit 必须全部一致。

### 第三阶段：生命周期与离线能力

前两阶段通过后，在 TextEdit 与 Codex 间交叉复验：

1. composition 中切换 input client 时，旧 composition 不提交、不串入新应用，进程级 panel owner 正确接管。
2. 旧 controller deactivate/close 不隐藏或清空后来激活的 session；停用再启用后新 session 状态干净。
3. 只终止精确 `RadishLex` 进程并由系统重新拉起后，输入能力恢复且没有旧候选窗或旧 composition。
4. 在开发者可接受的短时断网窗口内，composition、候选、选择和提交与联网时一致；恢复网络不改变输入状态。
5. 退出输入法时没有活动 session 阻止 runtime shutdown，两个 session 不重复初始化或破坏 librime 全局状态。

R01A 退出验收固定使用短时用户级安装：产物为 `target/macos-imk/native/RadishLexInputMethod.app`，以当前用户有效的 Apple Development identity 重建后复制到 `~/Library/Input Methods/RadishLexInputMethod.app`。复制后必须由 TIS 与系统设置证明确已枚举和加入，不能把目录存在视为成功；启用后会启动输入法进程，并在断网/恢复与进程重启子项中短时影响当前网络和输入状态。不写系统级 `/Library/Input Methods`，不读取用户 Rime 目录，不保留长期启用状态。

退出矩阵按 TextEdit 与 Codex 交叉执行，记录只保留通过/失败和错误类别：

| 场景 | 预期 |
| --- | --- |
| 全拼 composition、首候选、非首候选 | marked text、候选映射和最终提交正确 |
| 数字选择、四方向、翻页 | 高亮、页切换、display index 和 engine selection index 一致 |
| Backspace、Escape、Enter | 编辑、取消和提交语义明确，状态及时清空 |
| Command 等系统快捷键 | 不被输入法错误消费，仍由宿主应用处理 |
| 中英文混输、普通未消费按键 | 已消费中文输入与宿主普通字符边界正确 |
| composition 中切换 input client | 旧 composition 不提交或串入另一个应用 |
| 停用再启用、进程重启 | 新 session 状态干净，输入能力恢复 |
| 断网前后 | 输入能力、候选与提交行为一致 |
| 鼠标、焦点 | 同一 selection/commit 路径，宿主焦点不被 panel 抢走 |
| 多屏、全屏、Space | panel 跟随当前 client 与目标屏幕，不跨屏残留 |
| 输入菜单 | 只有系统 parent/mode；无产品重复标题、placeholder 或生命周期回归 |
| 5×1 横排候选 | 由执行者人工全屏观察确认，不以 app-scoped 抓图缺失判失败 |

只记录通过/失败、错误类别和脱敏环境信息，不记录输入正文或截图中的敏感内容。

InputMethodKit 候选条属于输入法进程的独立浮层。只截取宿主应用窗口的自动化工具可能看不到候选条，即使 marked text 和候选实际可见；候选布局应使用只含合成词的人工全屏观察确认，不能仅凭 app-scoped 截图判定“未显示”。

## 已关闭的 R01B 本地个人化回归验收

R01B 已于 2026-07-17 关闭，不重做已退出的完整 R01A 平台矩阵。固定 `r01b-shi-time-v1` case、build 34 身份、非选择 snapshot、TextEdit/重启/delete/restore 增减量、privacy/unknown/P0/secure 证据口径和授权 B 精确清理全部维护在 [R01B 验收 runbook](macos-r01b-personalization-acceptance.md)。该文档继续作为关闭证据和后续回归动作真相源；本节不复制会漂移的 case 细节。

## 回滚

R01A/R01B 每轮真实 smoke 无论通过还是失败都必须完整回滚，不保留开发输入源。只读状态入口为：

```bash
./scripts/cleanup-macos-imk.sh --status
```

实时来源监视入口为：

```bash
./scripts/cleanup-macos-imk.sh --monitor
```

两者都只使用公开 TIS API，不选择、启用或停用 source，不修改系统配置，也不记录输入正文；监视器在测试组和清理开始前用 `Ctrl-C` 终止。状态入口报告正式 Bundle ID、固定路径祖先安全性、用户级 bundle、父目录 kind/mode、Rime、userdb/sidecar 和四态进程。R01B 隐私值的 absent/false 精确恢复和授权 B 条件见专用 runbook；恢复失败不阻止继续移除输入源和 bundle，但授权 A 不得标记完成、不得申请授权 B。回滚顺序固定为：

1. 开发者先让所有参加测试且可能记住文稿级 source 的目标文稿手动切回系统输入法；执行者再在系统设置“键盘 -> 文字输入 -> 编辑”中选中 RadishLex 并点击“移除”。不能用公开 TIS API 停用代替该动作；若自动化无法精确识别设置行，应停下请求人工协助，不能猜测点击。
2. 只读确认当前 source 已不是 RadishLex，再处理其残余 TIS source；不得修改 `com.apple.HIToolbox` 或 TIS 私有数据库。
3. 在对应授权明确覆盖普通清理、并已完成系统设置移除后执行：

   ```bash
   ./scripts/cleanup-macos-imk.sh --authorized-after-settings-removal
   ```

   该入口只终止命令行匹配固定 bundle executable 的 `RadishLex` 进程并确认停止，再删除精确的 `~/Library/Input Methods/RadishLexInputMethod.app` 与本轮隔离的 `~/Library/Application Support/RadishLex/Rime`；它有意保留父目录、`userdb.sqlite3` 及其 sidecar。隔离 contract 会动态证明顺序和负向边界；RadishLex 父目录或任一删除目标祖先为 symlink/非目录、父目录不可读、进程状态不可观测，或终止后仍未停止时，入口会在路径删除前拒绝，并在真正删除前再次复核祖先。若仍有已选择 source 或 enabled 的不可选择 parent，入口也会在终止和删除前拒绝执行。
4. 完整重启 System Settings；macOS 26 已多次观察到配置项短暂回流。若 RadishLex 再次出现，必须再次真实移除并重新启动设置，直到摘要和现有列表只含原系统 source；门禁在不可选择 parent 仍为 `enabled=1` 时必须拒绝删除。关闭窗口或快捷键动作不等于设置扩展已经退出：需要通过应用菜单“退出系统设置”，并在重新打开前确认 `System Settings` 与 `KeyboardSettings.appex` 均已结束，否则“添加”目录可能继续持有旧缓存。
5. 删除 bundle 后若 TIS 暂留 `matches>0 enabled=0 selected=0`，打开现有列表和“添加 -> 简体中文”目录触发公开扫描，确认两处均无 RadishLex 后关闭窗口并重新运行清理入口。
6. 只有 TIS 达到 `matches=0 enabled=0 selected=0`、用户级 bundle 与隔离运行数据不存在、精确进程停止、隐私键已恢复，且设置摘要、现有列表与可添加目录均无 RadishLex，才算平台输入源回滚完成。
7. 步骤 1–6、隐私恢复、数据库关闭与 receipt 复核完成后，才申请授权 B，并只运行专用 `cleanup-macos-imk-r01b-test-userdb.sh --authorized-delete-r01b-test-userdb`。入口和拒绝边界见 R01B 专用 runbook；无法证明归属时始终保留 userdb 原位，且不得把本次回归验收标记为完成。
8. 只清理本轮生成的隔离 user data 与短期 staging；不删除 shared data 来源、用户其他输入法目录或任何非本轮数据。

仅调用 `TISDisableInputSource` 或移走 bundle 不会自动删除“所有输入法”中的用户配置项，也不能用 `com.apple.HIToolbox`、TIS 私有数据库或其他私有配置代替系统设置移除。平台输入源回滚证据必须同时满足设置列表、TIS、安装域和进程四项无残留；R01B 完整回滚还必须证明隐私键、测试数据路径与父目录 mode 回到安装前基线。短时缓存竞态只能通过公开刷新、等待与重复只读复核收敛。
