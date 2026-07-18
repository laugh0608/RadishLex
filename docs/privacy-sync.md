# RadishLex 隐私与同步设计

本文档定义 RadishLex 长期稳定的数据分级、服务端可见性、密钥、设备、删除、恢复、日志和威胁模型边界，读者是 core、sync、server、manager 与平台实现者。本文不记录当前实现清单、设备 smoke 流水、部署命令或临时整改状态；这些内容分别进入 `docs/status/current.md`、对应 runbook、ADR 和 devlog。

## 核心立场

输入法数据高度敏感。同步服务默认不可信，客户端才是用户数据和解密能力的真相源。

输入热路径必须完全本地可用。后端不能参与每次按键、候选生成、候选排序、文本提交或隐私判断。

## 数据分级

### P0：永不学习、永不同步

- 密码框和系统 secure text entry。
- 银行、支付、证件等敏感应用或场景。
- 用户开启隐私模式期间的输入。
- 平台明确标记为不可学习的内容。

P0 数据不写 selection event、negative feedback、user term 或同步摘要。平台无法可靠判断时，应优先按更严格等级处理。当前隐私模式可以只读使用隐私模式开启前已经存在的本地 P2 排序摘要，但本次输入仍按 P0 处理且不产生任何学习写入；secure input、敏感应用或上下文无法可靠判断时连既有个人化摘要也不读取，只使用 engine 顺序。

### P1：本地学习、默认不同步

- 原始选择事件。
- 负反馈详细事件。
- 应用和短语上下文统计。
- 本地审计批次与调试统计。

P1 可以在客户端压缩为不可还原原始输入的 P2 摘要，但压缩规则必须明确、可测试和可删除。

### P2：端到端加密同步

- 用户词库。
- 候选权重摘要。
- 输入方案和用户配置。
- 自定义短语。
- 设备设置。
- 删除 tombstone。

P2 只能以客户端加密对象传输和存储。服务端不得提供 plaintext payload API。

### P3：可公开下载

- 官方词库包。
- 输入方案模板。
- 模型包。
- UI 主题。

P3 包仍需要来源、版本、完整性和许可证校验，但不使用用户同步密钥。

## 服务端可见性

服务端可以看到：

- domain、device、显式签名算法和必要公钥 metadata；
- 加密对象 ID、类型、版本、base version 和 key epoch；
- 密文长度、ciphertext hash、创建和更新时间；
- 加入、授权、撤销和恢复记录的公开协议字段；
- 请求 ID、结果码、延迟和必要非敏感审计摘要。

服务端不得看到：

- 明文用户词、输入历史、自定义短语和联系人；
- 明文候选偏好、原始 selection/negative event；
- 明文应用上下文或 phrase context；
- sync master key、object key、recovery code、私钥或解密后的 wrapped material；
- 认证 token、完整请求/响应体或平台密钥内部材料。

## 加密对象

稳定外壳至少包含：

```text
SyncObject
  domain_id
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
  created_at_ms
  updated_at_ms
  signer metadata
  signature
```

要求：

- ciphertext hash 基于密文或 AAD + 密文，不得基于 plaintext。
- AAD 绑定会影响解释或路由的对象 metadata。
- nonce 在同一 key/epoch 下不得重复。
- 签名覆盖安全相关 metadata 和实际密文 hash，不能只覆盖密文长度。
- 算法、schema 和 signature version 必须显式可演进。
- 下载后先验证大小、hash、签名、版本和资源上限，再进入高成本解密或 KDF。

## P1 到 P2 的边界

允许同步的对象类型至少包括：

- `dictionary.user_terms`
- `dictionary.deleted_terms`
- `ranker.weights`
- `settings.profile`
- `settings.schema`
- `backup.snapshot`

禁止直接同步：

- `selection_events`
- `negative_feedback`
- 原始上下文记录
- `import_batches`

客户端内部可以提供 P2 payload encoder，但它不是 CLI 明文导出、FFI 管理接口或平台壳入口。任何同步 orchestration 都只接受已经分类、序列化并加密的对象；禁止为了联调增加 plaintext HTTP 上传路径。

同步预检只能输出类型、计数、状态和阻塞原因，不输出用户词、事件或 payload bytes。

## 密钥层级

```text
Recovery Code
  -> Recovery Wrapping Key
     -> wrapped Sync Master Key

Device Key Agreement / Wrapping Key
  -> wrapped Sync Master Key for one device

Sync Master Key + object identity + key epoch
  -> Object Encryption Key
```

要求：

