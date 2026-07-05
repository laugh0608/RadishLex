# Flutter manager 真实同步入口前置边界

本文档定义 Flutter manager 进入真实远端同步、恢复码和设备授权 UI / bridge 前必须满足的交互边界、状态门禁、错误语义和测试计划。读者是维护 `apps/radishlex-manager`、`ime-ffi` 管理接口、`ime-sync` 客户端接线和审阅隐私边界的协作者。本文不定义 Go server API 字段全集、加密协议细节、平台私钥 backend 实现、OIDC 登录、视觉稿或真实平台输入法壳。

## 当前结论

Phase 4 manager 本地验收已经有可复验证据。2026-07-05 阶段口径调整后，真实域名、正式证书和外部反代复验不再阻塞当前产品开发；它们保留为正式发布 / 真实用户开放前门禁。真实同步入口的下一步不是直接启用上传 / 下载按钮，而是在本地 Docker / 本地 HTTPS 证据支撑下，先把恢复码、设备授权、部署证据来源和平台私钥 backend 的进入条件固定为可测试的 UI / bridge 边界。

当前仍保持：

- 不打开真实远端同步主操作。
- 不提供恢复码生成、输入、轮换或撤销 UI。
- 不提供设备加入请求审批、授权成功路径或设备撤销 UI。
- 不新增明文同步 payload、恢复码、私钥、signature bytes、wrapped material bytes、encrypted payload bytes 的日志、诊断字段或 widget 可见数据。

2026-07-05 已落地 manager 侧 `SyncEntryState` / `ManagerSyncEntryGate` 的非上传实现，并补齐连接健康、`sync_connection_health.v1` 摘要回填、恢复码准备态、设备授权准备态、join request 状态、恢复码保存确认、恢复记录状态、授权包前置条件、设备撤销 / 丢失设备和 key epoch 风险提示的只读模型 / UI。恢复码 setup / restore 与设备 join / revocation 已拆成 `RecoverySetupReadiness`、`RecoveryRestoreReadiness`、`DeviceJoinReadiness` 和 `DeviceRevocationReadiness` 四组只读摘要，并通过 `SyncReadinessFlowSummary` 聚合为同一组 blocked flows、issue codes、next required evidence、source tags 和 user sync blocked 字段。Dart 侧已新增 `ManagerSyncReadinessBridgeSnapshot` 和 `manager_sync_readiness.v1` mapper，把未来 bridge 可返回的非敏感 readiness 状态码、前置条件和错误分类映射回同一组只读 model；未知值降级为 `unexpected_bridge_error_code` / `unexpected_bridge_required_evidence`，不透传 provider 原始异常、路径、token、恢复码、短码或 payload。`SyncInteractionEntryPlan` / `SyncInteractionActionIntent` 已把四条未来交互入口整理为可见性、intent status、blocker、required evidence 和 source tag 的非执行计划；该计划只用于同步页、设置页和诊断摘要展示，不创建恢复码、join request、授权包或撤销记录。同步页、设置页草案预览和诊断报告现在可以从 settings draft、本地 `local_smoke` 来源、平台 backend gate、endpoint / access token 存在性、连接健康回填摘要、恢复码关闭状态、设备授权关闭状态和可选 bridge readiness snapshot 派生阻塞说明；真实上传、恢复码生成、恢复码输入、join request 创建、授权成功和设备撤销路径仍关闭。

## 适用范围

纳入本文：

- Flutter manager 可展示的真实同步准备状态。
- 恢复码和设备授权进入 UI 前的用户路径。
- `ManagerBridge` / `ime-ffi` 后续可扩展的同步入口边界。
- 诊断、错误分类、测试和验收顺序。

不纳入本文：

- `SyncMasterKey`、object key、AAD、nonce、KDF 参数和签名格式的协议细节。
- Go server handler、SQLite migration、object storage 和 access token 的字段全集。
- 平台 Keychain / Keystore 的具体实现。
- 真实平台输入法壳或输入热路径。
- OIDC / Radish 产品账号体系。

