# Manager Sync Bridge Command Contract Checklist

本文档是 future `ManagerBridge` 同步命令 contract 的设计草案和审阅检查清单。读者是准备把 `recovery_setup`、`recovery_restore`、`join_request_authorization` 和 `device_revocation` 从非执行 preview 推进到真实 bridge 命令的维护者。本文不定义真实 Dart interface、C ABI、Rust FFI 函数、Go server API、加密 payload 字段、恢复码格式或设备授权包格式。

future contract 草案见 `docs/manager-sync-bridge-command-contract.md`。FFI / C ABI ownership 与 host smoke 设计见 `docs/manager-sync-ffi-command-boundary.md`。本文继续作为审阅检查清单和停止线，不替代草案文档。

## 当前结论

当前阶段只允许把真实命令前的 contract 约束写成可审阅清单和 design fixture。`manager_sync_action_command_preview.v1`、`SyncActionRequestPreview` 和 `SyncActionResultPreview` 仍是非执行预演层，不是 bridge DTO，也不能被包装成真实请求。

本检查清单的作用：

- 明确 future command contract 可以继承哪些 preview 口径。
- 明确真实 bridge contract 必须另行定义哪些字段、生命周期和错误语义。
- 在写 Dart / Rust / C ABI 代码前，先固定审阅顺序、停止线、fixture 输入和脱敏要求。

当前仍保持：

- 不修改 `ManagerBridge` contract。
- 不新增 `ime-ffi` C ABI。
- 不新增同步页按钮、点击回调或 settings draft action payload。
- 不生成恢复码、不输入恢复码、不创建 join request、不签名授权包、不撤销设备。
- 不上传、下载或合并真实远端 P2 对象。

当前可复验证据：

- `apps/radishlex-manager/test/fixtures/sync_bridge_command_contract_fixtures.dart`
- `apps/radishlex-manager/test/models/manager_sync_bridge_command_contract_test.dart`
- `docs/manager-sync-bridge-command-contract.md`
- `docs/manager-sync-ffi-command-boundary.md`

这组 fixture / 测试只验证 future contract 的安全形状、错误 envelope、幂等性状态码、forbidden material 拒绝样本和 diagnostics 不泄漏；它不定义真实 `ManagerBridge` 方法或 `ime-ffi` symbol。

## 与现有 Preview 的关系

future command contract 可以继承现有 preview 的这些口径：

- action id：`recovery_setup`、`recovery_restore`、`join_request_authorization`、`device_revocation`。
- readiness 阻塞语义：`blocked_by_readiness`、`not_executable_current_phase`、`requires_user_confirmation`、`ready_for_future_bridge_shape`。
- request boundary、result boundary、data policy 和 stop line 的命名。
- request allowed fields、result allowed fields 和 forbidden material policy 的 allowlist。
- action 级错误分类 allowlist。
- settings / sync / diagnostics 必须从同一组 model 派生的约束。

future command contract 必须另行定义这些内容：

- Dart `ManagerBridge` 方法名、参数类型、返回类型和异常语义。
- Rust FFI command 模型、buffer ownership、字符串编码、释放责任和错误 envelope。
- 是否需要 C ABI version bump、symbol 命名和 contract test。
- 真正可执行 command 的 request / result DTO。
- 临时 secret 字段的生命周期、内存处理、日志禁止规则和 widget 测试替代数据。
- 幂等性、重试策略、取消语义、超时语义和并发互斥规则。
- 与 `ime-sync`、`ime-crypto`、`ime-userdb` 和 Go server API 的职责分界。
- 诊断报告允许新增的聚合状态码和禁止字段。

preview 字段不能直接改名后当作 bridge DTO。真实 contract 设计完成前，preview 只能继续输出摘要字段。

## 设计前置条件

进入真实 command contract 设计评审前，需要先满足：

- `manager_sync_readiness.v1` 的 mapper、hygiene 测试和场景目录仍能覆盖四条 action 的 ready / blocked / unknown / unsafe 输入。
- `manager_sync_action_command_preview.v1` 的状态矩阵仍覆盖当前阶段关闭、readiness 阻塞、用户确认、future ready shape、missing intent 和 unknown intent status。
- `docs/manager-sync-action-protocol-preview.md` 中的 request boundary、result boundary、allowed fields、forbidden material policy 和错误分类没有未解释的漂移。
- `docs/manager-sync-entry-boundary.md` 中的恢复码、设备授权、部署证据、平台私钥 backend 和诊断停止线仍成立。
- 发布级目标部署证据、平台私钥 backend、恢复 / 授权实现测试仍未齐备时，真实 command 只能停留在设计和 fixture 层。

