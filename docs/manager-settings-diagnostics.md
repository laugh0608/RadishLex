# Manager Settings 与诊断报告字段参考

本文档是 Phase 4 Flutter manager 的 settings draft 与脱敏诊断报告字段参考，读者是维护 manager UI、Dart FFI bridge、测试和后续文档的人。本文只描述本地非 secret 草案格式、诊断报告字段、脱敏边界和验证口径；不定义真实远端同步协议、恢复码、设备授权 UI、Go server API 或 C ABI。

## 当前边界

- `ManagerBridge` contract 不暴露 settings JSON 字段级接口；UI 通过 `saveSettingsDraft` 保存完整草案。
- settings draft 只保存本地管理端非 secret 草案和 access token 存在性，不保存 token 文本、恢复码、私钥、signature bytes、wrapped material、payload bytes、证书、运行日志、文件路径或用户词条。
- 诊断报告只输出聚合计数、状态码、非敏感来源标签、聚合阻塞码和脱敏策略，不输出用户词、导入 / 导出文件内容、本机真实路径、请求 / 响应体或 native 原始错误明细。
- 诊断报告预览按本文字段索引展示分组、字段筛选和脱敏文本复制入口；复制内容与导出文本一致，仍只包含脱敏摘要。
- 设置页可以导入 `manager_sync_readiness.v1` 非敏感摘要用于本地开发联调；该摘要只保存在当前 manager 内存态，驱动同步页、设置页 gate preview 和诊断报告的派生字段，不写入 settings draft，也不改变 `ManagerBridge` contract 或 C ABI。
- 真实远端同步、恢复码和设备授权 UI 继续关闭；`preflight_ready` 只表示本地草案和预检条件可解释，不代表用户可用同步入口已开放。

## Settings Draft JSON

当前文件格式写入 `format_version: 1`。读取时只接受 `format_version == 1`；其他版本返回结构化 `settings_store_error`。

| 字段 | 类型 | 默认值 | 写入规则 | 说明 |
| --- | --- | --- | --- | --- |
| `format_version` | number | 必填 | 固定 `1` | settings draft 格式版本。 |
| `server_endpoint` | string | `""` | trim 后写入 | 自部署服务端草案。只接受 `http` / `https` URL，必须有 host，禁止 URL userinfo、query 和 fragment。 |
| `retain_sync_config` | bool | `false` | 原样写入 | 是否保留同步配置草案；为 `false` 时 `server_endpoint` 不参与 sync gate。 |
| `access_token_configured` | bool | `false` | 原样写入 | 是否已在本机安全位置配置 access token。只记录存在性，不保存 token 文本。 |
| `privacy_mode` | bool | `false` | 原样写入 | 为 `true` 时 sync gate 派生为 `sync_disabled_by_policy`。 |
| `diagnostics_export` | bool | `false` | 原样写入 | 是否允许管理端导出脱敏诊断摘要。 |
| `deployment_evidence_recorded` | bool | `false` | 原样写入 | 是否记录目标部署验证草案。必须与有效 `deployment_evidence_source` 同时成立才视为有部署证据。 |
| `deployment_evidence_source` | string | `""` | trim 后写入 | 目标部署验证来源标签，只允许 allowlist 值。 |

`deployment_evidence_source` allowlist：

| 值 | UI 标签 | 含义 |
| --- | --- | --- |
| `local_smoke` | `local smoke` | 本地短生命周期 smoke 已记录。 |
| `external_tls` | `external TLS` | 外部 TLS / 反代验证证据已记录。 |
| `backup_restore` | `backup restore` | 备份恢复演练证据已记录。 |
| `upgrade_rollback` | `upgrade rollback` | 升级 / 回滚演练证据已记录。 |

