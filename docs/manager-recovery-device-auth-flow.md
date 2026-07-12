# Manager 恢复码与设备授权交互进入条件

本文档定义 Flutter manager 在真实恢复码、设备授权和设备撤销操作开放前的只读交互状态机。读者是维护 `apps/radishlex-manager` 同步页、设置页、诊断报告和后续 bridge 扩展的人。本文不定义恢复码 KDF、签名格式、Go server API 字段全集、C ABI、真实恢复码输入框、授权按钮或设备撤销执行逻辑。

## 当前结论

当前阶段只允许展示进入条件和阻塞原因，不允许创建恢复码、输入恢复码、创建 join request、签名授权包、撤销设备或上传真实 P2 数据。Manager 通过 Dart 只读模型表达四条流程：

- `RecoverySetupReadiness`：首台设备启用同步前的恢复码生成、保存确认、恢复记录和首次上传门禁。
- `RecoveryRestoreReadiness`：新设备用恢复码恢复时的输入、恢复记录查询、失败限速和设备登记状态。
- `DeviceJoinReadiness`：新设备 join request、短码核对和授权包前置条件。
- `DeviceRevocationReadiness`：设备撤销、丢失设备风险提示和后续 key epoch 推进条件。

这些模型会被汇总为 `SyncReadinessFlowSummary`，用于同步页、设置页和诊断报告输出同一组 blocked flows、issue codes、next required evidence、source tags 和 user sync blocked 摘要。`SyncInteractionEntryPlan` / `SyncInteractionActionIntent` 在 readiness 之上派生四个未来操作入口：`recovery_setup`、`recovery_restore`、`join_request_authorization` 和 `device_revocation`，只输出 `visibility_status`、`intent_status`、`blocker`、`required_evidence` 和 `source_tag`，用于说明操作为何仍不可执行。`manager_sync_action_command_preview.v1` / `SyncActionCommandPreviewPlan` 再从 action intent 派生非执行命令预演，只输出 action id、intent status、execution status、blocker、required evidence、source tag、data policy、stop line、request boundary、result boundary、request / result allowed fields、forbidden material policy 和 action 级错误分类；它不创建 bridge 请求，不保存 settings 字段，也不携带任何命令 payload。Dart 侧 `ManagerSyncReadinessBridgeSnapshot` / `manager_sync_readiness.v1` mapper 已可把未来 bridge 的非敏感状态码映射回这四条 readiness；未知状态、未知前置条件和未知错误码必须降级为安全分类，不能透传原始异常或 secret。所有模型只输出状态码、阻塞码、前置条件摘要、来源标签、策略码、request / result 边界、允许字段码、禁止材料策略和错误分类；不得输出恢复码、短码、token、私钥、signature bytes、wrapped material、payload bytes、请求 / 响应体、真实路径或证据包正文。四条 action 的协议预演字段见 `docs/manager-sync-action-protocol-preview.md`。

即便未来 bridge snapshot 把四条 readiness 都映射为 ready，当前阶段的 action intent 仍派生为 `closed_current_phase`，`blocker` 为 `user_sync_entry_closed_current_phase`，`required_evidence` 为 `user_sync_entry_current_phase_open_required`。这条规则确保只读准备态不会被误解为真实恢复码生成、恢复码输入、join request 创建、设备授权成功或设备撤销入口已经开放。

command preview 的 `execution_status` 同样不会打开操作：`closed_current_phase` 映射为 `not_executable_current_phase`，`blocked` 映射为 `blocked_by_readiness`，`requires_confirmation` 映射为 `blocked_until_user_confirmation`。只有后续真实 bridge contract、恢复 / 授权实现测试、发布级部署证据和平台私钥 backend 全部齐备后，才允许把相关 action 推进到可执行命令设计。

## Transient Secret 交互生命周期

恢复码一次性展示、恢复码输入、短码核对和显式授权 / 撤销确认必须先满足独立生命周期约束，才能进入真实 UI 或 bridge 实现。当前阶段只固定交互边界，不显示、输入、复制、保存或传递真实 secret。

当前设计证据：

- `apps/radishlex-manager/test/fixtures/sync_transient_secret_interaction_fixtures.dart`
- `apps/radishlex-manager/test/models/manager_sync_transient_secret_interaction_test.dart`

其中 `syncRecoveryVisibleLayerFixtures` 固定 recovery setup / restore 的可见层文案和确认占位边界。这里的 copy 指 UI 文案，不是剪贴板复制动作；当前仍禁止自动复制、真实输入、保存或传递 secret。

