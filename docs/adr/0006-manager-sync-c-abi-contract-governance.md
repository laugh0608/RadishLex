# ADR 0006: Manager 同步命令 C ABI Contract 治理

本文档固定 future manager 同步命令进入真实 Rust host contract test、C ABI symbol 和 Dart native binding 前的治理边界。读者是后续实现 `ime-ffi` 同步命令、Flutter manager bridge、Rust sync / crypto 接线和审阅隐私边界的开发者。本文不定义当前可执行接口，不新增 C ABI symbol，不定义 Go server API、恢复码格式、授权包 payload、设备撤销 payload 或真实远端同步流程。

## 状态

Accepted

## 背景

Phase 4 manager 同步入口当前只做非上传治理。仓库已补齐真实 bridge 命令前 contract 草案、FFI command boundary、contract test plan、Rust host contract catalog、review catalog、source checklist、test design package、Dart fake binding replay、C ABI contract review matrix、implementation review package、Rust 内部非导出草案模块、C ABI wrapper 形状评审、result accessor field set 评审、summary storage / object count width 评审、command context owner scope 评审、worker thread policy 评审、Debug redaction test shape 评审和 host contract test gate 迁出条件草案。

这些材料证明 future command contract 的安全边界、ownership、错误分层和 smoke 计划可复验，但不证明 native command 已实现。若后续直接创建 C ABI symbol 或 `ManagerBridge` 可执行方法，容易绕过以下边界：

- 恢复码、短码、签名、wrapped material、payload bytes、请求 / 响应体和真实路径不得进入 manager 状态、settings、diagnostics、fixture golden 或日志。
- Rust-owned result handle、borrowed UTF-8 view、error handle 和 release function 必须有明确生命周期。
- panic 不得穿过 C ABI，unknown native status 不得被解释为可继续操作。
- 同一 sync domain 写入命令必须有串行化或结构化互斥状态。
- 当前阶段仍不打开恢复码生成 / 输入、join request 创建、授权成功、设备撤销或真实远端同步。

## 决策

Manager 同步命令采用单一 versioned C ABI command executor 方向，但在真实 symbol 前保持关闭：

```text
radishlex_manager_sync_command_execute_v1
radishlex_manager_sync_command_result_*
radishlex_manager_sync_command_result_free
```

该方向只作为治理目标，不代表当前已批准新增 symbol。进入真实实现前必须先满足本 ADR 的决策条件，并保持 `docs/manager-sync-ffi-command-boundary.md`、`docs/manager-sync-ffi-command-contract-test-plan.md`、`sync_ffi_command_boundary_fixtures.dart` 和 `sync_ffi_rust_host_contract_review_fixtures.dart` 同步。

真实 C ABI 设计必须先固定六类结构评审：

1. request struct layout
2. result struct layout
3. release / error lifecycle
4. panic / status boundary
5. command context serialization
6. forbidden material contract

这些评审项的当前测试真相源是 `syncFfiRustHostContractReviewItems`，状态为 `c_abi_contract_review_ready_no_native_symbol`。

## Implementation Review Package

`syncFfiRustHostImplementationReviewItems` 是本 ADR 到真实 Rust host test 之间的实现前审阅包。它把每个 C ABI contract review item 进一步映射到：

- 后续 Rust 侧可能需要的 `manager_sync_command.rs::*Draft` 类型 / 函数草案。
- 当前可复用的 `ime-ffi` 源码模式，例如 `ffi_status`、`ffi_ptr`、`ffi_release`、`read_utf8`、`read_ffi_bool`、error handle 和 sync preflight summary。
- 进入真实 Rust host test 前仍需确认的实现问题，例如 action id 数值、真实 C ABI action section encoding、future dynamic summary storage owner、future object count source、future worker queue API shape 和 diagnostics summary allowlist source。

该包的状态为 `rust_host_implementation_review_ready_no_native_symbol`。它已经驱动 `crates/ime-ffi/src/manager_sync_command.rs` 落地为内部非导出草案模块，但不代表 C ABI symbol、Dart native binding、`ManagerBridge` 可执行方法或 `crates/ime-ffi/tests/manager_sync_command_boundary.rs` 已经落地。