协议和存储细节继续以 `docs/sync-key-management.md`、`docs/production-recovery-flow.md`、`docs/sync-server-api-storage.md`、`docs/crypto-boundary.md` 和相关 ADR 为准。恢复码与设备授权只读交互状态机见 `docs/manager-recovery-device-auth-flow.md`。

## 进入条件

真实同步入口进入用户可用状态前，至少需要这些证据同时成立：

| 证据 | 要求 | 当前状态 |
| --- | --- | --- |
| 平台私钥 backend | 生产签名 backend 可在目标平台创建、加载、签名和删除非导出设备签名 key，且 capability / production gate 可被 manager 读取。 | 未满足；Apple 与 Android backend 均未解除 production gate。 |
| 部署运行证据 | 当前开发联调用本地 Docker、本地 HTTPS、短生命周期数据目录和 `local_smoke` 来源；正式发布 / 真实用户开放前再补目标环境外部 TLS、访问控制失败响应、备份恢复、升级回滚和日志脱敏复验。 | 本地 Docker / 本地 HTTPS 和多类 runtime smoke 已有证据；真实目标部署证据未形成，但不再阻塞 sync entry state helper / UI gate 的非上传开发。 |
| 连接健康 | Manager 只记录 endpoint 配置状态、access token 存在性、transport 分类、server state 摘要和非敏感错误码；本地服务启动后可用脚本采集 `sync_connection_health.v1`。 | 只读模型、同步页 section、settings 预览、诊断字段、脚本自测、`sync_connection_health.v1` 摘要映射和 settings draft 回填入口已接入；2026-07-05 已启动本地 Docker / 本地 HTTPS 服务采集非敏感本地运行摘要。 |
| 恢复码交互边界 | 恢复码只显示一次、用户确认保存、恢复记录创建 / 轮换 / 撤销、失败限速和日志脱敏测试齐备。 | 只读准备态已展示 `recovery_code_flow_closed`、`required_before_first_upload`、`recovery_record_not_created`、`recovery_record_creation_closed` 和 `blocked_until_recovery_code_saved`；setup / restore 子流程已输出 `recovery_setup_flow_closed`、`recovery_restore_flow_closed`、前置条件和错误分类；生成、输入、轮换和撤销交互仍未开始。 |
| 设备授权交互边界 | join request、短码核对、授权包、设备撤销、lost device 和 key epoch 说明能被 UI 表达并测试。 | 只读准备态已展示 `device_authorization_flow_closed`、`join_request_unavailable`、`authorization_package_not_created`、`authorization_package_prerequisites_blocked`、`lost_device_prior_material_not_recallable` 和 `key_epoch_rotation_not_started`；join / revocation 子流程已输出 `device_join_flow_closed`、`device_revocation_flow_closed`、短码核对状态和错误分类；加入请求、授权成功和撤销交互仍未开始。 |
| 客户端同步操作 | Flutter 只调用结构化 bridge；Rust 侧只接收 encrypted object 与 signed manifest，不暴露 plaintext payload。 | Rust / Go 侧已有底层证据；manager 真实远端操作入口未接。 |
| 诊断脱敏 | 诊断报告只输出状态、来源标签、聚合计数和脱敏策略，不输出 secret 或 payload bytes。 | sync entry gate 摘要字段已接入并有 widget / bridge 测试覆盖；后续真实同步字段新增前仍需补测试。 |

任一生产证据缺失时，manager 只能展示准备状态、不可用原因和本地预检摘要，不得提供会上传真实 P2 数据的主操作。部署证据缺失不阻止本地开发期状态派生、阻塞说明、诊断脱敏和本地联调入口。

目标部署证据包应先通过 `./scripts/check-sync-deployment-evidence.sh <evidence-file>`，并只通过 `deployment_evidence_summary.v1` 非敏感摘要交接给后续 UI / bridge 设计；证据包正文仍不进入 Flutter widget 状态、settings JSON 或诊断报告。manager 可以展示本地开发期 `local_smoke`，以及发布前已校验证据摘要中的 `external_tls`、`backup_restore`、`upgrade_rollback` 这类 allowlist 标签和聚合状态；证书、token、日志正文、请求 / 响应体、真实路径、`notes` 和 payload bytes 都必须留在 UI / bridge / 诊断之外。`local_smoke` 足以支撑本地同步开发和联调，不足以开放真实用户同步。

