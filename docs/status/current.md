# RadishLex 当前状态

本文档是 RadishLex 新会话和日常推进的短入口，读者是需要快速判断当前阶段、验证基线、停止线和下一步顺位的维护者与协作者。本文不包含历史流水、完整字段参考、接口协议细节或长篇推演；这些内容继续放在专题文档、runbook、ADR 和 `docs/devlogs/`。

## 复核日期

- 日期：2026-07-05
- 常态分支：`dev`
- 当前主题：Phase 3 自部署同步出口治理已形成证据链，Phase 4 Flutter manager 本地管理能力验收复查已完成；真实同步入口前置交互边界和目标部署证据包口径 / 校验 / 摘要入口已固定为文档和工具证据。2026-07-05 复查确认仓库内没有真实目标环境产生的 `deployment_evidence.v1`，当前仍阻塞于部署者提供非敏感证据包。

## 当前阶段

RadishLex 已完成 Rust core、Rime adapter、userdb、ranker、crypto、sync client 边界、Go sync server 和第一批 Flutter manager 本地 FFI bridge 的起步证据。当前推进重点不是打开真实远端同步，而是保持 Phase 4 manager 本地验收证据可复查，并继续补齐后续真实同步入口所需的平台私钥 backend 与真实目标部署运行证据。

目标部署运行证据采集当前状态：仓库内只有 `tests/fixtures/sync-deployment-evidence-valid.txt` 合成 fixture 和校验 / 摘要脚本，没有用户提供或目标环境产生的真实证据包。本轮不把合成 fixture 冒充真实部署证据；真实部署仍未验证，manager 继续保持 `deployment_unverified` / 不可用口径。

近期默认先读本文，再按任务选读：

- 总体架构和阶段边界：`docs/technical-plan.md`
- 阶段顺序和退出标准：`docs/roadmap.md`
- 管理端职责和停止线：`docs/manager-ui-boundary.md`
- 管理端本地验收口径：`docs/manager-local-acceptance.md`
- 真实同步入口前置边界：`docs/manager-sync-entry-boundary.md`
- settings draft 与诊断字段：`docs/manager-settings-diagnostics.md`
- 目标部署证据包与生产部署停止线：`docs/runbooks/sync-server-production-deployment.md`
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
- 同步页当前只展示 gate 状态、本地 P2 对象分类、设备 backend 状态和生产不可用原因，真实同步按钮保持关闭。

平台私钥 backend：

- `apple-keychain-v1` 已有 runbook、feature-gated 接线和非 smoke 测试，但真实 Keychain smoke 阻塞于 `ed25519-v1` 创建；生产签名继续被 gate 阻断。
- `android-keystore-v1` 已有 Rust bridge、JNI glue、Kotlin / Gradle harness、gated smoke 和 provider diagnostics；Pixel 9 Pro API 35 AVD 与 Pixel 10 Pro API 37 AVD 当前结果均为 `unsupported_signature_algorithm`，不解除生产门禁。

## 当前停止线

- 不打开真实远端同步、恢复码 UI 或设备授权成功路径，直到可用平台私钥 backend、真实目标部署运行证据和对应实现测试齐备；恢复 / 授权交互边界与目标部署证据包目前只作为文档和校验工具证据存在。
- 没有真实目标环境的 `deployment_evidence.v1` 通过校验并导出 `deployment_evidence_summary.v1` 前，不把目标部署状态写成完整通过，也不把合成 fixture 结论交接给 manager UI / bridge。
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

Sync server 部署预演：

```bash
./scripts/check-sync-server-deployment-rehearsal.sh --config-only
```

目标部署证据包校验：

```bash
./scripts/check-sync-deployment-evidence.sh --self-test
./scripts/check-sync-deployment-evidence.sh tests/fixtures/sync-deployment-evidence-valid.txt
./scripts/check-sync-deployment-evidence.sh --summary-json tests/fixtures/sync-deployment-evidence-valid.txt
```

完整部署预演、Docker `up`、Apple Keychain smoke、Android connected smoke 和真实平台输入法操作都需要明确授权或人工环境准备；默认不在普通仓库检查中运行。

## 近期推进顺位

1. 维护本文短入口，避免新会话默认阅读长周志。
2. 保持 Phase 4 manager 本地验收证据稳定；若后续改动触及验收范围，再按 `docs/manager-local-acceptance.md` 补精准 widget / helper / smoke 覆盖。
3. 真实同步入口后续 UI / bridge 必须遵守 `docs/manager-sync-entry-boundary.md`，在条件齐备前只能展示准备状态和不可用原因。
4. 下一批真实同步前置工作优先由部署者提供真实目标环境的非敏感 `deployment_evidence.v1`，通过 `./scripts/check-sync-deployment-evidence.sh <evidence-file>` 和 `--summary-json` 导出 `deployment_evidence_summary.v1`；没有真实证据包时只记录阻塞，不推进 sync entry state helper / UI gate 实现。
