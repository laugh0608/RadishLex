# Manager Sync FFI Command Boundary

本文档定义 future `ManagerBridge` 同步命令进入 `ime-ffi` / C ABI 前的 ownership、错误语义、secret 生命周期和 host smoke 设计。读者是准备审阅 Flutter manager、Dart FFI bridge、`crates/ime-ffi`、`ime-sync` 和 `ime-crypto` 接线的人。本文不定义当前可执行接口，不新增 C ABI symbol，不定义 Go server API 字段全集，不承载恢复码格式、KDF 参数、签名格式、授权包 payload 或真实网络执行流程。

## 当前结论

真实同步命令仍未开放。本文只把 `recovery_setup`、`recovery_restore`、`join_request_authorization` 和 `device_revocation` 进入 FFI 前必须满足的边界固定下来，作为后续实现前的审阅输入。

当前仍保持：

- 不修改 `apps/radishlex-manager/lib/src/bridge/manager_bridge.dart`。
- 不新增 `ime-ffi` symbol、Rust FFI handler 或 Dart native binding。
- 不新增同步页按钮、点击回调、settings draft action payload 或真实 bridge 调用。
- 不生成恢复码、不输入恢复码、不创建 join request、不签名授权包、不撤销设备。
- 不上传、下载或合并真实远端 P2 对象。

本文依赖：

- `docs/ffi-boundary.md`
- `docs/runbooks/ffi-platform-call-contract.md`
- `docs/manager-sync-entry-boundary.md`
- `docs/manager-sync-action-protocol-preview.md`
- `docs/manager-sync-bridge-command-contract-checklist.md`
- `docs/manager-sync-bridge-command-contract.md`
- `docs/manager-sync-ffi-command-contract-test-plan.md`
- `apps/radishlex-manager/test/fixtures/sync_ffi_command_boundary_fixtures.dart`
- `apps/radishlex-manager/test/models/manager_sync_ffi_command_boundary_test.dart`

## 推荐 ABI 策略

后续真正进入 C ABI 时，优先采用一个 versioned manager sync command envelope 入口，而不是立即新增四个专用 C ABI symbol。

推荐方向：

- Dart `ManagerBridge` 仍可以保留四个语义清晰的方法。
- Dart FFI binding 将四个方法映射到一个 versioned native command executor。
- Rust FFI 根据 `action_id`、`schema_version` 和 action-specific typed section 路由到 Rust sync / crypto 逻辑。
- native executor 返回一个 Rust-owned command result handle，Dart 只复制安全摘要字段，随后释放 handle。

这个选择的原因：

- 四条 action 共享 readiness 绑定、operation id、错误 envelope、幂等性、diagnostics allowlist 和 forbidden material policy。
- 单一 versioned executor 可以集中做 schema version、unknown action、redaction policy、panic boundary 和 forbidden material 检查。
- 后续 action 增减时先扩展 versioned request model 和 contract test，不让 C ABI symbol 数量在设计未稳定时扩散。
- Flutter 侧仍使用 typed Dart model，不把 C ABI envelope 直接暴露给 UI。

限制：

- versioned envelope 不等于 JSON tunnel。不得用任意 JSON request / response 承载同步命令。
- envelope 必须是可审计的 typed `repr(C)` request，包含固定 action enum、公共安全字段和 action-specific typed section。
- 任何未知 schema version、未知 action、未知 status、未知 error code 或不安全 redaction policy 都必须拒绝或降级为 allowlist 错误。
- 如果实现需要把 Go server 请求 / 响应体、encrypted payload bytes、signature bytes、wrapped material 或 provider exception 原文放入 envelope，应停止该方向。

四个专用 symbol 只有在下面条件同时满足时才重新评估：

- 四条 action 的生命周期和 ownership 明显分叉，单一 executor 反而隐藏关键约束。
- 每个 symbol 的 request / result `repr(C)` 结构、版本号、释放函数和 host smoke 都已独立固定。
- 专用 symbol 不会让 Dart FFI 层或 UI 层接触 opaque crypto material / transport payload。

