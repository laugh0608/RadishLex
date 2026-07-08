# Manager Sync Bridge Command Contract Draft

本文档定义 future `ManagerBridge` 同步命令 contract 的草案形状，读者是准备审阅 Flutter manager、Dart FFI bridge、`ime-ffi` 和 Rust sync / crypto 边界的人。本文不定义当前可执行接口，不修改 `ManagerBridge`，不新增 C ABI，不定义 Go server API 字段全集，不承载恢复码格式、签名格式、KDF 参数或授权包密文字段。

## 当前结论

真实同步命令仍未开放。本文只把后续可能进入 `ManagerBridge` 的四条同步命令整理成可审阅 contract 草案，并把每个字段归类为 `safe_summary`、`transient_secret`、`opaque_crypto_material` 或 `transport_payload`。

当前仍保持：

- 不修改 `apps/radishlex-manager/lib/src/bridge/manager_bridge.dart`。
- 不新增 `ime-ffi` C ABI、symbol 或 Rust FFI handler。
- 不新增同步页按钮、点击回调、settings draft action payload 或真实 bridge 调用。
- 不生成恢复码、不输入恢复码、不创建 join request、不签名授权包、不撤销设备。
- 不上传、下载或合并真实远端 P2 对象。

本文依赖：

- `docs/manager-sync-bridge-command-contract-checklist.md`
- `docs/manager-sync-action-protocol-preview.md`
- `docs/manager-sync-entry-boundary.md`
- `docs/manager-sync-ffi-command-boundary.md`
- `apps/radishlex-manager/test/fixtures/sync_bridge_command_contract_fixtures.dart`
- `apps/radishlex-manager/test/models/manager_sync_bridge_command_contract_test.dart`

## 与 Preview 的分工

`manager_sync_action_command_preview.v1` 继续负责当前 UI / diagnostics 可见的非执行摘要：

- action id、visibility、intent status、execution status。
- request / result boundary。
- allowed fields 和 forbidden material policy。
- stop line、data policy 和错误分类。

本文描述 future `ManagerBridge` contract 应如何从这些摘要进入真实命令审阅。preview 字段不能直接改名后成为 bridge DTO；真实 DTO 必须重新定义版本、字段分类、生命周期、错误 envelope、幂等性、并发和脱敏规则。

## 可见层与 Fixture 证据

当前已有多组 fixture / model test 证据，它们分别固定不同层级的安全形状，不能互相替代：

| 层级 | 代表 fixture / model | 证明内容 | 不能证明 |
| --- | --- | --- | --- |
| action protocol preview | `sync_action_protocol_fixtures.dart`、`manager_sync_action_preview_test.dart` | 四条 action 的 request / result boundary、allowed fields、forbidden material、错误分类和 execution status 派生稳定。 | 真实 `ManagerBridge` request / result DTO 已定义。 |
| bridge command contract draft | `sync_bridge_command_contract_fixtures.dart`、`manager_sync_bridge_command_contract_test.dart` | future bridge 字段能按 `safe_summary`、`transient_secret`、`opaque_crypto_material` 和 `transport_payload` 分类，且 diagnostics 只保留安全摘要。 | Dart interface、native binding 或 Rust executor 已实现。 |
| transient secret interaction | `sync_transient_secret_interaction_fixtures.dart`、`manager_sync_transient_secret_interaction_test.dart` | 恢复码一次性展示、恢复码输入、短码核对、授权确认和撤销确认的生命周期、禁止持久化目标和完成证据码可复验。 | 真实 secret 显示、输入、复制、保存或传递已开放。 |
| visible-layer confirmation detail | `syncRecoveryVisibleLayerFixtures`、`syncRecoveryFutureConfirmationDetailFixtures`、`syncDeviceAuthorizationFutureConfirmationDetailFixtures` | recovery setup / restore 的展示占位、保存确认、输入占位、查询、失败限速和设备登记状态，以及 join request 授权 / 设备撤销的短码核对、显式确认、授权包、丢失设备风险和 key epoch 状态已绑定 UI / diagnostics 非敏感字段。 | 真实恢复码展示、短码输入、授权签名、设备撤销或 key epoch 推进已开放。 |
| FFI command boundary | `sync_ffi_command_boundary_fixtures.dart`、`manager_sync_ffi_command_boundary_test.dart` | 推荐 ABI strategy、ownership、error status、host catalog、review catalog 和 forbidden material policy 可复验。 | C ABI symbol、Rust host test 文件或真实 command context 已存在。 |
| Dart fake binding replay | `manager_sync_ffi_binding_contract_test.dart`、`ffi_manager_bridge_test.dart` | fake native binding 能消费 host catalog 摘要，验证 copy / free、unknown status 降级和 capability missing 时 UI 关闭。 | 真实 dynamic library 已导出 future sync command symbol。 |