如果其中任一项不满足，应先修复 preview / readiness / evidence bundle 的证据链，再讨论真实 command。

## Command 级检查表

| Action | 必须满足的进入条件 | 真实 contract 必须另行定义 | 当前禁止 |
| --- | --- | --- | --- |
| `recovery_setup` | 平台私钥 backend ready、部署证据摘要可用、用户显式开始、恢复码保存确认路径已设计。 | 首台设备创建同步域、恢复记录创建、一次性恢复码展示、保存确认后的 first upload gate。 | 当前不生成恢复码、不创建恢复记录、不解锁首次上传。 |
| `recovery_restore` | 用户显式进入恢复、恢复记录可查询、失败限速和错误分类已设计、设备登记路径已设计。 | 恢复码临时输入、KDF / recovery record 校验、设备注册、恢复后拉取 / 合并前置状态。 | 当前不输入恢复码、不解开设备材料、不注册恢复设备。 |
| `join_request_authorization` | 新设备 join request、短码核对、旧设备 active 状态、授权确认和过期处理已设计。 | join request 读取、短码核对、授权包签名、wrapped key 写入、新设备可见授权结果。 | 当前不创建 join request、不签名授权包、不写 wrapped material。 |
| `device_revocation` | 目标设备摘要、用户风险确认、active signer、key epoch 推进策略已设计。 | revocation record、后续 key epoch、active 设备材料轮换和 revoked 设备后续拒绝。 | 当前不撤销设备、不签名 revocation record、不推进 key epoch。 |

## Request Contract 检查

每个真实 request contract 必须逐项回答：

- 这个 request 是否只能由用户显式操作触发。
- request 是否会读取 settings draft；如果读取，只能读取 endpoint、access token presence、deployment evidence source tag、connection health summary 这类非敏感配置摘要。
- request 是否需要临时 secret 输入；如果需要，字段必须标记为 transient，禁止进入 settings、日志、诊断、fixture golden 和截图。
- request 是否携带服务端请求体、响应体、payload bytes、signature bytes 或 wrapped material；如果携带，设计应被拒绝。
- request 如何绑定当前 readiness snapshot，避免 UI 显示 ready 后 bridge 使用另一套未审计状态。
- request 是否需要幂等键或 operation id；如果需要，必须是非敏感随机标识，不得复用恢复码、短码、token 或设备私钥派生值。
- request 的取消、超时、重复点击和并发互斥由哪一层负责。
- request 的 schema version、unknown field 策略和向后兼容策略。

真实 request DTO 的字段命名前，应先把字段归类为：

- `safe_summary`：可进入 UI、diagnostics 和 fixture 的状态码 / 来源标签 / 计数。
- `transient_secret`：只允许在一次用户操作内进入 bridge，不能持久化或记录。
- `opaque_crypto_material`：只允许在 Rust core / sync / crypto 内部存在，不能穿过 Flutter 可见层。
- `transport_payload`：只允许在 sync transport 内部存在，不能进入 ManagerBridge 可见 contract。

## Result Contract 检查

每个真实 result contract 必须逐项回答：

- 成功结果是否只返回状态码、对象类型摘要、版本摘要、设备状态摘要和下一步证据。
- 是否存在一次性展示材料，例如首台设备恢复码。如果存在，必须单独设计显示一次、确认保存、不可诊断、不可复制到日志的生命周期。
- 是否返回任何 KDF 输出、设备私钥、sync master key、signature bytes、wrapped material、encrypted payload bytes 或 Go server 响应体。如果返回，设计应被拒绝。
- 是否区分 `success`、`already_done`、`blocked_by_readiness`、`requires_user_confirmation`、`conflict`、`retryable_failure` 和 `fatal_failure`。
- 是否能让 settings / sync / diagnostics 只用同一组状态码解释结果。
- 是否定义了 unknown status 的降级策略。
- 是否定义了 result 进入诊断报告前的 allowlist mapper。

真实 result 不能让 Flutter manager 成为同步材料或授权包材料的真相源。Flutter 只能展示摘要和推动用户确认，材料生成、签名、加密、上传、下载和合并必须继续由 Rust / sync / crypto 边界承载。

