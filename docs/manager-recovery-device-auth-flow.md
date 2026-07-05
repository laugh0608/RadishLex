# Manager 恢复码与设备授权交互进入条件

本文档定义 Flutter manager 在真实恢复码、设备授权和设备撤销操作开放前的只读交互状态机。读者是维护 `apps/radishlex-manager` 同步页、设置页、诊断报告和后续 bridge 扩展的人。本文不定义恢复码 KDF、签名格式、Go server API 字段全集、C ABI、真实恢复码输入框、授权按钮或设备撤销执行逻辑。

## 当前结论

当前阶段只允许展示进入条件和阻塞原因，不允许创建恢复码、输入恢复码、创建 join request、签名授权包、撤销设备或上传真实 P2 数据。Manager 通过 Dart 只读模型表达四条流程：

- `RecoverySetupReadiness`：首台设备启用同步前的恢复码生成、保存确认、恢复记录和首次上传门禁。
- `RecoveryRestoreReadiness`：新设备用恢复码恢复时的输入、恢复记录查询、失败限速和设备登记状态。
- `DeviceJoinReadiness`：新设备 join request、短码核对和授权包前置条件。
- `DeviceRevocationReadiness`：设备撤销、丢失设备风险提示和后续 key epoch 推进条件。

这些模型会被汇总为 `SyncReadinessFlowSummary`，用于同步页、设置页和诊断报告输出同一组 blocked flows、issue codes、next required evidence、source tags 和 user sync blocked 摘要。Dart 侧 `ManagerSyncReadinessBridgeSnapshot` / `manager_sync_readiness.v1` mapper 已可把未来 bridge 的非敏感状态码映射回这四条 readiness；未知状态、未知前置条件和未知错误码必须降级为安全分类，不能透传原始异常或 secret。所有模型只输出状态码、阻塞码、前置条件摘要、来源标签和错误分类；不得输出恢复码、短码、token、私钥、signature bytes、wrapped material、payload bytes、请求 / 响应体、真实路径或证据包正文。

## 首台设备恢复码设置

当前关闭态：

| 字段 | 当前值 | 含义 |
| --- | --- | --- |
| `status` | `recovery_setup_flow_closed` | 首台设备恢复码设置流程未开放。 |
| `blocker` | `recovery_code_generation_closed` | 不允许生成恢复码。 |
| `entry_action_status` | `read_only_current_phase` | UI 只展示准备状态。 |
| `generated_code_status` | `not_generated` | 当前没有生成恢复码。 |
| `save_confirmation_status` | `required_before_first_upload` | 后续真实上传前必须确认已离线保存恢复码。 |
| `recovery_record_status` | `recovery_record_not_created` | 当前未创建恢复记录。 |
| `first_upload_gate` | `blocked_until_recovery_code_saved` | 恢复码确认前不得上传 P2 对象。 |

必须满足的进入条件：

- `platform_private_key_backend_ready`
- `release_deployment_evidence_summary_required`
- `explicit_user_start_required`

必须覆盖的错误分类：

- `recovery_code_required`
- `recovery_record_missing`
- `recovery_record_revoked`
- `local_data_inconsistent`

## 恢复码恢复新设备

当前关闭态：

| 字段 | 当前值 | 含义 |
| --- | --- | --- |
| `status` | `recovery_restore_flow_closed` | 恢复新设备流程未开放。 |
| `blocker` | `recovery_code_input_closed` | UI 不提供恢复码输入。 |
| `entry_action_status` | `read_only_current_phase` | UI 只展示准备状态。 |
| `code_input_status` | `input_not_available_current_phase` | 当前不接收恢复码明文。 |
| `recovery_record_lookup_status` | `not_checked_current_phase` | 当前不查询真实恢复记录。 |
| `attempt_limit_status` | `not_started` | 当前没有失败限速状态。 |
| `device_registration_status` | `blocked_until_recovery_success` | 新设备登记必须等待恢复成功。 |

必须覆盖的错误分类：

- `recovery_code_required`
- `recovery_code_invalid`
- `recovery_record_missing`
- `recovery_record_revoked`
- `authentication_required`
- `network_unreachable`

## 设备加入授权

当前关闭态：

| 字段 | 当前值 | 含义 |
| --- | --- | --- |
| `status` | `device_join_flow_closed` | 设备加入流程未开放。 |
| `blocker` | `join_request_creation_closed` | 不允许创建 join request。 |
| `entry_action_status` | `read_only_current_phase` | UI 只展示准备状态。 |
| `join_request_status` | `join_request_unavailable` | 当前没有可处理的加入请求。 |
| `short_code_verification_status` | `short_code_verification_not_started` | 当前没有短码核对。 |
| `authorization_package_status` | `authorization_package_not_created` | 当前不创建授权包。 |
| `authorization_package_preconditions` | `active_existing_device_required, join_request_pending_required, short_code_match_required` | 授权包必须等待已有活跃设备、待处理 join request 和短码核对满足。 |

必须覆盖的错误分类：

- `join_request_expired`
- `authorization_rejected`
- `device_revoked`
- `backend_unavailable`
- `network_unreachable`

## 设备撤销与 key epoch

当前关闭态：

| 字段 | 当前值 | 含义 |
| --- | --- | --- |
| `status` | `device_revocation_flow_closed` | 设备撤销流程未开放。 |
| `blocker` | `device_revocation_flow_closed` | 不允许执行撤销。 |
| `entry_action_status` | `read_only_current_phase` | UI 只展示准备状态。 |
| `revoke_device_status` | `closed_current_phase` | 当前不提供撤销操作。 |
| `active_device_requirement` | `active_existing_device_required` | 后续只有活跃设备能发起撤销。 |
| `lost_device_risk_notice` | `lost_device_prior_material_not_recallable` | UI 必须说明撤销无法追回旧设备已取得材料。 |
| `key_epoch_status` | `key_epoch_rotation_not_started` | 后续 key epoch 轮换尚未开始。 |

必须覆盖的错误分类：

- `device_revoked`
- `key_epoch_rotation_required`
- `local_data_inconsistent`
- `network_unreachable`

## UI 与诊断要求

- 同步页展示四条流程的 status、blocker 和错误分类摘要。
- 同步页和设置页 gate preview 展示同一组 readiness 聚合摘要和 `readiness_bridge_source` 来源标签，不提供操作按钮。
- 诊断报告输出字段必须保留在 `sync_gate` 分组，只输出状态码、来源标签、前置条件码和错误码。
- `manager_sync_readiness.v1` 只作为 future bridge mapper 的非敏感输入形状，不改变 `ManagerBridge` contract，不新增 C ABI。
- settings draft 不新增 secret 字段，不保存恢复码、短码、token、签名、wrapped material 或 payload bytes。
- widget fixture 必须使用合成设备、合成 endpoint 和状态码，不嵌入真实账号、真实路径、真实服务端响应或截图中的敏感内容。

## 进入真实实现前的停止线

- 平台私钥 backend production gate 未 ready 前，不创建恢复记录、join request、授权包或撤销记录。
- 发布级目标部署证据未形成前，不把同步展示为真实用户可用。
- 恢复码保存确认未完成前，不上传真实 P2 对象。
- join request、短码核对、授权包、撤销和 key epoch 错误语义未映射到 bridge 前，不开放对应操作。
- 任何明文恢复码、短码、token、私钥、signature bytes、wrapped material、payload bytes、请求 / 响应体或 provider 原始异常进入 UI、settings、诊断或 fixture，都必须停止并回退。