## C ABI 候选边界

以下是候选边界，不是当前实现承诺。

候选入口形态：

```text
radishlex_manager_sync_command_execute_v1(request, result_out, error_out)
radishlex_manager_sync_command_result_*()
radishlex_manager_sync_command_result_free(result)
```

候选 request 公共字段：

| 字段 | C ABI 形态 | 说明 |
| --- | --- | --- |
| `schema_version` | integer | 必须等于当前 command ABI version。 |
| `action_id` | integer enum | 四条 action 之一；未知值返回 `InvalidArgument`。 |
| `operation_id` | borrowed UTF-8 view | 非敏感随机标识；只在调用期间借用。 |
| `readiness_snapshot_id` | borrowed UTF-8 view | 当前净化 readiness / evidence 摘要绑定标识。 |
| `deployment_evidence_source_tag` | borrowed UTF-8 view | allowlist 来源标签，不是证据包正文。 |
| `device_backend_gate` | integer enum 或 borrowed UTF-8 view | 平台私钥 backend production gate 摘要。 |
| `explicit_user_start` | `u8` | 只接受 `0` 或 `1`。 |

候选 request action section 只允许承载 `safe_summary` 和受控 `transient_secret` 输入。`opaque_crypto_material` 和 `transport_payload` 不允许出现在 C ABI request 中。

候选 result handle 只允许暴露：

- `schema_version`
- `action_id`
- `command_status`
- `error_code`
- `retry_policy`
- `user_visible_summary_code`
- `diagnostics_summary_code`
- `next_required_evidence`
- 对象类型、计数、版本、时间或 source tag 的聚合摘要

候选 result handle 禁止暴露：

- 恢复码、短码、token、私钥、公钥原文。
- KDF 输出、sync master key、device key、signature bytes、wrapped material、encrypted payload bytes。
- Go server 请求体、响应体、错误 body、内部 `blob_ref`、日志正文或证据包正文。
- provider exception 原文、transport error body、本机真实路径。

## Ownership 规则

request ownership：

- request 结构由调用方分配，只在一次 C ABI 调用期间有效。
- request 中的 UTF-8 view 只允许被 Rust 在调用期间读取；Rust 如需跨 async / worker 边界使用，必须复制到 Rust-owned 安全结构。
- Flutter / Dart 不得把 request 结构或 pointer 存入 settings draft、诊断报告、fixture golden 或 widget state。
- operation id 必须是非敏感随机标识，不得由恢复码、短码、token、设备私钥或 payload hash 派生。

result ownership：

- `result_out` 成功时返回 Rust-owned opaque handle。
- Dart FFI 层必须立即把 string view / enum / count 复制到 Dart-owned 安全摘要 model，然后调用对应 `*_free`。
- result handle 释放后，所有从 result 借出的 view 立即失效。
- `radishlex_manager_sync_command_result_free(NULL)` 必须安全无效果。
- result handle 不得长期保存在 Flutter UI、settings store 或异步回调中。

error ownership：

- ABI 参数错误、非法 UTF-8、非法 bool、未知 schema version、未知 action 和 panic boundary 失败通过 `RadishLexError*` 表达。
- 可预期的 command 业务失败通过 command result envelope 表达，例如 readiness 阻塞、恢复记录缺失、join request 过期、网络不可达或版本冲突。
- 调用失败后 Dart 必须读取 `RadishLexStatusCode` 和错误消息，然后释放 `RadishLexError*`。
- Flutter 分支只能依赖 status code 和 allowlist command error code，不得依赖 provider 原始错误消息。

## Secret 生命周期

真实同步命令可能有两类短暂可见 secret：

- 用户输入的恢复码或短码。
- 首台设备 setup 后需要展示一次的恢复码。

规则：