2026-07-05 复查确认当前仓库内没有真实目标环境产生的 `deployment_evidence.v1`，只有合成 fixture 和校验 / 摘要工具。本状态只能记录为发布级目标部署证据缺失；不得把 fixture 摘要映射为真实用户同步证据，但可以继续推进本地 `sync entry state` 派生、阻塞说明和 UI gate。

## 状态门禁

现有 manager sync gate 状态继续保留，并按以下含义约束：

| 状态 | 含义 | 用户可做 | 禁止行为 |
| --- | --- | --- | --- |
| `local_only` | 未保留同步配置或未配置 server endpoint。 | 管理本地词库、查看本地预检。 | 上传 / 下载、恢复码、设备授权。 |
| `sync_disabled_by_policy` | 隐私模式或策略禁用同步。 | 查看禁用原因、继续本地管理。 | 任何远端同步主操作。 |
| `backend_unavailable` | 平台私钥 backend 不可用于生产签名。 | 查看 backend id、capability 和 production gate 原因。 | 生成恢复码、签名授权包、上传对象。 |
| `deployment_unverified` | 自部署目标尚无发布级运行证据，或只有本地 `local_smoke`。 | 查看本地联调来源和发布前缺口。 | 把远端同步显示为生产可用。 |
| `preflight_ready` | 本地 P2 对象、settings draft、本地 smoke 来源和预检解释齐备，但真实同步入口仍未开放。 | 查看下一项阻塞证据，执行本地联调范围内的非上传检查。 | 启用真实同步主操作、恢复码和设备授权成功路径。 |
| `ready_for_user_sync` | 平台 backend、目标部署、恢复码、设备授权和测试证据都满足。 | 后续可进入真实同步设置流程。 | 跳过恢复码确认、跳过设备授权、绕过 bridge。 |

`preflight_ready` 不是用户可用同步状态。它只说明本地草案和预检可以解释，不代表可以对真实远端上传数据。

当前 Flutter manager 还维护更细的 `sync.entry_state` 摘要，供 UI 和诊断展示：

| Entry state | 映射 | 含义 |
| --- | --- | --- |
| `local_only` | `sync.state = local_only` | 未保留自部署服务端草案。 |
| `sync_disabled_by_policy` | `sync.state = sync_disabled_by_policy` | 隐私模式或策略禁用同步。 |
| `backend_unavailable` | `sync.state = backend_unavailable` | 平台私钥 backend production gate 未 ready。 |
| `deployment_unverified` | `sync.state = deployment_unverified` | settings draft 没有有效部署证据来源标签。 |
| `local_smoke_ready` | `sync.state = preflight_ready` | 本地 Docker / 本地 HTTPS smoke 可支撑开发联调，但仍缺发布级部署证据。 |
| `blocked_before_user_sync` | `sync.state = preflight_ready` | 本地预检和非本地 evidence source 可解释，但恢复码、设备授权和真实同步入口仍关闭。 |
| `ready_for_user_sync` | `sync.state = ready_for_user_sync` | 预留给后续阶段；当前实现不会派生到该状态。 |

`sync.entry_blocker` 记录当前第一阻塞码，`sync.production_blockers` 记录聚合阻塞码，例如 access token 存在性、平台 backend、发布级部署证据、恢复码流程、设备授权流程和当前阶段用户同步入口关闭。二者均为非敏感状态码，不包含请求 / 响应体、证据包正文、token、恢复码、签名或 payload bytes。

## 连接健康边界

连接健康只读联调由两部分组成：

- Flutter manager 从 settings draft 派生 `SyncConnectionHealth`，展示 `connection_status`、`connection_blocker`、`endpoint_status`、`access_token_status`、`transport_mode`、`server_state_status` 和 `last_remote_error_code`。
- `SyncConnectionProbeSummary` 可从 `sync_connection_health.v1` 摘要映射回 Manager 连接健康模型；设置页可将该摘要回填为 settings draft 的 `sync_connection_health_summary` 子对象。映射只接受 allowlist 状态码，未知 `last_remote_error_code` 会降为 `unexpected_remote_error_code`，不会把任意脚本字段、原始 JSON、请求 / 响应体或 token 传播到 UI、settings JSON 或诊断。
- `./scripts/check-sync-server-connection-health.sh` 对本地 Docker / 本地 HTTPS 或短生命周期 HTTP 服务执行 `GET /api/v1/domains/<probe>/state`，输出 `sync_connection_health.v1` 非敏感摘要。

