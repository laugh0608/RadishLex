# RadishLex 当前状态

本文档是 RadishLex 新会话和日常推进的短入口，读者是需要快速判断当前阶段、验证基线、停止线和下一步顺位的维护者与协作者。本文不包含历史流水、完整字段参考、接口协议细节或长篇推演；这些内容继续放在专题文档、runbook、ADR 和 `docs/devlogs/`。

## 复核日期

- 日期：2026-07-08
- 常态分支：`dev`
- 当前主题：Phase 3 自部署同步出口治理已有本地 Docker / HTTPS、Go server、Rust sync / crypto 和部署证据校验证据链；Phase 4 Flutter manager 本地验收复查已完成。当前同步入口推进集中在非上传治理：sync entry gate、connection health、恢复码 setup / restore、设备 join / revocation readiness、`manager_sync_readiness.v1`、`manager_sync_evidence_bundle.v1`、action command preview、request / result preview、action protocol 矩阵和真实 bridge 命令前 contract 检查清单 / 草案 / FFI 命令边界 / contract test 计划。2026-07-07 已补真实 `ManagerBridge` 命令前 checklist、design fixture、model 测试、future contract 草案、FFI boundary、FFI contract test plan、fake native capability missing 回归、测试侧 binding harness、Rust host 合成输入样本 / host contract catalog / review catalog / Dart fake binding replay、真实 dynamic library symbol 缺席 smoke、transient secret 交互生命周期 fixture 和 recovery visible-layer / future confirmation detail 回归，覆盖 DTO 安全形状、恢复码一次性展示 / 输入占位、保存确认、失败限速、设备登记状态、短码核对占位、C ABI / FFI ownership、copy / free、unknown status 降级、错误 envelope、幂等性、并发、host smoke 计划、既有 `ime-ffi` 模式映射和诊断脱敏评审。2026-07-08 已补 join request 授权和设备撤销 future confirmation detail fixture / model test / 文档证据，固定 join request 状态、短码核对占位、授权显式确认、授权包状态、撤销显式确认、丢失设备风险提示和 key epoch 状态的 UI / diagnostics 非敏感状态码；并补 Rust host source-level checklist 与 test design package fixture / model test / 文档证据，把 host review item 绑定到现有 `ime-ffi` 源码和测试模式，再把 host contract case 绑定到 source checklist、样本、expected status、断言组和停止线。真实域名、证书和外部反代复验保留为发布 / 真实用户开放前门禁；当前仍不改变 `ManagerBridge` contract、不新增 C ABI、不打开真实同步、恢复码生成 / 输入、join request 创建、授权成功或设备撤销路径。

## 当前阶段

RadishLex 已完成 Rust core、Rime adapter、userdb、ranker、crypto、sync client 边界、Go sync server 和第一批 Flutter manager 本地 FFI bridge 的起步证据。当前推进重点是保持 Phase 4 manager 本地验收证据可复查，并在本地 Docker / 本地 HTTPS 证据支撑下维护恢复码 setup / restore、设备 join / revocation 的只读状态、readiness 聚合摘要、bridge readiness 错误分类映射、只读 action intent 进入计划、真实 bridge 命令前 contract 检查清单和诊断脱敏；真实用户同步开放仍等待平台私钥 backend、恢复 / 授权交互和发布级部署证据。

部署证据当前状态：仓库内已有本地 Docker / 本地 HTTPS、短生命周期 HTTP、备份恢复、外部 TLS 反代和升级回滚实现级 smoke；`tests/fixtures/sync-deployment-evidence-valid.txt` 只是合成 fixture。没有真实目标环境 `deployment_evidence.v1` 时，不记录发布级部署通过，但这不阻止继续开发 manager 同步入口的非上传状态、说明和本地联调路径。

近期默认先读本文，再按任务选读：

- 总体架构和阶段边界：`docs/technical-plan.md`
- 阶段顺序和退出标准：`docs/roadmap.md`
- 管理端职责和停止线：`docs/manager-ui-boundary.md`
- 管理端本地验收：`docs/manager-local-acceptance.md`
- 真实同步入口前置边界：`docs/manager-sync-entry-boundary.md`
- 恢复码 / 设备授权进入条件：`docs/manager-recovery-device-auth-flow.md`
- 同步 action 协议预演边界：`docs/manager-sync-action-protocol-preview.md`
- 同步 action 验收矩阵：`docs/manager-sync-action-acceptance-matrix.md`
- 真实 bridge 命令前检查清单：`docs/manager-sync-bridge-command-contract-checklist.md`
- future bridge 命令 contract 草案：`docs/manager-sync-bridge-command-contract.md`
- future FFI / C ABI 命令边界：`docs/manager-sync-ffi-command-boundary.md`
- FFI 测试计划：`docs/manager-sync-ffi-command-contract-test-plan.md`
- settings draft 与诊断字段：`docs/manager-settings-diagnostics.md`
- readiness 联调场景目录：`docs/manager-readiness-scenarios.md`
- 目标部署证据与生产停止线：`docs/runbooks/sync-server-production-deployment.md`
- 同步服务端 API / 存储：`docs/sync-server-api-storage.md`
- 平台私钥 backend 停止线：`docs/platform-private-key-backend-strategy.md`

