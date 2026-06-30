# RadishLex 隐私与同步设计

## 核心立场

输入法数据高度敏感。RadishLex 的同步服务必须默认不可信，客户端才是数据真相源。

服务端只应看到：

- 设备 ID。
- 加密对象 ID。
- 加密 blob 大小。
- 对象版本。
- 更新时间。
- 必要的同步元数据。

服务端不应看到：

- 明文用户词。
- 明文输入历史。
- 明文候选偏好。
- 明文应用上下文。
- 明文短语和联系人信息。

## 数据分级

### P0: 永不同步

- 密码框输入。
- 银行、支付、证件类敏感 App。
- 用户手动开启隐私模式期间的输入。
- 系统标记为 secure text entry 的内容。

### P1: 本地学习，默认不同步

- 应用上下文统计。
- 原始选择事件日志。
- 负反馈详细事件。

### P2: 加密同步

- 用户词库。
- 候选权重摘要。
- 输入方案配置。
- 自定义短语。
- 设备设置。

### P3: 可公开下载

- 官方词库包。
- 输入方案模板。
- 模型包。
- UI 主题。

## 加密对象模型

```text
SyncObject
  object_id
  object_type
  owner_device_id
  version
  base_version
  key_id
  key_epoch
  algorithm
  nonce
  encrypted_payload_len
  ciphertext_hash
  created_at
  updated_at
```

对象类型：

- `dictionary.user_terms`
- `dictionary.deleted_terms`
- `ranker.weights`
- `settings.profile`
- `settings.schema`
- `backup.snapshot`

## 同步前置检查

在 Go 后端真实同步和远端上传下载落地前，`radishlex-ime-cli sync preflight --db <path>` 只用于检查本地 userdb 的分类边界：

- P2 后续可加密同步：`dictionary.user_terms`、`ranker.weights`、`dictionary.deleted_terms`。
- P1 默认本地保留：`selection_events`、`negative_feedback`。
- 本地审计记录：`import_batches`。

该命令不得生成明文同步 payload，不连接后端，不输出用户词明文、原始事件明文或负反馈明细。它的作用是提前复验“哪些表可以进入后续加密对象，哪些表必须留在本地”。

`ime-userdb` 当前已有 Rust 内部 `UserDb::p2_plaintext_payloads()` 只读迭代器，供本地 integration test 通过 `ime-sync::SyncEnvelopeAssembler` 把 `dictionary.user_terms`、`ranker.weights` 和 `dictionary.deleted_terms` 装入 `ime-crypto` envelope，再派生 `ime-sync::EncryptedSyncObjectDraft`。该迭代器不是 CLI / FFI / 文件导出接口，不得作为明文同步文件或平台壳调用入口。

`crates/ime-sync/` 当前定义 payload 来源分类、同步对象类型、加密对象外壳草案、P2 envelope 组装边界、同步域、设备状态、加入请求、授权包、撤销记录、对象版本冲突草案模型、客户端解密后合并模型、remote client DTO / transport trait、std-only `http://` HTTP transport 和可选 bearer access token header。该合并模型已覆盖 tombstone 压过旧 user terms / ranker weights、旧 epoch 上传不能复活删除词和显式恢复语义；`ime-userdb` 已能把已解密 P2 JSON 解析为 merge input，并把被接受的 user terms、deleted tombstones 和 ranker weights 写回本地 SQLite。当前 remote client 只接受已加密 `AssembledSyncObject` 和 `SignedSyncObjectManifest`，不接受 plaintext payload；HTTP transport 只传递 encrypted payload、服务端可见 metadata 和可选 `Authorization` header，不记录请求体、响应体或 token。Rust 侧两客户端 userdb harness 已覆盖 P2 payload 加密上传、另一客户端下载密文、解密、合并写回、stale conflict 和 v2 重新上传；Go server runtime smoke 已覆盖第二设备授权和跨设备 object 版本链；Rust HTTP transport 直连 Go server 的短生命周期跨语言测试已覆盖 domain 初始化、signed encrypted object 上传、metadata / payload 读取和 stale conflict；Rust userdb 两客户端真实 Go HTTP 测试已覆盖设备授权、三类 P2 对象上传下载、客户端解密写回、stale conflict 和 v2 重新上传。Docker Compose 本地 / 部署态入口已补，本地通过 `https://localhost:7319` 验证，部署态通过同机 HTTP 上游对接外部 TLS 反代；生产部署 runbook 已固定外部 TLS、认证 / 访问控制、备份恢复、升级回滚和真实用户开放停止线；`apple-keychain-v1` 平台 runbook 和签名策略已固定，macOS backend 已在 `apple-keychain` feature 下接线并通过非 smoke 测试；真实 Keychain smoke 已运行但阻塞于 `ed25519-v1` 创建，backend status 已阻断生产签名，未进入签名成功或用户可用同步路径。Go server API / storage 边界见 `docs/sync-server-api-storage.md`，当前 Go module 已覆盖 metadata / storage / API / runtime 验证、SQLite-backed metadata repository、local object storage staged transaction、encrypted object version handler、recovery latest handler、device wrapped key bytes 承载、单用户 bearer access token 门禁、非敏感 audit events、短生命周期 HTTP smoke、真实 Go HTTP 两客户端测试和 Compose 运行边界，服务端只能保存密文对象、设备公钥、签名记录、版本和必要同步元数据。

