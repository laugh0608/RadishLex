# RadishLex 生产恢复流程设计

本文档定义 RadishLex 真实同步前的生产恢复流程边界，读者是后续实现 `ime-crypto`、`ime-sync`、Go sync server、管理 UI 同步恢复页面和审阅隐私边界的开发者。本文不包含 Flutter 页面布局、Go handler 实现、平台私钥存储 backend 实现、真实远端上传下载代码或平台输入法接入流程。

## 当前定位

当前已经完成：

- `docs/adr/0002-recovery-code-kdf.md` 固定恢复码格式、Argon2id KDF profile、恢复记录字段、AAD 绑定和失败限速口径。
- `ime-crypto` 已落地 `RecoveryCode`、`RecoveryKdfProfile`、恢复 wrapping key 和 `RecoveryMaterial` 加解密模型。
- `docs/adr/0003-device-signing-key-storage.md` 固定设备签名对象、canonical bytes、私钥存储抽象和错误语义。
- `ime-crypto` / `ime-sync` 已覆盖 signed recovery record、signed device authorization、signed device revocation 和客户端合并写回 userdb。
- `docs/sync-server-api-storage.md` 已固定 Go server 只保存恢复记录 metadata、包装密文、签名和必要同步元数据。
- Go server storage / API 已能校验 recovery wrapped material 的长度 / ciphertext hash，把 wrapped material 写入 local object storage，metadata 中保存 `signer_device_id`、signature 和 `blob_ref`，并通过 recovery latest handler 返回 latest active metadata 与 encrypted wrapped material；读取路径已覆盖限速和日志脱敏测试。

本阶段只固定生产恢复流程，不实现真实 UI、平台 Keychain / Keystore backend 或两客户端端到端同步。

## 设计目标

- 用户在没有旧设备可用时，可以凭离线保存的恢复码恢复同步域材料。
- 服务端默认不可信，不能通过恢复记录、账号密码、管理 token 或日志解密用户数据。
- 恢复码只恢复同步域材料，不能绕过设备状态、设备撤销、key epoch 或签名校验。
- 恢复成功的新设备必须进入正常设备生命周期：登记、激活、拉取密文对象、本地解密和客户端合并。
- 恢复记录可以轮换、撤销和替换；旧记录不得悄悄复活已撤销设备或旧 key epoch。
- 恢复流程不进入输入热路径，不同步 P0/P1 明细，不暴露 plaintext sync payload。

## 角色与状态

`RecoveryCode`：

- 客户端生成、用户离线保存。
- 不上传服务端，不写日志，不通过 FFI 明文导出。
- 用于派生 `RecoveryWrappingKey`，解开恢复记录中的同步域材料。

`RecoveryRecord`：

- 服务端可保存的恢复记录 metadata、KDF 参数、salt、envelope nonce、包装密文、状态和签名。
- 不包含恢复码明文、派生 key、`SyncMasterKey` 明文或 plaintext payload。

`RecoveryRecordStatus` 建议：

- `active`：当前可用于恢复。
- `superseded`：已被新恢复记录替换，默认不再作为 latest 返回。
- `revoked`：用户明确撤销，不允许恢复。
- `expired`：后续策略可选，用于强制轮换或安全策略。

`RecoveredDevice`：

- 使用恢复码解开同步域材料的新设备。
- 恢复完成后必须成为同步域中的 `active` 设备，拥有自己的设备签名 key 和 key agreement key。
- 不继承旧设备 ID、旧设备私钥或旧设备本地 userdb。

## 第一台设备初始化

初始化流程：

1. 第一台设备在本地生成同步域 ID、`SyncMasterKey`、设备签名 key 和设备 key agreement key。
2. 第一台设备创建 `active` 设备记录。
3. 客户端生成 `RecoveryCode`，并提示用户离线保存。
4. 客户端生成恢复记录 salt，按 `argon2id-v1` profile 派生 `RecoveryWrappingKey`。
5. 客户端用恢复 wrapping key 加密同步域恢复材料，生成 `RecoveryRecord`。
6. 客户端用设备签名 key 签名恢复记录 manifest。
7. 服务端只保存 domain metadata、设备公钥、signed recovery record 和包装密文。