部署证据来源标签与 `docs/runbooks/sync-server-production-deployment.md` 的目标部署证据包对齐，但 settings draft 只保存一个非敏感 allowlist 标签，不保存证据包正文。证据包正文进入交接材料前可用 `./scripts/check-sync-deployment-evidence.sh <evidence-file>` 校验格式和脱敏规则；通过后只能交接 `--summary-json` / `--summary-text` 导出的 `deployment_evidence_summary.v1` 非敏感摘要。该校验和摘要不改变 settings draft 只保存标签的边界。`local_smoke` 足以支撑当前开发期同步入口状态、阻塞说明和本地联调展示，不能代表目标部署可开放给真实用户；用户可用同步仍需要发布级部署证据、平台私钥 backend、恢复码和设备授权链路齐备。

settings draft 不得保存：

- 证据包全文、运行日志、curl 输出、请求体、响应体或 Nginx access log。
- `deployment_evidence_summary.v1` 之外的自由文本摘要、证据文件路径或 `notes`。
- 真实 token、URL query token、恢复码、私钥、signature bytes、wrapped material bytes、encrypted payload bytes。
- 证书正文、证书私钥、宿主机绝对路径、真实账号、真实用户词或完整服务端 URL 中的 credential。

诊断报告只能展示 `deployment_evidence_source` 的 allowlist 值或 `not_recorded`，以及 `deployment evidence missing` / `deployment evidence <label>` 这类摘要。诊断报告不得把目标部署证据包展开成逐项运行记录。

兼容规则：

- 旧 v1 文件如果缺少 `deployment_evidence_source`，即使 `deployment_evidence_recorded` 为 `true`，读取后也降级为未记录部署证据。
- 未知字段当前忽略；新增字段前必须先更新本文档和对应测试。
- 非 bool / 非 string 字段类型会返回 `settings_store_error`。
- 非 allowlist evidence source、带 userinfo / query / fragment 的 URL、缺少 scheme / host 的 URL 会返回 `invalid_argument`。

## Sync Gate 派生

当前 manager UI 先从 settings draft 与设备签名摘要派生 `SyncEntryState`，再映射到既有 `SyncUiState`：

1. `privacy_mode == true`：`entry_state = sync_disabled_by_policy`，`sync.state = sync_disabled_by_policy`。
2. `retain_sync_config == false` 或 `server_endpoint` 为空：`entry_state = local_only`，`sync.state = local_only`。
3. `device.production_gate != "ready"`：`entry_state = backend_unavailable`，`sync.state = backend_unavailable`。
4. `deployment_evidence_recorded` 未与有效 `deployment_evidence_source` 同时成立：`entry_state = deployment_unverified`，`sync.state = deployment_unverified`。
5. `deployment_evidence_source == local_smoke` 且前置项通过：`entry_state = local_smoke_ready`，`sync.state = preflight_ready`。
6. 非本地 evidence source 且前置项通过：`entry_state = blocked_before_user_sync`，`sync.state = preflight_ready`。

`sync.entry_blocker` 记录当前第一阻塞码，`sync.production_blockers` 记录聚合阻塞码。即使状态进入 `preflight_ready`，同步页的 `启用同步` 主按钮仍保持禁用。用户可用同步入口必须等待可用平台私钥 backend、发布级目标部署运行证据、恢复码和设备授权链路满足对应停止线。

当前 `sync.production_blockers` 还会聚合恢复码保存确认、恢复记录、授权包前置条件、设备撤销、丢失设备风险提示和 key epoch 状态，例如 `recovery_record_not_created`、`recovery_code_save_confirmation_required`、`authorization_package_prerequisites_blocked`、`lost_device_risk_notice_required` 和 `key_epoch_rotation_not_started`。恢复码 setup / restore 与设备 join / revocation 的 readiness 摘要只输出状态码、前置条件和错误分类，并通过 `SyncReadinessFlowSummary` 聚合为 blocked flows、issue codes、next required evidence、source tags 和 user sync blocked；这些值只用于解释入口阻塞，不代表已创建恢复记录、join request、授权包或撤销记录。`SyncInteractionEntryPlan` / `SyncInteractionActionIntent` 会把四条未来操作入口额外汇总为 action id、visibility、intent status、blocker、required evidence 和 source tag；这些字段只描述非执行进入计划，不代表按钮可点击或 bridge 操作已开放。Dart 侧 `manager_sync_readiness.v1` mapper 只接受 allowlist 状态码和来源标签，未知值降级为 `unexpected_bridge_error_code` / `unexpected_bridge_required_evidence`，不把原始 bridge 异常、路径、token、恢复码、短码或 payload 写入 UI、settings draft 或诊断报告。

