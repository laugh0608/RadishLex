# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、停止线和下一步的维护者与协作者。本文不记录完整历史流水、字段参考或操作步骤；详细事实进入稳定边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-19（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M3 端到端加密同步 Beta
- 当前产品主批次：wrapped epoch material 远端取得/轮换证据与 macOS backend 外部资格阻塞
- 已完成：M0 工程基础、M1 macOS 离线输入 Alpha、M2 本地个人化 MVP；R00、R01A、R02L、R01B、R06A 已退出
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；合成数据、短生命周期服务与受控集成测试可以继续

M1 已完成真实 macOS 离线输入；副屏与 VoiceOver 候选操作仍不受支持。M2 manager 已通过共享 userdb、migration、隐私、导入审计、删除恢复、并发和重启验收，并于 2026-07-18 回滚到零基线。

## M3 当前边界

M3 已具备 P2 envelope、signed manifest、Go server 设备/对象/密文/备份部署边界，以及关闭态 `sync_once`。对象同步使用 domain 内 `change_sequence`；设备信任链使用独立 `lifecycle_sequence`，两类 opaque cursor 不复用。userdb schema v7 持久化对象 cursor、remote observation、local revision、cycle journal、完整密文 outbox、可信 lifecycle cache和 wrapped epoch ciphertext cache；payload apply 与对象 cursor、已验证 lifecycle state 与 lifecycle cursor 分别原子提交。SQLite/settings 不保存明文 master key。`409 stale_base_version` 必须重新发现、验签解密、合并和签名。

风险矩阵已覆盖取消、retry exhaustion、revision race、lease recovery、签名/密文/AAD 篡改、epoch/revocation、decode/SQLite 失败、outbox prepare/ack 故障和残缺 migration 回滚。两个隔离 userdb 通过短生命周期 Go HTTP 服务完成上传、发现、合并回传、service 重建与第二轮零上传收敛；失败不推进 cursor、不清除 dirty，也不丢失可重放请求。

第四、第五实现批已落地默认关闭的 `DefaultSyncObjectProcessor`、`SyncCryptoProvider` 与 `ProductSyncCryptoProvider`。产品装载层组合可信设备生命周期源、本机授权的 epoch material store 和平台签名 backend；先确认本机 active 与 backend 产品资格，再读取 secret。preflight 冻结 signing profile、当前/历史 epoch、撤销 sequence 和 key material；缺少本机 profile、backend key 匹配、当前 epoch 或产品资格均在网络前失败关闭。

Go metadata schema v4 已追加 `initial_device`、`device_authorized`、`device_revoked` lifecycle 事件；授权和撤销与序列分配在同一事务，撤销原子冻结首个拒绝对象 sequence。Rust remote client 可获取 snapshot / 增量页，从本地初始信任锚验证完整授权和撤销链；加入 challenge 使用 `profile-sha256-v1` 摘要绑定两组完整设备公钥、算法、key id、domain / join request id 和有效期，服务端状态或仅 key id 不能替换公钥。乱序、序列缺口、epoch 跳变、公钥替换、inactive signer 和 cursor 分叉均失败关闭。

两个文件 userdb 已通过短生命周期真实 Go HTTP 完成 A 初始化、B 加入授权、P2 双向同步、B 撤销、epoch 2 更新、双方缓存与重启恢复；撤销前 6 条对象 sequence 保留，B 从 sequence 7 起被拒绝。`UserDb` 已直接实现 `SyncTrustedDeviceSource`。schema v7 把签名链已绑定的 key-agreement key id/public key 结构化保存，并从 v6 `record_json` 原子回填；`UserDb` 同时实现只返回密文的 `SyncWrappedEpochMaterialSource`。

wrapped epoch material v1 已落地：`p256-ecdh-hkdf-sha256-xchacha20poly1305-v1` 使用版本化 binary envelope、65-byte ephemeral public key、完整 metadata AAD 和裸密文 SHA-256；明文只含 active object key id 与 32-byte master key。`ProductWrappedEpochMaterialStore` 先比对可信 device/key id/public key，再经独立 key-agreement backend 解封历史/当前 epoch；wrong domain/device/key id、密文/AAD 篡改、locked/unavailable/denied backend、重复/未来 epoch 和 revoked 本机均失败关闭。secret buffer 使用析构清零，只在 Rust material/cycle snapshot 中短暂存在。

`apple-keychain` feature 已提供三类现有 Apple signing store 的 `SyncDeviceSigningBackend` adapter，以及独立 `AppleSecureEnclaveP256KeyAgreementStore`：key identity/application tag 与签名 key 分离，使用 `SecKeyCopyKeyExchangeResult`，不导出长期私钥。普通测试与仓库门禁不访问 Keychain；本批只取得编译、纯 Rust协议、合成 provider和 ciphertext 重启证据，没有创建、读取或删除真实系统条目，也没有形成独立 key-agreement backend 的实机资格。

