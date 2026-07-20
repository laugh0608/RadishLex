# Manager 同步 Action 验收矩阵

本文档定义 Flutter manager 在真实 bridge 命令开放前，对四条同步 action 的非敏感协议验收矩阵。读者是维护 `SyncActionCommandPreviewPlan`、settings / sync / diagnostics 可见层、fixture 场景和 future bridge contract 设计的人。本文不定义真实 `ManagerBridge` contract、C ABI、Go server API、恢复码生成、恢复码输入、join request 创建、授权包签名、设备撤销执行或同步上传逻辑。

## 当前结论

`manager_sync_action_command_preview.v1` 仍是非执行预演格式，只能从 `SyncInteractionEntryPlan` 派生安全摘要。验收矩阵的目标是确认不同 intent 状态下，execution status、request status、result status、allowed fields、forbidden material、error codes、data policy 和 stop line 保持一致；它不是 bridge DTO，也不能被 UI 当作可执行命令。

当前 Phase 4 中，即便 readiness 全部 ready，四条 action 仍必须保持当前阶段关闭，输出 `not_executable_current_phase`、`request_not_built_current_phase` 和 `result_not_available_current_phase`。只有后续真实 bridge contract、恢复 / 授权实现测试、平台私钥 backend 和发布级部署证据齐备后，才允许另行设计可执行请求 / 响应。

## 状态矩阵

| 场景 | Intent status | Execution status | Request status | Result status | 说明 |
| --- | --- | --- | --- | --- | --- |
| 当前阶段关闭 | `closed_current_phase` | `not_executable_current_phase` | `request_not_built_current_phase` | `result_not_available_current_phase` | 默认 Phase 4 状态；不能创建 request，也不能产生 result。 |
| readiness 阻塞 | `blocked` | `blocked_by_readiness` | `request_blocked_by_readiness` | `result_blocked_by_readiness` | 用于 backend、部署证据、恢复记录、join request、key epoch 等前置条件未满足。 |
| 需要用户确认 | `requires_confirmation` | `blocked_until_user_confirmation` | `request_blocked_until_user_confirmation` | `result_blocked_until_user_confirmation` | 只说明需要显式确认；不收集确认输入，不打开按钮。 |
| future ready shape | `ready` | `ready_for_future_bridge_command` | `request_shape_ready_for_future_bridge` | `result_shape_ready_for_future_bridge` | 只用于 future contract 形状验收；当前 UI 不能把它转成执行。 |
| missing intent | 无对应 intent | `blocked_by_missing_intent` | `request_blocked_by_missing_intent` | `result_blocked_by_missing_intent` | `previewFor()` fallback，证明缺失 action intent 时仍不可执行。 |
| unknown intent status | 未知 intent status | `blocked_by_unknown_intent_status` | `request_blocked_by_unknown_execution_status` | `result_blocked_by_unknown_execution_status` | 未来 native / bridge 新状态必须降级，不透传 provider 原文。 |

这些状态由 `apps/radishlex-manager/test/fixtures/sync_action_protocol_fixtures.dart` 中 `syncActionProtocolStatusScenarios` 维护，并由 `manager_sync_action_preview_test.dart` 覆盖四条 action 的矩阵组合。

## Action 级矩阵

每条 action 的 request / result preview 只能显示非敏感字段码。字段码用于说明 future request / result 的安全外壳，不代表当前已经构造 bridge payload。