设置页提供开发期“同步 readiness 摘要导入”入口，接受 `manager_sync_readiness.v1` JSON 摘要并只保留映射后的 `ManagerSyncReadinessBridgeSnapshot` 内存态。导入前必须检查 `format == manager_sync_readiness.v1` 和 `redaction_policy == summary_only_no_tokens_recovery_secret_or_payload_bytes`；不符合时只显示结构化错误码，例如 `readiness_summary_format_unsupported` 或 `readiness_summary_redaction_policy_unsupported`，不得应用到 UI 状态。Dart mapper 暴露同一套导入校验结果，合法样本、不支持格式、不安全 redaction 和未知状态码降级都应有 fixture / helper 覆盖，避免未来 bridge 接线时绕过 UI 预检；开发期场景目录见 [`docs/manager-readiness-scenarios.md`](manager-readiness-scenarios.md)。导入成功后，设置页、同步页和诊断报告只展示派生后的 readiness source、blocked flows、issue codes、next evidence、interaction plan 和 user sync blocked，不展示原始 JSON，也不持久化恢复码、短码、token、私钥、签名、wrapped material、payload bytes、请求 / 响应体或路径。诊断报告字段回归应遍历同一份 readiness 场景目录，确认 `sync.readiness_*`、`sync.interaction_*` 和 `sync.user_sync_enabled` 与同步页、settings gate preview 不分叉。清除导入摘要后回到 `manager_default_closed_readiness`。

## 连接健康摘要

Manager 当前只派生和展示本地连接健康摘要，不在 UI 中发起远端上传 / 下载。连接健康模型从 settings draft 读取 endpoint 和 access token 存在性，也可以从 `sync_connection_health.v1` 非敏感摘要映射只读探测结果。输出状态码：

- `not_configured`：未保留同步配置或 endpoint 为空。
- `sync_disabled_by_policy`：隐私模式禁用连接检查。
- `endpoint_invalid`：endpoint 缺少 scheme / host，或包含 userinfo、query、fragment、非 http(s) scheme。
- `access_token_missing`：endpoint 已配置但未记录 access token 存在性。
- `local_https_ready_for_probe`：本地 HTTPS endpoint 与 access token 存在性已满足，可执行只读探测。
- `local_http_ready_for_probe`：本地 HTTP endpoint 与 access token 存在性已满足，仅用于短生命周期本地开发探测。
- `external_probe_deferred`：外部 HTTPS 目标探测后置到正式发布 / 真实用户开放前。
- `unsupported_transport`：非本地 HTTP 被拒绝。
- `reachable`：只读探测已到达服务；若 `connection_blocker = none`，表示连接健康摘要可用于本地联调展示。
- `network_unreachable`：只读探测无法连接到目标端口或网络。
- `tls_error`：只读探测遇到 TLS 握手或信任链错误。
- `reachable_with_unexpected_status`：只读探测到达服务但返回了非预期 HTTP 状态。
- `probe_summary_invalid`：摘要格式或脱敏策略不符合 Manager allowlist。