## C ABI Wrapper Shape Review

`ManagerSyncCommandCAbiWrapperShapeReviewDraft` 是当前内部草案中的 C ABI wrapper 形状评审材料。它只记录候选 symbol 名称、executor 策略、result handle 策略和 error lifecycle 策略，不导出 symbol。

当前候选 symbol 与 dynamic library smoke 保持一致：

- `radishlex_manager_sync_command_execute_v1`
- `radishlex_manager_sync_command_result_action_id`
- `radishlex_manager_sync_command_result_status`
- `radishlex_manager_sync_command_result_error_code`
- `radishlex_manager_sync_command_result_retry_policy`
- `radishlex_manager_sync_command_result_summary`
- `radishlex_manager_sync_command_result_free`

当前所有候选 symbol 的状态都是 `planned_not_exported_current_phase`，`export_approved=false`。error lifecycle 继续复用现有 `radishlex_error_code`、`radishlex_error_message` 和 `radishlex_error_free`，不新增 manager 专用 error symbol。

`ManagerSyncCommandHostContractTestGateDraft` 当前状态为 `host_contract_test_gate_closed_no_native_symbol`，`ready_for_host_contract_test=false`。阻塞项包括 native symbol export、Dart native binding、`ManagerBridge` command、host contract test 文件和真实同步执行均未批准。`ManagerSyncCommandHostGateMigrationReviewDraft` 进一步固定 gate 迁出条件：result accessor field set、summary storage policy、object count `u64` width、command context owner scope、command worker thread policy、Debug redaction test shape、forbidden material redaction、ADR 批准、Dart binding、`ManagerBridge` command、host test 文件、dynamic library symbol smoke 和真实同步执行批准必须同时具备。它们只把进入 `crates/ime-ffi/tests/manager_sync_command_boundary.rs` 前需要复验的证据固化为内部草案，不创建该测试文件。

## Rust Internal Draft Module

`crates/ime-ffi/src/manager_sync_command.rs` 是当前阶段允许存在的内部实现草案。它只在 Rust crate 内部编译和测试，不在 `abi.rs` 导出 `extern "C"`，不加入动态库 symbol 列表，也不修改 Dart native binding。

当前内部草案只覆盖：

- versioned raw request draft、action id allowlist、borrowed UTF-8 view 读取、`u8` bool 校验和 forbidden material 拒绝。
- current-phase capability closed 结果摘要，返回稳定 `InvalidState`，只包含 allowlist summary code。
- action-specific section draft，按 action id 绑定当前阶段允许的确认状态码、前置证据码和 transient material 状态码；不接受短码值、恢复码值或 payload-shaped 字段。
- Rust-owned result handle draft、summary view draft、copy-before-release 断言、release 后 view 失效和 `release(None)` 无效果。
- result accessor field set review draft，把 `schema_version`、`action_id`、`command_status`、`error_code`、`retry_policy`、`user_visible_summary_code`、`diagnostics_summary_code`、`next_required_evidence`、对象摘要和 recorded-at 摘要绑定到当前候选 accessor / summary 组；对象计数固定为 `u64` 草案，全部保持 `planned_not_exported_current_phase`。
- summary storage review draft，当前阶段使用静态 summary code table，future 动态摘要必须由 Rust-owned result handle 持有；borrowed view 只在 release 前有效，不保存 caller pointer、provider message 或 transport payload。
- error handle draft，覆盖 read -> copy -> release 生命周期、release 后失效、`release(None)` 无效果和 forbidden provider message 拒绝。
- 内部 panic boundary helper，将 unwind 映射为稳定 `InternalError`，不回显 panic payload。
- manager sync command context draft，同一 sync domain 重入返回结构化 `InvalidState`，guard drop 后释放 domain；context owner scope 绑定创建线程，跨线程使用返回结构化 `InvalidState`，且不持有 Flutter widget state、settings payload、Dart pointer 或平台 UI object。
- command worker thread policy review draft，当前阶段只允许 caller thread owner check；真实同步前必须另行评审单一串行 worker、owner 迁移、消息边界和取消语义，不允许后台远端重试或 secret payload 队列。
- C ABI wrapper shape review draft，固定候选 executor / result accessor / release symbol 名称、现有 error lifecycle symbol 复用和 export approval 关闭态。
- host contract test gate draft 和 gate migration review draft，记录当前进入真实 host contract test 文件前仍关闭的阻塞项、required evidence 和迁出条件。
- Debug redaction test shape review draft，覆盖 request error、command result、result accessor field、owner scope、worker policy 和 gate migration review 的 Debug 脱敏断言形状。
- Debug / error message / result summary 不回显 operation id、readiness snapshot、source tag、backend gate 或 secret-shaped 片段。

