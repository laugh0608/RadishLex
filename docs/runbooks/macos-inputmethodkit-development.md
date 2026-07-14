# macOS InputMethodKit 开发构建与人工 smoke

本文档供执行 RadishLex M1 macOS 开发 bundle 构建、授权后安装、真实应用输入和回滚的维护者使用。它不包含发布签名、公证、普通用户分发、Rime schema 下载、真实用户词库迁移或 M2 学习验证；稳定边界见 `docs/macos-inputmethodkit-boundary.md`。

## 无系统改动的默认验证

```bash
./scripts/check-macos-imk.sh
```

该命令只在 `target/` 下构建 contract bundle、smoke executable 和只读 TIS 状态工具，不写用户级或系统级 `Input Methods` 目录，不注册输入源，不启动或重启输入法服务。contract bundle 使用合成 demo engine，只证明 wrapper、bundle 和 FFI 调用链，不证明真实 Rime 或 InputMethodKit 可用。

## native bundle 前提

1. 使用已有、显式指定的 `librime` include/lib；构建脚本不安装依赖。
2. 准备来源与许可证已确认的 shared data/schema，并复制到与任何真实用户输入法目录无关的隔离目录。
3. shared data 必须包含 `default.yaml` 和 `<schema-id>.schema.yaml`，许可证文件必须非空并显式传入。
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

`RADISHLEX_RIME_DEPLOY_ON_START` 默认 `1`，只接受 `0` 或 `1`。产物为 `target/macos-imk/native/RadishLexInputMethod.app`。检查入口会验证 plist、单一全拼 mode metadata、当前架构、关键 FFI symbol、完整 bundle 签名，以及 copied shared data/许可证清单。构建会递归收集 `librime` 的全部非系统 dylib，重写为 bundle 内 `@rpath`，复制逐库许可证并生成签名后哈希清单；任何残留外部绝对依赖都会使门禁失败。shared data 中的 symlink 同样会被拒绝。默认使用 ad-hoc 开发签名；真实安装 smoke 需要调用方通过 `RADISHLEX_CODESIGN_IDENTITY` 提供当前用户可用的 Apple Development identity。检查不会启动或安装 bundle。

## 授权停止线

以下步骤会修改本机输入法状态，未取得当次明确授权时必须停止：

- 写入用户级或系统级 `Input Methods` 目录；
- 使用系统设置启用输入源；
- 启动、终止或重启输入法相关进程；
- 注销、重新登录或重启 macOS 以刷新输入源；
- 在真实应用输入框执行 smoke；
- 删除已安装 bundle 或修改系统输入源配置。

集中验收可以使用一次明确授权覆盖同一冻结 build 的 Apple Development 签名、用户级安装、一次注销/登录、系统设置添加与选择、真实应用 smoke、精确进程重启、短时断网和完整清理。授权必须逐项列出这些影响；未被列入的动作仍须停止。只要 build、路径和动作范围没有变化，已覆盖的步骤不在登录前后重复请求授权。若代码、构建号、Bundle ID、mode ID、安装域或清理目标变化，本次授权立即失效。

取得授权后，执行者应先记录准备安装的生成 bundle 路径、安装域和回滚目标。启用与移除优先通过系统设置人工完成，不把注册、服务重启或系统数据库修改写进自动门禁。

## 集中验收冻结点

进入真实动作前必须冻结源码与产物，至少记录：

1. Git 工作区干净，当前提交、分支和相对 `origin/dev` 的领先状态明确；验收过程中不修改源码或重建另一个 build。
2. `CFBundleVersion`、Bundle ID、mode ID、schema id 与安装文件名符合当前文档；R01A 当前候选固定为 `build 31`。
3. Apple Development bundle 已通过 native 门禁、完整递归签名和 `codesign --verify --deep --strict`。
4. 记录生成 bundle 主程序、FFI dylib 和 native manifest 的 SHA-256；复制后逐字节复核生成产物与安装副本，确保系统测试的就是冻结产物。
5. 安装目标仅为 `~/Library/Input Methods/RadishLexInputMethod.app`，运行数据仅为本轮隔离的 `~/Library/Application Support/RadishLex/Rime`；不得读取或复用用户现有 Rime 数据。

冻结后发现源码或产物问题，应取消本次真实动作并回到仓库修复。不得在已登录、已添加或已选择输入法的现场边改边重建。`build 31` 的 Apple Development 签名、真实安装、注销/登录、系统设置添加与集中验收均尚未授权；仓库内 ad-hoc contract/native 通过不能扩张为这些动作的授权。

