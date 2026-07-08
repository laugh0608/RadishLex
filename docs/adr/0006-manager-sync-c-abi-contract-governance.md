# ADR 0006: Manager 同步命令 C ABI Contract 治理

本文档固定 future manager 同步命令进入真实 Rust host contract test、C ABI symbol 和 Dart native binding 前的治理边界。读者是后续实现 `ime-ffi` 同步命令、Flutter manager bridge、Rust sync / crypto 接线和审阅隐私边界的开发者。本文不定义当前可执行接口，不新增 C ABI symbol，不定义 Go server API、恢复码格式、授权包 payload、设备撤销 payload 或真实远端同步流程。

## 状态

Accepted

## 背景

Phase 4 manager 同步入口当前只做非上传治理。仓库已补齐真实 bridge 命令前 contract 草案、FFI command boundary、contract test plan、Rust host contract catalog、review catalog、source checklist、test design package、Dart fake binding replay 和 C ABI contract review matrix。

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

`syncFfiRustHostImplementationReviewItems` 是本 ADR 到真实 Rust host test 之间的实现前审阅包。它不新增代码实现，只把每个 C ABI contract review item 进一步映射到：

- 后续 Rust 侧可能需要的 `manager_sync_command.rs::*Draft` 类型 / 函数草案。
- 当前可复用的 `ime-ffi` 源码模式，例如 `ffi_status`、`ffi_ptr`、`ffi_release`、`read_utf8`、`read_ffi_bool`、error handle 和 sync preflight summary。
- 进入真实 Rust host test 前仍需确认的实现问题，例如 action id 数值、action-specific section layout、result accessor field set、domain guard storage scope 和 Debug redaction test shape。

该包的状态为 `rust_host_implementation_review_ready_no_native_symbol`。它只说明真实实现需要评审哪些 Rust 类型 / 函数边界，不代表 `crates/ime-ffi/src/manager_sync_command.rs`、C ABI symbol 或 `crates/ime-ffi/tests/manager_sync_command_boundary.rs` 已经落地。

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
- result envelope 只能包含 allowlist command status、error code、retry policy、user-visible summary code、diagnostics summary code、next required evidence、object type / count / version summary 和 recorded-at summary。
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
- `syncFfiRustHostImplementationReviewItems` 覆盖全部 C ABI contract review items，并且 proposed Rust artifacts 仍保持 draft 状态。
- request struct、result struct、release function、error handle、panic boundary 和 command context 已有明确 Rust 类型草案或等价实现说明。
- forbidden material contract 已映射到 test assertions。
- Dart fake binding replay 继续证明 unknown native status、copy / free 和错误分类保持安全阻塞。
- 真实 dynamic library smoke 仍能证明未批准前 candidate sync command symbol 缺席。
- `docs/status/current.md`、`docs/manager-sync-ffi-command-boundary.md`、`docs/manager-sync-ffi-command-contract-test-plan.md` 和本 ADR 同步更新。

## 验证口径

普通推进至少运行：

```bash
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
