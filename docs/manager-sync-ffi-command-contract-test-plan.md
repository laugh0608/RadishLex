# Manager Sync FFI Command Contract Test Plan

本文档定义 future manager 同步命令进入 Rust / Dart contract test 前的测试分层、证据映射和停止线。读者是准备从 `sync_ffi_command_boundary_fixtures.dart` 推进到 `ime-ffi` host contract test、Dart FFI binding test 和 manager 可见层回归的人。本文不定义真实 `ManagerBridge` 方法，不新增 C ABI symbol，不定义 Go server API、恢复码格式、签名格式、授权包 payload 或真实远端同步流程。

## 当前结论

当前阶段只能把 FFI command boundary 转成 design fixture、model test 和测试计划。真实 C ABI、Dart native binding、恢复码输入、join request、授权成功和设备撤销仍保持关闭。

当前已完成：

- `apps/radishlex-manager/test/fixtures/sync_ffi_command_boundary_fixtures.dart`
- `apps/radishlex-manager/test/models/manager_sync_ffi_command_boundary_test.dart`
- `apps/radishlex-manager/test/models/manager_sync_ffi_binding_contract_test.dart`
- `apps/radishlex-manager/test/fixtures/sync_transient_secret_interaction_fixtures.dart`
- `apps/radishlex-manager/test/models/manager_sync_transient_secret_interaction_test.dart`
- `apps/radishlex-manager/test/ffi_manager_bridge_test.dart` 中 fake native capability missing 回归
- settings gate preview、同步页和诊断报告中的 recovery visible-layer 状态码回归
- `syncFfiCommandBoundaryRustHostContractCases` host contract catalog fixture
- `syncFfiCommandBoundaryRustHostReviewItems` Rust host review fixture
- `syncFfiCommandBoundaryRustHostSourceChecklistItems` Rust host source-level checklist fixture
- `syncFfiCommandBoundaryRustHostTestDesignItems` Rust host test design fixture
- `syncFfiRustHostContractReviewItems` Rust host C ABI contract review fixture
- Dart fake native binding 对 host catalog 的非执行 replay 回归
- `apps/radishlex-manager/tool/ffi_bridge_smoke.dart` 中真实 dynamic library future sync command symbol 缺席检查
- `docs/manager-sync-ffi-command-boundary.md`
- `crates/ime-ffi/src/manager_sync_command.rs` 内部 C ABI wrapper shape review 与 host contract test gate 草案

这些证据只证明未来 contract 的安全边界、ownership、错误分层和 smoke 计划可复验，不证明 native command 已实现。

2026-07-08 已新增并扩展 `crates/ime-ffi/src/manager_sync_command.rs` 内部非导出草案模块，把 implementation review package 中的 request parsing、action-specific section、当前阶段 capability gate、result handle / release、error handle lifecycle、panic boundary、sync domain guard、C ABI wrapper shape review、host contract test gate 和 forbidden material 检查先落到 Rust 单元测试。该模块不新增 C ABI symbol，不创建 `crates/ime-ffi/tests/manager_sync_command_boundary.rs`，不修改 Dart native binding 或 `ManagerBridge`。

## 测试分层

| 层级 | 名称 | 当前状态 | 目标 |
| --- | --- | --- | --- |
| L0 | 文档边界 | 已完成 | 固定 ABI strategy、ownership、secret 生命周期、错误分层、host smoke 和停止线。 |
| L1 | Dart design fixture / model test | 已完成 | 用合成 fixture 验证当前没有 native symbol，future request / result 只含安全摘要，forbidden material 不进入输出。 |
| L2 | Rust host contract test 计划 | 已补评审材料、source-level checklist、test design fixture、C ABI contract review matrix，并落地内部非导出 Rust draft module / wrapper shape review / gate 草案 | 在新增 symbol 前先明确测试文件、输入样本、错误码、释放责任、既有 FFI 源码模式映射、candidate symbol 关闭态和 forbidden material 断言。 |
| L3 | Dart FFI binding contract test 计划 | 已接入 host catalog replay | 在 Dart native binding 扩展前先明确 capability missing、copy / free、unknown status、host catalog 状态重放和错误映射测试。 |
| L4 | Manager 可见层回归 | 部分已有 | 确认 settings / sync / diagnostics 不展示 secret、不写 settings draft、不打开按钮。 |
| L5 | Gated platform / deployment smoke | 后续阶段 | 平台私钥 backend、真实 Keychain / Keystore、目标部署证据和真实端到端同步必须单独授权或人工准备。 |