真实 contract 设计时应按这个顺序提升证据强度：先补可见层和 fixture，确认状态码、生命周期和 forbidden material policy；再补 Dart contract / binding 测试；最后在 C ABI 评审通过后补 Rust host test 和真实 native symbol。任一层新增字段时，都必须同步更新 `docs/manager-settings-diagnostics.md`、本文件和对应 fixture，不应只修改 UI 文案或测试断言。

## 候选 Dart Interface

以下只是候选形状，不是当前代码接口。真实接线前必须另行评审命名、错误类型、返回模型、测试和 C ABI 映射。

```dart
abstract interface class ManagerBridge {
  Future<ManagerSyncCommandResult> startRecoverySetup(
    ManagerSyncRecoverySetupRequest request,
  );

  Future<ManagerSyncCommandResult> restoreWithRecoveryCode(
    ManagerSyncRecoveryRestoreRequest request,
  );

  Future<ManagerSyncCommandResult> authorizeJoinRequest(
    ManagerSyncJoinAuthorizationRequest request,
  );

  Future<ManagerSyncCommandResult> revokeDevice(
    ManagerSyncDeviceRevocationRequest request,
  );
}
```

候选接口要求：

- 每个方法只能由用户显式操作触发，不能由 settings draft 保存、页面加载或连接健康探测自动触发。
- 每个方法必须绑定当前净化后的 readiness / evidence 摘要，不能绕过 `SyncEntryState` / `ManagerSyncEntryGate`。
- 每个方法返回同一类 `ManagerSyncCommandResult`，失败用结构化 command error envelope 表达，不透传 native 原始异常。
- transient secret 只允许在一次用户操作中进入 bridge，不能进入 settings、诊断、日志、fixture golden 或截图。
- Flutter manager 不长期持有同步主密钥、设备私钥、wrapped material、signature bytes、encrypted payload bytes 或 Go server 请求 / 响应体。

## Common DTO

候选 request 需要包含的公共安全字段：

| 字段 | 分类 | 说明 |
| --- | --- | --- |
| `schema_version` | `safe_summary` | 候选 contract 版本；未知版本必须拒绝。 |
| `action_id` | `safe_summary` | 四条 action 之一。 |
| `operation_id` | `safe_summary` | 非敏感随机操作 ID；不得由恢复码、短码、token 或私钥派生。 |
| `readiness_snapshot_id` | `safe_summary` | 当前净化 readiness / evidence 摘要的绑定标识或摘要标签。 |
| `deployment_evidence_source_tag` | `safe_summary` | allowlist 来源标签，不是证据包正文。 |
| `device_backend_gate` | `safe_summary` | 平台私钥 backend production gate 摘要。 |
| `explicit_user_start` | `safe_summary` | 用户显式触发状态。 |

候选 result 需要包含的公共安全字段：

| 字段 | 分类 | 说明 |
| --- | --- | --- |
| `schema_version` | `safe_summary` | result contract 版本。 |
| `action_id` | `safe_summary` | 与 request action 一致。 |
| `command_status` | `safe_summary` | `success`、`already_done`、`blocked_by_readiness`、`requires_user_confirmation`、`conflict`、`retryable_failure`、`fatal_failure` 或 `unexpected_bridge_error`。 |
| `error_code` | `safe_summary` | allowlist 错误码；成功时为 `none`。 |
| `retry_policy` | `safe_summary` | `not_retryable`、`retry_after_user_action`、`retry_after_network_recovery` 或 `retry_after_conflict_resolution`。 |
| `next_required_evidence` | `safe_summary` | 下一项非敏感证据码。 |
| `diagnostics_summary_code` | `safe_summary` | 诊断报告可记录的聚合状态码。 |

禁止进入 common DTO：

- bearer token、Authorization header、URL credential 或 query token。
- 恢复码、短码、同步主密钥、设备私钥、公钥原文。
- KDF 输出、signature bytes、wrapped material、encrypted payload bytes。
- Go server 请求体、响应体、错误 body、日志正文或证据包正文。
- 本机真实路径、provider exception 原文或 native 原始错误字符串。

## Action DTO 分类

### `recovery_setup`

request `safe_summary`：

- `deployment_evidence_source_tag`
- `device_backend_gate`
- `explicit_user_start`
- `save_confirmation_status`

result `safe_summary`：

- `status_code`
- `recovery_record_status`
- `first_upload_gate`
- `next_required_evidence`

`transient_secret`：