同步页的“服务连接健康”和设置页的“同步门禁草案”只展示 `connection_status`、`connection_blocker`、`endpoint_status`、`access_token_status`、`transport_mode`、`server_state_status`、`last_remote_error_code`、`connection_probe_source`、`connection_probe_recorded_at`、`auth_status`、`http_status`、`http_status_class` 和 `local_insecure_tls`。这些字段不得包含完整 endpoint、token、请求 / 响应体、证书、真实路径或 payload bytes。

本地 Docker / 本地 HTTPS 服务启动后，可使用 `./scripts/check-sync-server-connection-health.sh` 采集 `sync_connection_health.v1` 非敏感摘要。脚本只执行 `GET /api/v1/domains/<probe>/state` 读请求：`404 not_found` 表示服务和认证路径可达且 probe domain 不存在，`401 unauthenticated` 表示访问控制门禁可达但 token 缺失或失败。脚本输出不得写入 token、endpoint credential 或响应体正文。Manager 只接受约定 allowlist 字段；未知远端错误码会降为 `unexpected_remote_error_code`。

设置页提供开发期“连接健康摘要回填”入口，接受 `sync_connection_health.v1` JSON 摘要并只保存净化后的 `sync_connection_health_summary` 子对象。该对象不保存原始 JSON、完整 endpoint、token、请求 / 响应体、证书或 payload bytes；非法 JSON 只显示结构化错误码，不进入 settings draft。

## 诊断报告格式

当前诊断报告文本格式为 `manager.diagnostics.v1`，`redaction_policy` 固定为 `summary_only_no_terms_paths_tokens_or_payload_bytes`。

Manager UI 预览会保留完整脱敏文本，并额外按 `runtime`、`settings_draft`、`local_data`、`latest_import_batch`、`sync_gate` 和 `redaction` 分组展示字段。预览筛选只作用于本地 UI，不改变 `ManagerDiagnosticsReport` 数据模型、导出格式或 `ManagerBridge` contract。

## 预览、筛选、复制与导出一致性

诊断报告预览是同一份脱敏报告的可读索引，不是新的报告格式：

- section 筛选只在 `runtime`、`settings_draft`、`local_data`、`latest_import_batch`、`sync_gate` 和 `redaction` 分组之间切换可见字段。
- 关键字筛选只匹配 section title、字段 key、字段 value 和字段 kind，用于快速定位字段；筛选不删除报告内容。
- `脱敏文本` 区域始终显示 `ManagerDiagnosticsReport.toRedactedText()` 的完整结果。
- 复制按钮始终复制完整 `toRedactedText()`，不受当前 section 或关键字筛选影响。
- 导出入口必须导出与 `toRedactedText()` 同语义的脱敏摘要，不允许因为当前筛选状态导出字段子集。
- 预览、复制和导出都不得输出用户词、导入 / 导出文件内容、真实路径、token、恢复码、私钥、signature bytes、wrapped material bytes 或 encrypted payload bytes。
- 如果后续新增诊断字段，必须先补本文字段索引、脱敏说明和 widget / smoke 覆盖，再接入预览或导出。
- 如果 settings 页导入了 `manager_sync_readiness.v1` 摘要，预览和导出必须使用当前内存态 snapshot 生成同一份派生诊断摘要；没有导入摘要时仍可使用 bridge 默认诊断出口。

报告头：

| 字段 | 说明 |
| --- | --- |
| `format` | 诊断报告格式版本，当前为 `manager.diagnostics.v1`。 |
| `generated_at` | manager snapshot 的生成时间摘要。 |
| `redaction_policy` | 当前脱敏策略标识。 |

## 诊断字段索引

### `runtime`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `runtime.bridge_mode` | `configuration` | fixture、Dart FFI 或 injected native binding 摘要。 |
| `runtime.userdb` | `configuration` | userdb 配置来源摘要，不输出真实路径。 |
| `runtime.native_library` | `configuration` | native library 配置来源摘要，不输出真实路径。 |
| `runtime.settings_store` | `configuration` | settings store 来源摘要。 |
| `runtime.sync_endpoint` | `configuration` | sync endpoint 是否配置的摘要，不输出 URL token 或凭据。 |
| `runtime.last_error_code` | `error_code` | 最近一次结构化错误码，默认 `none`。 |

