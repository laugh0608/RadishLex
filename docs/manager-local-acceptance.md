# Flutter manager 本地验收口径

本文档定义 Phase 4 Flutter manager 当前本地管理能力的验收口径。读者是维护 `apps/radishlex-manager`、审阅阶段进度和决定是否进入下一批管理端工作的协作者。本文不定义真实远端同步协议、恢复码 UI、设备授权成功路径、Go server 新 API、C ABI 扩展或平台输入法壳。

## 当前结论

Flutter manager 当前进入本地能力验收收口阶段。已落地能力覆盖本地词库管理、导入导出、import batches、学习摘要、rank explain 摘要、sync preflight、设备 backend gate、settings draft、sync gate 草案、连接健康、`manager_sync_readiness.v1` 内存态导入、readiness 聚合、开发期 evidence bundle 同源回归、只读交互进入计划、`manager_sync_action_command_preview.v1` 非执行命令预演、request / result preview、脱敏诊断报告和结构化错误分类。

本阶段验收只证明管理端可以安全、可审计地管理本机数据和解释同步不可用原因；不证明用户可用远端同步已经开放。

当前 action preview 相关验收只证明 settings / sync / diagnostics 可以使用同一份非敏感模型解释 future bridge 命令形状和停止线；不证明已经存在真实 `ManagerBridge` method、C ABI、request payload、result material、恢复码生成 / 输入、join request 创建、授权成功或设备撤销执行。

2026-07-05 复查结论：按下方退出标准逐项核对后，Phase 4 Flutter manager 本地验收当前没有影响退出标准的证据缺口。后续缺口集中在真实同步入口所需的平台私钥 backend、目标部署运行证据、恢复码 / 设备授权产品实现和对应测试，不阻塞本地管理能力验收。

## 验收范围

纳入验收：

- 本地 userdb 词条查看、搜索、审计详情和空态。
- 用户词条删除确认，并通过 tombstone 防止旧设备或旧备份复活。
- 词库导入检查、用户确认导入、导入历史、导入批次联动审计和结果文案。
- 用户词库导出结果文案，诊断导出不得混入词库内容。
- 学习状态聚合摘要，P1 原始事件不展示。
- `rank explain` 非敏感摘要、筛选和候选贡献项详情。
- `sync preflight` 本地 P2 对象分类、local-only 事件计数、设备 backend capability 和 production gate 说明。
- settings draft v1 保存、读取、输入校验、部署证据来源 allowlist 和 sync gate 派生。
- `manager_sync_readiness.v1` 非敏感摘要导入、清除、错误降级和当前 manager 内存态派生。
- `manager_sync_evidence_bundle.v1` 开发期 fixture 对 settings / sync / diagnostics 同源派生的回归覆盖。
- `manager_sync_action_command_preview.v1` 非执行 command preview、request / result status、allowed fields、forbidden material policy 和 action 级错误分类。
- 脱敏诊断报告预览、section 筛选、关键字筛选、复制和导出一致性。
- `ManagerBridge` 结构化失败分类和用户可见文案。
- 真实 Dart FFI bridge 在临时 SQLite userdb、临时 settings JSON 和合成 TSV 上的本地管理 smoke。

不纳入验收：

- 真实远端同步上传 / 下载主操作。
- 恢复码生成、确认保存、轮换、撤销或恢复加入 UI。
- 设备加入请求审批、设备授权成功路径和设备撤销 UI。
- 平台私钥 backend 生产可用性判定。
- 长期运行 Go server、真实部署目标、真实 token、真实证书或真实用户数据。
- 平台输入法壳、输入热路径和候选窗行为。

## 退出标准映射

| Phase 4 退出标准 | 当前验收口径 | 证据入口 |
| --- | --- | --- |
| 用户能通过 UI 管理已学习词 | 词库页支持本地词条查看、搜索、审计详情、删除确认、导入检查、导入、导出和导入历史审计；真实 Dart FFI bridge 可对临时 userdb 执行 list / delete / inspect / import / export。 | `test/screens/dictionary_test.dart`、`test/screens/manager_home_actions_test.dart`、`test/ffi_manager_bridge_test.dart`、`./scripts/check-manager-ffi-smoke.sh` |
| 用户能配置自部署后端 | 设置页保存非 secret `settings draft`，校验 `server_endpoint`、`retain_sync_config`、`privacy_mode`、`diagnostics_export` 和部署证据来源标签；配置只派生本地 sync gate，不连接真实远端。 | `test/screens/settings_test.dart`、`test/ffi_manager_bridge_test.dart`、`docs/manager-settings-diagnostics.md` |
| 用户能看到同步预检状态和生产不可用原因 | 同步页展示 sync gate、P2 对象分类、local-only 事件、设备 backend capability、production gate、连接健康、readiness 聚合、只读 interaction plan、action command preview、request / result preview 和停止线；诊断报告也输出 gate source / stop line 的脱敏摘要。 | `test/screens/sync_test.dart`、`test/screens/settings_diagnostics_test.dart`、`test/models/manager_sync_action_preview_test.dart`、`./scripts/check-manager.sh` |
| 用户可用同步开关在条件齐备前保持关闭 | `启用同步` 按钮保持禁用；`preflight_ready`、readiness 全 ready 和 future ready shape 都只表示本地草案或 future contract 形状可解释，不代表远端同步开放。 | `test/screens/sync_test.dart`、`test/screens/settings_test.dart`、`docs/manager-ui-boundary.md`、`docs/manager-sync-action-acceptance-matrix.md` |