允许展示和记录：

- endpoint 是否配置、是否因 userinfo / query / fragment / scheme 被拒绝。
- access token 是否已配置，不展示 token 文本。
- transport 分类：`local_https`、`local_http`、`external_https`、`remote_http` 或 `invalid_endpoint`。
- server state 摘要：`read_only_probe_pending`、`auth_gate_reachable`、`domain_missing_expected`、`domain_state_returned`、`not_checked_*`。
- 远端错误码：`unauthenticated`、`not_found`、`network_unreachable`、`tls_error` 等结构化码。
- 回填摘要来源和记录时间：`local_docker_https`、`local_http`、`external_https_probe`、`imported_summary`、`recorded_at` UTC 时间或 `not_recorded`。
- 只读探测认证 / HTTP / TLS 摘要：`auth_status`、`http_status`、`http_status_class` 和 `local_insecure_tls`。

2026-07-05 本地 Docker / 本地 HTTPS 只读探测记录为开发联调摘要：`connection_status=reachable`、`server_state_status=domain_missing_expected`、`auth_status=not_required_for_local_probe`、`http_status=404`、`last_remote_error_code=not_found`、`local_insecure_tls=allowed`。这说明本地 Caddy internal TLS 能到达 Go sync server 并返回预期空 domain 状态，不构成发布级目标部署证据。

禁止展示和记录：

- 完整 token、Authorization header、URL credential、query token。
- 原始 JSON、请求体、响应体、curl 输出、Go handler 原始错误、日志正文。
- payload bytes、signature bytes、wrapped material bytes、恢复材料、私钥材料。

连接健康只读探测不能解锁真实用户同步入口。即使本地服务返回 `domain_missing_expected` 或 `domain_state_returned`，同步页仍必须保持上传 / 下载、恢复码和设备授权成功路径关闭。

## 用户路径边界

### 首台设备启用同步

进入真实实现前，首台设备路径应满足：

1. 用户已配置自部署服务端草案，并保留同步配置。
2. manager 展示目标部署证据来源标签，且标签来自 allowlist。
3. Rust bridge 返回平台私钥 backend `production_gate == ready`。
4. 用户明确选择进入同步设置流程。
5. 客户端创建同步域、设备签名材料和恢复记录。
6. manager 显示恢复码一次，并要求用户确认已经离线保存。
7. 只有确认保存后，才允许后续上传加密 P2 对象。

停止线：

- 不允许用 settings draft 中的 server endpoint 自动触发上传。
- 不允许在恢复码未确认保存前把设备标记为用户可用同步状态。
- 不允许把 bearer token、恢复码或私钥材料保存进 settings draft 或诊断报告。

### 恢复码恢复新设备

恢复路径应满足：

1. 新设备只能在用户显式进入恢复流程后请求恢复。
2. manager 输入框只接收恢复码明文，不把明文写入持久化状态、日志、诊断或截图测试。
3. Rust bridge 完成 KDF、恢复记录校验和同步域材料解锁。
4. UI 只展示结构化结果：成功、恢复码错误、恢复记录缺失、恢复记录撤销、失败次数受限、网络不可达或服务端认证失败。
5. 成功后，新设备仍必须登记为受控设备，并进入后续对象拉取 / 合并流程。

停止线：

- 不把恢复码作为服务端登录密码。
- 不在 Flutter widget 层展示 KDF 参数、KDF 输出、wrapped material bytes 或恢复记录密文。
- 不把 native 原始错误字符串透传给用户或诊断报告。

### 设备授权加入

已有设备授权路径应满足：

1. 新设备生成加入请求和短码。
2. 旧设备读取 join request，只展示设备摘要、短码、创建时间、过期时间和非敏感来源。
3. 用户在旧设备核对短码后确认授权。
4. Rust bridge 签名授权包，并为新设备写入 wrapped key 密文。
5. 新设备只能读取自己的授权结果，并在本地解开同步材料。