L2 / L3 只在 L0 / L1 继续通过、且 contract 文档没有未解决分歧时进入。L5 不能绕过 L2 / L3，也不能用本地 fixture 代替真实平台或发布级部署证据。

## 当前 Design Fixture 映射

`sync_ffi_command_boundary_fixtures.dart` 是当前测试真相源，覆盖：

- 推荐策略：`single_versioned_manager_sync_command_executor`
- 当前 native symbol 关闭态：`current_native_symbols = []`
- 候选 symbol 名称：`radishlex_manager_sync_command_execute_v1`、`radishlex_manager_sync_command_result_*`、`radishlex_manager_sync_command_result_free`
- 禁止策略：JSON tunnel、未评审专用 symbol、Flutter 可见 transport payload、Dart owned opaque crypto material
- request common field policy：schema version、action enum、operation id、readiness snapshot、deployment evidence source、device backend gate、explicit user start
- result summary field policy：command status、error code、retry policy、summary code、next evidence 和对象摘要
- ownership rule：caller-owned request、borrowed UTF-8 view、Rust-owned result、Dart copy / free、error read / free、`free(NULL)` 无效果
- transient secret rule：显式用户操作、调用期 borrowed view、Rust 短生命周期内部复制、不进入 settings / diagnostics / logs / golden
- ABI status 分层：`InvalidArgument`、`InvalidState`、`SyncError`、`InternalError`
- Rust host smoke 项和 Dart FFI smoke 项
- forbidden material 拒绝样本复用 `sync_bridge_command_contract_fixtures.dart`
- Rust host contract catalog：目标测试文件、target test name、sample id、ABI input case、expected status、required evidence 和当前 `planned_no_native_symbol` 状态
- Rust host review catalog：把每个 host contract case 绑定到既有 `ime-ffi` 模式、评审主题、非敏感证据码、停止线和当前 `review_ready_no_native_symbol` 状态
- Rust host source-level checklist：把每个 review item 的既有模式绑定到具体源码引用、测试引用和当前 `source_checklist_ready_no_native_symbol` 状态
- Rust host test design package：把每个 host contract case 绑定到 source checklist、样本、expected status、断言组、forbidden output category、实现守卫和当前 `test_design_ready_no_native_symbol` 状态
- Rust host C ABI contract review matrix：把真实 Rust host test 文件前必须评审的 request struct、result struct、release / error lifecycle、panic / status boundary、command context serialization 和 forbidden material contract 固定为 `c_abi_contract_review_ready_no_native_symbol` 状态
- Rust host implementation review package：把每个 C ABI review item 绑定到后续 Rust `manager_sync_command.rs::*Draft` 草案、当前可复用源码模式、实现说明和仍需确认的问题，状态为 `rust_host_implementation_review_ready_no_native_symbol`
- Rust 内部非导出 draft module：`crates/ime-ffi/src/manager_sync_command.rs` 已覆盖 request parsing、action-specific section allowlist、bool / UTF-8 校验、当前阶段 `InvalidState` 摘要、result handle / release、error handle lifecycle、panic boundary、sync domain guard、C ABI wrapper shape review、host contract test gate 和 forbidden material 拒绝；仍不导出 C ABI symbol。

新增或修改 future command 字段时，应先更新这份 fixture，再更新文档和测试。不要让文档表格、Dart fixture 和未来 Rust contract test 各自维护不同字段列表。

## Rust Host Contract Test 计划

后续进入 Rust host contract test 前，建议测试文件为：

```text
crates/ime-ffi/tests/manager_sync_command_boundary.rs
```

测试入口在真实 symbol 评审前只能作为计划，不应先写会暗示 symbol 已存在的测试。真实 C ABI 设计获批后，Rust host contract test 至少覆盖：

