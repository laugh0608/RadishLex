# macOS InputMethodKit reference probe

本目录用于隔离验证 `IMKCandidates` 事件路由和单 mode 输入源 metadata，读者是 R01A macOS 实机验收的维护者。它不包含 Rime、RadishLex FFI、用户词库、学习、同步、产品输入源实现或发布资产，也不能作为正式输入法安装包。

## 当前结论

2026-07-12 在 macOS 26.5.2 上完成一次 root-only Apple Development 用户级安装：TIS 只枚举一个 `TISTypeKeyboardInputMethodWithoutModes` source，并报告 `select_capable=1` 与 `zh-Hans`；但系统设置简体中文可添加列表不显示它，probe 无法按合规路径启用。该 root-only metadata 假设已失败，候选事件模型尚未获得实机验证；清理后设置列表、TIS、bundle、独立数据和精确进程均无残留。

当前实现是使用全新 ID 与路径的单 mode probe。2026-07-13 跨登录后它进入系统设置简体中文可添加列表，并完成一次授权实机判定：marked composition、固定候选和首项视觉高亮正常；按右方向键后高亮未移动，Space 仍提交 index 0。该结果证伪当前“controller 返回 `NO` 后由候选窗移动选择并回调”的事件假设，但尚不能区分事件未被候选窗消费、候选窗未改变选择或 callback 未触发。实机已停止并完成系统设置、TIS、bundle、独立数据和进程零残留清理；当前源码只保留为失败复现基线，不得直接重装或据此修改正式实现。

## 固定边界与已证伪假设

- Bundle ID 为 `org.radishlex.inputmethod.macos.reference-probe-mode`，mode source ID 为 `org.radishlex.inputmethod.macos.reference-probe-mode.Pinyin`，bundle 文件名为 `RadishLexIMKModeReferenceProbe.app`。
- plist 声明一个可见 mode，并在 root 与 mode 两层固定 ID、图标和本地化名称；预期只有 mode 可选择，root 只用于验证 parent 呈现。
- 候选固定为五个合成字符串，不读取用户目录或外部数据。
- 进程只创建一个 `IMKCandidates`，panel type 固定为 `kIMKSingleRowSteppingCandidatePanel`。
- 本轮使用 `IMKCandidatesSendServerKeyEventFirst=YES`；Space 和 Enter 由 controller 处理，四方向键返回 `NO` 交给候选面板。Apple SDK header 说明未处理事件应继续发送候选窗，但 macOS 26.5.2 实机未产生选择迁移，因此该事件假设已经证伪。
- 候选通过 `candidates:` 与 `updateCandidates` 提供；本轮尝试只通过 `candidateSelectionChanged:` 和 `candidateSelected:` 同步选择，未获得方向键 callback 成功证据。
- 不调用 `setCandidateData`、`clearSelection` 或 `selectCandidateWithIdentifier:`。

## 无安装验证

```bash
./scripts/check-macos-imk-reference-probe.sh
```

该入口只在 `target/macos-imk/reference-probe-mode/` 构建并 ad-hoc 签名 probe，执行纯状态 contract 和 metadata 检查，不复制 bundle、不注册或选择输入源，也不启动输入法进程。Apple Development 签名必须在获得当次明确授权后，通过 `RADISHLEX_REFERENCE_PROBE_CODESIGN_IDENTITY` 显式提供。

## 实机证据与清理停止线

真实 probe 必须人工确认四方向视觉高亮、callback 索引和最终提交一致，Space 提交当前合成候选，Enter 提交原始 composition；TIS 只允许一个可选择 mode，输入菜单不得出现空白 parent、重复标题或图标错位。失败时停止，不生成下一个 probe 版本。

每次实机结束必须先在系统设置点击“移除”，然后才能在已授权范围执行：

```bash
./scripts/cleanup-macos-imk-reference-probe.sh --authorized-after-settings-removal
```

清理入口只处理精确 probe bundle、独立运行数据和精确进程名，并使用公开 TIS API 验证状态；它不会修改 `com.apple.HIToolbox`、TIS 私有数据库、正式 RadishLex bundle 或正式用户数据。可发现的 mode 在未加入现有输入法列表时仍可能报告 `enabled=1`，因此删除前门禁拒绝任何 `selected=1` source 或仍为 `enabled=1` 的不可选择 parent；删除后仍要求 TIS 达到 `matches=0 enabled=0 selected=0`。