### `settings_draft`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `settings.retain_sync_config` | `configuration` | 是否保留同步配置草案。 |
| `settings.server_endpoint` | `configuration` | `configured` 或 `not_configured`，不输出完整 endpoint。 |
| `settings.access_token` | `configuration` | `configured` 或 `not_configured`，只表示 access token 存在性。 |
| `settings.privacy_mode` | `configuration` | 隐私模式草案。 |
| `settings.diagnostics_export` | `configuration` | 诊断导出草案。 |
| `settings.deployment_evidence` | `configuration` | 部署证据标签摘要，例如 `deployment evidence external TLS` 或 `deployment evidence missing`。 |
| `settings.deployment_evidence_source` | `configuration` | allowlist source 或 `not_recorded`。 |

### `local_data`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `local.user_terms` | `aggregate_count` | 本地用户词条聚合计数。 |
| `local.deleted_terms` | `aggregate_count` | 删除 tombstone 聚合计数。 |
| `local.selection_events` | `aggregate_count` | P1 selection event 聚合计数，不输出原始事件。 |
| `local.suppressed_terms` | `aggregate_count` | 抑制词聚合计数。 |
| `local.import_batches` | `aggregate_count` | 导入批次聚合计数。 |

### `latest_import_batch`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `import_batch.present` | `aggregate_count` | 是否存在导入批次。 |
| `import_batch.total_records` | `aggregate_count` | 最新批次总记录数。 |
| `import_batch.imported_terms` | `aggregate_count` | 最新批次导入词条数。 |
| `import_batch.inserted_terms` | `aggregate_count` | 最新批次新增词条数。 |
| `import_batch.updated_terms` | `aggregate_count` | 最新批次更新词条数。 |
| `import_batch.skipped_deleted_terms` | `aggregate_count` | 因 tombstone 跳过的词条数。 |
| `import_batch.skipped_duplicate_terms` | `aggregate_count` | 因重复跳过的词条数。 |

