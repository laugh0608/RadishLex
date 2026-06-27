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
  encrypted_payload
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

在 Go 后端、`ime-sync` 和 `ime-crypto` 落地前，`radishlex-ime-cli sync preflight --db <path>` 只用于检查本地 userdb 的分类边界：

- P2 后续可加密同步：`dictionary.user_terms`、`ranker.weights`、`dictionary.deleted_terms`。
- P1 默认本地保留：`selection_events`、`negative_feedback`。
- 本地审计记录：`import_batches`。

该命令不得生成明文同步 payload，不连接后端，不输出用户词明文、原始事件明文或负反馈明细。它的作用是提前复验“哪些表可以进入后续加密对象，哪些表必须留在本地”。

`crates/ime-sync/` 当前只定义 payload 来源分类、同步对象类型和加密对象外壳草案。真实加密、hash 计算、签名、设备授权、上传下载和冲突合并执行器仍属于后续阶段。

`docs/crypto-boundary.md` 已补 `ime-crypto` 进入实现前的客户端加密边界。后续服务端可见 hash 必须是 ciphertext hash 或密文加 AAD 的 hash，不得是 plaintext payload hash。

## 设备授权

推荐流程：

1. 第一台设备初始化主密钥。
2. 第一台设备生成恢复码。
3. 新设备生成设备密钥对。
4. 旧设备扫描新设备二维码或输入配对码。
5. 旧设备为新设备加密同步密钥。
6. 新设备开始拉取密文对象。

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

Docker Compose 服务：

```text
services:
  radishlex-server:
  radishlex-data:
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