当前内部草案不覆盖：

- C ABI `extern "C"` wrapper、真实 result accessor symbol、真实 release function symbol 或新的 manager 专用 error handle symbol。
- 跨线程 / 异步 command worker、Rust sync / crypto 接线或远端 transport。
- `ManagerBridge` 方法、Flutter 可见操作按钮、settings action payload、恢复码生成 / 输入、join request 创建、授权成功或设备撤销。

## Request Struct 规则

request struct 必须保持 summary / enum / borrowed view 形状：

- `schema_version` 使用稳定整数表示，unknown version 返回 `InvalidArgument`。
- `action_id` 使用稳定整数或 allowlist enum，unknown action 返回 `InvalidArgument`。
- `operation_id`、`readiness_snapshot_id`、`deployment_evidence_source_tag` 使用 borrowed UTF-8 view，Rust 侧只允许在调用期复制非敏感摘要。
- `explicit_user_start` 使用 `u8` bool，非法值返回 `InvalidArgument`。
- action-specific section 只能包含安全状态码、确认状态码、前置证据码和非敏感摘要，不包含恢复码值、短码值、signature bytes、wrapped material、transport payload 或请求 / 响应体。
- request 由 caller 在调用期持有；Rust 不保存 caller pointer，不把 request view 放入异步状态。

## Result Struct 规则

result 必须由 Rust-owned handle 承载，并只暴露 safe summary view：

- Dart / 平台 binding 必须复制 summary 后调用 release function。
- result view 在 release 后失效，不得跨调用或跨 isolate 持有。
- result accessor field set 只能包含 `schema_version`、`action_id`、allowlist command status、error code、retry policy、user-visible summary code、diagnostics summary code、next required evidence、object type / `u64` count / version summary 和 recorded-at summary。
- current-phase summary 可来自静态 code table；future dynamic summary 必须由 Rust-owned result handle 持有，不能借用 caller request、provider message 或 transport payload。
- result 不包含 token、恢复码、短码、signature bytes、wrapped material、payload bytes、请求 / 响应体、provider exception 原文、真实路径或平台 key backend secret。
- `*_free(NULL)` 必须安全返回。

## Error 与 Panic 规则

错误语义必须稳定区分：

- `InvalidArgument`：unknown schema、unknown action、非法 bool、非法 UTF-8、空指针、输出指针非法。
- `InvalidState`：当前阶段 command capability 关闭、同一 sync domain 写入互斥、owner context 不满足。
- `SyncError`：Rust sync / crypto 返回结构化错误，但不能泄漏 payload 或 provider 原文。
- `InternalError`：panic boundary 捕获或内部不变量失败。

规则：

- panic 必须由 Rust C ABI entry 捕获，不能穿过 C ABI。
- error handle 必须支持 read -> free 生命周期。
- unknown native status 在 Dart binding 侧降级为安全阻塞，不得启用按钮、重试上传或写 settings action。
- error message 只允许诊断摘要，不得包含真实路径、请求体、响应体、payload bytes、secret-shaped 字段或平台 provider 原文。

## Command Context 规则

真实 command context 必须先定义：

