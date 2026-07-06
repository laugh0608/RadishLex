# Manager 同步 Action 协议预演边界

本文档定义 Flutter manager 在真实 bridge 命令开放前，对 `recovery_setup`、`recovery_restore`、`join_request_authorization` 和 `device_revocation` 四条 action 允许展示的非敏感 request / result 边界、错误分类、data policy 与 stop line。读者是维护 manager sync/settings/diagnostics、future `ManagerBridge` readiness 接线和后续真实同步交互设计的人。本文不定义真实 `ManagerBridge` contract、C ABI、Go server API、恢复码 KDF、签名格式、授权包 payload 或设备撤销执行逻辑。

## 当前结论

`manager_sync_action_command_preview.v1` 仍是非执行命令预演格式，不是 bridge 请求格式。它只能从 `SyncInteractionEntryPlan` 派生摘要字段，用于 settings gate preview、同步页和诊断报告展示。当前阶段即便四条 readiness 都为 ready，action 仍必须保持 `execution_status = not_executable_current_phase`，不得创建恢复码、输入恢复码、创建 join request、签名授权包、撤销设备或上传真实 P2 数据。

允许展示：

- action id、visibility、intent status、execution status。
- blocker、required evidence、source tag。
- data policy、stop line。
- request boundary、result boundary。
- allowlist 错误分类。

禁止展示：

- 恢复码、短码、token、私钥、公钥原文、signature bytes、wrapped material、payload bytes。
- KDF 输出、恢复记录密文、授权包密文、revocation record bytes、key epoch material。
- 请求 / 响应体、Go server 原始错误、provider exception 原文、真实路径或证据包正文。

## Action 边界表

| Action | Request boundary | Result boundary | Data policy | Stop line |
| --- | --- | --- | --- | --- |
| `recovery_setup` | `request_summary_only_no_recovery_code_generation` | `result_summary_only_no_recovery_record_or_wrapped_material` | `no_recovery_code_or_wrapped_material` | `no_recovery_code_generation_current_phase` |
| `recovery_restore` | `request_summary_only_no_recovery_code_input` | `result_summary_only_no_unwrapped_device_material` | `no_recovery_code_input_or_device_secret` | `no_recovery_code_input_current_phase` |
| `join_request_authorization` | `request_summary_only_no_join_request_or_short_code` | `result_summary_only_no_authorization_package_or_signature` | `no_short_code_signature_or_wrapped_material` | `no_join_request_or_authorization_package_current_phase` |
| `device_revocation` | `request_summary_only_no_device_signature_or_key_epoch` | `result_summary_only_no_revocation_record_or_key_epoch_material` | `no_signature_key_epoch_or_wrapped_material` | `no_device_revocation_current_phase` |

这些边界的含义是：UI、fixture、settings preview 和 diagnostics 只能描述未来 request / result 的安全外壳，不能携带真实命令 payload。后续真实 bridge contract 设计时，应重新定义结构化请求 / 响应，并复用这些停止线作为验收前置，而不是把 preview 字段直接当作 bridge DTO。

## 错误分类

`manager_sync_action_command_preview.v1` 的 action 级错误分类只允许使用以下 allowlist。未知 native 错误、provider 原始异常和 payload-shaped 字段不得进入 preview；需要降级时应映射为已有安全分类或阻塞真实接线。

| Action | 错误分类 |
| --- | --- |
| `recovery_setup` | `configuration_missing`, `backend_unavailable`, `deployment_unverified`, `recovery_code_required`, `recovery_record_missing`, `recovery_record_revoked`, `local_data_inconsistent` |
| `recovery_restore` | `configuration_missing`, `authentication_required`, `deployment_unverified`, `recovery_code_required`, `recovery_code_invalid`, `recovery_record_missing`, `recovery_record_revoked`, `network_unreachable`, `local_data_inconsistent` |
| `join_request_authorization` | `configuration_missing`, `authentication_required`, `backend_unavailable`, `deployment_unverified`, `join_request_expired`, `authorization_rejected`, `device_revoked`, `network_unreachable`, `local_data_inconsistent` |
| `device_revocation` | `configuration_missing`, `authentication_required`, `backend_unavailable`, `deployment_unverified`, `device_revoked`, `key_epoch_rotation_required`, `network_unreachable`, `local_data_inconsistent` |

## UI / 诊断字段

设置页 gate preview、同步页和诊断报告必须从同一个 `SyncActionCommandPreviewPlan` 派生字段，不允许各自维护一套口径。

诊断报告字段：

- `sync.action_command_data_policy`
- `sync.action_command_stop_lines`
- `sync.action_command_request_boundaries`
- `sync.action_command_result_boundaries`
- `sync.action_command_error_codes`

同步页可以把四条 action 拆成单行展示；设置页和诊断报告可以展示聚合摘要。两者都不得新增按钮、点击回调、bridge 调用或 settings draft 持久化字段。

## Fixture 与测试验收

场景目录中的 readiness 和 evidence bundle fixture 必须同时携带 action command preview 的 request boundary、result boundary 和 error code 预期。当前验收入口：

- `apps/radishlex-manager/test/fixtures/sync_readiness_bridge_fixtures.dart`
- `apps/radishlex-manager/test/fixtures/sync_evidence_bundle_fixtures.dart`
- `apps/radishlex-manager/test/models/manager_sync_entry_gate_test.dart`
- `apps/radishlex-manager/test/screens/settings_test.dart`
- `apps/radishlex-manager/test/screens/settings_diagnostics_test.dart`
- `apps/radishlex-manager/test/screens/sync_test.dart`

测试必须覆盖：

- 四条 action 的 request boundary、result boundary、data policy、stop line 和错误分类摘要稳定。
- readiness ready 但当前阶段关闭时仍输出 `not_executable_current_phase`。
- readiness 阻塞时只输出 `blocked_by_readiness`。
- unsafe redaction、未知 native 状态、payload-shaped 字段和 secret-shaped 字段不进入 UI、settings preview、diagnostics 或 fixture 预期输出。

## 进入真实 bridge 前的停止线

- 没有真实 bridge contract、恢复 / 授权实现测试、发布级部署证据和平台私钥 backend 前，不允许把 preview 转成可执行命令。
- request boundary 和 result boundary 只描述安全摘要，不能承载恢复码、短码、签名、wrapped material、payload bytes 或请求 / 响应体。
- 错误分类必须是 allowlist 码；任何 provider 原始异常、真实路径、token、恢复码或 payload-shaped 字段进入 preview，必须停止并回退。
- settings draft 不新增 action payload 字段；diagnostics 只输出聚合状态码和策略码。
