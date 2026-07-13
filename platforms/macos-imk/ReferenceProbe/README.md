# macOS InputMethodKit reference probe

本目录用于隔离验证 `IMKCandidates` 事件路由和单 mode 输入源 metadata，读者是 R01A macOS 实机验收的维护者。它不包含 Rime、RadishLex FFI、用户词库、学习、同步、产品输入源实现或发布资产，也不能作为正式输入法安装包。

## 当前结论

2026-07-12 在 macOS 26.5.2 上完成一次 root-only Apple Development 用户级安装：TIS 只枚举一个 `TISTypeKeyboardInputMethodWithoutModes` source，并报告 `select_capable=1` 与 `zh-Hans`；但系统设置简体中文可添加列表不显示它，probe 无法按合规路径启用。该 root-only metadata 假设已失败，候选事件模型尚未获得实机验证；清理后设置列表、TIS、bundle、独立数据和精确进程均无残留。

本目录使用全新 ID 与路径保存单 mode probe。2026-07-13 跨登录后它进入系统设置简体中文可添加列表，并完成两次授权实机判定。第一条隐式 fallback 路径中，marked composition、固定候选和首项视觉高亮正常，但右方向键后高亮未移动，Space 仍提交 index 0。失败实机完成零残留清理后，本批一度改为只在 composition 与候选窗同时活动时，由 controller 对四方向显式调用候选面板公开 `keyDown:`，并记录调用前后 panel identifier 与 callback index。第二次实体键盘判定确认右方向事件抵达显式转发路径，但调用前后 identifier 相同、callback index 保持 0、视觉高亮未移动，Space 仍提交 index 0“候选甲”。隐式 fallback 与显式 `keyDown:` 两条假设均已证伪；第二次测试也已完成设置项、TIS、bundle、独立数据和精确进程零残留清理。已证伪的显式路径与静态门禁不保留在当前源码，正式实现结论不变。

## 固定边界与已证伪假设

- Bundle ID 为 `org.radishlex.inputmethod.macos.reference-probe-mode`，mode source ID 为 `org.radishlex.inputmethod.macos.reference-probe-mode.Pinyin`，bundle 文件名为 `RadishLexIMKModeReferenceProbe.app`。
- plist 声明一个可见 mode，并在 root 与 mode 两层固定 ID、图标和本地化名称；预期只有 mode 可选择，root 只用于验证 parent 呈现。
- 候选固定为五个合成字符串，不读取用户目录或外部数据。
- 进程只创建一个 `IMKCandidates`，panel type 固定为 `kIMKSingleRowSteppingCandidatePanel`。
- `IMKCandidatesSendServerKeyEventFirst=YES`；Space 和 Enter 由 controller 处理。Apple SDK header 说明未处理事件应继续发送候选窗，但 macOS 26.5.2 实机没有产生选择迁移；显式调用候选面板 `keyDown:` 的短时实验同样失败，实验代码已回退，当前源码只保留最初的隐式 fallback 基线用于静态参考。
- 候选通过 `candidates:` 与 `updateCandidates` 提供；选择只由 `candidateSelectionChanged:` 和 `candidateSelected:` 更新。两条失败路径的 identifier、callback、视觉与提交证据只保留在 R01A 正式文档和周志中，不继续扩展 probe。
- 不调用 `setCandidateData`、`clearSelection` 或 `selectCandidateWithIdentifier:`。

## 无安装验证

```bash
./scripts/check-macos-imk-reference-probe.sh
```

该入口只在 `target/macos-imk/reference-probe-mode/` 构建并 ad-hoc 签名 probe，执行纯状态 contract 和 metadata 检查，不复制 bundle、不注册或选择输入源，也不启动输入法进程。Apple Development 签名必须在获得当次明确授权后，通过 `RADISHLEX_REFERENCE_PROBE_CODESIGN_IDENTITY` 显式提供。

## 实机证据与清理停止线

该 probe 已完成诊断并停止生成新签名 build。四方向视觉高亮、callback 索引和最终提交一致，Space 提交当前合成候选，Enter 提交原始 composition，以及唯一可选择 mode/菜单呈现等条目只保留为历史判定口径；当前正式实现改由 `docs/macos-inputmethodkit-boundary.md` 定义的 AppKit candidate panel 推进。

每次实机结束必须先在系统设置点击“移除”，然后才能在已授权范围执行：

```bash
./scripts/cleanup-macos-imk-reference-probe.sh --authorized-after-settings-removal
```

清理入口只处理精确 probe bundle、独立运行数据和精确进程名，并使用公开 TIS API 验证状态；它不会修改 `com.apple.HIToolbox`、TIS 私有数据库、正式 RadishLex bundle 或正式用户数据。可发现的 mode 在未加入现有输入法列表时仍可能报告 `enabled=1`，因此删除前门禁拒绝任何 `selected=1` source 或仍为 `enabled=1` 的不可选择 parent；删除后仍要求 TIS 达到 `matches=0 enabled=0 selected=0`。