规则：

- 恢复码展示必须是一次明确操作，不能静默写入服务端或日志。
- 如果用户跳过保存恢复码，管理 UI 必须显示“无恢复记录”或等价状态。
- 第一台设备可以后续补建恢复记录，但仍必须使用 active 设备签名。
- committed fixture 只能使用合成恢复码，不能包含真实用户恢复码。

## 恢复记录轮换

轮换场景：

- 用户怀疑恢复码泄漏。
- 恢复码已使用过，建议生成新恢复码。
- KDF profile 升级。
- 同步域 key epoch 推进后，需要让恢复记录绑定当前 epoch。

轮换流程：

1. active 设备本地生成新 `RecoveryCode`。
2. active 设备用当前同步域材料创建新 `RecoveryRecord`。
3. active 设备签名新恢复记录 manifest。
4. 客户端提交新记录，并把旧记录标记为 `superseded` 或 `revoked`。
5. 服务端更新 latest recovery record 指向新记录。

规则：

- 轮换不得复用旧恢复码 secret 或旧 salt。
- 新记录必须绑定当前 `domain_id`、`key_epoch`、KDF 参数、salt、nonce、包装密文长度和 ciphertext hash。
- 服务端不能自行生成恢复码、KDF 输出或恢复包装密文。
- 旧记录保留 metadata 可用于审计，但不应继续作为 `latest` 返回。

## 恢复记录撤销

撤销场景：

- 用户确认恢复码丢失或泄漏。
- 用户决定只允许已有设备授权新设备。
- 设备撤销后需要禁用旧 epoch 恢复材料。

撤销流程：

1. active 设备创建 signed recovery record revocation。
2. 服务端验证签名设备 active、记录存在、状态允许撤销。
3. 服务端把记录状态改为 `revoked`。
4. 后续 `recovery-records/latest` 不返回 revoked 记录。

规则：

- 撤销恢复记录不等于删除服务端所有密文对象。
- 撤销恢复记录不能删除用户词 tombstone，也不能替代 `dictionary.deleted_terms`。
- 如果所有恢复记录都撤销，用户必须依赖已有 active 设备授权新设备。

## 新设备使用恢复码加入

恢复加入流程：

1. 新设备生成自己的设备签名 key 和 key agreement key。
2. 新设备向服务端读取 active recovery record metadata 和包装密文。
3. 新设备本地校验恢复码格式和校验段。
4. 新设备按恢复记录 KDF profile 派生 `RecoveryWrappingKey`。
5. 新设备用恢复 wrapping key 解开同步域材料。
6. 新设备验证恢复记录签名、domain、key epoch 和 AAD。
7. 新设备创建 device join request，或直接提交 signed recovered-device activation request。
8. 服务端保存新设备公钥，并将设备状态置为 `active`，前提是恢复记录有效且未撤销。
9. 新设备拉取密文对象，在本地解密、合并并写回 userdb。
10. 管理 UI 提示用户轮换恢复码。

是否需要旧设备确认：

- 恢复码路径用于没有旧设备可用的场景，因此不要求旧设备在线确认。
- 恢复码路径仍必须创建新设备身份，不允许复用旧设备身份。
- 服务端只根据 signed recovery record、恢复记录状态、设备公钥和限速规则接受恢复加入；服务端不验证恢复码明文。

恢复后 key epoch：

- 如果恢复记录绑定当前 `key_epoch`，新设备可以接收当前 epoch 后续对象。
- 如果恢复记录绑定旧 epoch，客户端必须按策略触发恢复记录轮换或 key epoch 推进，不得用旧记录解锁撤销后的新对象。
- 被撤销设备不能用旧恢复材料重新变成 active；恢复流程创建的是新设备。

## 全部设备丢失

当用户没有任何旧 active 设备时：

