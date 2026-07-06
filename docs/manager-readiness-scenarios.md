# Manager Readiness 场景目录

本文档定义 Flutter manager 开发期 `manager_sync_readiness.v1` 联调场景。读者是维护 settings / sync / diagnostics 状态派生、准备未来 `ManagerBridge` readiness 接线和审阅真实同步入口停止线的协作者。本文不定义 C ABI、真实恢复码生成、join request 创建、设备授权成功、设备撤销操作或 Go server 新 API。

## 当前边界

- 这些场景只用于本地开发、fixture、widget / helper / model 测试和人工复核。
- 场景输入必须是非敏感摘要，只允许状态码、阻塞码、前置证据码、来源标签和错误分类。
- 场景不得包含真实 token、恢复码、短码、私钥、signature bytes、wrapped material、payload bytes、请求 / 响应体、真实路径或 provider exception 原文；用于验证拒绝 / 降级路径的合成 leak fragment 只能停留在不安全输入 fixture，预期输出、UI 和诊断文本不得传播。
- 即便场景显示四条 readiness 全部 ready，当前 Phase 4 仍派生 `user_sync_entry_closed_current_phase`，不打开真实远端同步、恢复码或设备授权操作。
- 场景目录是 future bridge mapper 的验收输入，不是新增 `ManagerBridge` contract，也不是 C ABI。
- 场景目录同时驱动模型、settings gate preview、诊断报告和同步页代表场景回归，避免 settings / sync / diagnostics 对同一份 readiness 摘要派生出不同口径。
- 开发期 `manager_sync_evidence_bundle.v1` 只在测试 fixture 中组合 readiness 摘要、连接健康摘要、部署证据来源和设备 production gate；它不新增生产 bridge，也不改变 settings draft 持久化格式。
- `manager_sync_action_command_preview.v1` 由同一份场景目录派生非执行命令预演，用于确认 action intent 只产生 execution status、data policy、stop line、request boundary、result boundary 和错误分类，不创建 bridge 命令。

## 场景表

| 场景 ID | 输入 | 预期来源 | entry blocker | blocked flows | issue codes | next evidence | interaction statuses | user sync |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `all_ready_current_phase_closed` | `manager_sync_readiness.v1` ready 摘要 | `ffi_native_readiness` | `user_sync_entry_closed_current_phase` | `none` | `none` | `none` | 四条 action 均为 `closed_current_phase` | `false` |
| `recovery_record_missing` | 恢复 setup 阻塞，其他 readiness ready | `fixture_readiness` | `recovery_record_missing` | `recovery_setup` | `recovery_record_missing` | `platform_private_key_backend_ready`, `release_deployment_evidence_summary_required`, `explicit_user_start_required` | `recovery_setup=blocked`，其余 action 为 `closed_current_phase` | `false` |
| `join_request_expired` | 设备 join 阻塞，其他 readiness ready | `fixture_readiness` | `join_request_expired` | `device_join` | `join_request_expired`, `authorization_rejected` | `join_request_expired`, `short_code_match_required`, `join_request_pending_required` | `join_request_authorization=blocked`，其余 action 为 `closed_current_phase` | `false` |
| `platform_backend_blocked` | 设备 production gate 为 `blocked`，无导入 readiness | `manager_default_closed_readiness` | `backend_unavailable` | 四条 readiness 均阻塞 | 默认关闭态 issue codes | 默认关闭态 next evidence | 四条 action 均为 `closed_current_phase` | `false` |
| `deployment_evidence_local_smoke_only` | 只有 `local_smoke` 证据，发布级证据不足 | `manager_default_closed_readiness` | `release_deployment_evidence_required` | 四条 readiness 均阻塞 | 默认关闭态 issue codes | 默认关闭态 next evidence | 四条 action 均为 `closed_current_phase` | `false` |
| `deployment_evidence_missing` | 未记录目标部署证据 | `manager_default_closed_readiness` | `deployment_unverified` | 四条 readiness 均阻塞 | 默认关闭态 issue codes | 默认关闭态 next evidence | 四条 action 均为 `closed_current_phase` | `false` |
| `unknown_native_status_sanitized` | 摘要 format / redaction 合法，但包含未知 native 状态和敏感形态字段 | `unknown_bridge_readiness_source` | `recovery_record_missing` | 四条 readiness 均阻塞 | 只保留 allowlist 错误和 `unexpected_bridge_error_code` | 只保留 allowlist evidence 和 `unexpected_bridge_required_evidence` | 未知 setup 状态降级为 `closed_current_phase`，其余明确阻塞 action 为 `blocked` | `false` |
| `unsupported_format_rejected` | `format != manager_sync_readiness.v1` | `manager_default_closed_readiness` | `recovery_code_flow_closed` | 四条 readiness 均阻塞 | 默认关闭态 issue codes | 默认关闭态 next evidence | 四条 action 均为 `closed_current_phase` | `false` |
| `unsafe_redaction_rejected` | `redaction_policy` 不是安全摘要策略 | `manager_default_closed_readiness` | `recovery_code_flow_closed` | 四条 readiness 均阻塞 | 默认关闭态 issue codes | 默认关闭态 next evidence | 四条 action 均为 `closed_current_phase` | `false` |