- `one_time_recovery_code_display_placeholder`
- `recovery_code_save_confirmation_ack`

`opaque_crypto_material`：

- `sync_master_key`
- `recovery_record_ciphertext`
- `wrapped_recovery_material`

`transport_payload`：

- `recovery_record_upload_request_body`
- `recovery_record_upload_response_body`

当前禁止：生成恢复码、创建恢复记录、解锁首次上传。

### `recovery_restore`

request `safe_summary`：

- `deployment_evidence_source_tag`
- `restore_attempt_status`
- `recovery_record_lookup_status`
- `device_registration_status`

result `safe_summary`：

- `status_code`
- `restore_result_status`
- `device_registration_status`
- `next_required_evidence`

`transient_secret`：

- `recovery_code_input_transient_placeholder`

`opaque_crypto_material`：

- `kdf_output`
- `unwrapped_device_material`
- `sync_master_key`

`transport_payload`：

- `recovery_record_lookup_request_body`
- `recovery_record_lookup_response_body`

当前禁止：输入恢复码、解开设备材料、注册恢复设备。

### `join_request_authorization`

request `safe_summary`：

- `join_request_status`
- `short_code_verification_status`
- `active_device_requirement`
- `authorization_package_preconditions`

result `safe_summary`：

- `status_code`
- `authorization_package_status`
- `device_state_status`
- `next_required_evidence`

`transient_secret`：

- `short_code_verification_transient_placeholder`
- `explicit_authorization_confirmation`

`opaque_crypto_material`：

- `authorization_package_ciphertext`
- `device_signature`
- `wrapped_device_key`

`transport_payload`：

- `join_request_fetch_response_body`
- `authorization_package_upload_request_body`

当前禁止：创建 join request、签名授权包、写入 wrapped material。

### `device_revocation`

request `safe_summary`：

- `target_device_status_summary`
- `active_device_requirement`
- `lost_device_risk_acknowledgement`
- `key_epoch_status`

result `safe_summary`：

- `status_code`
- `revocation_record_status`
- `key_epoch_status`
- `next_required_evidence`

`transient_secret`：

- `explicit_revocation_confirmation`

`opaque_crypto_material`：

- `revocation_record_signature`
- `new_key_epoch_material`
- `wrapped_epoch_material`

`transport_payload`：

- `revocation_upload_request_body`
- `key_epoch_delivery_response_body`

当前禁止：撤销设备、签名 revocation record、推进 key epoch。

## 错误 Envelope

真实 command 的错误 envelope 必须是结构化安全摘要。候选字段：

| 字段 | 分类 | 说明 |
| --- | --- | --- |
| `action_id` | `safe_summary` | 触发失败的 action。 |
| `command_status` | `safe_summary` | 阻塞、冲突、可重试失败或不可恢复失败。 |
| `error_code` | `safe_summary` | allowlist 错误码。 |
| `retry_policy` | `safe_summary` | 重试建议。 |
| `user_visible_summary_code` | `safe_summary` | UI 可映射的非敏感文案码。 |
| `diagnostics_summary_code` | `safe_summary` | 诊断报告可记录的摘要码。 |
| `next_required_evidence` | `safe_summary` | 下一步证据码。 |

错误码至少覆盖：

- `configuration_missing`
- `authentication_required`
- `backend_unavailable`
- `deployment_unverified`
- `recovery_code_required`
- `recovery_code_invalid`
- `recovery_record_missing`
- `recovery_record_revoked`
- `join_request_expired`
- `authorization_rejected`
- `device_revoked`
- `key_epoch_rotation_required`
- `network_unreachable`
- `version_conflict`
- `local_data_inconsistent`
- `unexpected_bridge_error`

禁止将 provider exception、Go server 原始错误、请求 / 响应体、token、恢复码、短码、signature bytes、wrapped material、payload bytes、真实路径写入 envelope。

## 幂等性与并发

真实 contract 必须在接线前固定这些行为：

| Action | 幂等性要求 |
| --- | --- |
| `recovery_setup` | 重复开始不能静默轮换恢复码；已有 active recovery record 时返回结构化状态并要求用户确认下一步。 |
| `recovery_restore` | 恢复码错误不能改变本地同步域；成功恢复后的重复请求返回 `already_restored` 或等价安全状态。 |
| `join_request_authorization` | 过期、短码不匹配或已撤销设备不能继续；重复授权同一 join request 返回 `already_authorized` 或等价安全状态。 |
| `device_revocation` | 已撤销设备重复撤销返回 `already_revoked`；key epoch 推进必须检测冲突，不能重复生成不一致材料。 |

并发规则：