### `sync_gate`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `sync.state` | `gate` | 当前 sync UI 状态码。 |
| `sync.state_label` | `gate` | 用户可见状态标签。 |
| `sync.state_source` | `gate` | 状态来源说明。 |
| `sync.entry_state` | `gate` | 细分入口状态码，例如 `backend_unavailable`、`local_smoke_ready` 或 `blocked_before_user_sync`。 |
| `sync.entry_blocker` | `gate` | 当前第一阻塞码，例如 `backend_unavailable`、`release_deployment_evidence_required` 或 `recovery_code_flow_closed`。 |
| `sync.local_evidence_source` | `gate` | deployment evidence allowlist source 或 `not_recorded`。 |
| `sync.production_blockers` | `gate` | 聚合阻塞码列表，不含证据包正文、token、恢复码、签名或 payload bytes。 |
| `sync.readiness_blocked_flows` | `gate` | 当前阻塞用户同步的 readiness flow id 列表。 |
| `sync.readiness_issue_codes` | `error_code` | 四条 readiness flow 的 blocker 与错误分类去重摘要。 |
| `sync.readiness_next_required_evidence` | `gate` | 四条 readiness flow 的下一步前置条件 / 状态证据去重摘要。 |
| `sync.readiness_source_tags` | `gate` | readiness flow 摘要来源标签，只包含 Manager 内部只读 model source。 |
| `sync.readiness_bridge_source` | `gate` | readiness bridge snapshot 来源标签；默认 `manager_default_closed_readiness`，未来 bridge 输入只能使用 allowlist 或安全降级标签。 |
| `sync.readiness_user_sync_blocked` | `gate` | readiness flow 是否仍阻断用户可用同步入口。 |
| `sync.interaction_actions` | `gate` | 四条未来交互入口 action id 摘要：`recovery_setup`、`recovery_restore`、`join_request_authorization` 和 `device_revocation`。 |
| `sync.interaction_visibility` | `gate` | action intent 可见性摘要；当前均为只读可见的 `visible`。 |
| `sync.interaction_statuses` | `gate` | action intent 状态摘要，例如 `closed_current_phase`、`blocked` 或 `requires_confirmation`；当前不表示可执行操作。 |
| `sync.interaction_blockers` | `gate` | action intent 阻塞码摘要，例如 `recovery_code_generation_closed` 或 `user_sync_entry_closed_current_phase`。 |
| `sync.interaction_required_evidence` | `gate` | action intent 下一步前置条件 / 停止线摘要；不包含恢复码、短码、token 或 payload。 |
| `sync.interaction_source_tags` | `gate` | action intent 来源标签，来自同一组 readiness source tag。 |
| `sync.user_sync_enabled` | `gate` | 当前用户可用真实同步入口是否开放；当前阶段应为 `false`。 |
| `sync.recovery_status` | `gate` | 恢复码流程结构化状态；当前为 `recovery_code_flow_closed`。 |
| `sync.recovery_blocker` | `gate` | 恢复码流程当前阻塞码；当前为 `recovery_code_flow_closed`。 |
| `sync.recovery_save_confirmation` | `gate` | 首台设备上传前的恢复码保存确认要求；当前为 `required_before_first_upload`。 |
| `sync.recovery_record_status` | `gate` | 恢复记录状态摘要；当前为 `recovery_record_not_created`。 |
| `sync.recovery_record_blocker` | `gate` | 恢复记录创建阻塞码；当前为 `recovery_record_creation_closed`。 |
| `sync.recovery_first_upload_gate` | `gate` | 首次上传 P2 对象前的恢复码门禁；当前为 `blocked_until_recovery_code_saved`。 |
| `sync.recovery_readiness_blockers` | `gate` | 恢复码准备清单聚合阻塞码，不含恢复码明文或恢复材料。 |
| `sync.recovery_setup_status` | `gate` | 首台设备恢复码设置流程状态；当前为 `recovery_setup_flow_closed`。 |
| `sync.recovery_setup_blocker` | `gate` | 首台设备恢复码设置阻塞码；当前为 `recovery_code_generation_closed`。 |
| `sync.recovery_setup_action_status` | `gate` | 恢复码设置入口动作状态；当前为 `read_only_current_phase`。 |
| `sync.recovery_setup_prerequisites` | `gate` | 恢复码设置前置条件摘要，不含恢复码或私钥材料。 |
| `sync.recovery_setup_error_codes` | `error_code` | 恢复码设置需要覆盖的错误分类摘要。 |
| `sync.recovery_restore_status` | `gate` | 恢复码恢复新设备流程状态；当前为 `recovery_restore_flow_closed`。 |
| `sync.recovery_restore_blocker` | `gate` | 恢复码恢复新设备阻塞码；当前为 `recovery_code_input_closed`。 |
| `sync.recovery_restore_code_input` | `gate` | 恢复码输入入口状态；当前为 `input_not_available_current_phase`。 |
| `sync.recovery_restore_error_codes` | `error_code` | 恢复码恢复新设备需要覆盖的错误分类摘要。 |
| `sync.device_authorization_status` | `gate` | 设备授权流程结构化状态；当前为 `device_authorization_flow_closed`。 |
| `sync.device_authorization_blocker` | `gate` | 设备授权流程当前阻塞码；当前为 `device_authorization_flow_closed`。 |
| `sync.join_request_status` | `gate` | 加入请求结构化状态；当前为 `join_request_unavailable`。 |
| `sync.authorization_package_status` | `gate` | 授权包只读状态；当前为 `authorization_package_not_created`。 |
| `sync.authorization_package_blocker` | `gate` | 授权包前置条件阻塞码；当前为 `authorization_package_prerequisites_blocked`。 |
| `sync.authorization_package_preconditions` | `gate` | 授权包前置条件摘要；当前只包含 active device、pending join request 和短码核对这类状态码，不含短码或 payload。 |
| `sync.device_revocation_status` | `gate` | 设备撤销入口状态；当前为 `closed_current_phase`。 |
| `sync.lost_device_risk` | `gate` | 丢失设备风险提示摘要；当前为 `lost_device_prior_material_not_recallable`。 |
| `sync.key_epoch_status` | `gate` | 后续 key epoch 推进状态；当前为 `key_epoch_rotation_not_started`。 |
| `sync.device_authorization_readiness_blockers` | `gate` | 设备授权准备清单聚合阻塞码，不含短码、签名或 wrapped material。 |
| `sync.device_join_status` | `gate` | 设备加入流程状态；当前为 `device_join_flow_closed`。 |
| `sync.device_join_blocker` | `gate` | 设备加入流程阻塞码；当前为 `join_request_creation_closed`。 |
| `sync.device_join_action_status` | `gate` | 设备加入入口动作状态；当前为 `read_only_current_phase`。 |
| `sync.device_join_short_code` | `gate` | 短码核对状态；当前为 `short_code_verification_not_started`，不输出短码。 |
| `sync.device_join_error_codes` | `error_code` | 设备加入和授权包前置条件需要覆盖的错误分类摘要。 |
| `sync.device_revocation_flow_status` | `gate` | 设备撤销流程状态；当前为 `device_revocation_flow_closed`。 |
| `sync.device_revocation_blocker` | `gate` | 设备撤销流程阻塞码；当前为 `device_revocation_flow_closed`。 |
| `sync.device_revocation_active_requirement` | `gate` | 发起撤销的活跃设备要求；当前为 `active_existing_device_required`。 |
| `sync.device_revocation_error_codes` | `error_code` | 设备撤销和 key epoch 推进需要覆盖的错误分类摘要。 |
| `sync.connection_status` | `gate` | 服务连接健康状态码，例如 `access_token_missing`、`local_https_ready_for_probe` 或 `external_probe_deferred`。 |
| `sync.connection_blocker` | `gate` | 服务连接健康阻塞码，例如 `access_token_missing` 或 `read_only_probe_not_run`。 |
| `sync.connection_probe_source` | `gate` | 回填摘要来源，例如 `local_docker_https`、`local_http`、`external_https_probe` 或 `not_recorded`。 |
| `sync.connection_probe_recorded_at` | `timestamp` | 回填摘要记录时间；未回填时为 `not_recorded`。 |
| `sync.endpoint_status` | `gate` | endpoint 配置状态，例如 `configured`、`not_configured`、`invalid_userinfo`。 |
| `sync.access_token_status` | `gate` | access token 存在性，`configured` 或 `not_configured`。 |
| `sync.transport_mode` | `gate` | endpoint 传输分类，例如 `local_https`、`local_http`、`external_https` 或 `remote_http`。 |
| `sync.server_state_status` | `gate` | 服务状态摘要，例如 `read_only_probe_pending`、`not_checked_access_token_missing` 或脚本输出的 `domain_missing_expected`。 |
| `sync.connection_auth_status` | `gate` | 只读探测认证摘要，例如 `not_required_for_local_probe`、`required`、`accepted` 或 `not_checked`。 |
| `sync.connection_http_status` | `status_code` | 只读探测 HTTP 状态码；未探测时为 `0`。 |
| `sync.connection_http_status_class` | `gate` | 只读探测 HTTP 状态分类，例如 `client_error`、`network_error` 或 `not_checked`。 |
| `sync.connection_local_insecure_tls` | `gate` | 本地 TLS 信任策略摘要，例如 `allowed` 或 `system_trust`。 |
| `sync.last_remote_error_code` | `error_code` | 最近一次远端错误分类；未探测时为 `none` 或配置类错误码。 |
| `sync.action_stop_line` | `gate` | 当前真实同步入口停止线。 |
| `sync.deployment_evidence` | `gate` | 部署证据标签摘要。 |
| `sync.syncable_objects` | `aggregate_count` | 可同步 P2 对象聚合计数。 |
| `sync.local_only_events` | `aggregate_count` | 只本地保留事件聚合计数。 |
| `device.backend` | `gate` | 设备签名 backend id 摘要。 |
| `device.capability` | `gate` | backend capability 状态摘要。 |
| `device.production_gate` | `gate` | production gate 原始状态摘要。 |
| `device.production_gate_label` | `gate` | production gate 用户可见标签。 |