`docs/crypto-boundary.md` 已补 `ime-crypto` 客户端加密边界，并已落地本地 AEAD envelope、ciphertext hash、device wrapping、recovery material 和撤销后 key epoch 解密边界测试。对象版本服务端可见 hash 当前对齐 Rust envelope 的 AAD + encrypted payload hash；device wrapping 和 recovery wrapped material 使用裸密文 bytes 的 hash / length 校验。任何 hash 都不得是 plaintext payload hash。

`docs/sync-key-management.md` 已补真实同步前的同步密钥与设备生命周期边界，固定设备授权、恢复码、设备撤销、key epoch、服务端可见元数据和冲突方向；`docs/adr/0002-recovery-code-kdf.md` 已固定恢复码 KDF、格式和恢复记录边界，`docs/adr/0003-device-signing-key-storage.md` 已固定设备签名和私钥存储边界，`docs/sync-server-api-storage.md` 已固定 Go sync server 的 API、SQLite metadata、对象存储、版本冲突、恢复 / 撤销记录、错误语义和验证口径，`docs/production-recovery-flow.md` 已固定生产恢复记录创建 / 轮换 / 撤销、新设备恢复加入和失败限速，`docs/runbooks/sync-server-production-deployment.md` 已固定生产部署边界，`docs/adr/0004-platform-private-key-storage-backend.md` 已固定平台私钥存储 backend 边界，`docs/adr/0005-apple-platform-signing-strategy.md` 已固定 Apple 平台签名策略，`docs/runbooks/apple-keychain-signing-backend.md` 已固定 Apple Keychain backend 平台验证边界；`ime-crypto` 已落地恢复码 KDF Rust 模型、恢复记录解密测试、Ed25519 test-memory signing key store、platform backend capability metadata、unavailable backend 明确失败、revoked key 阻断、feature-gated macOS Keychain backend、signed sync object manifest 和 signed recovery record；`ime-sync` 已落地 signed device authorization / revocation、remote object client DTO、HTTP transport 和 bearer token header；`ime-userdb` 已补 Rust 侧两客户端同步边界测试和真实 Go HTTP 两客户端测试；Go server 已起步 metadata / storage / API / runtime 验证模型、SQLite-backed metadata repository、local object storage staged transaction、签名验证、device wrapping encrypted key bytes 承载、recovery wrapped material 读取、metadata API、object version 上传下载、版本冲突、错误语义、单用户 bearer access token 门禁、非敏感 audit events、双设备 HTTP smoke、Rust HTTP transport 直连 Go server 的短生命周期跨语言测试、Rust userdb 两客户端真实 Go HTTP 测试和 Docker Compose 本地 / 部署态入口。进入用户可用同步前，应继续保持日志脱敏、payload hash / length、stale conflict 和客户端解密后合并写回证据，并补备份恢复演练、外部 TLS 验证和可用平台私钥 backend；Apple 平台若继续推进，应单独调查原生非导出 Ed25519 支持矩阵或新增独立 backend / 算法 ADR。

## 设备授权

推荐流程：

1. 第一台设备初始化主密钥。
2. 第一台设备生成恢复码。
3. 新设备生成设备密钥对。
4. 旧设备扫描新设备二维码或输入配对码。
5. 旧设备为新设备加密同步密钥。
6. 新设备开始拉取密文对象。

详细设备授权、恢复码、撤销和 key epoch 规则见 `docs/sync-key-management.md`。生产恢复流程见 `docs/production-recovery-flow.md`。Go server API、存储字段、对象上传下载边界和错误语义见 `docs/sync-server-api-storage.md`。