## 复查记录

| 复查项 | 结论 | 说明 |
| --- | --- | --- |
| 本地词库管理 | 通过 | `dictionary_test.dart` 覆盖查看、搜索、空态、词条审计、删除确认、导入检查、确认导入、导入历史和导出；`manager_home_actions_test.dart` 固定结果文案与失败分类；FFI smoke 覆盖临时 userdb 的 inspect / import / delete / export。 |
| 自部署后端配置草案 | 通过 | `settings_test.dart` 覆盖 server endpoint 草案、隐私模式、部署证据来源和 sync gate 派生；`ffi_manager_bridge_test.dart` 覆盖 settings JSON v1 持久化、旧字段降级、非法版本 / URL / evidence source 拒绝。 |
| 同步预检与生产不可用原因 | 通过 | `sync_test.dart` 覆盖 `backend_unavailable`、`local_only`、P2 对象分类、local-only 计数、backend capability、production gate、连接健康、readiness 代表场景和 action preview 可见层；`settings_diagnostics_test.dart` 覆盖 gate source / stop line、readiness、interaction plan、action command preview、request / result preview 的脱敏诊断展示。 |
| 同步开关关闭 | 通过 | `sync_test.dart` 和 `settings_test.dart` 均断言 `sync-enable-button` 处于禁用态，包括 `preflight_ready` 草案状态。 |
| 隐私与脱敏 | 通过 | `settings_diagnostics_test.dart` 和 `ffi_manager_bridge_test.dart` 断言诊断文本不包含用户词和本地路径；FFI smoke 断言诊断报告不包含临时 db、settings、导入路径或用户词。 |
| 真实 Dart FFI bridge | 通过 | `check-manager-ffi-smoke.sh` 构建真实 `radishlex-ime-ffi` 动态库，并用临时 SQLite userdb、临时 settings JSON 和合成 TSV 复验本地管理链路。 |

## 验证入口

本地 manager 改动的标准验收命令：

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
git diff --check
./scripts/check-repo.sh
```

各命令覆盖：

- `./scripts/check-manager.sh`：执行 `dart format --set-exit-if-changed .`、`flutter analyze` 和 `flutter test`，覆盖 manager 默认 fixture、页面级 widget tests、action helper tests、Dart model / mapper tests、action preview tests 和 settings store tests。
- `./scripts/check-manager-ffi-smoke.sh`：构建 `radishlex-ime-ffi` 动态库，并用临时 SQLite userdb、临时 settings JSON、合成 TSV 和导出文件复验真实 Dart FFI bridge 的本地管理链路。
- `git diff --check`：检查空白、尾随空格和补丁文本问题。
- `./scripts/check-repo.sh`：执行仓库 text hygiene、文档预算、Go server 测试、Rust workspace 测试和 doc-tests，确认 manager 文档变更没有破坏仓库基线。

普通文档-only 修改可以只运行 `git diff --check` 和 `./scripts/check-repo.sh`；若文档修改改变验收口径、测试口径或 manager 可见行为，应同时运行 `./scripts/check-manager.sh` 和 `./scripts/check-manager-ffi-smoke.sh`。

## 隐私与数据检查

验收过程中必须确认：

- 诊断报告不包含用户词、真实路径、token、恢复码、私钥、signature bytes、wrapped material bytes 或 payload bytes。
- 学习页不展示 P1 原始 selection event、原始输入历史、窗口标题或应用上下文明细。
- sync preflight 和 settings draft 只展示状态、聚合计数和 allowlist 来源标签，不保存运行日志、证书正文或请求 / 响应体。
- 测试使用合成词、临时 SQLite、临时 settings JSON、合成 TSV 和虚构设备状态。
- Flutter widget 层只展示结构化错误码和分类，不透传 native 原始错误明细。

## 当前缺口

当前缺口不是 Phase 4 本地验收阻塞项，但会阻止真实用户同步入口：

- `apple-keychain-v1` 真实 Keychain smoke 仍阻塞于 `ed25519-v1` 创建。
- `android-keystore-v1` 在 Pixel 9 Pro API 35 AVD 和 Pixel 10 Pro API 37 AVD 上仍为 `unsupported_signature_algorithm`。
- `docs/manager-sync-entry-boundary.md` 已固定恢复码 / 设备授权交互边界、UI 状态门禁、bridge 错误分类、诊断脱敏和测试计划；发布级目标部署运行证据、平台私钥 backend 生产可用性和真实用户同步 UI / bridge 仍未形成可用产品链路，但本地 Docker / 本地 HTTPS 足以支撑下一步非上传状态开发。
- manager 没有真实远端 sync client 操作入口，当前只做本地 preflight、readiness / evidence bundle 非敏感预演、action command preview 和不可用原因解释。

## 后续推进

验收文档稳定后，近期推进顺位应从继续拆分现有 manager UI，转为按退出标准复查缺口：

1. 若 manager 本地验收缺证据，补精准 widget / helper / smoke 覆盖。
2. 若真实同步入口要进入实现，先按 `docs/manager-sync-entry-boundary.md` 推进 sync entry state helper / UI gate 的非上传状态派生和本地联调测试；发布级部署证据、平台私钥 backend 证据齐备前，不打开真实用户同步开关。
3. 若继续优化 manager 代码结构，只在文件职责继续增长或测试边界变弱时拆分，不为目录整齐新增无实际职责的层。
