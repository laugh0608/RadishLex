# Manager 同步 Readiness 场景

本文用于说明 Flutter Manager 开发期 readiness 测试的输入边界和代表场景。它不定义同步命令、审批流程、部署证据包或产品开放条件；产品停止线见 `docs/manager-sync-entry-boundary.md`。

## 测试目的

`manager_sync_readiness.v1` 只把 Rust / native 提供的非敏感状态摘要映射为 Manager 可展示的关闭原因。测试需要证明：

- 所有 readiness 显示 ready 时，真实用户同步仍保持关闭。
- 平台 backend、恢复记录和设备授权阻塞能够稳定显示。
- 未知 native 状态被降级为安全错误分类，不透传原始内容。
- 不支持的 format 或 redaction policy 会被拒绝并回到默认关闭态。
- settings、sync 页面和 diagnostics 对同一 snapshot 的状态一致。

## 代表场景

| 场景 | 预期 |
| --- | --- |
| all ready | `user_sync_entry_closed_current_phase`，`userSyncEnabled == false` |
| platform backend blocked | `backend_unavailable`，入口不可用 |
| recovery record missing | readiness 显示 `recovery_record_missing`，不生成恢复码 |
| unknown native status | 映射为 allowlist 安全类别，敏感片段不进入 UI / diagnostics |
| unsafe redaction | 拒绝导入，使用 `manager_default_closed_readiness` |
| privacy mode | `sync_disabled_by_policy` 优先于其他 readiness 状态 |

测试不需要为每个未来命令维护 request / result shape。当前 capability 缺席、禁用按钮、稳定关闭状态和脱敏错误已经足以表达产品边界。

## 数据与脱敏

readiness fixture 只能使用合成设备、合成 endpoint 和公开状态码。以下内容不得进入 fixture、widget 文本或 diagnostics：

- token、恢复码、短码、私钥、签名或 wrapped material；
- payload bytes、请求体、响应体或 provider 原始异常；
- 真实用户路径、联系人、输入历史或远端日志正文。

## 复验入口

- `apps/radishlex-manager/test/fixtures/sync_readiness_bridge_fixtures.dart`
- `apps/radishlex-manager/test/models/manager_sync_entry_gate_test.dart`
- `apps/radishlex-manager/test/models/manager_sync_transient_secret_interaction_test.dart`
- `apps/radishlex-manager/test/screens/settings_test.dart`
- `apps/radishlex-manager/test/screens/settings_diagnostics_test.dart`
- `apps/radishlex-manager/test/screens/sync_test.dart`

这些测试只证明 Manager 安全解释当前关闭态，不代表恢复、授权或远端同步已经实现。
