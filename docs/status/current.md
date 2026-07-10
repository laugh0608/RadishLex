# RadishLex 当前状态

本文档是 RadishLex 新会话和日常推进的短入口，读者是需要快速判断当前阶段、验证基线、停止线和下一步顺位的维护者与协作者。本文不包含历史流水、完整字段参考、接口协议细节或长篇推演；这些内容继续放在专题文档、runbook、ADR 和 `docs/devlogs/`。

## 复核日期

- 日期：2026-07-10
- 常态分支：`dev`
- 当前主题：进入 2026 年 7 月项目稳定化整改。当前主批次是 R00 文档真相源与停止线收敛，随后优先打通 macOS InputMethodKit 真实本地输入纵向链，并同步建立 Clippy、Flutter 与 native bundle 质量门禁。整改期间停止扩展与真实输入链无关的 readiness、evidence、preview、approval 和 no-symbol 证明层；真实用户同步继续保持关闭。

## 当前阶段

RadishLex 已落地 Rust workspace、Rime adapter、userdb、ranker、crypto、sync client 边界、Go sync server 和 Flutter manager 原型，但还没有真实平台输入法纵向链，正常 manager 产品包也尚未形成真实 FFI 与持久化配置闭环。当前不再把本地 manager 只读预演或实现前治理证据视为阶段完成；近期阶段证据重新收敛为真实输入、数据正确性、安全同步和产品构建。完整整改顺序、停止线和退出标准以 `docs/remediation/2026-07-project-stabilization.md` 为临时执行入口。

部署证据当前状态：仓库内已有本地 Docker / 本地 HTTPS、短生命周期 HTTP、备份恢复、外部 TLS 反代和升级回滚实现级 smoke；`tests/fixtures/sync-deployment-evidence-valid.txt` 只是合成 fixture。没有真实目标环境 `deployment_evidence.v1` 时，不记录发布级部署通过，但这不阻止继续开发 manager 同步入口的非上传状态、说明和本地联调路径。

近期默认先读本文，再按任务选读：

- 当前整改批次、停止线和退出条件：`docs/remediation/2026-07-project-stabilization.md`
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
- manager 同步命令 C ABI ADR：`docs/adr/0006-manager-sync-c-abi-contract-governance.md`
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
- 同步入口治理已覆盖 entry gate、connection health、恢复码 / 设备授权 readiness、evidence bundle、只读 action intent、action protocol、真实 bridge command contract 草案、FFI command boundary、transient secret 生命周期、recovery / device confirmation detail、Rust host catalog / checklist / test design / C ABI matrix / implementation review package，以及 `crates/ime-ffi/src/manager_sync_command.rs` 内部非导出 draft module 的 admission、migration 和 execution gate review。相关 model、settings、sync、diagnostics、Dart fake native / binding contract 回归、Rust 单元测试和真实 FFI smoke 已证明 settings / sync / diagnostics 同源派生，且真实 dynamic library 当前不导出 future sync command symbol。host 侧材料除内部 draft module 外仍是 design fixture：只绑定源码模式、测试样本、状态码、断言组、required decisions、evidence 和 Rust `*Draft` 草案，不新增 native symbol。内部 draft module 当前覆盖 request parsing、action-specific section、handle lifecycle、panic boundary、sync domain guard、C ABI wrapper shape、host contract gate、result accessor field set、owner / worker policy、Debug redaction、gate readiness、admission、native export、Dart binding、ManagerBridge、host test file approval、real sync execution gate、real sync evidence bundle 和 forbidden material redaction；Dart fake native binding 已能重放对应阻塞项。候选 executor / result accessor / release symbol 仍只是字符串级评审材料，host gate 仍关闭，不提供真实 `extern "C"` wrapper、真实 native result accessor / release symbol、manager 专用 error symbol、跨线程 command worker 或 Rust sync / crypto 接线。恢复码、短码、授权、撤销和 key epoch 仍只保留占位状态、状态码与完成证据码，不写 settings action payload，不泄漏 token、恢复码、短码、signature、wrapped material、payload bytes、请求 / 响应体或真实路径；真实同步按钮保持关闭。

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

1. 完成 R00：修正 `AGENTS.md` / `CLAUDE.md` 快速认知，压缩当前状态，清理 roadmap 实现流水，并给现有 manager sync review 资产建立保留、归档、删除或转化清单。
2. 启动 R06 的首批质量门禁：修正 `ime-ffi` Clippy 裸指针契约，把 Flutter、Go race、MSRV 和 macOS native bundle 检查接入 CI。
3. 推进 R01：返回完整 KeyOutcome，建立 engine / ranker / userdb runtime，收口 librime 全局生命周期，并打通 macOS InputMethodKit 真实本地输入纵向链。
4. R01 完成前不再新增 future sync gate / evidence / preview 资产；R03 同步收敛与密码协议完成前，真实远端同步、恢复码、设备授权成功和设备撤销路径继续保持关闭。