## 已落地能力

Rust 输入与学习链路：

- `ime-core` 已提供平台无关输入会话、候选、提交和 engine trait。
- `ime-engine-rime` 已通过 `native-rime` feature 接入真实 `librime` adapter，默认构建不依赖本机 Rime。
- `ime-userdb` 与 `ime-ranker` 已覆盖本地用户词、选择事件、负反馈、删除 tombstone、导入导出、P2 payload、合并写回和可解释排序。
- `ime-cli` 已提供 `demo`、`rime`、`dict`、`learn`、`rank explain` 和 `sync preflight` 复验入口。

同步与加密链路：

- `ime-crypto` 已覆盖本地 envelope、AAD、nonce、ciphertext hash、恢复码 KDF、Ed25519 签名、device wrapping、recovery material 和平台 backend capability / unavailable 模型。
- `ime-sync` 已覆盖 P2 envelope 组装、设备生命周期、对象版本冲突、客户端合并模型、remote client DTO、std-only `http://` transport 和 bearer token header。
- `server/sync-server` 已覆盖 SQLite metadata、local blob storage、签名验签、device wrapping 密文承载、recovery latest、对象版本上传 / 下载、bearer token 门禁、request id、panic recovery、审计日志、Docker Compose、本地 / 部署态 runbook、备份恢复、外部 TLS 反代和升级回滚 smoke。
- `docs/runbooks/sync-server-production-deployment.md` 已固定目标部署证据包模板、校验入口和非敏感摘要导出，明确外部 TLS、访问控制失败响应、备份恢复、升级回滚和日志脱敏的非敏感记录口径。
- Rust userdb 两客户端真实 Go HTTP 测试已覆盖设备授权、三类 P2 对象上传下载、解密写回、stale conflict 和 v2 重新上传。

Flutter manager：

- `apps/radishlex-manager` 已提供 macOS Flutter 起步工程，默认使用合成 fixture。
- 显式配置本地 SQLite userdb 与 `ime-ffi` 动态库后，可通过真实 Dart FFI bridge 管理本地词库、导入导出、import batches、learning status、rank explain、sync preflight、settings draft 和脱敏诊断报告。
- UI 已拆分 manager shell、跨页 action 编排、词库子组件、学习子组件、同步子组件、设置诊断子组件、页面级 widget tests、action helper 回归测试、Dart model 分组和动态 FFI bridge 分层。
- `docs/manager-local-acceptance.md` 已将 Phase 4 本地验收范围、退出标准映射、验证命令、隐私检查和真实同步停止线整理为可复验入口；2026-07-05 复查确认当前本地验收无影响退出标准的证据缺口。
- `docs/manager-sync-entry-boundary.md` 已固定真实同步入口进入 UI / bridge 前的恢复码、设备授权、状态门禁、错误分类、诊断脱敏和测试计划。
- 同步入口治理已覆盖 `SyncEntryState` / `ManagerSyncEntryGate`、连接健康、恢复码 setup / restore、设备 join / revocation readiness、`manager_sync_readiness.v1` 导入、`manager_sync_evidence_bundle.v1` 场景、只读 action intent、`manager_sync_action_command_preview.v1`、action protocol 矩阵、真实 bridge command contract 草案 / fixture、FFI command boundary、contract test plan、transient secret 交互生命周期、recovery visible-layer 文案 / 确认占位、join / revocation future confirmation detail、Rust host contract catalog、Rust host review catalog、Rust host source checklist 和 Rust host test design package。相关 model、settings、sync、diagnostics、Dart fake native / binding contract 回归和真实 FFI smoke 已证明 settings / sync / diagnostics 同源派生，测试侧 future binding 遵守 summary copy -> free、unknown native status 安全降级和既有 bridge failure 分类，并已用 host catalog replay 反查 ABI status、input case、required evidence 和 command error allowlist 的非敏感映射；真实 dynamic library 当前不导出 future sync command symbol；host catalog 已把 capability closed、unknown schema/action、invalid bool、null pointer、invalid UTF-8、unenveloped sync error、panic boundary 和 same-domain concurrent command 绑定到建议 Rust host test name 与状态码，review catalog 进一步把每个 case 绑定到既有 `ime-ffi` 版本检查、输入校验、panic boundary、释放路径、error handle、summary 输出和 owner-thread 状态模式，source checklist 已把这些模式绑定到具体源码 / 测试引用，test design package 已把每个 case 绑定到 source checklist、合成样本、expected status、断言组和 forbidden output category，但仍是 design fixture；恢复码一次性展示、保存确认、恢复码输入、恢复记录查询、失败限速、设备登记、短码核对、授权显式确认、授权包状态、撤销显式确认、丢失设备风险提示和 key epoch 状态只保留占位状态、状态码与完成证据码，不写 settings action payload，不泄漏 token、恢复码、短码、signature、wrapped material、payload bytes、请求 / 响应体或真实路径；即使 bridge readiness 全部 ready，也只输出 `not_executable_current_phase` 和 `user_sync_entry_closed_current_phase`，不打开真实操作。诊断字段以 `sync.entry_*`、`sync.readiness_*`、`sync.interaction_*`、`sync.action_*`、`sync.connection_*`、`sync.recovery_*`、`sync.device_*` 等非敏感摘要为主；真实同步按钮保持关闭。