| 测试项 | 断言 |
| --- | --- |
| `contract_reports_command_capability_closed` | ABI contract 能报告当前 command capability 关闭或未启用；未启用时返回稳定 `InvalidState`。 |
| `invalid_request_inputs_return_stable_status` | unknown schema version、unknown action、非法 bool、非法 UTF-8、空指针返回 `InvalidArgument`，并可读取 / 释放 error。 |
| `result_handle_copy_then_free` | result 是 Rust-owned handle；string view 可复制；释放后 view 失效；`free(NULL)` 无效果。 |
| `envelope_allowlist_only` | result envelope 只包含 allowlist command status、error code、retry policy 和 next evidence。 |
| `forbidden_material_absent_from_native_outputs` | token、恢复码、短码、signature bytes、wrapped material、payload bytes、请求 / 响应体和真实路径不出现在 result、error message 或 Debug 输出。 |
| `panic_boundary_returns_internal_error` | panic 不穿过 C ABI，映射为 `InternalError`。 |
| `sync_domain_command_serialization` | 同一 sync domain 的写入命令被串行化或返回结构化互斥状态；operation id 不含敏感派生材料。 |

Rust host contract test 的 fixture 输入只能使用合成值：

- 合成 operation id：`op_test_non_secret_001`
- 合成 readiness snapshot id：`readiness_snapshot_test_001`
- 合成 source tag：`local_smoke`
- 合成 backend gate：`blocked` / `ready_for_contract_test`
- 合成 forbidden material 样本来自 `sync_bridge_command_contract_fixtures.dart` 的字符串类别，不使用真实 token、真实恢复码、真实路径或真实 payload。

当前 `syncFfiCommandBoundaryRustHostInputSamples` 已固定第一批 Rust host 输入样本，覆盖 current-phase capability closed、unknown schema version、unknown action、invalid bool、null pointer、invalid UTF-8、unenveloped sync error、panic boundary 和 same-domain concurrent command，预期状态分别映射到 `InvalidState`、`InvalidArgument`、`SyncError` 和 `InternalError`。`syncFfiCommandBoundaryRustHostContractCases` 再把这些样本绑定到建议的 Rust host test name、host smoke case、required evidence 和 `planned_no_native_symbol` 实现状态。

`syncFfiCommandBoundaryRustHostReviewItems` 是进入真实 Rust host test 之前的评审材料包，逐项绑定 host contract case 与现有 `ime-ffi` 模式，包括 `radishlex_ffi_contract` 版本检查、`ffi_status` / `ffi_ptr` / `ffi_release` panic boundary 与释放路径、UTF-8 和 bool 输入校验、error handle read / free、summary output pointer guard、平台 binding copy-before-release 测试、sync preflight summary 输出和 session owner-thread `InvalidState` 策略。`syncFfiCommandBoundaryRustHostSourceChecklistItems` 再把这些模式绑定到具体源码引用，例如 `crates/ime-ffi/src/abi.rs::ffi_status`、`crates/ime-ffi/src/abi.rs::read_utf8`、`crates/ime-ffi/src/abi.rs::read_ffi_bool`、`crates/ime-ffi/src/session.rs::RadishLexSession::ensure_owner_thread`、`crates/ime-ffi/tests/ffi_contract_and_dictionary.rs::platform_binding_style_copies_views_before_releasing_handles` 和 `apps/radishlex-manager/tool/ffi_bridge_smoke.dart::_expectFutureSyncCommandSymbolsAbsent`。这两组 fixture 还固定 `add_c_abi_symbol`、`add_manager_bridge_method`、`execute_remote_sync`、`write_settings_action`、`create_setup_or_authorization_material`、`connect_go_server` 和 `touch_platform_key_backend` 停止线；状态分别为 `review_ready_no_native_symbol` 和 `source_checklist_ready_no_native_symbol`，仍不创建真实 Rust test 文件或 symbol。

`syncFfiCommandBoundaryRustHostTestDesignItems` 是 source checklist 之后、真实 Rust host test 文件之前的测试设计包。它把每个 host contract case 绑定到对应 source checklist、合成 sample、expected status、断言组、forbidden output category 和 implementation guard；状态为 `test_design_ready_no_native_symbol`。该 fixture 让后续 `crates/ime-ffi/tests/manager_sync_command_boundary.rs` 的测试项可以按目录转写，但当前仍不新增该测试文件、不新增 C ABI symbol、不修改 Dart native binding。