### `redaction`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `redaction.user_terms` | `policy` | 用户词条已省略。 |
| `redaction.file_paths` | `policy` | 文件路径已省略。 |
| `redaction.tokens` | `policy` | token 已省略。 |
| `redaction.payload_bytes` | `policy` | payload bytes 已省略。 |

## 禁止输出

settings draft、诊断报告、测试 fixture 和截图均不得包含：

- 用户输入历史、候选明细、联系人、窗口标题、真实用户词条或完整 userdb dump。
- 导入文件原文、导出文件内容、真实本机路径、真实账号、真实 token、恢复码、私钥、签名 bytes、wrapped material bytes、encrypted payload bytes。
- Go server 请求体 / 响应体、内部 `blob_ref`、证书正文、日志正文或 provider exception 原文。

## 验证入口

当前字段和边界由以下验证覆盖：

```text
flutter test
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
git diff --check
./scripts/check-repo.sh
```

关键测试覆盖：

- settings draft v1 写入 / 读取、access token 存在性持久化、旧 v1 缺 `deployment_evidence_source` 降级、未知格式拒绝、非法 URL 和非法 evidence source 拒绝。
- Dart helper 覆盖隐私策略、backend gate、本地 `local_smoke`、非本地 evidence source、恢复码关闭状态、setup / restore readiness、保存确认要求、恢复记录未创建、设备授权关闭状态、join / revocation readiness、join request 不可用、授权包前置条件、撤销 / 丢失设备、key epoch 风险提示和只读 action intent 进入计划派生。
- 连接健康 helper 覆盖 endpoint 缺失、URL userinfo 拒绝、access token 缺失、本地 HTTPS 可探测、外部 HTTPS 探测后置、`sync_connection_health.v1` 摘要回填、未知远端错误码净化和 settings draft 持久化。
- 设置页 deployment evidence source 下拉、`deployment_unverified` 到 `preflight_ready` 的本地草案派生、真实同步按钮继续禁用。
- 同步页展示 `sync.entry_state`、`sync.entry_blocker`、`sync.local_evidence_source`、`sync.production_blockers`、readiness 聚合摘要、只读 action intent 进入计划、`sync.user_sync_enabled`、服务连接健康、恢复码 setup / restore、恢复记录、保存确认、设备 join / revocation、授权包前置条件、丢失设备风险和 key epoch 状态，真实同步按钮继续禁用。
- 诊断报告包含 gate source / stop line / evidence source / entry gate / readiness 聚合摘要 / interaction intent 摘要 / connection health / recovery setup / recovery restore / device join / device revocation / authorization package / lost device / key epoch 摘要，并保持用户词、路径、token 和 payload bytes 脱敏。
- FFI smoke 使用临时 SQLite userdb、临时 settings JSON 和合成数据复验真实 Dart FFI bridge，不连接真实同步后端。

涉及目标部署证据包格式、摘要或交接材料时，追加 `./scripts/check-sync-deployment-evidence.sh --self-test`、对应证据文件校验和 `--summary-json` 摘要输出检查。