| Action | Data policy | Stop line | Request allowed fields | Result allowed fields |
| --- | --- | --- | --- | --- |
| `recovery_setup` | `no_recovery_code_or_wrapped_material` | `no_recovery_code_generation_current_phase` | `deployment_evidence_source_tag`, `device_backend_gate`, `explicit_user_start`, `save_confirmation_status` | `status_code`, `recovery_record_status`, `first_upload_gate`, `next_required_evidence` |
| `recovery_restore` | `no_recovery_code_input_or_device_secret` | `no_recovery_code_input_current_phase` | `deployment_evidence_source_tag`, `restore_attempt_status`, `recovery_record_lookup_status`, `device_registration_status` | `status_code`, `restore_result_status`, `device_registration_status`, `next_required_evidence` |
| `join_request_authorization` | `no_short_code_signature_or_wrapped_material` | `no_join_request_or_authorization_package_current_phase` | `join_request_status`, `short_code_verification_status`, `active_device_requirement`, `authorization_package_preconditions` | `status_code`, `authorization_package_status`, `device_state_status`, `next_required_evidence` |
| `device_revocation` | `no_signature_key_epoch_or_wrapped_material` | `no_device_revocation_current_phase` | `target_device_status_summary`, `active_device_requirement`, `lost_device_risk_acknowledgement`, `key_epoch_status` | `status_code`, `revocation_record_status`, `key_epoch_status`, `next_required_evidence` |

所有 action 的 request / result preview 都必须携带同一组 forbidden material policy：

- `recovery_secret_material`
- `join_verifier_material`
- `bearer_credential_material`
- `signature_material`
- `wrapped_sync_material`
- `opaque_transport_content`

错误分类继续使用 `docs/manager-sync-action-protocol-preview.md` 中的 action allowlist。未知错误、provider exception、真实路径、请求 / 响应体、payload-shaped 字段或 secret-shaped 字段不能进入 preview；需要表达未知情况时只能降级为安全分类。

## Fixture 真相源

`apps/radishlex-manager/test/fixtures/sync_action_protocol_fixtures.dart` 是 action protocol 矩阵的测试真相源，包含：

- `syncActionProtocolExpectations`：四条 action 的 data policy、stop line、request boundary、result boundary、allowed fields 和 error codes。
- `syncActionProtocolStatusScenarios`：当前阶段关闭、readiness 阻塞、用户确认、future ready shape 和未知 intent status。
- `syncActionExpectedExecutionSummary()`、`syncActionExpectedRequestStatusSummary()`、`syncActionExpectedResultStatusSummary()`：settings、sync、diagnostics 和 model 测试共享的状态派生函数。
- `syncActionExpectedExecutionStatuses()`、`syncActionExpectedRequestStatuses()`、`syncActionExpectedResultStatuses()`：同步页可见层测试使用的去重状态列表。

`sync_readiness_bridge_fixtures.dart` 和 `sync_evidence_bundle_fixtures.dart` 继续定义 readiness / evidence bundle 场景，但 action protocol 的状态派生、聚合摘要和 action 级边界不再由各测试文件各自维护。

## 验收入口

主要测试：

- `apps/radishlex-manager/test/models/manager_sync_action_preview_test.dart`
- `apps/radishlex-manager/test/models/manager_sync_entry_gate_test.dart`
- `apps/radishlex-manager/test/screens/settings_test.dart`
- `apps/radishlex-manager/test/screens/settings_diagnostics_test.dart`
- `apps/radishlex-manager/test/screens/sync_test.dart`

建议验证：

```bash
flutter test test/models/manager_sync_action_preview_test.dart test/models/manager_sync_entry_gate_test.dart test/screens/settings_test.dart test/screens/settings_diagnostics_test.dart test/screens/sync_test.dart
./scripts/check-manager.sh
git diff --check
```

如果改动触及文档预算、仓库级门禁或公共 fixture，再补跑：

```bash
./scripts/check-repo.sh
```

## 停止线

- 不把 `manager_sync_action_command_preview.v1` 转成真实 bridge request。
- 不新增 `ManagerBridge` method，不新增 C ABI，不写 settings draft action payload。
- 不新增按钮、点击回调、真实 bridge 调用、恢复码输入框、join request 创建入口、授权成功入口或设备撤销入口。
- 不携带恢复码、短码、token、signature bytes、wrapped material、payload bytes、请求 / 响应体、provider exception 原文或真实路径。
- 不因为 future ready shape 测试通过而解除当前 Phase 4 的 `user_sync_entry_closed_current_phase` 阻塞。