## 错误语义检查

真实 command contract 的错误 envelope 至少需要区分：

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

错误 envelope 必须包含：

- action id。
- command status。
- allowlist error code。
- retry policy。
- user visible summary code。
- diagnostics summary code。
- next required evidence。

错误 envelope 禁止包含：

- provider exception 原文。
- Go server 原始错误、请求体、响应体或日志正文。
- token、恢复码、短码、私钥、公钥原文、signature bytes、wrapped material 或 payload bytes。
- 本机真实路径、证据包正文、URL credential、query token。

## 幂等性与并发检查

真实 command contract 必须为四条 action 明确幂等性：

| Action | 幂等性要求 |
| --- | --- |
| `recovery_setup` | 重复开始不能静默轮换恢复码；已有 active recovery record 时必须返回结构化状态并要求用户确认下一步。 |
| `recovery_restore` | 恢复码错误不能改变本地同步域；成功恢复后的重复请求应返回 `already_restored` 或等价安全状态。 |
| `join_request_authorization` | 过期、短码不匹配或已撤销设备不能继续；重复授权同一 join request 应返回 `already_authorized` 或等价安全状态。 |
| `device_revocation` | 已撤销设备重复撤销应返回 `already_revoked`；key epoch 推进必须可检测冲突，不能重复生成不一致材料。 |

并发规则至少需要覆盖：

- 同一 action 重复点击。
- 四条 action 之间的互斥关系。
- 本地 userdb 写入和远端 sync request 的顺序。
- 网络超时后的重试。
- 进程退出或 manager 重启后的恢复状态。

## 诊断与日志检查

真实 command 接线前，必须先定义诊断字段变更表。允许进入诊断报告的内容只能是：

- action id。
- command status。
- allowlist error code。
- retry policy。
- next required evidence。
- source tag。
- 对象类型、计数、版本或时间的聚合摘要。

禁止进入诊断报告、日志、测试 golden 或截图：

- 恢复码、短码、token、私钥、公钥原文。
- KDF 输出、signature bytes、wrapped material、encrypted payload bytes。
- Go server 请求 / 响应体、证据包正文、真实路径。
- 原始 native exception、provider exception 或 transport error body。

任何新增诊断字段都必须同步更新：

- `docs/manager-settings-diagnostics.md`
- `docs/manager-sync-entry-boundary.md`
- 相关 widget / diagnostics 回归测试

## Fixture 与测试进入顺序

真实 command code 前的测试准备顺序：

1. 先补 design fixture，覆盖 request / result DTO 的安全字段、transient secret 替代占位和 forbidden material 拒绝样本。
2. 再补 Dart mapper / model 测试，确认 unknown status、unknown error code 和 unsafe redaction 降级。
3. 再补 settings / sync / diagnostics 可见层测试，确认不会新增可执行按钮或 settings draft action payload。
4. 再补 Rust FFI contract test，确认 buffer ownership、错误 envelope、字符串编码和释放责任。
5. 再补端到端 smoke；涉及真实平台私钥 backend、系统 Keychain / Keystore 或真实部署环境时，必须单独获得授权。

当前阶段已起步第 1 到第 3 步的设计准备：design fixture 覆盖四条 action 的安全 request / result 样本、transient secret 替代占位、forbidden material 拒绝样本、allowlist 错误 envelope、幂等性状态和代表 diagnostics 场景。后续仍不进入真实 command code，直到本文停止线齐备。

## 进入真实实现前的停止线

满足以下条件前，不允许把 checklist 转成真实 bridge 命令：

- 平台私钥 backend 在目标平台解除 production gate，并有 smoke 或等价证据。
- 恢复码 setup / restore 与设备 join / revocation 的真实交互测试设计完成。
- 发布级目标部署证据通过校验，并只以 `deployment_evidence_summary.v1` 摘要进入 manager。
- `ManagerBridge` contract、C ABI、Rust FFI ownership、host smoke 设计和错误 envelope 已有专题文档或 ADR。
- settings / sync / diagnostics 对新增 command 状态的可见层回归已准备。
- forbidden material 的拒绝测试已覆盖 token、恢复码、短码、signature bytes、wrapped material、payload bytes、请求 / 响应体和真实路径。

如果实现过程中发现必须让 Flutter 长期持有同步材料、授权包材料或 payload bytes，应停止该方向，回到 Rust / sync / crypto 边界重新设计。