平台私钥 backend：

- `apple-keychain-v1` 已有 runbook、feature-gated 接线和非 smoke 测试，但真实 Keychain smoke 阻塞于 `ed25519-v1` 创建；生产签名继续被 gate 阻断。
- `android-keystore-v1` 已有 Rust bridge、JNI glue、Kotlin / Gradle harness、gated smoke 和 provider diagnostics；Pixel 9 Pro API 35 AVD 与 Pixel 10 Pro API 37 AVD 当前结果均为 `unsupported_signature_algorithm`，不解除生产门禁。

## 当前停止线

- 不打开真实用户远端同步、恢复码 UI 或设备授权成功路径，直到可用平台私钥 backend、恢复 / 授权交互实现测试和发布级部署证据齐备；本地 Docker / 本地 HTTPS 可以用于开发联调和非上传状态验证。
- 没有真实目标环境的 `deployment_evidence.v1` 通过校验并导出 `deployment_evidence_summary.v1` 前，不把目标部署状态写成发布可用；可以把本地 smoke 作为开发期 `local_smoke` 证据展示和测试，但不得宣称真实用户同步已经开放。
- 不新增明文同步 payload、P1 原始事件导出、token / 恢复码 / 私钥 / wrapped material / payload bytes 日志或诊断字段。
- 不让 Flutter manager 绕过 `ManagerBridge` / `ime-ffi` 直接读写 Rust 内部结构或 SQLite schema。
- 不把 Go server 做成候选排序服务、在线转换服务或明文词库服务。
- 不在当前阶段推进完整平台输入法壳；Phase 5 前仍以管理端本地能力和验证基线治理为主。

## 验证基线

日常仓库级检查：

```bash
./scripts/check-repo.sh
```

Flutter manager 改动：

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
```

Sync server 本地部署 / 联调：

```bash
./scripts/check-sync-server-deployment-rehearsal.sh
./scripts/check-sync-server-deployment-rehearsal.sh --config-only
./scripts/check-sync-server-connection-health.sh --self-test
```

目标部署证据包校验：

```bash
./scripts/check-sync-deployment-evidence.sh --self-test
./scripts/check-sync-deployment-evidence.sh tests/fixtures/sync-deployment-evidence-valid.txt
./scripts/check-sync-deployment-evidence.sh --summary-json tests/fixtures/sync-deployment-evidence-valid.txt
```

完整部署预演会启动短生命周期 Docker 容器，适合当前同步开发联调；如本机 Docker 不可用，应记录阻塞。连接健康脚本默认只输出 `sync_connection_health.v1` 非敏感摘要，实际本地服务未启动时只记录不可达，不写入 token、endpoint credential、请求 / 响应体或 payload bytes。2026-07-05 本地 Docker / 本地 HTTPS 只读探测在宿主网络环境返回 `connection_status=reachable`、`server_state_status=domain_missing_expected`、`auth_status=not_required_for_local_probe`、`http_status=404`、`last_remote_error_code=not_found`，只作为开发联调摘要，不作为发布级目标部署证据。Apple Keychain smoke、Android connected smoke、真实平台输入法操作和正式发布级部署复验需要明确授权或人工环境准备；默认不在普通仓库检查中运行。

## 近期推进顺位

1. 维护本文短入口，避免新会话默认阅读长周志。
2. 保持 Phase 4 manager 本地验收证据稳定；若后续改动触及验收范围，再按 `docs/manager-local-acceptance.md` 补精准 widget / helper / smoke 覆盖。
3. 下一批产品开发继续围绕本地 Docker / 本地 HTTPS 联调，维护恢复码 setup / restore、设备 join / revocation 的只读状态机、readiness 聚合摘要、bridge readiness mapper、settings 本地 readiness 摘要导入、readiness 场景目录、evidence bundle 预演场景、settings / sync / diagnostics 同源回归、只读 action intent 进入计划、action command preview、action protocol 验收矩阵、request / result boundary、request / result allowed fields、forbidden material policy、真实 bridge 命令前 contract 检查清单 / 草案、FFI command boundary、contract test plan、transient secret 生命周期、recovery / device authorization future confirmation detail、Rust host contract catalog、Rust host review catalog、Rust host source checklist、Rust host test design package、Dart fake binding host catalog replay、诊断脱敏和错误分类测试。下一步可基于 test design package 评审是否进入真实 Rust host contract test 文件；仍不改变 `ManagerBridge` contract、不新增 C ABI、不打开真实同步、恢复码生成 / 输入、join request 创建、授权成功或设备撤销路径。
4. 同步联调用本地 Docker / 本地 HTTPS 和短生命周期数据目录；正式发布或真实用户开放前，再补真实目标环境的非敏感 `deployment_evidence.v1` 并导出 `deployment_evidence_summary.v1`。