停止线：

- 旧设备不是 `active` 时不能授权。
- join request 过期或短码不匹配时不能继续。
- Flutter 不接触 wrapped key bytes、signature bytes 或同步域材料。
- 服务端只转发 metadata 和密文材料，不参与解密。

### 设备撤销

撤销路径应满足：

1. manager 显示设备列表时只展示设备 ID 摘要、状态、授权时间、最后可见时间和非敏感 backend 状态。
2. 用户撤销设备前，UI 必须说明历史对象和撤销前已取得的材料无法被技术上追回。
3. Rust bridge 产生 revocation record，并推进后续对象使用的新 `key_epoch`。
4. 只有仍 `active` 的设备能获得新 epoch 包装记录。
5. 被撤销设备不能再签名后续对象或接收新 epoch 材料。

停止线：

- 不承诺追回撤销前已经同步到旧设备的历史数据。
- 不用设备撤销替代用户词删除；词条删除仍通过 `dictionary.deleted_terms` tombstone 表达。
- 不在 UI 中展示设备私钥、公钥原文、签名 bytes 或 wrapped material bytes。

## Bridge 边界

后续扩展 `ManagerBridge` 或 `ime-ffi` 时，应优先使用结构化请求 / 响应模型：

允许返回：

- sync gate 状态和来源。
- deployment evidence allowlist 标签。
- device backend id、capability、production gate 和非敏感原因码。
- join request 摘要、设备状态、过期时间和短码。
- 恢复码流程的结构化状态，不含恢复码明文。
- 最近同步对象数量、对象类型、版本和时间摘要。
- 错误分类和用户可读非敏感文案。

当前 Dart 准备层接受的 readiness bridge 摘要格式为 `manager_sync_readiness.v1`，redaction policy 为 `summary_only_no_tokens_recovery_secret_or_payload_bytes`。该摘要只作为 future bridge mapper 的输入形状，不是新增 `ManagerBridge` contract，也不是 C ABI。摘要字段必须按 allowlist 映射到 `RecoverySetupReadiness`、`RecoveryRestoreReadiness`、`DeviceJoinReadiness` 和 `DeviceRevocationReadiness`；未知状态、未知前置条件或未知错误码必须降级为安全分类，不能进入 UI、settings draft 或诊断报告原文。即便四条 readiness 都映射为 ready，当前 UI 仍只显示 `user_sync_entry_closed_current_phase`，不打开真实同步按钮或任何恢复码 / 设备授权操作。

禁止返回：

- P1 原始事件。
- 明文同步 payload。
- bearer token、恢复码、同步主密钥、设备私钥。
- KDF 输出、signature bytes、wrapped material bytes、encrypted payload bytes。
- Go server 请求体 / 响应体、内部 `blob_ref` 或本机真实路径。
- native 原始错误字符串。

错误分类至少应覆盖：

- `configuration_missing`
- `authentication_required`
- `backend_unavailable`
- `deployment_unverified`
- `recovery_code_required`
- `recovery_code_invalid`
- `recovery_record_revoked`
- `join_request_expired`
- `authorization_rejected`
- `device_revoked`
- `network_unreachable`
- `version_conflict`
- `local_data_inconsistent`

## 诊断与日志

诊断报告可以新增真实同步入口字段，但必须保持摘要化：

