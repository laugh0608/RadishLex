# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、停止线和下一步的维护者与协作者。本文不记录完整历史流水、字段参考或操作步骤；详细事实进入稳定边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-19（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M3 端到端加密同步 Beta
- 当前产品主批次：macOS backend 外部资格阻塞与生产 sync provider 可信装载
- 已完成：M0 工程基础、M1 macOS 离线输入 Alpha、M2 本地个人化 MVP；R00、R01A、R02L、R01B、R06A 已退出
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；合成数据、短生命周期服务与受控集成测试可以继续

M1 已完成真实 macOS 离线输入；副屏与 VoiceOver 候选操作仍不受支持。M2 manager 已通过共享 userdb、migration、隐私、导入审计、删除恢复、并发和重启验收，并于 2026-07-18 回滚到零基线。

## M3 当前边界

M3 已具备 P2 envelope、signed manifest、Go server 设备/对象/密文/备份部署边界，以及关闭态 `sync_once`。Go/Rust 使用 domain 内 `change_sequence` 与 opaque cursor；userdb schema v5 持久化 cursor、remote observation、local revision、cycle journal 和完整密文 outbox，并原子提交 payload、observation、revision 与 cursor。`409 stale_base_version` 必须重新发现、验签解密、合并和签名。

风险矩阵已覆盖取消、retry exhaustion、revision race、lease recovery、签名/密文/AAD 篡改、epoch/revocation、decode/SQLite 失败、outbox prepare/ack 故障和残缺 migration 回滚。两个隔离 userdb 通过短生命周期 Go HTTP 服务完成上传、发现、合并回传、service 重建与第二轮零上传收敛；失败不推进 cursor、不清除 dirty，也不丢失可重放请求。

第四实现批已落地默认关闭的 `DefaultSyncObjectProcessor` 与 `SyncCryptoProvider`：preflight 冻结 cycle 级 signing handle/public key、当前写 epoch、历史可读 epoch、远端 signer profile、撤销 change sequence 和 key material；默认 provider 在网络前返回 `backend_unavailable`，产品构造器执行 production gate，合成 backend 只能走显式测试构造器。通用 processor 已替换双 userdb HTTP fixture 中的专用实现，并证明同周期 snapshot 不漂移、历史 epoch 在退役前可读、撤销阈值后的对象被拒绝、下一周期采用轮换 epoch 生成新 outbox。

签名层已支持显式 Ed25519/P-256 profile 与 Rust/Go verifier；缺少或混用算法时失败关闭。普通 DPK P-256 产品进程生命周期可用但私钥可导出，production gate 明确拒绝；Secure Enclave 已证明 lifecycle、不可导出、hardware-backed、denied 与真实锁屏失败关闭，但 unsupported 和最终产品资格仍缺外部环境证据。

这些测试仍不是用户可用同步。当前生产阻塞项是：

- 普通 DPK P-256 可导出，Apple/Android Ed25519 也未满足生产私钥条件；`test-memory-v1` 只允许测试且无 fallback。
- Secure Enclave 已完成 lifecycle、denied 与真实设备锁屏证据并清理合成 key；unsupported 和产品资格仍待独立环境。
- 缺少发布级目标部署运行证据，以及真实产品 provider 的设备生命周期/epoch material 装载、设备恢复、撤销和 key epoch 全流程。
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
2. 设计并实现默认关闭的生产 `SyncCryptoProvider` 装载层：从 Rust 可信设备生命周期与本地 epoch material store 形成 cycle snapshot，只把平台 backend handle 交给签名端口；缺少 active device、签名 profile、接受 epoch、当前写 epoch 或 production-qualified backend 时必须在网络前阻断。
3. 用合成设备授权、撤销和 epoch 轮换记录接入 provider 门禁，覆盖撤销前历史对象、撤销后新 sequence、旧设备无法取得新 epoch、重启后 snapshot 重建和无测试 backend fallback；真实 backend 资格通过前不得接 `ManagerBridge`。
4. 只有 Rust service、生产 provider 与 backend 三条门禁均通过后，才接窄 `ManagerBridge` command/status；最后完成两个真实客户端、恢复/设备授权/撤销/key epoch 与发布级目标部署证据，满足后才评估开放用户同步。

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
