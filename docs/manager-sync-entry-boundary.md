# Flutter manager 真实同步入口前置边界

本文档定义 Flutter manager 进入真实远端同步、恢复码和设备授权 UI / bridge 前必须满足的交互边界、状态门禁、错误语义和测试计划。读者是维护 `apps/radishlex-manager`、`ime-ffi` 管理接口、`ime-sync` 客户端接线和审阅隐私边界的协作者。本文不定义 Go server API 字段全集、加密协议细节、平台私钥 backend 实现、OIDC 登录、视觉稿或真实平台输入法壳。

## 当前结论

Phase 4 manager 本地验收已经有可复验证据，真实同步入口的下一步不是直接启用上传 / 下载按钮，而是先把恢复码、设备授权、目标部署证据和平台私钥 backend 的进入条件固定为可测试的 UI / bridge 边界。

当前仍保持：

- 不打开真实远端同步主操作。
- 不提供恢复码生成、输入、轮换或撤销 UI。
- 不提供设备加入请求审批、授权成功路径或设备撤销 UI。
- 不新增明文同步 payload、恢复码、私钥、signature bytes、wrapped material bytes、encrypted payload bytes 的日志、诊断字段或 widget 可见数据。

## 适用范围

纳入本文：

- Flutter manager 可展示的真实同步准备状态。
- 恢复码和设备授权进入 UI 前的用户路径。
- `ManagerBridge` / `ime-ffi` 后续可扩展的同步入口边界。
- 诊断、错误分类、测试和验收顺序。

不纳入本文：

- `SyncMasterKey`、object key、AAD、nonce、KDF 参数和签名格式的协议细节。
- Go server handler、SQLite migration、object storage 和 access token 的字段全集。
- 平台 Keychain / Keystore 的具体实现。
- 真实平台输入法壳或输入热路径。
- OIDC / Radish 产品账号体系。

协议和存储细节继续以 `docs/sync-key-management.md`、`docs/production-recovery-flow.md`、`docs/sync-server-api-storage.md`、`docs/crypto-boundary.md` 和相关 ADR 为准。

## 进入条件

真实同步入口进入用户可用状态前，至少需要这些证据同时成立：

| 证据 | 要求 | 当前状态 |
| --- | --- | --- |
| 平台私钥 backend | 生产签名 backend 可在目标平台创建、加载、签名和删除非导出设备签名 key，且 capability / production gate 可被 manager 读取。 | 未满足；Apple 与 Android backend 均未解除 production gate。 |
| 目标部署运行证据 | 目标部署完成外部 TLS、访问控制失败响应、备份恢复、升级回滚和日志脱敏复验，并按 `docs/runbooks/sync-server-production-deployment.md` 保留非敏感证据包；settings draft 只保存 allowlist 来源标签。 | 部署 runbook、证据包模板、预演入口、证据包校验脚本和非敏感摘要导出已存在，真实目标部署证据仍未作为用户可用同步条件闭合。 |
| 恢复码交互边界 | 恢复码只显示一次、用户确认保存、恢复记录创建 / 轮换 / 撤销、失败限速和日志脱敏测试齐备。 | 本文固定 UI / bridge 边界；产品实现仍未开始。 |
| 设备授权交互边界 | join request、短码核对、授权包、设备撤销、lost device 和 key epoch 说明能被 UI 表达并测试。 | 本文固定 UI / bridge 边界；产品实现仍未开始。 |
| 客户端同步操作 | Flutter 只调用结构化 bridge；Rust 侧只接收 encrypted object 与 signed manifest，不暴露 plaintext payload。 | Rust / Go 侧已有底层证据；manager 真实远端操作入口未接。 |
| 诊断脱敏 | 诊断报告只输出状态、来源标签、聚合计数和脱敏策略，不输出 secret 或 payload bytes。 | 本地验收已覆盖；真实同步字段新增前仍需补测试。 |

任一证据缺失时，manager 只能展示准备状态、不可用原因和本地预检摘要，不得提供会上传真实 P2 数据的主操作。

目标部署证据包应先通过 `./scripts/check-sync-deployment-evidence.sh <evidence-file>`，并只通过 `deployment_evidence_summary.v1` 非敏感摘要交接给后续 UI / bridge 设计；证据包正文仍不进入 Flutter widget 状态、settings JSON 或诊断报告。manager 只能展示来自已校验证据包摘要的 `local_smoke`、`external_tls`、`backup_restore`、`upgrade_rollback` 这类 allowlist 标签和聚合状态；证书、token、日志正文、请求 / 响应体、真实路径、`notes` 和 payload bytes 都必须留在 UI / bridge / 诊断之外。`local_smoke` 只能证明实现级路径，不足以开放用户可用同步。

## 状态门禁

现有 manager sync gate 状态继续保留，并按以下含义约束：