- `sync.entry_state`
- `sync.entry_blocker`
- `sync.local_evidence_source`
- `sync.production_blockers`
- `sync.readiness_blocked_flows`
- `sync.readiness_issue_codes`
- `sync.readiness_next_required_evidence`
- `sync.readiness_source_tags`
- `sync.readiness_bridge_source`
- `sync.readiness_user_sync_blocked`
- `sync.interaction_actions`
- `sync.interaction_visibility`
- `sync.interaction_statuses`
- `sync.interaction_blockers`
- `sync.interaction_required_evidence`
- `sync.interaction_source_tags`
- `sync.user_sync_enabled`
- `sync.recovery_status`
- `sync.recovery_blocker`
- `sync.recovery_save_confirmation`
- `sync.recovery_record_status`
- `sync.recovery_record_blocker`
- `sync.recovery_first_upload_gate`
- `sync.recovery_readiness_blockers`
- `sync.recovery_setup_status`
- `sync.recovery_setup_blocker`
- `sync.recovery_setup_action_status`
- `sync.recovery_setup_prerequisites`
- `sync.recovery_setup_error_codes`
- `sync.recovery_restore_status`
- `sync.recovery_restore_blocker`
- `sync.recovery_restore_code_input`
- `sync.recovery_restore_error_codes`
- `sync.device_authorization_status`
- `sync.device_authorization_blocker`
- `sync.join_request_status`
- `sync.authorization_package_status`
- `sync.authorization_package_blocker`
- `sync.authorization_package_preconditions`
- `sync.device_revocation_status`
- `sync.lost_device_risk`
- `sync.key_epoch_status`
- `sync.device_authorization_readiness_blockers`
- `sync.device_join_status`
- `sync.device_join_blocker`
- `sync.device_join_action_status`
- `sync.device_join_short_code`
- `sync.device_join_error_codes`
- `sync.device_revocation_flow_status`
- `sync.device_revocation_blocker`
- `sync.device_revocation_active_requirement`
- `sync.device_revocation_error_codes`
- `sync.connection_status`
- `sync.connection_blocker`
- `sync.endpoint_status`
- `sync.access_token_status`
- `sync.transport_mode`
- `sync.server_state_status`
- `sync.last_remote_error_code`
- `sync.last_object_counts`
- `device.active_count`
- `device.pending_count`
- `device.revoked_count`

当前已接入 `sync.entry_state`、`sync.entry_blocker`、`sync.local_evidence_source`、`sync.production_blockers`、`sync.readiness_*`、`sync.interaction_*`、`sync.user_sync_enabled`、`sync.connection_status`、`sync.connection_blocker`、`sync.endpoint_status`、`sync.access_token_status`、`sync.transport_mode`、`sync.server_state_status`、`sync.last_remote_error_code`、`sync.recovery_status`、`sync.recovery_blocker`、`sync.recovery_save_confirmation`、`sync.recovery_record_status`、`sync.recovery_record_blocker`、`sync.recovery_first_upload_gate`、`sync.recovery_readiness_blockers`、`sync.recovery_setup_*`、`sync.recovery_restore_*`、`sync.device_authorization_status`、`sync.device_authorization_blocker`、`sync.join_request_status`、`sync.authorization_package_status`、`sync.authorization_package_blocker`、`sync.authorization_package_preconditions`、`sync.device_revocation_status`、`sync.lost_device_risk`、`sync.key_epoch_status`、`sync.device_authorization_readiness_blockers`、`sync.device_join_*` 和 `sync.device_revocation_*`。这些字段只能输出状态码、聚合计数、时间摘要、action id、intent status、前置条件码和 allowlist 来源标签。`sync.readiness_bridge_source` 只能记录 `manager_default_closed_readiness`、`ffi_native_readiness`、`fixture_readiness`、`injected_test_readiness`、`unknown_bridge_readiness_source` 或 `bridge_readiness_summary_invalid` 这类来源标签。不得输出恢复码、token、短码、请求 / 响应体、签名、wrapped material、payload bytes、用户词、真实路径或 provider exception 原文。

日志允许记录操作类型、状态码、聚合计数、耗时和非敏感错误码。截图和测试 fixture 必须使用合成词、虚构设备、虚构服务端和合成短码。

## 测试计划

进入真实同步 UI / bridge 前，应至少补齐以下测试：