`syncFfiRustHostContractReviewItems` 是 test design package 之后、真实 Rust host test 文件和 C ABI symbol 之前的结构评审矩阵。它把 request struct layout、result struct layout、release / error lifecycle、panic / status boundary、command context serialization 和 forbidden material contract 分别绑定到已有 test design item、source checklist、required decision、required evidence、forbidden output category 和 implementation guard；状态为 `c_abi_contract_review_ready_no_native_symbol`。该矩阵用于评审真实 C ABI request / result struct、释放函数、panic boundary 和 command context 策略是否齐备，并已绑定决策记录 `docs/adr/0006-manager-sync-c-abi-contract-governance.md`；不代表已经批准新增 symbol 或 `ManagerBridge` 可执行方法。

`syncFfiRustHostImplementationReviewItems` 是 C ABI review matrix 之后、真实 Rust host test 文件之前的实现前审阅包。它把每个 review item 绑定到 `crates/ime-ffi/src/manager_sync_command.rs::*Draft` 草案、当前可复用的 `ime-ffi` 源码模式、required implementation notes 和 unresolved implementation questions；状态为 `rust_host_implementation_review_ready_no_native_symbol`。当前已按该包新增并扩展 `crates/ime-ffi/src/manager_sync_command.rs` 内部非导出草案模块，用单元测试覆盖 request parsing、action-specific section、current-phase gate、result handle / release、error handle lifecycle、panic boundary、sync domain guard、C ABI wrapper shape review、host contract test gate 和 forbidden material redaction；但仍不新增 native symbol，不创建真实 host contract test 文件，不修改 Dart native binding，不打开真实同步。

当前 `ManagerSyncCommandHostContractTestGateDraft` 的状态是 `host_contract_test_gate_closed_no_native_symbol`，进入真实 Rust host contract test 文件前必须先证明 native symbol export、Dart native binding、`ManagerBridge` command、host test 文件和真实同步执行已经按 ADR / 文档 / smoke 证据获得批准。否则 L2 仍停留在内部单元测试和 smoke 缺席检查。

除内部 Rust draft module 及其单元测试外，这些仍是 design fixture，不新增真实 symbol。

Rust test 不应：

- 连接 Go server。
- 读取真实输入法目录。
- 创建 Keychain / Keystore item。
- 写真实 userdb 或真实 settings。
- 生成恢复码、签名授权包、撤销记录或 encrypted P2 payload。

## Dart FFI Binding Contract Test 计划

后续进入 Dart FFI binding contract test 前，建议优先使用 fake native binding，不直接要求真实 dynamic library symbol 存在。测试位置可继续放在：

```text
apps/radishlex-manager/test/
```

建议测试项：

| 测试项 | 断言 |
| --- | --- |
| `native_command_capability_missing_keeps_ui_closed` | native capability 缺失时 manager 仍派生当前阶段关闭，不启用按钮。 |
| `dart_copies_result_then_releases_handle` | Dart binding 复制 summary 后释放 native handle，不把 pointer 存入 snapshot / settings / async state。 |
| `forbidden_material_not_visible_to_manager` | forbidden material 不进入 diagnostics、settings draft、widget text。 |
| `transient_secret_not_persisted` | 恢复码 / 短码输入替代样本不写 settings JSON、diagnostics export 或 golden。 |
| `unknown_native_status_downgrades_to_safe_blocker` | unknown status / error code 降级为安全阻塞，不透传 native 原文。 |
| `ffi_and_command_errors_map_to_bridge_categories` | `ffi_library_load_failed`、`InvalidArgument`、`SyncError` 和 command envelope error 映射到既有 bridge failure / action error 分类。 |

Dart FFI binding test 的实现顺序：

1. 先用 fake native binding 验证 mapper、copy / free 调用记录和错误分类；当前已覆盖 capability missing 时 manager 继续保持当前阶段关闭、settings 不写 action payload、diagnostics 不出现 future command request / result 或 forbidden material，并用测试侧 binding harness 覆盖 summary copy -> free 顺序、unknown native status 安全降级、`ffi_library_load_failed` / `invalid_argument` / `sync_error` 分类、command error allowlist，以及从 `syncFfiCommandBoundaryRustHostContractCases` 和 host input sample 重放出的 ABI status / input case / required evidence 非敏感摘要。
2. 再用真实 dynamic library 做 capability missing smoke，确认当前没有 sync command symbol 时 UI 保持关闭；当前 `ffi_bridge_smoke.dart` 已检查 `radishlex_manager_sync_command_execute_v1` 和 result accessor / free 候选 symbol 均未导出。
3. 只有真实 C ABI symbol 评审通过后，才把 dynamic library smoke 扩展到 result handle 读取和释放。

