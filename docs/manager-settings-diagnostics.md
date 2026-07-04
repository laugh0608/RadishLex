# Manager Settings 与诊断报告字段参考

本文档是 Phase 4 Flutter manager 的 settings draft 与脱敏诊断报告字段参考，读者是维护 manager UI、Dart FFI bridge、测试和后续文档的人。本文只描述本地非 secret 草案格式、诊断报告字段、脱敏边界和验证口径；不定义真实远端同步协议、恢复码、设备授权 UI、Go server API 或 C ABI。

## 当前边界

- `ManagerBridge` contract 不暴露 settings JSON 字段级接口；UI 通过 `saveSettingsDraft` 保存完整草案。
- settings draft 只保存本地管理端非 secret 草案，不保存 token、恢复码、私钥、signature bytes、wrapped material、payload bytes、证书、运行日志、文件路径或用户词条。
- 诊断报告只输出聚合计数、状态码、非敏感来源标签和脱敏策略，不输出用户词、导入 / 导出文件内容、本机真实路径、请求 / 响应体或 native 原始错误明细。
- 真实远端同步、恢复码和设备授权 UI 继续关闭；`preflight_ready` 只表示本地草案和预检条件可解释，不代表用户可用同步入口已开放。

## Settings Draft JSON

当前文件格式写入 `format_version: 1`。读取时只接受 `format_version == 1`；其他版本返回结构化 `settings_store_error`。

| 字段 | 类型 | 默认值 | 写入规则 | 说明 |
| --- | --- | --- | --- | --- |
| `format_version` | number | 必填 | 固定 `1` | settings draft 格式版本。 |
| `server_endpoint` | string | `""` | trim 后写入 | 自部署服务端草案。只接受 `http` / `https` URL，必须有 host，禁止 URL userinfo。 |
| `retain_sync_config` | bool | `false` | 原样写入 | 是否保留同步配置草案；为 `false` 时 `server_endpoint` 不参与 sync gate。 |
| `privacy_mode` | bool | `false` | 原样写入 | 为 `true` 时 sync gate 派生为 `sync_disabled_by_policy`。 |
| `diagnostics_export` | bool | `false` | 原样写入 | 是否允许管理端导出脱敏诊断摘要。 |
| `deployment_evidence_recorded` | bool | `false` | 原样写入 | 是否记录目标部署验证草案。必须与有效 `deployment_evidence_source` 同时成立才视为有部署证据。 |
| `deployment_evidence_source` | string | `""` | trim 后写入 | 目标部署验证来源标签，只允许 allowlist 值。 |

`deployment_evidence_source` allowlist：

| 值 | UI 标签 | 含义 |
| --- | --- | --- |
| `local_smoke` | `local smoke` | 本地短生命周期 smoke 已记录。 |
| `external_tls` | `external TLS` | 外部 TLS / 反代验证证据已记录。 |
| `backup_restore` | `backup restore` | 备份恢复演练证据已记录。 |
| `upgrade_rollback` | `upgrade rollback` | 升级 / 回滚演练证据已记录。 |

兼容规则：

- 旧 v1 文件如果缺少 `deployment_evidence_source`，即使 `deployment_evidence_recorded` 为 `true`，读取后也降级为未记录部署证据。
- 未知字段当前忽略；新增字段前必须先更新本文档和对应测试。
- 非 bool / 非 string 字段类型会返回 `settings_store_error`。
- 非 allowlist evidence source、带 userinfo 的 URL、缺少 scheme / host 的 URL 会返回 `invalid_argument`。

## Sync Gate 派生

当前 manager UI 从 settings draft 与设备签名摘要派生同步状态：

1. `privacy_mode == true`：`sync_disabled_by_policy`。
2. `retain_sync_config == false` 或 `server_endpoint` 为空：`local_only`。
3. `device.production_gate != "ready"`：`backend_unavailable`。
4. `deployment_evidence_recorded` 未与有效 `deployment_evidence_source` 同时成立：`deployment_unverified`。
5. 以上均通过：`preflight_ready`。

即使状态进入 `preflight_ready`，同步页的 `启用同步` 主按钮仍保持禁用。用户可用同步入口必须等待可用平台私钥 backend、目标部署运行证据、恢复码和设备授权链路满足对应停止线。

## 诊断报告格式

当前诊断报告文本格式为 `manager.diagnostics.v1`，`redaction_policy` 固定为 `summary_only_no_terms_paths_tokens_or_payload_bytes`。

报告头：

| 字段 | 说明 |
| --- | --- |
| `format` | 诊断报告格式版本，当前为 `manager.diagnostics.v1`。 |
| `generated_at` | manager snapshot 的生成时间摘要。 |
| `redaction_policy` | 当前脱敏策略标识。 |

## 诊断字段索引