- transient secret 只能在用户显式操作内出现。
- transient secret 不得进入 settings draft、诊断报告、日志、测试 golden、截图或 crash report。
- 用户输入的恢复码 / 短码进入 FFI 时只能作为 borrowed view；Rust 侧如需处理，必须复制到短生命周期内部结构，并在操作结束后清理。
- 恢复码 setup 的一次性展示材料必须单独设计生命周期。它可以作为受控一次性 UI display material 返回，但不得复用普通 command result summary handle，也不得进入 diagnostics mapper。
- 一次性展示材料必须有明确的用户确认保存状态；未确认保存前不能解锁首次上传。
- 如果某个平台无法避免输入法候选、系统自动填充、日志或截图捕获 transient secret，应先补平台交互设计，不进入真实 command。

opaque crypto material 和 transport payload 的规则：

- 只允许存在于 Rust sync / crypto / transport 内部。
- 不穿过 Flutter `ManagerBridge` 可见 contract。
- 不通过 `RadishLexBuffer`、string view、JSON envelope、diagnostics 字段或 error message 暴露。
- 不由 Flutter 长期持有、缓存、重试或合并。

## 线程、并发与取消

同步 command 不应复用输入热路径 `RadishLexSession*`。

后续实现前必须先决定：

- manager sync command 是否使用独立 Rust command context handle。
- Dart FFI 是否使用专门的串行 command worker，避免阻塞 Flutter UI isolate。
- 同一 action 的重复点击由 UI 和 Rust 双层阻止还是合并。
- 四条 action 是否全局互斥，或只在修改同一 sync domain / key epoch 时互斥。
- 网络超时后如何凭 `operation_id` 做安全重试。
- 进程退出后如何从 Rust / userdb / sync 可审计状态恢复摘要，而不是从 Flutter 临时状态恢复 secret。

当前建议：

- 不把同步 command 绑到输入 session owner thread。
- 使用 manager 专用 command context 或 stateless executor，由 Rust 内部保证同一 sync domain 的修改串行化。
- Flutter 只展示 pending / blocked / result 摘要，不持有重试所需的 payload。
- 取消只取消 UI 等待和后续展示；已经进入 Rust / sync / transport 的事务必须由 Rust 以幂等状态收敛。

## 错误语义

C ABI 层错误和 command 业务错误必须分层。

ABI 层使用现有 `RadishLexStatusCode`：

| Status | 用途 |
| --- | --- |
| `InvalidArgument` | 空指针、非法 UTF-8、非法 bool、未知 schema version、未知 action、未知 enum。 |
| `InvalidState` | 当前构建不支持 sync command、command context 未初始化、并发互斥冲突。 |
| `SyncError` | Rust sync command 执行入口出现可归类同步错误，但无法形成安全 result envelope。 |
| `InternalError` | panic boundary 捕获或内部不变量失败。 |

command result envelope 使用 allowlist 错误码：

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

映射规则：

- native provider exception 只能映射为 allowlist code 和 summary code。
- Go server HTTP status 只能映射为安全 transport / auth / conflict / unavailable 类别。
- 请求 / 响应体、error body、provider message、真实路径和 token 不能进入 `RadishLexError` message 或 command result envelope。
- `unexpected_bridge_error` 必须保守阻塞真实操作，并要求补证据或实现修正，不能作为继续上传 / 授权的可重试成功路径。

## Host Smoke 设计

真实 C ABI symbol 前，先补 Rust host smoke 设计和 Dart FFI smoke 设计。两者都不连接真实远端同步，不读取真实输入法目录，不触碰真实 Keychain / Keystore，除非单独进入 gated 平台 smoke。

当前这组 smoke 仍是 design fixture / model test 证据，不是已落地的 Rust C ABI smoke。测试真相源位于 `sync_ffi_command_boundary_fixtures.dart`，用于固定推荐 ABI 策略、候选 symbol 名称、request / result field policy、ownership rule、ABI status 分层、Rust host smoke 项和 Dart FFI smoke 项；进入 Rust / Dart contract test 的分层计划见 `docs/manager-sync-ffi-command-contract-test-plan.md`。