- 同一 action 重复点击必须被 UI 和 bridge 双层阻止或合并。
- 四条 action 之间的互斥关系必须显式定义，不能让恢复、授权和撤销同时修改同一同步域。
- 本地 userdb 写入、Rust sync 状态更新和远端 upload / download 的顺序必须由 Rust 边界控制。
- 网络超时后的重试必须携带非敏感 `operation_id` 或等价幂等标识。
- manager 重启后只能从 Rust / sync / userdb 可审计状态恢复摘要，不能从 Flutter 临时状态恢复 secret。

## FFI 与 C ABI 问题清单

真实实现前必须按 `docs/manager-sync-ffi-command-boundary.md` 另行解决：

- 是否采用四个专用 C ABI symbol，还是一个 versioned command envelope 入口；当前建议优先评审单一 versioned executor，但不得把它做成任意 JSON tunnel。
- 如果采用专用 symbol，每个 symbol 的 request / result 结构、版本号和释放函数如何命名。
- 如果采用 envelope，如何避免把 JSON request / response body、payload bytes 或 secret 字段传到 Flutter 可见层。
- Rust 分配 result / error buffer 的所有权、释放函数、空指针规则和 `*_free(NULL)` 行为。
- UTF-8 字符串使用 `RadishLexStringView` 还是 Rust-owned buffer。
- command 是否绑定 owner thread；如果绑定，和现有 `RadishLexSession*` owner-thread policy 如何并存。
- panic boundary 和 `RadishLexStatusCode` 映射是否需要扩展。
- 真实 platform backend 调用失败时，如何降级成 allowlist 错误码。
- FFI host smoke 和 Dart FFI smoke 如何覆盖 forbidden material 不穿越边界。

在这些问题有专题文档或 ADR 前，不应新增 C ABI symbol。

## 诊断与 Settings

真实 command 接线前，诊断报告只能新增安全摘要字段。允许：

- action id。
- command status。
- allowlist error code。
- retry policy。
- next required evidence。
- source tag。
- 对象类型、计数、版本或时间的聚合摘要。

禁止：

- settings draft action payload。
- 恢复码、短码、token、私钥、公钥原文。
- KDF 输出、signature bytes、wrapped material、encrypted payload bytes。
- Go server 请求 / 响应体、证据包正文、真实路径。
- 原始 native exception、provider exception 或 transport error body。

如果后续新增 diagnostics 字段，必须同步更新：

- `docs/manager-settings-diagnostics.md`
- `docs/manager-sync-entry-boundary.md`
- settings / sync / diagnostics widget tests

## 测试映射

当前已覆盖：

- `sync_bridge_command_contract_fixtures.dart`：四条 action 的安全 request / result 样本、transient / opaque / transport 字段分类、错误 envelope、幂等性状态和 forbidden material 拒绝样本。
- `manager_sync_bridge_command_contract_test.dart`：验证安全形状只暴露摘要字段、错误 envelope 只使用 allowlist、诊断报告仍停留在 preview 摘要，不出现 future request / result payload。
- `manager_sync_action_preview_test.dart`：验证 preview 状态矩阵和 request / result preview 派生。
- `manager_sync_entry_gate_test.dart`、settings / sync / diagnostics tests：验证 readiness、evidence bundle 和 UI / diagnostics 不分叉。

后续真实 contract 前还需要：

- Dart contract model 测试，覆盖未知 schema version、unknown status、unknown error code 和 unsafe redaction。
- Rust FFI contract test，覆盖 buffer ownership、错误 envelope、字符串编码、释放责任和 panic boundary。
- Dart FFI smoke，覆盖 forbidden material 不进入 Dart-visible result。
- 平台私钥 backend smoke 或等价证据。
- 发布级 `deployment_evidence_summary.v1` 验证。

## 进入实现前的停止线

满足以下条件前，不允许把本文草案转成真实 `ManagerBridge` 方法或 C ABI：

- 平台私钥 backend 在目标平台解除 production gate。
- 恢复码 setup / restore 与设备 join / revocation 的真实交互测试设计完成。
- 发布级目标部署证据通过校验，并只以 `deployment_evidence_summary.v1` 摘要进入 manager。
- Dart contract、Rust FFI ownership、C ABI symbol / envelope 策略和错误 envelope 已完成评审。
- settings / sync / diagnostics 的可见层回归已准备。
- forbidden material 拒绝测试覆盖 token、恢复码、短码、signature bytes、wrapped material、payload bytes、请求 / 响应体和真实路径。

如果真实实现需要 Flutter 长期持有同步材料、授权包材料或 payload bytes，应停止该方向，回到 Rust / sync / crypto 边界重新设计。