默认关闭态 issue codes 和 next evidence 的完整字符串以 `apps/radishlex-manager/test/fixtures/sync_readiness_bridge_fixtures.dart` 中 `syncReadinessDefaultIssueCodes` 和 `syncReadinessDefaultNextEvidence` 为测试真相源，避免文档复制超长字段后漂移。

## Evidence Bundle 预演场景

`manager_sync_evidence_bundle.v1` 的测试目录用于把 readiness、`sync_connection_health.v1`、部署证据来源和设备 gate 放在同一个 snapshot 中复验。bundle 的 redaction policy 为 `summary_only_no_endpoint_tokens_recovery_secret_or_payload_bytes`，只允许摘要字段，不允许 endpoint credential、token、恢复码、短码、请求 / 响应体、真实路径、signature bytes、wrapped material 或 payload bytes。

| 场景 ID | 组合输入 | 预期 entry blocker | 连接状态 | 连接 blocker | readiness source | user sync |
| --- | --- | --- | --- | --- | --- | --- |
| `all_ready_external_probe_current_phase_closed` | 四条 readiness ready，外部 HTTPS 探测可达 | `user_sync_entry_closed_current_phase` | `reachable` | `none` | `ffi_native_readiness` | `false` |
| `local_https_reachable_recovery_record_missing` | 本地 HTTPS 探测可达，恢复记录缺失 | `recovery_record_missing` | `reachable` | `none` | `fixture_readiness` | `false` |
| `network_unreachable_all_ready_current_phase_closed` | 四条 readiness ready，本地 HTTPS 探测网络不可达 | `user_sync_entry_closed_current_phase` | `network_unreachable` | `network_unreachable` | `ffi_native_readiness` | `false` |
| `unsafe_connection_summary_rejected` | connection summary redaction policy 不安全 | `user_sync_entry_closed_current_phase` | `probe_summary_invalid` | `probe_summary_format_unsupported` | `ffi_native_readiness` | `false` |
| `unsafe_readiness_summary_rejected` | readiness redaction policy 不安全 | `recovery_code_flow_closed` | `reachable` | `none` | `manager_default_closed_readiness` | `false` |
| `unsafe_readiness_summary_downgraded` | readiness 摘要合法但含未知状态和敏感形态字段 | `recovery_record_missing` | `reachable` | `none` | `unknown_bridge_readiness_source` | `false` |

这些场景确认连接健康是解释性证据：可达、不可达或摘要被拒绝都不会打开 `启用同步`、恢复码、join request、授权成功或设备撤销路径。command preview 也必须保持非执行：ready 但阶段关闭时只显示 `not_executable_current_phase`，readiness 阻塞时只显示 `blocked_by_readiness`，request / result boundary 只显示摘要边界，request / result allowed fields 只显示非敏感字段码，错误分类只显示 allowlist 码。

## Action Command Protocol 预期

`manager_sync_action_command_preview.v1` 的场景预期由 fixture 常量维护，避免 settings、sync 和 diagnostics 三处漂移。

