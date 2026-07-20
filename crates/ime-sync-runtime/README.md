# ime-sync-runtime 组件说明

本文说明 `radishlex-ime-sync-runtime` 的职责、资格运行状态机、输入输出契约和开发验证入口，面向维护 Manager 同步执行、Rust FFI 与 Flutter bridge 的开发者。本文不定义真实用户同步产品开关、Go API、密码协议或页面布局；同步 cycle 语义见 [ime-sync 组件说明](../ime-sync/README.md)，跨语言所有权见 [FFI 边界](../../docs/ffi-boundary.md)，产品停止线见 [Manager 同步入口边界](../../docs/manager-sync-entry-boundary.md)。

## 组件定位

`ime-sync-runtime` 是 Manager 同步能力的 Rust 产品组合层。它复用既有 `ime-sync` orchestration、`ime-userdb` repository 与 `ime-crypto`，负责一次隔离的本地 HTTPS 合成资格运行；它不进入输入热路径，也不拥有真实用户同步入口。

```text
Flutter Manager
      │ start / poll / cancel / free
      ▼
    ime-ffi ── opaque run handle
      │
      ▼
ime-sync-runtime
  ├─ ime-sync HTTPS + orchestration
  ├─ ime-userdb 临时双客户端
  └─ qualification-only crypto provider
      │
      ▼
loopback HTTPS sync server
```

职责固定如下：

- `ime-sync-runtime` 组合网络、密码和本地存储，管理 worker、取消、超时、跨进程单运行和临时资源。
- `ime-sync` 继续拥有协议、transport、验签解密、冲突恢复和 `sync_once` 语义。
- `ime-userdb` 继续拥有 SQLite schema、transaction、cursor、journal 和 outbox。
- `ime-ffi` 只复制版本化参数、包装 Rust-owned handle，并返回固定快照。
- Flutter 只展示安全摘要、接受单次 transient 输入和发起取消，不复制同步状态机。

资格 provider 只能用于合成执行，不能获得生产 backend 身份，不能装载平台密钥，也不能改变 `user_sync_enabled`。

## 请求契约

`QualificationRequest` format v1 只接受下列字段：

| 字段 | 约束 |
| --- | --- |
| `endpoint` | 最多 2048 bytes；仅 `https://localhost`、`https://127.0.0.1` 或 `https://[::1]`，可带非零端口和单个结尾 `/` |
| `access_token` | 32 到 4096 bytes 的可见 ASCII，不允许空格或控制字符 |
| `local_ca_der` | 可选，1 到 65536 bytes；只作为本次进程内附加 trust anchor |
| `timeout_ms` | 100 到 120000 ms，约束整次资格运行而不是单个 HTTP 调用 |

endpoint 不接受 HTTP、非 loopback 地址、userinfo、path、query 或 fragment。调用方不能提供 userdb 路径、domain/device/object ID、payload、master key、wrapped material、签名或平台 key identity。

请求在 `start` 返回前复制到 Rust-owned 内存。token 和 CA 不进入 settings、diagnostics、日志或结果；终态快照的 `transient_inputs_cleared` 用于证明运行侧输入生命周期已经结束。

## 运行状态与阶段

run state 只允许：

| 状态 | 语义 |
| --- | --- |
| `created` | format 中保留的初始值；正常 `start` 返回的 handle 已进入 `running` |
| `running` | worker 正在执行当前 phase |
| `cancelling` | 已接受首次取消请求，等待当前受控点或 cycle cancellation 生效 |
| `completed` | 收敛、内容检查和清理均完成 |
| `failed` | 请求外的运行错误或内部错误，错误类别和失败 phase 可用 |
| `cancelled` | 取消已完成，worker 已停止 |

phase 按稳定顺序推进：