`syncRecoveryFutureConfirmationDetailFixtures` 进一步固定 recovery setup / restore 的 future confirmation detail：setup 只允许表达保存确认仍缺失、首次上传仍被阻塞和后续必须满足的平台 / 部署 / 显式启动前置条件；restore 只允许表达输入仍关闭、恢复记录未查询、失败限速未开始和设备登记等待恢复成功。该 detail 只作为 fixture / model test 证据，不创建 settings action，不生成 bridge request，不渲染 secret，也不允许自动复制到剪贴板。

覆盖的 transient 交互面：

| 交互面 | 绑定 action | 当前状态 | 允许输出 |
| --- | --- | --- | --- |
| 一次性恢复码展示占位 | `recovery_setup` | `display_not_available_current_phase` | 展示占位状态、保存确认状态、恢复记录状态和首次上传 gate 摘要。 |
| 恢复码输入占位 | `recovery_restore` | `input_not_available_current_phase` | 恢复尝试状态、恢复记录查询状态、失败限速状态和设备登记状态码。 |
| 短码核对输入占位 | `join_request_authorization` | `input_not_available_current_phase` | join request 状态、短码核对状态和授权包前置条件摘要。 |
| 授权显式确认 | `join_request_authorization` | `confirmation_not_available_current_phase` | 用户确认状态码和授权包状态摘要。 |
| 撤销显式确认 | `device_revocation` | `confirmation_not_available_current_phase` | 用户确认状态码、撤销记录状态和 key epoch 摘要。 |

生命周期规则：

- 只能由用户显式操作触发，不能由页面加载、settings draft 保存、连接健康探测或 diagnostics export 自动触发。
- 明文值只允许存在于调用期或可见 modal 期，提交、取消、导航离开或超时后必须清除。
- 不启用自动填充、输入建议、自动复制到剪贴板或把 secret 放入路由参数。
- 不写入 settings JSON、诊断报告、日志、widget golden、crash report、analytics event 或异步 snapshot state。
- UI 和 diagnostics 只能记录状态码、确认码、阻塞码、attempt / lookup / gate 摘要和下一步 evidence code。
- 完成证据只能是 `*_ack_code_only`、`*_status_summary` 或同类非敏感状态码，不能包含恢复码、短码、signature、wrapped material 或 payload bytes。

当前阶段继续禁止：

- `generate_recovery_code`
- `persist_recovery_code`
- `input_recovery_code`
- `unwrap_device_material`
- `sign_authorization_package`
- `write_wrapped_material`
- `revoke_device`
- `advance_key_epoch`

可见层状态码：

| 作用面 | UI 状态字段 | diagnostics 字段 | 当前状态码 |
| --- | --- | --- | --- |
| setup 一次性展示占位 | `setup display status` | `sync.recovery_setup_display_status` | `display_not_available_current_phase` |
| setup 保存确认占位 | `setup save confirmation` | `sync.recovery_setup_save_confirmation` | `required_before_first_upload` |
| setup 恢复记录 / 首次上传 gate | `recovery record`, `first upload gate` | `sync.recovery_record_status`, `sync.recovery_first_upload_gate` | `recovery_record_not_created`, `blocked_until_recovery_code_saved` |
| restore 输入占位 | `code input` | `sync.recovery_restore_code_input` | `input_not_available_current_phase` |
| restore 记录查询占位 | `restore lookup` | `sync.recovery_restore_lookup_status` | `not_checked_current_phase` |
| restore 失败限速占位 | `restore attempt limit` | `sync.recovery_restore_attempt_limit` | `not_started` |
| restore 设备登记占位 | `restore device registration` | `sync.recovery_restore_device_registration` | `blocked_until_recovery_success` |

这些字段只允许输出状态码和确认边界码，例如 `setup_visible_text_status_codes_only`、`save_confirmation_ack_code_only` 和 `restore_visible_text_status_codes_only`。测试会用 fixture 反查当前 UI / diagnostics 字段，确认它们不包含 settings action payload、bridge request payload、请求 / 响应体、真实路径或 secret material。

future confirmation detail 当前固定为：

| detail | 绑定 action | 当前决策状态 | 允许证据 |
| --- | --- | --- | --- |
| `recovery_setup_save_confirmation_detail` | `recovery_setup` | `confirmation_not_available_current_phase` | `save_confirmation_ack_code_only`、`recovery_record_status_summary`、`first_upload_gate_summary` |
| `recovery_restore_attempt_confirmation_detail` | `recovery_restore` | `confirmation_not_available_current_phase` | `restore_attempt_status_code`、`recovery_record_lookup_summary`、`attempt_limit_status_summary`、`device_registration_status_summary` |
| `join_request_authorization_confirmation_detail` | `join_request_authorization` | `confirmation_not_available_current_phase` | `short_code_match_status_code`、`authorization_package_precondition_summary`、`explicit_authorization_ack_code_only`、`authorization_package_status_summary` |
| `device_revocation_confirmation_detail` | `device_revocation` | `confirmation_not_available_current_phase` | `explicit_revocation_ack_code_only`、`revocation_record_status_summary`、`key_epoch_status_summary` |