Rust host smoke 至少覆盖：

- ABI contract 版本识别和 sync command capability 未启用时的 `InvalidState`。
- unknown schema version、unknown action、非法 bool、非法 UTF-8 和空指针返回稳定 status 并释放 error。
- request 中 borrowed UTF-8 view 在调用内复制，调用后不持有外部 pointer。
- result handle 的 string view 复制、释放和 `*_free(NULL)`。
- command result envelope 只包含 allowlist status / error / retry / evidence 字段。
- forbidden material 样本不会出现在 result、error message 或 Debug 输出中。
- panic boundary 捕获后返回 `InternalError`，不穿过 C ABI。
- 同一 sync domain 的并发 command 返回结构化互斥状态或被串行化。

Dart FFI smoke 至少覆盖：

- `DynamicRadishLexManagerNativeBinding` 能识别 sync command capability 缺失，并保持当前 UI 关闭态。
- Dart 复制 result 摘要后立即释放 native handle。
- forbidden material 样本不会进入 `ManagerDiagnosticsReport`、settings draft 或 widget 可见文本。
- 用户输入的 transient secret 不写入 settings JSON、诊断导出或测试 golden。
- native 返回 unknown status / unknown error code 时，mapper 降级为安全阻塞。
- `ffi_library_load_failed`、`InvalidArgument`、`SyncError` 和 command envelope error 能映射成已有 bridge failure / action error 分类。

后续端到端 smoke 另行设计：

- 本地 Docker / 本地 HTTPS 只允许作为开发联调，不构成发布级证据。
- 涉及真实平台私钥 backend、系统 Keychain / Keystore、Android connected test、Apple Keychain item 或目标部署环境时，必须单独授权或人工准备。
- 发布级目标部署证据必须通过 `deployment_evidence_summary.v1` 非敏感摘要进入 manager，不把证据包正文传入 FFI。

## Dart Bridge 映射

future Dart 层仍应保持两级模型：

- `ManagerBridge` 暴露 typed action 方法和 typed result。
- `DynamicRadishLexManagerNativeBinding` 只处理 native pointer、copy / free、status code 和安全 DTO 映射。

Dart 层禁止：

- 把 native envelope 当作 `Map<String, dynamic>` 透传到 UI。
- 把 transient secret 存进 `ManagerSnapshot`、`ManagerSettingsDraft` 或 `ManagerDiagnosticsReport`。
- 在 widget tests 里使用真实恢复码、短码、token、签名、wrapped material 或 payload bytes。
- 用原始 `RadishLexError.message` 作为用户可见文案或诊断字段。

Dart 层允许：

- 显示 action id、command status、retry policy、next required evidence 和用户可读 summary code。
- 显示对象计数、版本摘要、设备状态摘要和时间摘要。
- 对一次性恢复码展示使用单独 UI 状态和测试替代占位，但必须避开 diagnostics / settings。

## 进入实现前停止线

满足以下条件前，不允许新增 sync command C ABI symbol：

- 本文的 ABI strategy、ownership、secret 生命周期、错误分层和 host smoke 设计已经评审。
- `docs/manager-sync-bridge-command-contract.md` 的 DTO 分类与本文一致。
- Dart contract model 测试覆盖 ABI strategy、unknown schema、unknown status、unknown error code、unsafe redaction、ownership、host smoke 计划和 forbidden material。
- Rust FFI ownership test 和 Dart FFI smoke 设计完成。
- 平台私钥 backend、恢复 / 授权交互测试和发布级部署证据的停止线仍清晰。
- settings / sync / diagnostics 可见层回归能够证明真实 command 不会泄漏 secret 或 payload。

如果真实实现需要 Flutter 长期持有同步材料、授权包材料、signature bytes、wrapped material、payload bytes 或 Go server 请求 / 响应体，应停止该方向，回到 Rust / sync / crypto 边界重新设计。