- 只要存在 active recovery record 且用户持有正确恢复码，新设备可以恢复同步域材料。
- 如果没有 active recovery record，服务端不能帮助解密用户数据。
- 如果恢复记录存在但恢复码丢失，服务端只能删除密文数据或保留密文备份，不能恢复明文。

管理 UI 和文档必须明确：

- 恢复码是离线恢复同步域的必要材料。
- 自部署服务端管理员权限不能替代恢复码。
- 删除同步域密文数据是不可逆操作，除非用户另有本地设备或本地备份。

## 设备撤销后的恢复

设备撤销与恢复流程必须共享 key epoch 边界：

- 撤销设备后，后续对象使用新 `key_epoch`。
- 新恢复记录应绑定撤销后的当前 `key_epoch`。
- 旧 epoch recovery record 不得恢复撤销后的新对象。
- 如果用户从旧恢复记录恢复，只能读取该记录可解开的历史材料；后续必须通过 active 设备或新恢复策略进入当前 epoch。
- 服务端在 `current_key_epoch` 推进后，应拒绝低于当前 epoch 的新对象写入。

历史对象策略仍按 `docs/sync-key-management.md`：

- 默认不承诺技术追回已被旧设备取得的历史密钥。
- 历史重加密是独立能力，需要单独设计成本、验证和 UI 告知。

## 失败与限速

客户端失败类型：

- 恢复码格式错误。
- 校验段不匹配。
- KDF profile 不支持或低于安全下限。
- AAD 不匹配。
- AEAD 解密失败。
- 解密出的 key role 或 domain 不匹配。
- 恢复记录状态不是 active。

服务端失败类型：

- recovery record 不存在。
- recovery record 已撤销或被替换。
- 读取频率过高。
- 设备登记冲突。
- 签名无效。
- 设备状态不允许恢复加入。

限速规则：

- 客户端应在连续恢复失败后增加本地等待时间。
- 服务端应对 `domain_id`、`recovery_record_id`、IP、设备指纹和时间窗做读取限速。
- 限速不能替代恢复码强度；攻击者拿到恢复记录后仍可能离线尝试。
- 错误响应不得区分“恢复码接近正确”这类信息，不回显恢复码片段或派生材料。

## 日志与审计

允许记录：

- recovery record id
- domain id
- device id
- 操作类型
- 状态变更
- server time
- 错误分类

禁止记录：

- 恢复码明文或片段。
- `RecoveryWrappingKey`、`SyncMasterKey`、object key、device private key。
- encrypted recovery key bytes、payload bytes 或请求体。
- 明文用户词、input code、reading、候选偏好、P1 事件或本地数据库路径中的敏感片段。

客户端审计 UI 后续可以展示：

- 当前是否存在 active recovery record。
- 最近恢复记录轮换时间。
- 最近恢复成功时间。
- 恢复码是否建议轮换。
- 撤销 / 替换记录数量。

## 与 Go server 的边界

Go server 可以：

- 保存 signed recovery record metadata。
- 保存 encrypted wrapped material bytes，并以 `blob_ref` 关联 metadata。
- 保存 recovery record status。
- 返回 latest active recovery record。
- 对恢复记录读取和替换限速。
- 校验签名设备 active、记录状态和字段完整性。

Go server 不可以：

- 接收恢复码明文。
- 执行 Argon2id KDF。
- 解密恢复包装密文。
- 根据账号密码、管理 token 或服务端管理员权限生成同步域材料。
- 判断 userdb payload 合并结果。

API 和 storage 字段见 `docs/sync-server-api-storage.md`，本文件只固定恢复业务流程。

当前 Go storage 边界：

- `PutRecoveryRecord` 接收 recovery metadata 和 wrapped material bytes，写入前校验长度与 ciphertext hash，并要求 signer device 是 `active`。
- `LatestRecoveryWrappedMaterial` 返回 latest active recovery metadata 和 encrypted wrapped material bytes，读取前复验 blob 长度和 ciphertext hash。
- `GET /api/v1/domains/{domain_id}/recovery-records/latest` 使用读取限速，响应不暴露内部 `blob_ref`，日志不包含 wrapped material bytes。
- 恢复记录 blob 缺失、长度不一致或 hash 不一致时，应返回 `storage_unavailable`，不能把损坏密文当作可恢复状态。
- 服务端错误响应不得区分“恢复码接近正确”或泄漏 KDF 输出、AAD、wrapped material bytes。