recovery detail 必须继续绑定当前 diagnostics 字段：`sync.recovery_setup_display_status`、`sync.recovery_setup_save_confirmation`、`sync.recovery_first_upload_gate`、`sync.recovery_restore_code_input`、`sync.recovery_restore_lookup_status`、`sync.recovery_restore_attempt_limit` 和 `sync.recovery_restore_device_registration`。device authorization detail 必须继续绑定 `sync.join_request_status`、`sync.device_join_short_code`、`sync.authorization_package_preconditions`、`sync.authorization_package_status`、`sync.authorization_package_blocker`、`sync.device_revocation_status`、`sync.device_revocation_active_requirement`、`sync.lost_device_risk`、`sync.key_epoch_status` 和 `sync.device_revocation_flow_status`。它们只能输出状态码，不输出恢复码、短码、token、signature、wrapped material、payload bytes、请求 / 响应体或真实路径。

设备 join / revocation confirmation detail 当前已固定到 fixture / model test 证据层，但仍是非执行状态码层，不是 UI 操作实现：

- join request 授权只能表达 `join_request_status`、`short_code_verification_status`、`authorization_package_preconditions`、显式确认状态和授权包状态摘要；不得显示短码内容、创建 join request、签名授权包或写入 wrapped material。
- 设备撤销只能表达 `target_device_status_summary`、`active_device_requirement`、`lost_device_risk_acknowledgement`、显式确认状态、撤销记录状态和 key epoch 状态摘要；不得执行撤销、签名 revocation record 或推进 key epoch。
- UI / diagnostics 字段应复用 `sync.device_join_*`、`sync.authorization_package_*`、`sync.device_revocation_*`、`sync.lost_device_risk` 和 `sync.key_epoch_status`，并只输出状态码、前置条件码、确认码和错误分类。

## 首台设备恢复码设置

当前关闭态：

| 字段 | 当前值 | 含义 |
| --- | --- | --- |
| `status` | `recovery_setup_flow_closed` | 首台设备恢复码设置流程未开放。 |
| `blocker` | `recovery_code_generation_closed` | 不允许生成恢复码。 |
| `entry_action_status` | `read_only_current_phase` | UI 只展示准备状态。 |
| `generated_code_status` | `not_generated` | 当前没有生成恢复码。 |
| `one_time_display_status` | `display_not_available_current_phase` | 当前不展示一次性恢复码，只展示状态码。 |
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
- 同步页和设置页 gate preview 展示同一组 readiness 聚合摘要、只读 action intent 进入计划、action command preview、request / result boundary、request / result allowed fields、forbidden material policy、错误分类和 `readiness_bridge_source` 来源标签，不提供操作按钮。
- 设置页 gate preview、同步页和诊断报告必须使用同一份 readiness 场景目录做代表场景回归，至少覆盖全部 ready 但当前阶段关闭、恢复记录缺失、join request 过期、未知 native 状态清洗和 unsafe redaction 拒绝。
- 诊断报告输出字段必须保留在 `sync_gate` 分组，只输出状态码、来源标签、前置条件码和错误码。
- `manager_sync_readiness.v1` 只作为 future bridge mapper 的非敏感输入形状，不改变 `ManagerBridge` contract，不新增 C ABI。
- `manager_sync_action_command_preview.v1` 只作为 future bridge 命令前的非执行协议预演，不改变 `ManagerBridge` contract，不新增 C ABI，不生成 request payload 或 result material。
- settings draft 不新增 secret 字段，不保存恢复码、短码、token、签名、wrapped material 或 payload bytes。
- widget fixture 必须使用合成设备、合成 endpoint 和状态码，不嵌入真实账号、真实路径、真实服务端响应或截图中的敏感内容。

## 进入真实实现前的停止线

- 平台私钥 backend production gate 未 ready 前，不创建恢复记录、join request、授权包或撤销记录。
- 发布级目标部署证据未形成前，不把同步展示为真实用户可用。
- 恢复码保存确认未完成前，不上传真实 P2 对象。
- join request、短码核对、授权包、撤销和 key epoch 错误语义未映射到 bridge 前，不开放对应操作。
- 任何明文恢复码、短码、token、私钥、signature bytes、wrapped material、payload bytes、请求 / 响应体或 provider 原始异常进入 UI、settings、诊断或 fixture，都必须停止并回退。
