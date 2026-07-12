# macOS InputMethodKit 开发构建与人工 smoke

本文档供执行 RadishLex M1 macOS 开发 bundle 构建、授权后安装、真实应用输入和回滚的维护者使用。它不包含发布签名、公证、普通用户分发、Rime schema 下载、真实用户词库迁移或 M2 学习验证；稳定边界见 `docs/macos-inputmethodkit-boundary.md`。

## 无系统改动的默认验证

```bash
./scripts/check-macos-imk.sh
```

该命令只在 `target/` 下构建 contract bundle 和 smoke executable，不写用户级或系统级 `Input Methods` 目录，不注册输入源，不启动或重启输入法服务。contract bundle 使用合成 demo engine，只证明 wrapper、bundle 和 FFI 调用链，不证明真实 Rime 或 InputMethodKit 可用。

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

取得授权后，执行者应先记录准备安装的生成 bundle 路径、安装域和回滚目标。启用动作优先通过系统设置人工完成，不把注册、服务重启或系统数据库修改写进自动门禁。

## 安装准备与会话刷新

1. 先确认 Apple Development identity 与完整证书链有效，并用该 identity 重建 native bundle；不导出、记录或提交私钥。
2. 对生成 bundle 执行 `codesign --verify --deep --strict`，确认 build number、mode id、图标、本地化资源和 native dependency manifest 都属于同一次构建。
3. 安装时必须先移除旧目标再复制完整 bundle，不能用 `ditto` 或 Finder 叠加覆盖旧签名资源。
4. 正式开发身份固定为 Bundle ID `org.radishlex.inputmethod.macos`、mode ID `org.radishlex.inputmethod.macos.Pinyin` 和 bundle 文件名 `RadishLexInputMethod.app`；同一身份只保留一个待扫描安装副本。
5. macOS 26.5.1 已确认 TIS 会对失败的 Bundle ID/安装路径保留负缓存：旧 `org.radishlex.inputmethod` 与 `RadishLex.app` 在签名和 metadata 修正后仍不重新枚举，而相同产品二进制使用全新 ID 与路径可立即出现。开发与回滚不得继续复用旧身份或旧路径，也不得修改 TIS 私有数据库清缓存。
6. 构建目录、废纸篓、用户级与系统级副本的 LaunchServices 重复记录会干扰诊断，应在安装前注销或移出扫描路径。即时注册成功不能替代 TIS source 枚举证据。

登录后先用 TIS 查询或系统设置确认目标 source 确实存在，再申请启用和真实应用 smoke 授权。不要直接修改 `com.apple.HIToolbox` defaults，不把自注册逻辑放进输入法进程。

## 人工 smoke

至少在两个不同的 macOS 应用输入框中使用合成输入复验：

1. composition、首候选和非首候选提交；
2. 数字选择、空格、翻页、方向键、Backspace、Escape 和 Enter；
3. 中英文混输与未消费普通键/快捷键回到宿主；
4. input client 切换、停用再启用和进程重启；
5. 断网前后输入行为一致；
6. 退出时没有活动 session 阻止 runtime shutdown。

R01A 退出验收固定使用短时用户级安装：产物为 `target/macos-imk/native/RadishLexInputMethod.app`，以当前用户有效的 Apple Development identity 重建后复制到 `~/Library/Input Methods/RadishLexInputMethod.app`。该动作会让系统重新枚举输入源、在系统设置中新增 RadishLex 项、启动输入法进程，并在断网/恢复与进程重启子项中短时影响当前网络和输入状态；不写系统级 `/Library/Input Methods`，不读取用户 Rime 目录，不保留长期启用状态。

退出矩阵按 TextEdit 与 Codex 交叉执行，记录只保留通过/失败和错误类别：

| 场景 | 预期 |
| --- | --- |
| 全拼 composition、首候选、非首候选 | marked text、候选映射和最终提交正确 |
| 数字选择、方向键、翻页 | 高亮、页切换和 engine selection index 一致 |
| Backspace、Escape、Enter | 编辑、取消和提交语义明确，状态及时清空 |
| Command 等系统快捷键 | 不被输入法错误消费，仍由宿主应用处理 |
| 中英文混输、普通未消费按键 | 已消费中文输入与宿主普通字符边界正确 |
| composition 中切换 input client | 旧 composition 不提交或串入另一个应用 |
| 停用再启用、进程重启 | 新 session 状态干净，输入能力恢复 |
| 断网前后 | 输入能力、候选与提交行为一致 |
| 5×1 横排候选 | 由执行者人工全屏观察确认，不以 app-scoped 抓图缺失判失败 |

只记录通过/失败、错误类别和脱敏环境信息，不记录输入正文或截图中的敏感内容。

InputMethodKit 候选条属于输入法进程的独立浮层。只截取宿主应用窗口的自动化工具可能看不到候选条，即使 marked text 和候选实际可见；候选布局应使用只含合成词的人工全屏观察确认，不能仅凭 app-scoped 截图判定“未显示”。

## 回滚

R01A 每轮真实 smoke 无论通过还是失败都必须完整回滚，不保留开发输入源。顺序固定为：

1. 先在系统设置“键盘 -> 文字输入 -> 编辑”中选中 RadishLex 并点击“移除”；不能用公开 TIS API 停用代替该动作。
2. 确认当前输入源已切回系统输入法，再只处理 RadishLex 残余 TIS source；不得修改 `com.apple.HIToolbox` 或 TIS 私有数据库。
3. 移除本轮安装的 `~/Library/Input Methods/RadishLexInputMethod.app`；若输入法进程仍持有旧 bundle，在授权范围内终止该进程。
4. 删除 bundle 后重新打开系统设置输入源列表；macOS 26 已多次观察到用户配置项短暂回流。若 RadishLex 再次出现，必须再次点击“移除”并回到仅含原系统输入源的摘要。
5. 复核 TIS 中 RadishLex `matches=0`、用户级 bundle 不存在、RadishLex 进程已停止、系统设置列表无 RadishLex 输入源残留。
6. 只清理本轮生成的隔离 user data 与短期 staging；不删除 shared data 来源、用户其他输入法目录或任何非本轮数据。

仅调用 `TISDisableInputSource` 或移走 bundle 不会自动删除“所有输入法”中的用户配置项。最终证据必须同时满足设置列表、TIS、安装域和进程四项无残留。