| 状态 | 含义 | 用户可做 | 禁止行为 |
| --- | --- | --- | --- |
| `local_only` | 未保留同步配置或未配置 server endpoint。 | 管理本地词库、查看本地预检。 | 上传 / 下载、恢复码、设备授权。 |
| `sync_disabled_by_policy` | 隐私模式或策略禁用同步。 | 查看禁用原因、继续本地管理。 | 任何远端同步主操作。 |
| `backend_unavailable` | 平台私钥 backend 不可用于生产签名。 | 查看 backend id、capability 和 production gate 原因。 | 生成恢复码、签名授权包、上传对象。 |
| `deployment_unverified` | 自部署目标尚无可用运行证据。 | 补充非敏感部署证据来源标签。 | 把远端同步显示为生产可用。 |
| `preflight_ready` | 本地 P2 对象、settings draft 和预检解释齐备，但真实同步入口仍未开放。 | 查看下一项阻塞证据。 | 启用同步主操作、恢复码和设备授权成功路径。 |
| `ready_for_user_sync` | 平台 backend、目标部署、恢复码、设备授权和测试证据都满足。 | 后续可进入真实同步设置流程。 | 跳过恢复码确认、跳过设备授权、绕过 bridge。 |

`preflight_ready` 不是用户可用同步状态。它只说明本地草案和预检可以解释，不代表可以对真实远端上传数据。

## 用户路径边界

### 首台设备启用同步

进入真实实现前，首台设备路径应满足：

1. 用户已配置自部署服务端草案，并保留同步配置。
2. manager 展示目标部署证据来源标签，且标签来自 allowlist。
3. Rust bridge 返回平台私钥 backend `production_gate == ready`。
4. 用户明确选择进入同步设置流程。
5. 客户端创建同步域、设备签名材料和恢复记录。
6. manager 显示恢复码一次，并要求用户确认已经离线保存。
7. 只有确认保存后，才允许后续上传加密 P2 对象。

停止线：

- 不允许用 settings draft 中的 server endpoint 自动触发上传。
- 不允许在恢复码未确认保存前把设备标记为用户可用同步状态。
- 不允许把 bearer token、恢复码或私钥材料保存进 settings draft 或诊断报告。

### 恢复码恢复新设备

恢复路径应满足：

1. 新设备只能在用户显式进入恢复流程后请求恢复。
2. manager 输入框只接收恢复码明文，不把明文写入持久化状态、日志、诊断或截图测试。
3. Rust bridge 完成 KDF、恢复记录校验和同步域材料解锁。
4. UI 只展示结构化结果：成功、恢复码错误、恢复记录缺失、恢复记录撤销、失败次数受限、网络不可达或服务端认证失败。
5. 成功后，新设备仍必须登记为受控设备，并进入后续对象拉取 / 合并流程。

停止线：

- 不把恢复码作为服务端登录密码。
- 不在 Flutter widget 层展示 KDF 参数、KDF 输出、wrapped material bytes 或恢复记录密文。
- 不把 native 原始错误字符串透传给用户或诊断报告。

### 设备授权加入

已有设备授权路径应满足：

1. 新设备生成加入请求和短码。
2. 旧设备读取 join request，只展示设备摘要、短码、创建时间、过期时间和非敏感来源。
3. 用户在旧设备核对短码后确认授权。
4. Rust bridge 签名授权包，并为新设备写入 wrapped key 密文。
5. 新设备只能读取自己的授权结果，并在本地解开同步材料。

停止线：

- 旧设备不是 `active` 时不能授权。
- join request 过期或短码不匹配时不能继续。
- Flutter 不接触 wrapped key bytes、signature bytes 或同步域材料。
- 服务端只转发 metadata 和密文材料，不参与解密。

### 设备撤销

撤销路径应满足：

1. manager 显示设备列表时只展示设备 ID 摘要、状态、授权时间、最后可见时间和非敏感 backend 状态。
2. 用户撤销设备前，UI 必须说明历史对象和撤销前已取得的材料无法被技术上追回。
3. Rust bridge 产生 revocation record，并推进后续对象使用的新 `key_epoch`。
4. 只有仍 `active` 的设备能获得新 epoch 包装记录。
5. 被撤销设备不能再签名后续对象或接收新 epoch 材料。

停止线：

- 不承诺追回撤销前已经同步到旧设备的历史数据。
- 不用设备撤销替代用户词删除；词条删除仍通过 `dictionary.deleted_terms` tombstone 表达。
- 不在 UI 中展示设备私钥、公钥原文、签名 bytes 或 wrapped material bytes。

## Bridge 边界

后续扩展 `ManagerBridge` 或 `ime-ffi` 时，应优先使用结构化请求 / 响应模型：

允许返回：

- sync gate 状态和来源。
- deployment evidence allowlist 标签。
- device backend id、capability、production gate 和非敏感原因码。
- join request 摘要、设备状态、过期时间和短码。
- 恢复码流程的结构化状态，不含恢复码明文。
- 最近同步对象数量、对象类型、版本和时间摘要。
- 错误分类和用户可读非敏感文案。