### `runtime`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `runtime.bridge_mode` | `configuration` | fixture、Dart FFI 或 injected native binding 摘要。 |
| `runtime.userdb` | `configuration` | userdb 配置来源摘要，不输出真实路径。 |
| `runtime.native_library` | `configuration` | native library 配置来源摘要，不输出真实路径。 |
| `runtime.settings_store` | `configuration` | settings store 来源摘要。 |
| `runtime.sync_endpoint` | `configuration` | sync endpoint 是否配置的摘要，不输出 URL token 或凭据。 |
| `runtime.last_error_code` | `error_code` | 最近一次结构化错误码，默认 `none`。 |

### `settings_draft`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `settings.retain_sync_config` | `configuration` | 是否保留同步配置草案。 |
| `settings.server_endpoint` | `configuration` | `configured` 或 `not_configured`，不输出完整 endpoint。 |
| `settings.privacy_mode` | `configuration` | 隐私模式草案。 |
| `settings.diagnostics_export` | `configuration` | 诊断导出草案。 |
| `settings.deployment_evidence` | `configuration` | 部署证据标签摘要，例如 `deployment evidence external TLS` 或 `deployment evidence missing`。 |
| `settings.deployment_evidence_source` | `configuration` | allowlist source 或 `not_recorded`。 |

### `local_data`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `local.user_terms` | `aggregate_count` | 本地用户词条聚合计数。 |
| `local.deleted_terms` | `aggregate_count` | 删除 tombstone 聚合计数。 |
| `local.selection_events` | `aggregate_count` | P1 selection event 聚合计数，不输出原始事件。 |
| `local.suppressed_terms` | `aggregate_count` | 抑制词聚合计数。 |
| `local.import_batches` | `aggregate_count` | 导入批次聚合计数。 |

### `latest_import_batch`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `import_batch.present` | `aggregate_count` | 是否存在导入批次。 |
| `import_batch.total_records` | `aggregate_count` | 最新批次总记录数。 |
| `import_batch.imported_terms` | `aggregate_count` | 最新批次导入词条数。 |
| `import_batch.inserted_terms` | `aggregate_count` | 最新批次新增词条数。 |
| `import_batch.updated_terms` | `aggregate_count` | 最新批次更新词条数。 |
| `import_batch.skipped_deleted_terms` | `aggregate_count` | 因 tombstone 跳过的词条数。 |
| `import_batch.skipped_duplicate_terms` | `aggregate_count` | 因重复跳过的词条数。 |

### `sync_gate`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `sync.state` | `gate` | 当前 sync UI 状态码。 |
| `sync.state_label` | `gate` | 用户可见状态标签。 |
| `sync.state_source` | `gate` | 状态来源说明。 |
| `sync.action_stop_line` | `gate` | 当前真实同步入口停止线。 |
| `sync.deployment_evidence` | `gate` | 部署证据标签摘要。 |
| `sync.syncable_objects` | `aggregate_count` | 可同步 P2 对象聚合计数。 |
| `sync.local_only_events` | `aggregate_count` | 只本地保留事件聚合计数。 |
| `device.backend` | `gate` | 设备签名 backend id 摘要。 |
| `device.capability` | `gate` | backend capability 状态摘要。 |
| `device.production_gate` | `gate` | production gate 原始状态摘要。 |
| `device.production_gate_label` | `gate` | production gate 用户可见标签。 |

### `redaction`

| 字段 | kind | 说明 |
| --- | --- | --- |
| `redaction.user_terms` | `policy` | 用户词条已省略。 |
| `redaction.file_paths` | `policy` | 文件路径已省略。 |
| `redaction.tokens` | `policy` | token 已省略。 |
| `redaction.payload_bytes` | `policy` | payload bytes 已省略。 |

## 禁止输出

settings draft、诊断报告、测试 fixture 和截图均不得包含：

- 用户输入历史、候选明细、联系人、窗口标题、真实用户词条或完整 userdb dump。
- 导入文件原文、导出文件内容、真实本机路径、真实账号、真实 token、恢复码、私钥、签名 bytes、wrapped material bytes、encrypted payload bytes。
- Go server 请求体 / 响应体、内部 `blob_ref`、证书正文、日志正文或 provider exception 原文。

## 验证入口

当前字段和边界由以下验证覆盖：

```text
flutter test
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
git diff --check
./scripts/check-repo.sh
```

关键测试覆盖：

- settings draft v1 写入 / 读取、旧 v1 缺 `deployment_evidence_source` 降级、未知格式拒绝、非法 URL 和非法 evidence source 拒绝。
- 设置页 deployment evidence source 下拉、`deployment_unverified` 到 `preflight_ready` 的本地草案派生、真实同步按钮继续禁用。
- 诊断报告包含 gate source / stop line / evidence source 摘要，并保持用户词、路径、token 和 payload bytes 脱敏。
- FFI smoke 使用临时 SQLite userdb、临时 settings JSON 和合成数据复验真实 Dart FFI bridge，不连接真实同步后端。