- 当前阶段 capability gate 在所有真实操作前执行。
- 同一 sync domain 的写入命令串行化，或返回结构化互斥状态。
- command context owner scope 必须明确；当前内部草案绑定创建线程，跨线程使用返回 `InvalidState`，后续若引入专用 worker 必须重新评审 owner 迁移规则。
- command worker thread policy 必须先评审；真实同步前只允许一个 manager sync 串行 worker 方向，不允许 Flutter UI isolate 阻塞、secret payload 排队或后台远端重试。
- `operation_id` 只能是非敏感幂等摘要，不从 secret、payload 或用户输入派生。
- command context 不持有 Flutter widget state、settings draft payload、Dart pointer 或平台 UI object。
- 后台远端重试、隐式设备状态推进和真实上传必须等真实同步入口开放后单独设计。

## 停止线

在本 ADR 的实现前评审通过前，不允许：

- 新增 C ABI symbol。
- 新增 `ManagerBridge` 可执行同步方法。
- 创建 `crates/ime-ffi/tests/manager_sync_command_boundary.rs` 中暗示 symbol 已存在的真实调用测试。
- 执行远端同步、连接 Go server 或触碰平台 key backend。
- 写 settings action payload。
- 创建恢复码、join request、授权包、撤销记录或 encrypted P2 payload。
- 在 diagnostics、settings、widget text、fixture golden 或日志中输出 forbidden material。

## 进入真实 Rust Host Test 的条件

创建 `crates/ime-ffi/tests/manager_sync_command_boundary.rs` 前必须满足：

- `syncFfiRustHostContractReviewItems` 覆盖全部 `syncFfiCommandBoundaryRustHostTestDesignItems`。
- `syncFfiRustHostImplementationReviewItems` 覆盖全部 C ABI contract review items，并且内部 Rust draft module 的单元测试继续通过。
- request struct、result struct、result accessor field set、summary storage、object count width、release function、error handle、panic boundary、command context、owner scope、worker thread policy 和 Debug redaction test shape 已有明确 Rust 类型草案或等价实现说明。
- C ABI wrapper 形状评审必须继续证明候选 symbol 仅为字符串级审阅材料，host contract test gate 和 gate migration review 必须通过 ADR / 文档 / smoke 证据明确打开后才能创建真实测试文件。
- forbidden material contract 已映射到 test assertions。
- Dart fake binding replay 继续证明 unknown native status、copy / free 和错误分类保持安全阻塞。
- 真实 dynamic library smoke 仍能证明未批准前 candidate sync command symbol 缺席。
- `docs/status/current.md`、`docs/manager-sync-ffi-command-boundary.md`、`docs/manager-sync-ffi-command-contract-test-plan.md` 和本 ADR 同步更新。

## 验证口径

普通推进至少运行：

```bash
cargo test -p radishlex-ime-ffi manager_sync_command
flutter test test/models/manager_sync_ffi_command_boundary_test.dart
flutter test test/models/manager_sync_ffi_command_boundary_test.dart test/models/manager_sync_ffi_binding_contract_test.dart
./scripts/check-manager.sh
./scripts/check-docs.sh
git diff --check
```

如果后续真实 C ABI symbol 落地，还必须新增 Rust host contract test、Dart FFI binding contract test、真实 dynamic library smoke 和 manager 可见层回归；涉及平台私钥 backend、恢复 / 授权交互或发布级部署证据时再扩大验证范围。

## 后果

收益：

- 在真实 symbol 前固定 request / result / release / error / panic / command context 的共同边界。
- 让 Rust host test、Dart binding test 和 manager 可见层回归使用同一证据链。
- 降低 future command 接线时泄漏 transient secret、payload、provider 原文或真实路径的风险。

代价：

- 真实同步命令仍不能立即实现或暴露。
- 后续如果 C ABI shape 变化，必须同步更新 fixture、ADR、FFI boundary、contract test plan 和回归测试。
- 进入真实 Rust host test 前仍需要一次实现级评审，而不是只依赖文档通过。
