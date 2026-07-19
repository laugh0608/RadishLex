# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、停止线和下一步的维护者与协作者。本文不记录完整历史流水、字段参考或操作步骤；详细事实进入稳定边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-19（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M3 端到端加密同步 Beta
- 当前产品主批次：macOS backend 外部资格阻塞与关闭态 Rust sync orchestration
- 已完成：M0 工程基础、M1 macOS 离线输入 Alpha、M2 本地个人化 MVP；R00、R01A、R02L、R01B、R06A 已退出
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；合成数据、短生命周期服务与受控集成测试可以继续

M1 已完成真实 macOS 离线输入；副屏与 VoiceOver 候选操作仍不受支持。M2 本地学习与 manager 验收、回滚于 2026-07-18 完成。

## M2 关闭证据

manager 产品态已证明 ABI v5、共享 userdb、migration、导入审计、删除恢复、rank explain、双端刷新、并发与重启一致，且失败不回退 fixture。隐私模式零学习；secure field 只证明系统路由旁路。

最终回滚已恢复零基线，详见 manager 验收 runbook。

## M3 当前边界

M3 已具备 P2 envelope、signed manifest、两客户端授权/上传/下载/解密/合并/conflict 测试，以及 Go server 设备、join authorization、对象版本、密文存储、备份恢复、外部 TLS 和升级回滚受控证据。

关闭态 orchestration 第一实现批已落地：Go/Rust 使用 domain 内 `change_sequence` 与 opaque cursor 分页发现；userdb schema v5 持久化 cursor、remote observation、local revision、cycle journal 和完整密文 outbox；payload apply、观察、revision 与 cursor 在同一 transaction 提交。Rust `sync_once` 已覆盖 prepared outbox 恢复、上传 ack，以及 `409 stale_base_version` 后重新发现、验签解密、合并和重新签名；测试只使用合成 P2 与 `test-memory-v1`。

第二风险批已证明取消在网络与 outbox 前失败关闭、transport retry exhaustion 保留完全相同的 prepared outbox 供下轮重放、旧 revision ack 不清除并发产生的新 dirty revision、活跃 lease 拒绝重入且过期 lease 可恢复。两个隔离 userdb 已通过真实短生命周期 Go HTTP 服务执行完整 `sync_once`：首轮上传、发现下载、验签解密、合并回传、service 重建与第二轮零上传收敛均成立；签名、密文和认证 metadata 篡改在应用事务前被拒绝。该证据仍只使用合成 P2、共享测试 master key 与 `test-memory-v1`，不构成产品 backend 或真实用户同步资格。

ADR 0006 已接受 `ecdsa-p256-sha256-v1` 与 `ed25519-v1` 共存，固定 P-256 公钥、签名和 canonical encoding。Rust/Go verifier、共享正负向 fixture、显式 `signing_algorithm` metadata 和历史 Ed25519 migration 已落地；新请求缺少或混用算法时失败关闭。

独立 `apple-keychain-p256-v1` 已接通 Apple Security、FFI 与 manager Release native library，并在 provisioning-backed 产品进程完成 DPK 创建、重载、签名、Rust/Go 验签、删除和 cleanup；禁止 legacy fallback，敏感 bytes 不进入 Dart，故 `compiled/runtime_available/can_create/can_sign=true`。

评审同时确认普通软件 DPK P-256 私钥可由平台 API 导出，不能继续声明 `exportable=false`。当前 capability 已改为 `exportable=true`，`ensure_production_signing_allowed()` 明确拒绝；`product_qualified/user_sync_enabled=false`。这不是 Secure Enclave、hardware-backed、user presence 或 backup migration 证据。

这些测试仍不是用户可用同步。当前生产阻塞项是：

- `apple-keychain-p256-v1` 的 DPK 软件运行时可用，但私钥可导出，未满足生产 backend 的不可导出条件；locked 矩阵不能改变该资格结论，故本 backend 不进入真实同步。既有 `apple-keychain-v1` 与已测 Android Keystore 环境也仍未证明不可导出 `ed25519-v1` signing key；`test-memory-v1` 只允许测试且无 fallback。
- `apple-secure-enclave-p256-v1` 已完成 qualification lifecycle、ad-hoc denied 与真实设备锁屏 probe；锁屏签名返回 `locked/-25308` 并失败关闭，解锁 cleanup 已删除合成 key且确认 missing。unsupported 和产品资格仍待独立证据。
- 缺少发布级目标部署运行证据，以及真实产品的同步 cursor/orchestration、设备恢复、撤销和 key epoch 全流程。
- `ManagerBridge` 仍无真实同步、恢复码、设备加入、授权、撤销或轮换命令；现有 readiness 只证明关闭态。

开发者当前没有真实不支持 Secure Enclave 的目标环境，unsupported 与产品资格因此保持外部阻塞；不能在当前设备伪造，也不能据此开放资格字段。关闭态 orchestration 可继续补风险矩阵，但不降低生产 backend 或真实用户同步门禁。

## 当前停止线

- M3 退出前不开放真实用户同步，不上传非受控真实 P2 数据，不提供恢复码、设备授权、撤销或轮换的产品成功入口。
- 不把 `test-memory-v1`、普通文件、SQLite、settings、generic password item 或可导出 seed 静默伪装成生产非导出 backend。
- P0 永不学习/同步；P1 原始事件只留本地，不进入 payload、manager、诊断、日志或提交记录。
- 输入热路径继续完全本地；Go server 不解密、不排序、不保存明文用户词或候选偏好。
- M2 已通过第二平台选择门禁，但当前仍集中完成 M3 macOS 同步 Beta，不同时展开第二真实平台主线。
- M4 前不宣称普通用户安装包、最终 librime/schema 分发、App Group 迁移、公证或发布供应链已经完成。

## 下一步顺位

1. 将 unsupported 保留为外部环境阻塞：目标环境可得时按独立授权执行 probe，确认明确 unsupported、无残留且不回退普通 DPK/test memory；通过后再逐字段评审 `product_qualified/user_presence_required/backup_migratable`。当前设备不能替代该证据。
2. 收完关闭态 orchestration 剩余高风险矩阵：补旧 key epoch、revoked device、解码/事务失败 cursor 不前移、迁移与 apply/outbox crash point；将现有 manual stale conflict、service 状态机 conflict 与双 userdb HTTP 收敛证据组合为可重复门禁。任何失败不得丢失 dirty/outbox 或推进未应用 cursor。
3. 将当前测试 processor 收敛为产品可复用但默认关闭的 Rust crypto processor 边界，明确设备公钥/profile、sync master/object key、epoch 与撤销状态的提供者；生产 backend 资格通过前不得接 `ManagerBridge`、不得把 test key material 做成运行时 fallback。
4. 只有 Rust service 与生产 backend 两条门禁均通过后，才接窄 `ManagerBridge` command/status；最后完成两个真实客户端、恢复/设备授权/撤销/key epoch 与发布级目标部署证据，满足后才评估开放用户同步。

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