上一正式 `build 30` 虽完成 Apple Development 签名、用户级安装、注销/登录、系统设置添加和实体键盘观察，但系统当时开启了“自动切换到文稿的输入法”。实体输入后 TIS 显示 TextEdit 已切回系统拼音，所以候选高亮迁移与 Space 提交不具备 RadishLex 来源归属，不能写成正式通过；该轮观察到的候选窗固定屏幕左下角则形成定位缺陷输入。完整清理后 TIS 为 `matches=0 enabled=0 selected=0`，bundle、隔离运行数据和精确进程均不存在。

## 安装准备与会话刷新

1. 先确认 Apple Development identity 与完整证书链有效，并用该 identity 重建 native bundle；不导出、记录或提交私钥。
2. 对生成 bundle 执行 `codesign --verify --deep --strict`，确认 build number、mode id、图标、本地化资源和 native dependency manifest 都属于同一次构建。
3. 安装时必须先移除旧目标再复制完整 bundle，不能用 `ditto` 或 Finder 叠加覆盖旧签名资源。
4. 正式开发身份固定为 Bundle ID `org.radishlex.inputmethod.macos`、mode ID `org.radishlex.inputmethod.macos.Pinyin` 和 bundle 文件名 `RadishLexInputMethod.app`；同一身份只保留一个待扫描安装副本。
5. macOS 26.5.1 已确认 TIS 会对失败的 Bundle ID/安装路径保留负缓存：旧 `org.radishlex.inputmethod` 与 `RadishLex.app` 在签名和 metadata 修正后仍不重新枚举，而相同产品二进制使用全新 ID 与路径可立即出现。开发与回滚不得继续复用旧身份或旧路径，也不得修改 TIS 私有数据库清缓存。
6. 构建目录、废纸篓、用户级与系统级副本的 LaunchServices 重复记录会干扰诊断，应在安装前注销或移出扫描路径。即时注册成功不能替代 TIS source 枚举证据。
7. 对全新 Bundle ID/安装路径，用户级副本可能已被 TIS 解析却不进入当前登录会话的系统设置可添加目录。注销/重新登录只能是实现与验收矩阵冻结后、开发者主动安排的集中验收边界，不能作为逐 build 日常调试机制。若已取得注销授权，应保持签名、bundle 和路径不变，只执行一次；登录后先只读复核 TIS 与系统设置，不提前启用、选择、注册或重建。若当次不适合打断登录会话或登录后仍不出现，停止并按清理停止线回滚，不能连续更换 ID、路径或修改私有数据库。

登录后先用 TIS 查询或系统设置确认目标 source 确实存在。若集中授权已明确覆盖添加、选择和 smoke，可在只读复核通过后继续；否则停在动作前重新取得授权。不要直接修改 `com.apple.HIToolbox` defaults，不把自注册逻辑放进输入法进程。

候选事件或 metadata 的 reference probe 使用独立说明与精确清理入口，见 `platforms/macos-imk/ReferenceProbe/README.md`；probe 证据不能替代正式 native bundle smoke。

## 人工 smoke

集中 smoke 使用同一安装产物，按以下顺序推进。前一阶段的硬性条件失败后不继续扩大矩阵，不修改现场 build；只记录脱敏错误类别并进入完整回滚。

### 第一阶段：输入与候选一致性

至少在 TextEdit 中先判定：

1. 先聚焦目标 TextEdit 文稿与明确插入点，再选择 RadishLex；不能先选择输入法后再切入可能触发文稿级自动切换的窗口。
2. 选择后立即运行 `./scripts/cleanup-macos-imk.sh --status` 精确查询正式 Bundle ID，确认 Pinyin mode 为 `selected=1`；完成一组实体输入后立即再次查询。输入前后任一查询不是精确 RadishLex selected source，该组候选事件和提交一律标记为“来源不可归属”，不得继续写成通过证据。
3. 来源归属成立后，判定全拼 composition、首候选、非首候选和连续提交正确。
4. 左右及上下方向移动时，视觉高亮、controller display index 与 Space 最终提交始终指向同一候选；keyUp 与 modifier-only 不把选择拉回首项。
5. 候选 panel 必须跟随当前文字光标，不得固定在屏幕原点、左下角或旧 client；靠近边缘时的上下放置与裁剪留到第二阶段扩大验证。
6. 数字选择、翻页、Backspace、Escape、Enter、取消和重置语义明确。
7. 中英文混输边界正确，Command 快捷键与未消费普通按键继续由宿主接收。
8. 鼠标点击候选与 VoiceOver accessibility press 均通过当前候选选择语义提交，不产生第二套平台提交路径。

