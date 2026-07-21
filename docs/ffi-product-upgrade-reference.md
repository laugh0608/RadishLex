# 产品升级 FFI 参考

本文定义 ABI v8 的产品升级 startup gate 与 Manager/InputMethod candidate validation 结构、调用方责任和证据边界，面向维护 C header、Swift/Objective-C host 与升级协调器的开发者。本文不包含完整 receipt 状态机、安装器操作或历史版本流水；通用所有权与错误规则见 [FFI 边界](ffi-boundary.md)，产品状态机见 [macOS 数据升级协调器](macos-data-upgrade-coordinator.md)。

## Startup gate

```text
RadishLexProductUpgradeStartupGateRequest {
  version: u32
  data_root_path: *const c_char
  expected_owner_id: u32
}

RadishLexProductUpgradeStartupGateResult {
  version: u32
  decision: u32
  error_code: u32
  receipt_state: u32
}
```

request/result version 均固定为 v1。`data_root_path` 必须由平台从用户域解析固定产品目录，`expected_owner_id` 必须来自当前 effective uid；UI、命令行、环境变量和普通业务代码不能提供这两个字段。路径字符串只需在调用期间存活，Rust 不保存平台指针。

调用方必须同时检查 FFI status、result version 和具名 decision 常量。只有以下 decision 允许启动：

```text
RADISHLEX_STARTUP_GATE_ALLOWED_FIRST_LAUNCH
RADISHLEX_STARTUP_GATE_ALLOWED_NO_UPGRADE_STATE
RADISHLEX_STARTUP_GATE_ALLOWED_TERMINAL_RECEIPT
```

`RADISHLEX_STARTUP_GATE_BLOCKED_UPGRADE_IN_PROGRESS` 与 `RADISHLEX_STARTUP_GATE_FAILED_CLOSED` 都阻止启动。error code 区分 upgrade in progress、active guard、unsafe root/state、interrupted artifact、invalid receipt、unexpected object、identity changed 和 I/O；`receipt_state = 0` 表示没有可报告的已解析状态。调用方必须使用 C header 的具名常量，不依赖 Rust enum discriminant，也不能把未知值或仅 `error_code = 0` 推断为允许。

## Candidate validation

```text
RadishLexManagerUpgradeValidationRequest {
  version: u32
  candidate_path: *const c_char
  settings_path: *const c_char
}

RadishLexInputMethodUpgradeValidationRequest {
  version: u32
  candidate_path: *const c_char
  shared_data_path: *const c_char
  validation_user_data_path: *const c_char
  schema: *const c_char
}

RadishLexUpgradeValidationSummary {
  version: u32
  schema_version: i64
  management_queries_checked: u32
  settings_checked: u32
  personalized_runtime_checked: u32
  candidate_signals_read: u32
}
```

两个 request version 和共享 summary version 均固定为 v1。字符串只在调用期间借用；summary 不拥有指针或 heap handle。

路径字段只允许各 bundle 内无参数原生 host 填充：

- Manager 固定 candidate 和 settings backup，成功必须返回目标 schema、`management_queries_checked = 1`、`settings_checked = 1`；
- InputMethod 固定 candidate、本 bundle RimeData/schema 和短生命周期隔离 Rime user data，成功必须返回同一目标 schema、`personalized_runtime_checked = 1`、`candidate_signals_read = 1`。

路径字段的存在不授权 Dart、InputMethod controller、安装参数或通用 CLI 接受任意路径。FFI 只完成端点内的真实产品读取；原生 host 还必须在调用前后复验 candidate 全字节不变、WAL/SHM/journal 零残留，并清理 InputMethod 临时 Rime data。

## Evidence 转换

validation summary 不能直接持久化进 receipt，也不能由 UI 拼装。持有 upgrade guard 的协调核心必须再次复验：

- 当前 persisted receipt 仍为同一 operation 和 `candidate_migrated`；
- candidate identity、目标 schema 与 sidecar 状态未漂移；
- Manager/InputMethod summary version 和各自固定 check bit 精确匹配。

双端通过后，协调核心再次以 read-only current-schema connection 打开 candidate，才把两个 summary 转换为 validation evidence v1 并推进 `candidate_verified`。Manager 或 InputMethod 明确失败分别进入带稳定 failure code 的 `aborted_preserved`。receipt 只保存状态结论，不保存 summary、host stdout/stderr、路径、正文或内容 hash。

## 调用顺序与验证

正常产品启动顺序和平台 wrapper 失败处理见 [FFI 平台调用契约](runbooks/ffi-platform-call-contract.md)。产品 host 的固定路径、参数拒绝和临时目录边界见 [macOS 产品升级宿主](../platforms/macos-product/README.md)。

```bash
cargo test -p radishlex-ime-ffi --all-targets
./scripts/check-manager-product.sh
./scripts/check-macos-imk.sh
```

这些检查只能使用合成临时数据，不得执行真实 Application Support candidate validation。