恢复码一次性展示、恢复码输入、短码核对和显式授权 / 撤销确认的 transient secret 生命周期已由 `docs/manager-recovery-device-auth-flow.md`、`sync_transient_secret_interaction_fixtures.dart` 和 `manager_sync_transient_secret_interaction_test.dart` 固定。当前仍不显示、输入、复制、保存或传递真实 secret。

`syncRecoveryVisibleLayerFixtures` 进一步固定 recovery setup / restore 的可见层文案和确认占位：一次性展示状态、保存确认、恢复输入、恢复记录查询、失败限速和设备登记状态必须在 UI / diagnostics 中输出同一组非敏感状态码，且不写 settings action payload、不创建 bridge request payload、不暴露请求 / 响应体或真实路径。

## Manager 可见层回归

每次扩展 command contract、FFI boundary 或 fake native mapper 时，至少复验：

- `manager_sync_ffi_command_boundary_test.dart`
- `manager_sync_bridge_command_contract_test.dart`
- `manager_sync_action_preview_test.dart`
- `manager_sync_entry_gate_test.dart`
- `settings_test.dart`
- `settings_diagnostics_test.dart`
- `sync_test.dart`

可见层必须继续证明：

- 不新增真实操作按钮、点击回调或 bridge 调用。
- 不写 settings draft action payload。
- 不在 diagnostics 中输出 token、恢复码、短码、signature bytes、wrapped material、payload bytes、请求 / 响应体、provider exception 或真实路径。
- 即便 readiness 和 future shape 都 ready，当前阶段仍输出 `not_executable_current_phase` / `user_sync_entry_closed_current_phase`。
- recovery setup / restore 可见层只展示 `display_not_available_current_phase`、`required_before_first_upload`、`input_not_available_current_phase`、`not_checked_current_phase`、`not_started` 和 `blocked_until_recovery_success` 等状态码，不展示或输入真实 secret。

## 验证命令

当前阶段建议：

```bash
cargo test -p radishlex-ime-ffi manager_sync_command
flutter test test/models/manager_sync_ffi_command_boundary_test.dart test/models/manager_sync_bridge_command_contract_test.dart
flutter test test/ffi_manager_bridge_test.dart test/models/manager_sync_ffi_command_boundary_test.dart
flutter test test/models/manager_sync_ffi_binding_contract_test.dart
flutter test test/models/manager_sync_transient_secret_interaction_test.dart
flutter test test/screens/sync_test.dart test/screens/settings_test.dart test/screens/settings_diagnostics_test.dart
./scripts/check-manager-ffi-smoke.sh
./scripts/check-manager.sh
git diff --check
./scripts/check-docs.sh
```

触及入口文档、仓库脚本、Rust FFI 或公共验证基线时补跑：

```bash
./scripts/check-repo.sh
```

真实 C ABI symbol 出现后再新增：

```bash
cargo test -p radishlex-ime-ffi --test manager_sync_command_boundary
./scripts/check-manager-ffi-smoke.sh
```

上述命令不代表可以触碰真实 Keychain / Keystore、目标部署环境或真实远端同步；这些仍需单独授权。

## 进入实现前停止线

满足以下条件前，不允许新增 sync command C ABI symbol 或 `ManagerBridge` 可执行方法：

- L0 / L1 继续通过，且本文测试计划已同步到 contract checklist、FFI boundary 和 current status。
- Rust host contract test 的输入、错误、ownership、panic boundary、既有 FFI 模式映射和 forbidden material 断言已完成评审。
- Dart FFI binding contract test 已能覆盖 capability missing、copy / free、unknown status、host catalog replay、错误分类和可见层关闭态。
- 一次性恢复码展示材料和恢复码 / 短码输入的 transient secret 生命周期已有独立交互设计。
- 平台私钥 backend、恢复 / 授权交互测试和发布级部署证据的停止线仍清晰。
- 实现方案不要求 Flutter 长期持有同步材料、授权包材料、signature bytes、wrapped material、payload bytes 或 Go server 请求 / 响应体。

如果任一项无法满足，应继续停留在 design fixture / 文档层，回到 Rust / sync / crypto 边界重新设计。