禁止返回：

- P1 原始事件。
- 明文同步 payload。
- bearer token、恢复码、同步主密钥、设备私钥。
- KDF 输出、signature bytes、wrapped material bytes、encrypted payload bytes。
- Go server 请求体 / 响应体、内部 `blob_ref` 或本机真实路径。
- native 原始错误字符串。

错误分类至少应覆盖：

- `configuration_missing`
- `authentication_required`
- `backend_unavailable`
- `deployment_unverified`
- `recovery_code_required`
- `recovery_code_invalid`
- `recovery_record_revoked`
- `join_request_expired`
- `authorization_rejected`
- `device_revoked`
- `network_unreachable`
- `version_conflict`
- `local_data_inconsistent`

## 诊断与日志

诊断报告可以新增真实同步入口字段，但必须保持摘要化：

- `sync.entry_state`
- `sync.entry_blocker`
- `sync.recovery_status`
- `sync.device_authorization_status`
- `sync.last_remote_error_code`
- `sync.last_object_counts`
- `device.active_count`
- `device.pending_count`
- `device.revoked_count`

这些字段只能输出状态码、聚合计数、时间摘要和 allowlist 来源标签。不得输出恢复码、token、请求 / 响应体、签名、wrapped material、payload bytes、用户词、真实路径或 provider exception 原文。

日志允许记录操作类型、状态码、聚合计数、耗时和非敏感错误码。截图和测试 fixture 必须使用合成词、虚构设备、虚构服务端和合成短码。

## 测试计划

进入真实同步 UI / bridge 前，应至少补齐以下测试：

| 层级 | 测试重点 |
| --- | --- |
| Dart helper | 从 settings draft、backend gate、部署证据摘要、恢复码状态和设备授权状态派生 sync entry state。 |
| Widget tests | 每个阻塞状态的按钮禁用、文案、诊断入口和下一步提示；`preflight_ready` 下仍不能启用真实同步。 |
| Bridge mapper tests | native 状态和错误分类映射到 Dart model，不透传原始错误字符串或 secret 字段。 |
| FFI smoke | 使用临时 SQLite、临时 settings、合成设备和短生命周期 bridge 复验本地状态读取，不连接真实用户后端。 |
| Rust tests | 恢复记录创建 / 撤销、join request 过期、授权签名、设备撤销后 key epoch 推进和旧设备阻断。 |
| Go / Rust integration | 真实 HTTP handler 只传 metadata 和密文；错误响应、audit log 和 Debug 输出脱敏。 |
| 诊断测试 | 新增诊断字段不包含用户词、路径、token、恢复码、signature bytes、wrapped material bytes、payload bytes、证据包正文或 `notes`。 |

文档或 UI 只改变本地展示口径时，可以先跑 `./scripts/check-manager.sh`、`git diff --check` 和 `./scripts/check-repo.sh`。涉及目标部署证据包格式、摘要或交接材料时，应追加 `./scripts/check-sync-deployment-evidence.sh --self-test`，并对实际证据文件执行 `./scripts/check-sync-deployment-evidence.sh <evidence-file>` 和 `./scripts/check-sync-deployment-evidence.sh --summary-json <evidence-file>`。涉及真实 Dart FFI bridge、同步状态读取或诊断导出时，应追加 `./scripts/check-manager-ffi-smoke.sh`。涉及 Rust / Go 同步客户端或 server handler 时，应按对应 crate / server 测试扩大验证。

## 推进顺序

后续实现应按以下顺序推进：

1. 保持 Phase 4 本地验收证据稳定。
2. 按生产部署 runbook 补目标部署运行证据包，导出 `deployment_evidence_summary.v1` 非敏感摘要，并保持 settings draft 只记录非敏感来源标签。
3. 取得至少一个可用平台私钥 backend 的生产签名证据，或补新的平台 / 算法 ADR 输入。
4. 在 manager 中只接入 sync entry state 派生和阻塞说明，仍不上传数据。
5. 补恢复码和设备授权 bridge 的结构化状态与错误测试。
6. 在恢复码确认、设备授权、部署证据和 backend gate 全部满足后，再开放用户可用同步入口。

任何一步如果需要新增 Go server API、C ABI、settings 字段、诊断字段或同步对象类型，必须先更新对应专题文档和测试口径。

## 当前停止线

- 平台私钥 backend 未解除 production gate 前，不提供恢复码创建、设备授权成功路径或真实同步主操作。
- 目标部署证据未形成前，不把远端同步展示为生产可用。
- 恢复码确认保存流程未完成前，不上传真实用户 P2 对象。
- join request、授权包、撤销和 key epoch 错误语义未映射前，不开放设备授权 UI。
- 任何明文 payload、P1 原始事件、恢复码、token、私钥、signature bytes、wrapped material bytes 或 encrypted payload bytes 进入 widget、日志、诊断或 fixture，都必须停止并回退。