| 层级 | 测试重点 |
| --- | --- |
| Dart sync gate helper | 从 settings draft、backend gate、部署证据摘要、恢复码 setup / restore 状态和设备 join / revocation 状态派生 sync entry state、readiness 聚合摘要和只读 action intent 进入计划。 |
| Dart connection helper | 从 endpoint、access token 存在性、transport 分类和 `sync_connection_health.v1` 回填摘要派生连接健康摘要。 |
| Widget tests | 每个阻塞状态的按钮禁用、文案、诊断入口、连接健康 section、恢复码 / 设备授权四条只读流程摘要和下一步提示；`preflight_ready` 下仍不能启用真实同步。 |
| Bridge mapper tests | `manager_sync_readiness.v1` 非敏感摘要映射到 Dart readiness model，不透传原始错误字符串、真实路径、token、恢复码、短码或 secret 字段；未知错误降级为安全分类。 |
| FFI smoke | 使用临时 SQLite、临时 settings、合成设备和短生命周期 bridge 复验本地状态读取，不连接真实用户后端。 |
| Rust tests | 恢复记录创建 / 撤销、join request 过期、授权签名、设备撤销后 key epoch 推进和旧设备阻断。 |
| Go / Rust integration | 真实 HTTP handler 只传 metadata 和密文；错误响应、audit log 和 Debug 输出脱敏。 |
| 诊断测试 | 新增诊断字段不包含用户词、路径、token、恢复码、signature bytes、wrapped material bytes、payload bytes、证据包正文或 `notes`。 |

文档或 UI 只改变本地展示口径时，可以先跑 `./scripts/check-manager.sh`、`git diff --check` 和 `./scripts/check-repo.sh`。涉及目标部署证据包格式、摘要或交接材料时，应追加 `./scripts/check-sync-deployment-evidence.sh --self-test`，并对实际证据文件执行 `./scripts/check-sync-deployment-evidence.sh <evidence-file>` 和 `./scripts/check-sync-deployment-evidence.sh --summary-json <evidence-file>`。涉及真实 Dart FFI bridge、同步状态读取或诊断导出时，应追加 `./scripts/check-manager-ffi-smoke.sh`。涉及 Rust / Go 同步客户端或 server handler 时，应按对应 crate / server 测试扩大验证。

## 推进顺序

后续实现应按以下顺序推进：

1. 保持 Phase 4 本地验收证据稳定。
2. 用本地 Docker / 本地 HTTPS 和现有 smoke 作为开发期同步证据，保持 settings draft 只记录非敏感来源标签。
3. manager sync entry state 派生、恢复码准备态、设备授权准备态、恢复码 setup / restore、设备 join / revocation、readiness 聚合摘要、只读 action intent 进入计划、恢复码保存确认、恢复记录、授权包前置条件、撤销 / 丢失设备和 key epoch 风险提示已经以非上传形式接入，继续不上传真实用户数据。
4. 同步服务连接健康和错误分类已经以只读模型、UI、诊断字段、脚本自测、本地 Docker / 本地 HTTPS 运行摘要、Manager 摘要映射和 settings draft 回填入口接入；后续继续使用 `sync_connection_health.v1` 非敏感字段。
5. Dart 侧 `manager_sync_readiness.v1` bridge mapper、错误分类降级和 `SyncInteractionEntryPlan` 已接入；下一步保持四条只读流程和 action intent 稳定，继续准备真实交互实现前的协议 / 测试输入，但不上传真实 P2 数据。
6. 取得至少一个可用平台私钥 backend 的生产签名证据，或补新的平台 / 算法 ADR 输入。
7. 正式发布 / 真实用户开放前，按生产部署 runbook 补目标部署运行证据包，导出 `deployment_evidence_summary.v1` 非敏感摘要。
8. 在恢复码确认、设备授权、发布级部署证据和 backend gate 全部满足后，再开放用户可用同步入口。

任何一步如果需要新增 Go server API、C ABI、settings 字段、诊断字段或同步对象类型，必须先更新对应专题文档和测试口径。

## 当前停止线

- 平台私钥 backend 未解除 production gate 前，不提供恢复码创建、设备授权成功路径或真实同步主操作。
- 发布级目标部署证据未形成前，不把远端同步展示为生产可用；但不阻止本地 Docker / 本地 HTTPS 下的开发联调。
- 恢复码确认保存流程未完成前，不上传真实用户 P2 对象。
- join request、授权包、撤销和 key epoch 错误语义未映射前，不开放设备授权 UI。
- 任何明文 payload、P1 原始事件、恢复码、token、私钥、signature bytes、wrapped material bytes 或 encrypted payload bytes 进入 widget、日志、诊断或 fixture，都必须停止并回退。