视觉索引、选择索引与最终提交任一不一致，或候选窗抢走宿主输入焦点，均直接判定失败。

### 第二阶段：AppKit 平台行为

第一阶段通过后继续判定：

1. 候选 panel 始终为 nonactivating，鼠标选择、VoiceOver 导航和候选更新后宿主输入框仍保持正确焦点。
2. 浅色与深色外观下选中态清晰；长候选允许压缩显示，同时保留完整 tooltip 和 accessibility label。
3. 输入框靠近屏幕上下左右边缘时，候选窗选择可见的上下位置并限制在当前屏幕 `visibleFrame` 内。
4. 主副显示器、全屏应用和不同 Space 中定位正确，不残留到错误屏幕或错误 client。
5. 输入菜单只接受系统自动生成的 parent/mode 层级和已记录的 macOS 26 空白 command 行限制；产品不得生成重复标题、空菜单、disabled placeholder 或额外 mode。

VoiceOver、实体鼠标、多显示器和全屏行为必须由人工观察；自动 contract 或只截宿主窗口的截图不能替代这些证据。

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
| 鼠标、VoiceOver、焦点 | 同一 selection/commit 路径，宿主焦点不被 panel 抢走 |
| 多屏、全屏、Space | panel 跟随当前 client 与目标屏幕，不跨屏残留 |
| 输入菜单 | 只有系统 parent/mode；无产品重复标题、placeholder 或生命周期回归 |
| 5×1 横排候选 | 由执行者人工全屏观察确认，不以 app-scoped 抓图缺失判失败 |

只记录通过/失败、错误类别和脱敏环境信息，不记录输入正文或截图中的敏感内容。

InputMethodKit 候选条属于输入法进程的独立浮层。只截取宿主应用窗口的自动化工具可能看不到候选条，即使 marked text 和候选实际可见；候选布局应使用只含合成词的人工全屏观察确认，不能仅凭 app-scoped 截图判定“未显示”。

## 回滚

R01A 每轮真实 smoke 无论通过还是失败都必须完整回滚，不保留开发输入源。只读状态入口为：

```bash
./scripts/cleanup-macos-imk.sh --status
```

它只使用公开 TIS API 查询正式 Bundle ID，并报告精确用户级 bundle、隔离运行数据和精确进程状态，不修改系统配置。回滚顺序固定为：

1. 先在系统设置“键盘 -> 文字输入 -> 编辑”中选中 RadishLex 并点击“移除”；不能用公开 TIS API 停用代替该动作。
2. 确认当前输入源已切回系统输入法，再只处理 RadishLex 残余 TIS source；不得修改 `com.apple.HIToolbox` 或 TIS 私有数据库。
3. 在本次集中授权明确覆盖清理、并已完成系统设置移除后执行：

   ```bash
   ./scripts/cleanup-macos-imk.sh --authorized-after-settings-removal
   ```

   该入口只删除精确的 `~/Library/Input Methods/RadishLexInputMethod.app`、本轮隔离的 `~/Library/Application Support/RadishLex/Rime`，并终止精确 `RadishLex` 进程。若仍有已选择 source 或 enabled 的不可选择 parent，入口会在删除前拒绝执行。
4. 删除 bundle 后重新打开系统设置输入源列表；macOS 26 已多次观察到用户配置项短暂回流，build 30 清理也确认系统设置列表更新与 TIS 缓存失效可能短时竞态。若 RadishLex 再次出现，必须再次点击“移除”并回到仅含原系统输入源的摘要。
5. 重新运行清理入口；只有 TIS 达到 `matches=0 enabled=0 selected=0`、用户级 bundle 与隔离运行数据不存在、精确进程停止且系统设置列表无 RadishLex 输入源残留，才算完成回滚。
6. 只清理本轮生成的隔离 user data 与短期 staging；不删除 shared data 来源、用户其他输入法目录或任何非本轮数据。

仅调用 `TISDisableInputSource` 或移走 bundle 不会自动删除“所有输入法”中的用户配置项，也不能用 `com.apple.HIToolbox`、TIS 私有数据库或其他私有配置代替系统设置移除。最终证据必须同时满足设置列表、TIS、安装域和进程四项无残留；短时缓存竞态只能通过公开刷新、等待与重复只读复核收敛。