- sync master key 不上传明文。
- object key 按 domain/object/epoch 派生，避免跨对象复用。
- 设备撤销后推进 key epoch，撤销设备不能解密后续对象。
- recovery wrapping 和 device wrapping 使用独立 role、algorithm ID 和 AAD。
- secret 类型应减少 Clone，并在生命周期结束时清零或使用受控 secret container。

## 平台私钥

平台 backend 必须报告：

- backend ID 与版本；
- signing/key-agreement algorithm；
- 是否不可导出；
- 是否声明硬件保护；
- 当前 capability 和 production status；
- 创建、加载、签名、删除和失效错误。

禁止从平台名称推断算法一定可用，也禁止 unavailable backend 静默回退到内存私钥或普通文件。

协议必须允许算法演进。平台原生 P-256 与“由平台密钥封装的 Ed25519 seed”具有不同保护语义，必须使用不同 backend/algorithm ID 和测试矩阵。

当前设备签名 allowlist 为 `ed25519-v1` 与 `ecdsa-p256-sha256-v1`。后者只接受 65-byte SEC1 uncompressed public key 和 64-byte P1363 signature；两者复用既有 canonical builder，算法 id 本身被签名。设备登记必须绑定 algorithm、key id 与 public key，服务端不能按长度猜测算法，验签失败也不能尝试另一 profile。历史行回填 Ed25519 只能发生在明确 schema migration，新请求不得默认。

`apple-keychain-p256-v1` 只表示 Apple 原生 P-256 `SecKey` 候选，和 Ed25519 `apple-keychain-v1` 分离。基础生命周期 gated smoke 已通过，但产品进程访问、locked/denied 和 capability status 未闭环前仍保持 production status 关闭；不可从“Apple Keychain”名称或基础 smoke 推断 Secure Enclave、hardware-backed、user presence 或可迁移能力。

## 新设备授权

推荐流程：

1. 新设备生成签名和密钥协商/封装能力。
2. 新设备创建带过期时间、challenge 和短码的 join request。
3. 已授权设备验证用户确认、设备身份和协议版本。
4. 已授权设备为新设备封装当前 sync master key。
5. 授权签名绑定双方设备 ID、接收公钥材料、challenge、key epoch、算法、nonce 和 wrapped ciphertext hash。
6. 服务端验证签名和设备状态后激活新设备。
7. 新设备下载、验签并解封同步密钥，再开始对象同步。

短码只用于人机确认，不能代替密码学 challenge 或签名绑定。授权包、短码和 wrapped key 不进入日志或长期诊断。

## 设备撤销

- 只有 active 已授权设备可以签署撤销。
- 撤销记录绑定被撤销设备、执行设备、原因、旧 epoch、新 epoch 和时间。
- 撤销后服务端拒绝被撤销设备上传新对象或获取新 wrapped key。
- 客户端为后续对象轮换 key epoch；旧对象是否重加密由恢复与历史策略决定。
- 设备丢失场景不得要求丢失设备参与。

## 删除语义

删除强于普通降权。用户删除词条时必须：

1. 在本地原子更新 user term 状态。
2. 写入带稳定身份与版本的 tombstone。
3. 清理或屏蔽相关 ranker 权重。
4. 将 tombstone 作为 P2 对象同步。

合并要求：

- 旧 active term、旧 weight、旧选择事件和旧备份不能压过新 tombstone。
- 显式恢复必须是新的用户意图，并使用比 tombstone 更新的稳定版本。
- 同时间戳冲突使用确定性 device/object tie-break，不能依赖到达顺序。
- 合并必须把本地当前状态作为一等输入，并满足交换律、结合律和幂等性。

## 恢复码与恢复记录

- 恢复码由客户端生成，不上传明文。
- KDF 使用版本化 Argon2id profile，并同时限制最小和最大 memory、iterations、parallelism、salt 和 output。
- recovery record 保存 salt、KDF profile、algorithm、nonce、wrapped key metadata、ciphertext hash 和签名。
- 签名绑定实际 wrapped ciphertext hash，服务端不能替换密文后只更新 hash。
- 恢复记录可创建、轮换和撤销；旧恢复码在撤销后不可继续加入设备。
- 恢复失败需要服务端和客户端双层限速，但不能依赖可伪造 header 作为唯一身份。

## 备份与恢复

服务端备份必须成对包含：

- SQLite metadata；
- encrypted blob directory；
- migration/version 信息；
- 恢复所需的部署配置，但不包含未加密 secret 导出。

客户端仍是明文真相源。服务端备份恢复后必须重新验证 metadata/blob 长度与 hash，不能把缺失或不匹配的 blob 当作成功对象。

用户应能：