## 与管理 UI 的边界

后续管理 UI 可以提供：

- 生成恢复码。
- 确认用户已保存恢复码。
- 轮换恢复码。
- 撤销恢复记录。
- 使用恢复码加入同步域。
- 显示恢复失败原因的非敏感分类。

管理 UI 不应：

- 把恢复码保存到远端。
- 在普通日志、崩溃报告、截图或 analytics 中包含恢复码。
- 把恢复码当作登录密码。
- 在输入法热路径中请求恢复码。
- 在没有用户确认的情况下自动生成并覆盖恢复记录。

## 实施顺序

1. 已完成恢复码 KDF ADR 与 Rust model。
2. 本文档固定生产恢复流程、记录状态、轮换、撤销、恢复加入和失败处理。
3. 已补平台私钥存储 backend ADR 与 Rust capability / unavailable backend 模型，明确生产设备签名 key 不应穿过 FFI、CLI 或 Go server。
4. 已在 Go server storage 验证模型中覆盖 recovery record metadata、`blob_ref` 分配、wrapped material staged blob 写入与 hash / length 校验，不接触恢复码明文。
5. 已补 Go server recovery latest handler，覆盖 wrapped material 读取、状态、限速和日志脱敏验证。
6. 后续真实平台 backend 通过验证后，管理 UI 再接入用户可见恢复流程。
7. 最后再接两客户端端到端同步和真实上传下载。

## 验证口径

进入真实远端同步前必须覆盖：

- 初始化同步域时可以创建 active recovery record，且服务端记录不含恢复码明文。
- 恢复记录轮换使用新 secret、新 salt 和新签名，旧记录不再作为 latest 返回。
- 撤销恢复记录后不能再用于新设备恢复。
- 新设备恢复后拥有新的 device id 和设备公钥，不复用旧设备身份。
- 错误恢复码、错误 AAD、错误 domain、错误 key epoch 和弱 KDF profile 都失败。
- 恢复成功后仍遵守 active / revoked / lost 设备状态和 key epoch。
- 全部设备丢失但恢复码存在时，可以恢复同步域材料；恢复码丢失时服务端不能解密。
- 日志和错误对象不包含恢复码、派生 key、同步主密钥、wrapped material bytes 或 plaintext payload。
- 恢复流程不导出 CLI / FFI 明文同步 payload，不进入输入热路径。
- Go recovery handler 必须能同时返回 latest metadata 和 encrypted wrapped material bytes，并在返回前复验 blob 长度 / ciphertext hash；只返回 metadata 不构成可恢复链路。

## 停止线

- 平台私钥存储真实 smoke、权限错误和备份迁移语义未通过平台验证前，不提供用户可用恢复 UI；当前 `apple-keychain-v1` 已完成 runbook、Apple 签名策略 ADR、feature-gated macOS backend 接线和非 smoke 测试，但真实 smoke 阻塞于 `ed25519-v1` 创建，backend status 已阻断生产签名，不代表可用后端；`android-keystore-v1` 已完成 runbook、feature-gated Rust store、Rust bridge wrapper、bridge contract、合成 bridge 单测、ignored smoke 门禁、仓库内 Kotlin / Gradle harness、`@JvmStatic` facade 和 gated instrumented smoke，已补 Rust raw JNI glue，但尚未运行 Android target build、Android Gradle build 或真实 Android Keystore smoke。
- Go server 恢复记录 API 若未持续覆盖 wrapped material 读取、签名、状态、限速和日志脱敏，不接真实恢复客户端。
- 恢复码、同步主密钥或设备私钥可能进入服务端日志、错误响应、崩溃报告或截图时，必须停止并修正设计。
- 恢复流程不能绕过设备撤销、key epoch 或客户端合并语义。