1. `validate_request`：请求已在构造边界通过格式校验，run 获取单运行所有权。
2. `prepare_workspace`：创建 `0700` 临时目录、marker、隔离 userdb 和合成身份。
3. `create_domain`：在 loopback 服务创建合成 domain 与首设备。
4. `authorize_second_device`：登记第二个合成设备及签名 profile。
5. `client_a_first_sync`：A 上传第一版本。
6. `prepare_conflict`：A 增加合成冲突词并预留 stale-base outbox。
7. `client_b_merge`：B 下载、合并并上传下一版本。
8. `client_a_conflict_recovery`：A 观察 stale-base conflict，重新发现、合并并上传。
9. `verify_convergence`：连续执行两轮 B/A 同步，最后一轮双方上传数必须为零，并复核三条合成词均存在。
10. `cleanup`：删除临时 userdb、marker 和工作目录，释放单运行所有权。
11. `complete`：成功终态。

状态和 phase 必须由调用方按 enum 映射。未知值、非法 state transition、非终态却声称 worker 已停止或成功态缺少全部成功条件时，Dart binding 必须失败关闭。

## 快照与错误

`QualificationRunSnapshot` format v1 只包含：

- state、phase；
- 多次 `sync_once` 累加的 discovered、downloaded、applied、uploaded、conflicts、retries；
- 固定的 convergence round 数；
- `temporary_files_cleaned`、`worker_stopped`、`transient_inputs_cleared`；
- 可选 error code、error phase 和 retryable flag。

成功不能只看 `state=completed`。产品 binding 必须同时要求 `phase=complete`、`convergence_rounds=2`、至少观察一次 conflict，并确认三个清理 flag 全部为真。计数是资格摘要，不是用户数据统计，也不能持久化为 readiness 或部署证据。

错误类别固定为：`invalid_request`、`already_running`、`unauthenticated`、`tls_rejected`、`transport_timeout`、`server_unavailable`、`protocol_rejected`、`crypto_rejected`、`local_storage_failed`、`conflict_not_observed`、`convergence_failed`、`cancelled` 和 `internal`。UI 只能根据 allowlist 后的类别与 `retryable` 给出说明，不显示底层 HTTP body、provider exception、文件路径或平台错误正文。

## 并发、取消与清理

- 同一进程通过原子状态只允许一个 active run；同一用户临时目录中的 Unix socket 再阻止多个 Manager 进程并发运行。
- `poll`、`cancel` 和 `free` 可在不同线程间迁移，但调用方必须串行；`free` 不能与其他调用重叠。
- 首次 `cancel` 返回已请求，重复取消幂等；运行已经终止时不再改变结果。
- `free` 或 Rust `Drop` 遇到非终态 run 时先请求取消，再 join worker，不能留下悬空线程。
- 新 run 只清理同用户、精确名称且 marker 内容匹配的旧资格工作区；不扫描或删除其他临时目录。
- 正常、失败和取消路径都必须清理临时 userdb 与 transient 输入。panic 会转换为 `internal`，调用方不能把缺失的 cleanup flag 当作成功。

## 隐私与产品停止线

资格运行只使用代码生成的合成 domain/device/object 和合成词，不读取真实 `~/Library/Application Support/RadishLex/userdb.sqlite3`，不访问 Keychain/Secure Enclave，也不接触输入历史。服务端仍按不可信边界处理，所有 payload 继续经过现有加密、签名和验证链。

该运行证明 Manager bridge、FFI 生命周期、严格 HTTPS、同步编排、冲突恢复和资源清理可以共同工作。它不证明真实用户同步已开放，不替代正式域名/证书、生产部署、恢复/授权/撤销 UI 或平台私钥资格证据。

## 开发验证

组件与 FFI contract：

```bash
cargo test -p radishlex-ime-sync-runtime --all-targets
cargo test -p radishlex-ime-ffi --all-targets
cargo clippy -p radishlex-ime-sync-runtime -p radishlex-ime-ffi \
  --all-targets -- -D warnings
```

Manager mapper/widget 与产品 bundle：

```bash
./scripts/check-manager.sh
./scripts/check-manager-product.sh
```

真实短生命周期 Caddy HTTPS、bearer 负向响应和完整合成双客户端链：

```bash
./scripts/check-sync-server-local-https.sh
```

最后一项会启动短生命周期 Docker 服务，只能使用合成数据，并应在结束时确认 Compose 资源清零。