签名层已支持显式 Ed25519/P-256 profile 与 Rust/Go verifier；缺少或混用算法时失败关闭。普通 DPK P-256 产品进程生命周期可用但私钥可导出，production gate 明确拒绝；Secure Enclave 已证明 lifecycle、不可导出、hardware-backed、denied 与真实锁屏失败关闭，但 unsupported 和最终产品资格仍缺外部环境证据。

这些测试仍不是用户可用同步。当前生产阻塞项是：

- 普通 DPK P-256 可导出，Apple/Android Ed25519 也未满足生产私钥条件；`test-memory-v1` 只允许测试且无 fallback。
- Secure Enclave 已完成 lifecycle、denied 与真实设备锁屏证据并清理合成 key；unsupported 和产品资格仍待独立环境。
- 缺少发布级目标部署运行证据、device wrapped record 的用户态下载 API/真实双客户端轮换链、恢复码设备恢复，以及独立 Secure Enclave key-agreement backend 的授权实机证据；本地产品 adapter 已落地，但 secret material 链仍保持关闭。
- `ManagerBridge` 仍无真实同步、恢复码、设备加入、授权、撤销或轮换命令；现有 readiness 只证明关闭态。

开发者当前没有真实不支持 Secure Enclave 的目标环境，unsupported 与产品资格因此保持外部阻塞；不能在当前设备伪造，也不能据此开放资格字段。关闭态 provider/lifecycle 实现可继续推进，但不降低生产 backend 或真实用户同步门禁。

## 当前停止线

- M3 退出前不开放真实用户同步，不上传非受控真实 P2 数据，不提供恢复码、设备授权、撤销或轮换的产品成功入口。
- 不把 `test-memory-v1`、普通文件、SQLite、settings、generic password item 或可导出 seed 静默伪装成生产非导出 backend。
- P0 永不学习/同步；P1 原始事件只留本地，不进入 payload、manager、诊断、日志或提交记录。
- 输入热路径继续完全本地；Go server 不解密、不排序、不保存明文用户词或候选偏好。
- M2 已通过第二平台选择门禁，但当前仍集中完成 M3 macOS 同步 Beta，不同时展开第二真实平台主线。
- M4 前不宣称普通用户安装包、最终 librime/schema 分发、App Group 迁移、公证或发布供应链已经完成。

## 下一步顺位

1. 将 unsupported 保留为外部环境阻塞：目标环境可得时按独立授权执行 probe，确认明确 unsupported、无残留且不回退普通 DPK/test memory；通过后再逐字段评审 `product_qualified/user_presence_required/backup_migratable`。当前设备不能替代该证据。
2. 为 device wrapping record 增加受状态/资源上限约束的读取 API 与 Rust remote source，把当前 ciphertext cache、真实 lifecycle cache和产品 provider 组合进两个文件 userdb 的 Go HTTP 授权/轮换/撤销/重启链；被撤销设备必须在网络或材料读取前拿不到新 epoch。
3. 补恢复码恢复、recovery record 轮换与新设备身份/key-agreement key 创建；需要真实创建、读取或删除 Keychain/Secure Enclave 条目时单独申请授权，不能把合成 backend 记为实机证据。
4. lifecycle/material/recovery 产品链和发布部署稳定后才接窄 `ManagerBridge` command/status；最终满足恢复、授权、撤销、轮换和发布门禁后再评估开放用户同步。

## 验证入口

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
./scripts/check-manager-product.sh
./scripts/check-macos-imk.sh
./scripts/check-repo.sh
./scripts/check-docs.sh
./scripts/check-text-files.sh
git diff --check
cmp -s AGENTS.md CLAUDE.md
```

平台 Keychain/Keystore smoke、Docker 长流程、真实系统设置、用户数据和发布部署需要对应环境或明确授权。

## 阅读索引

- [产品路线图](../roadmap.md)：里程碑与退出标准。
- [隐私与同步](../privacy-sync.md)：数据分级、密文边界和用户可用停止线。
- [同步 Payload](../sync-payload.md)：P2 对象、remote client 与两客户端证据。
- [同步编排](../sync-orchestration.md)：Rust 状态机、discovery cursor、transaction、outbox 与冲突恢复边界。
- [同步密钥管理](../sync-key-management.md)：设备、恢复、撤销和 key epoch。
- [ADR 0006](../adr/0006-device-signature-algorithm-profiles.md)：Ed25519/P-256 profile、编码、迁移、错误与 Apple backend 边界。
- [平台私钥 Backend 策略](../platform-private-key-backend-strategy.md)：当前证据与算法/backend 决策顺序。
- [Manager 同步入口](../manager-sync-entry-boundary.md)：M3 UI/bridge 与 transient secret 边界。
- [M2 manager 验收 runbook](../runbooks/macos-m2-manager-product-acceptance.md)：关闭证据与回滚流程。
- [macOS 平台边界](../macos-inputmethodkit-boundary.md)：M1/M2 输入与隐私稳定结论。
- [本周周志](../devlogs/2026-W29.md)：完整验证和交接流水。