## 删除语义

删除必须同步。

原因：

- 用户删除某个词后，旧设备不能在下次同步时把它恢复。
- 需要 tombstone 记录删除意图。

建议：

```text
DeletedTerm
  term_id
  text_hash
  reading_hash
  deleted_at
  deleted_by_device
```

明文删除记录只保存在客户端。服务端看到的仍是加密 blob。

## 备份恢复

备份应是一个加密快照：

- 包含用户词库。
- 包含候选权重摘要。
- 包含设置。
- 不包含 P0 数据。
- 默认不包含原始事件日志。

恢复方式：

- 使用恢复码。
- 或使用已有设备授权。

恢复码路径只在客户端解开同步域材料，服务端只保存 signed recovery record 和包装密文；生产恢复流程的轮换、撤销、失败限速和日志边界见 `docs/production-recovery-flow.md`。

## 审计能力

客户端管理 UI 应提供：

- 最近同步对象数量。
- 最近上传时间。
- 最近下载时间。
- 哪些类别参与同步。
- 服务端地址。
- 当前设备列表。
- 一键停止同步。
- 一键删除本机学习数据。
- 一键从服务端删除当前账号密文数据。

## 后端部署

MVP 部署：

```text
radishlex-server
sqlite
local object storage
```

SQLite 只保存 domain、device、join request、authorization、revocation、recovery record、object metadata、blob ref 和非敏感审计事件；local object storage 只保存 encrypted object payload、recovery wrapped material，以及 device authorization wrapped key bytes 这类密文材料。Go storage 和 API 已覆盖 wrapped key bytes、recovery wrapped material、encrypted object payload 的 hash / length 复验和读取边界；对象版本 hash 按 Rust envelope AAD + encrypted payload 复验，wrapped key / recovery wrapped material 按裸密文 bytes 复验。两客户端合并写回、Docker Compose 本地 / 部署态入口、单用户 bearer access token 门禁和日志脱敏已经有测试或 runbook 证据；真实用户可用同步前仍必须补备份恢复演练、外部 TLS 真实验证和可用平台私钥 backend。业务删除通过 `dictionary.deleted_terms` 加密对象表达，服务端级删除只用于用户明确清空同步域密文数据或管理员清理整域数据。

## 日志与错误脱敏

服务端 audit event、runtime log、错误响应和 Rust remote client Debug 输出都属于隐私边界的一部分。

允许记录：

- route name / event type。
- domain id、device id、opaque object id、object type。
- object version、result code、HTTP status。
- encrypted byte length、server time、latency。

禁止记录：

- 请求体或响应体。
- access token 或 `Authorization` header。
- encrypted payload bytes、base64 payload、signature bytes、wrapped material bytes、recovery material bytes。
- 明文用户词、input code、reading、P1 原始事件、ranker 明细、窗口标题、联系人或恢复码。
- panic / stack trace 中的请求 JSON。

Rust remote client 的 `SyncRemoteRequest`、`RemoteObjectVersion` 和 `RemoteObjectPayload` Debug 输出必须继续按长度脱敏；`SyncRemoteError` 不得保存 request body、response body 或 payload bytes。

Docker Compose 服务：

```text
local services:
  sync-server:
  sync-gateway:

deployment services:
  sync-server:

local volumes:
  sync-server-data:
  sync-gateway-data:
  sync-gateway-config:
```

后期可选：

- Postgres。
- S3-compatible object storage。
- OIDC 登录。
- 多用户隔离。

## 威胁模型

### 服务端被入侵

攻击者获得：

- 密文 blob。
- 设备 ID。
- 版本元数据。

攻击者不应获得：

- 明文词库。
- 明文输入习惯。

### 单台设备丢失

应对：

- 允许从其他设备撤销该设备。
- 撤销后轮换同步密钥。
- 后续对象不再对旧设备可解密。
- 撤销前旧设备已经取得的历史密钥无法被技术上追回；如不重加密历史对象，管理 UI 必须明确展示该限制。
- 新设备恢复必须生成新的设备身份和平台私钥，不复用旧设备私钥。

### 用户误删

应对：

- 加密备份快照。
- 本地回收站。
- 明确展示删除范围。

## 默认设置

建议默认：

- 开启本地学习。
- 关闭明细事件同步。
- 开启用户词加密同步。
- 开启隐私模式快捷开关。
- 对敏感 App 默认禁学。
- iOS 未开启 full access 时不同步。