| 摘要 | 预期 |
| --- | --- |
| data policy | `no_recovery_code_or_wrapped_material`, `no_recovery_code_input_or_device_secret`, `no_short_code_signature_or_wrapped_material`, `no_signature_key_epoch_or_wrapped_material` |
| stop line | `no_recovery_code_generation_current_phase`, `no_recovery_code_input_current_phase`, `no_join_request_or_authorization_package_current_phase`, `no_device_revocation_current_phase` |
| request boundary | `request_summary_only_no_recovery_code_generation`, `request_summary_only_no_recovery_code_input`, `request_summary_only_no_join_request_or_short_code`, `request_summary_only_no_device_signature_or_key_epoch` |
| result boundary | `result_summary_only_no_recovery_record_or_wrapped_material`, `result_summary_only_no_unwrapped_device_material`, `result_summary_only_no_authorization_package_or_signature`, `result_summary_only_no_revocation_record_or_key_epoch_material` |
| request allowed fields | `deployment_evidence_source_tag`, `device_backend_gate`, `explicit_user_start`, `save_confirmation_status`, `restore_attempt_status`, `recovery_record_lookup_status`, `device_registration_status`, `join_request_status`, `short_code_verification_status`, `active_device_requirement`, `authorization_package_preconditions`, `target_device_status_summary`, `lost_device_risk_acknowledgement`, `key_epoch_status` |
| result allowed fields | `status_code`, `recovery_record_status`, `first_upload_gate`, `next_required_evidence`, `restore_result_status`, `device_registration_status`, `authorization_package_status`, `device_state_status`, `revocation_record_status`, `key_epoch_status` |
| forbidden material | `recovery_secret_material`, `join_verifier_material`, `bearer_credential_material`, `signature_material`, `wrapped_sync_material`, `opaque_transport_content` |
| error codes | `configuration_missing`, `backend_unavailable`, `deployment_unverified`, `recovery_code_required`, `recovery_record_missing`, `recovery_record_revoked`, `local_data_inconsistent`, `authentication_required`, `recovery_code_invalid`, `network_unreachable`, `join_request_expired`, `authorization_rejected`, `device_revoked`, `key_epoch_rotation_required` |

完整 action 级边界见 [`docs/manager-sync-action-protocol-preview.md`](manager-sync-action-protocol-preview.md)。场景 fixture 中的 `expectedActionCommandRequestBoundaries`、`expectedActionCommandResultBoundaries`、`expectedActionRequestAllowedFields`、`expectedActionResultAllowedFields`、`expectedActionForbiddenMaterials` 和 `expectedActionCommandErrorCodes` 是测试真相源；未来新增 action、字段码或错误分类前必须先更新该专题文档、fixture 和 settings / sync / diagnostics 回归。

## 复验入口

主要覆盖：

- `apps/radishlex-manager/test/fixtures/sync_readiness_bridge_fixtures.dart`：场景输入、预期来源、预期派生摘要和 action request / result protocol preview 预期。
- `apps/radishlex-manager/test/fixtures/sync_evidence_bundle_fixtures.dart`：组合 readiness、connection health、部署证据来源、设备 gate 和 action request / result protocol preview 的预演输入。
- `apps/radishlex-manager/test/models/manager_sync_entry_gate_test.dart`：遍历场景目录，校验 import result、entry gate、readiness 摘要、interaction intent、action command preview、request / result status、request / result allowed fields、错误分类、envelope schema 和脱敏边界。
- `apps/radishlex-manager/test/screens/settings_test.dart`：验证 settings 导入、清除、同步页状态和诊断导出一致性，并固定 settings gate preview 的代表场景可见层和 command preview 回归。
- `apps/radishlex-manager/test/screens/settings_diagnostics_test.dart`：遍历场景目录，校验诊断报告中的 `sync.readiness_*`、`sync.interaction_*`、`sync.action_command_*`、`sync.user_sync_enabled` 和脱敏文本。
- `apps/radishlex-manager/test/screens/sync_test.dart`：固定 ready 但当前阶段关闭、恢复记录缺失、join request 过期、未知 native 状态清洗和 unsafe redaction 拒绝等代表场景，校验同步页预检区、command preview 可见字段和入口按钮关闭状态。
- `apps/radishlex-manager/test/screens/manager_home_actions_test.dart`：验证 settings draft 保存后保留导入 readiness，导入态诊断使用当前 snapshot。

建议命令：

```bash
flutter test test/models/manager_sync_entry_gate_test.dart test/screens/manager_home_actions_test.dart test/screens/settings_test.dart test/screens/settings_diagnostics_test.dart test/screens/sync_test.dart
./scripts/check-manager.sh
git diff --check
./scripts/check-repo.sh
```