- 导出本地加密备份；
- 验证恢复码；
- 查看非敏感恢复记录状态；
- 撤销恢复记录；
- 从恢复设备重新建立受信任设备集合。

## Manager 可见性

Manager 可以显示：

- 当前设备和授权状态；
- 最近同步时间、待同步计数和结构化错误；
- key epoch、恢复记录状态和撤销提示；
- 后端 endpoint 的净化显示与连接健康分类；
- 隐私模式、学习开关和删除/导出操作。

Manager 不得显示或持久化：

- token、recovery code、短码历史；
- signature bytes、wrapped material、payload bytes；
- 原始请求/响应体；
- 明文 P1 事件或真实敏感路径。

诊断只使用 allowlist 字段和结构化状态码。真实同步操作必须通过 Rust sync/crypto 边界，Flutter 不自行构造签名或解密 payload。

## 后端部署

生产部署必须：

- 使用 HTTPS/TLS，明确外部反代或内置终止边界；
- 强制非空认证，无认证只允许显式 loopback 开发模式；
- 在解码 JSON/base64 前限制请求体，限制响应体和字符串/ID 长度；
- 配置连接、读、写、idle 和总超时；
- 使用持久化或可控限速策略，避免伪造 header 绕过；
- 支持 graceful shutdown、迁移、备份恢复和升级回滚；
- 仅记录非敏感 audit metadata，并处理审计写入失败。

Docker/反代操作步骤见对应 runbook，当前部署证据见 `docs/status/current.md` 和 devlog，不复制到本文。

## 日志、错误与测试数据

允许记录：

- request ID、route、result code 和 latency；
- device/domain/object 的受控 opaque ID；
- version、key epoch、密文大小和重试分类；
- backend capability 与非敏感平台错误码。

禁止记录：

- 明文输入、用户词、联系人和上下文；
- token、恢复码、私钥、密钥 seed；
- wrapped key、signature、nonce 或 payload 的实际 bytes；
- 完整请求/响应体和真实用户文件路径。

测试只使用合成词、虚构设备、虚构 App ID 和公开样例。性能或长期统计不得上传真实输入流。

## 威胁模型

至少考虑：

- 服务端或管理员读取存储；
- 数据库或 blob 备份泄漏；
- 中间人、恶意反向代理和错误 TLS；
- 被撤销或长期离线设备重放旧状态；
- 服务端替换公钥、wrapped ciphertext、hash 或版本 metadata；
- 恶意 KDF 参数、超大请求/响应和慢连接资源消耗；
- 本地恶意应用读取 manager/IME 共享目录；
- debug、panic、截图和诊断泄漏；
- 冲突顺序导致删除复活或设备分叉。

不承诺在设备已被完全控制时保护该设备上的当前明文；目标是缩小暴露面、阻止服务端解密、支持撤销和减少持久敏感材料。

## 默认设置

- 默认本地学习开启，但 P0 场景自动禁学。
- 隐私模式默认关闭；开启后当前输入停止学习，但不删除既有本地词库和排序摘要。
- P1 默认不同步。
- P2 只有在用户配置同步、设备授权和安全门禁通过后才同步。
- 诊断默认脱敏，详细敏感日志不存在“临时开启”后门。
- 无网络或后端不可用时输入功能不受影响。
- 真实同步失败必须明确展示，不降级为明文、匿名或未认证上传。

## 用户可用同步停止线

在以下条件全部满足前保持关闭：

- 平台私钥 backend 在目标平台真实可用；
- 授权、恢复、撤销和 key epoch 有完整实现测试；
- merge 确定收敛，删除与显式恢复通过多设备测试；
- 签名绑定公钥和密文 hash，KDF/解析资源上限已验证；
- Rust 客户端使用生产 HTTPS transport 和完整 sync orchestration；
- Go server 的认证、限速、请求上限、备份恢复和部署证据达到发布要求；
- Manager 只通过真实 Rust bridge 执行操作并保持 secret 生命周期边界。

## 相关文档

- [当前状态](status/current.md)
- [技术方案](technical-plan.md)
- [同步 Payload](sync-payload.md)
- [加密边界](crypto-boundary.md)
- [同步密钥管理](sync-key-management.md)
- [生产恢复流程](production-recovery-flow.md)
- [Sync Server API/Storage](sync-server-api-storage.md)
- [平台私钥策略](platform-private-key-backend-strategy.md)
- [设备签名算法 Profile ADR](adr/0006-device-signature-algorithm-profiles.md)
- [Manager Boundary](manager-ui-boundary.md)
- [生产部署 Runbook](runbooks/sync-server-production-deployment.md)
