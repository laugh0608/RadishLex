# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、停止线和下一步的维护者与协作者。本文不记录完整历史流水、字段参考或操作步骤；详细事实进入稳定边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-19（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M3 端到端加密同步 Beta
- 当前产品主批次：恢复设备 activation / lifecycle 归约与 macOS backend 外部资格阻塞
- 已完成：M0 工程基础、M1 macOS 离线输入 Alpha、M2 本地个人化 MVP；R00、R01A、R02L、R01B、R06A 已退出
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；合成数据、短生命周期服务与受控集成测试可以继续

M1 已完成真实 macOS 离线输入；副屏与 VoiceOver 候选操作仍不受支持。M2 manager 已通过共享 userdb、migration、隐私、导入审计、删除恢复、并发和重启验收，并于 2026-07-18 回滚到零基线。

## M3 当前边界

M3 已具备 P2 envelope、signed manifest、Go 密文服务与关闭态 `sync_once`。对象 `change_sequence` 与设备 `lifecycle_sequence` 隔离；userdb schema v7 原子持久化 cursor、journal/outbox、可信公开 lifecycle 和 wrapped ciphertext cache，不保存明文 master key。失败不推进 cursor、不清除 dirty；`409 stale_base_version` 必须重新发现、验签解密、合并并签名。

产品 crypto 装载组合可信 lifecycle、独立 epoch material store 与平台 signing backend。preflight 先确认本机 active、backend 产品资格和 public key 匹配，再读取 secret，并冻结当前/历史 epoch 与撤销阈值。`ProductWrappedEpochMaterialStore` 以独立 P-256 key-agreement backend 解封 `p256-ecdh-hkdf-sha256-xchacha20poly1305-v1`；错误 domain/device/key id、AAD/密文篡改、locked/unavailable backend、未来/重复 epoch 和 revoked 本机均失败关闭。secret 只短暂存在于 Rust material/cycle snapshot。

Go metadata schema v7 已落地 signed lifecycle、`profile-sha256-v1` 公钥绑定、结构化 recipient key-agreement key id、wrapping signature source 与 recovery v2 metadata。Rust 从本地初始信任锚验证授权/撤销链；乱序、缺口、epoch 跳变、公钥替换、inactive signer 和 cursor 分叉均拒绝。

独立 signed epoch distribution 已落地：active distributor 为当前 epoch 的完整 active cohort 分别生成签名 wrapped record，Go `POST .../epoch-distributions` 在同一事务验证 cohort、签名、recipient key 与 64 KiB/4 MiB 上限后原子提交 metadata；精确重放幂等，同 locator 分叉返回 `conflict_epoch_distribution`，任一坏签名或漏发均不产生可读取的部分 metadata。精确 GET 返回 signature source，Rust 按可信 distributor profile 复验后才能缓存与解封。

短生命周期真实 Go HTTP 证据使用 A/B/C 三个合成设备：B 被撤销后 active A/C 获得 epoch 2，坏签名与不完整 cohort 先失败且无部分可读记录，完整批次重放幂等、分叉冲突；A/C 下载、验签、缓存并在 userdb/provider 重启后恢复当前与历史 epoch，B 在 blob/material 读取前被拒绝。日志不含 wrapping key id、nonce、签名、密文或合成明文。

`recovery-record-v2` 已绑定 predecessor、当前 epoch、固定 Argon2id/envelope profile、裸密文 hash 与独立派生的 activation Ed25519 公钥。Go 原子创建/轮换 latest，旧记录进入 `superseded`，重放幂等，陈旧 predecessor 或同 id 分叉返回 `conflict_recovery_record`；schema v7 只保留 v1 迁移数据，产品客户端拒绝直接使用。Rust 按可信 active signer/current epoch 验签后才解封。真实 Go HTTP 已证明两次恢复码轮换、旧码拒绝新记录、metadata/ciphertext/AAD 篡改失败、备份重启和日志脱敏；恢复设备 activation 尚未进入 lifecycle。

`apple-keychain` feature 已提供 Apple signing adapters 与独立 `AppleSecureEnclaveP256KeyAgreementStore`，但本批未调用系统 API、未创建/读取/删除真实条目。普通 DPK P-256 可导出而被 production gate 拒绝；Secure Enclave signing 已有 lifecycle、不可导出、hardware-backed、denied 与真实锁屏证据，unsupported、最终产品资格和独立 key-agreement 实机资格仍缺外部证据。

当前生产阻塞项：恢复设备 activation/lifecycle、发布级目标部署运行证据、合格生产 signing/key-agreement backend，以及窄 `ManagerBridge` 产品链均未闭环。开发者当前没有真实 unsupported 环境，不得在当前设备伪造，也不得据此开启 `product_qualified` 或 `user_sync_enabled`。

## 当前停止线

- M3 退出前不开放真实用户同步，不上传非受控真实 P2 数据，不提供恢复码、设备授权、撤销或轮换的产品成功入口。
- 不把 `test-memory-v1`、普通文件、SQLite、settings、generic password item 或可导出 seed 静默伪装成生产非导出 backend。
- P0 永不学习/同步；P1 原始事件只留本地，不进入 payload、manager、诊断、日志或提交记录。
- 输入热路径继续完全本地；Go server 不解密、不排序、不保存明文用户词或候选偏好。
- M2 已通过第二平台选择门禁，但当前仍集中完成 M3 macOS 同步 Beta，不同时展开第二真实平台主线。
- M4 前不宣称普通用户安装包、最终 librime/schema 分发、App Group 迁移、公证或发布供应链已经完成。

## 下一步顺位

1. 用合成 backend 实现 `recovered_device_activation` possession proof、`recovery_record_rotated` / `device_recovered` lifecycle 事件和新 signing/key-agreement profile；成功事务随后必须为完整 active cohort 分发当前 epoch，并覆盖重复使用、乱序、撤销、重启与日志脱敏。
2. 补发布级目标部署运行证据；unsupported 继续保留为外部环境阻塞，目标环境可得时再按独立授权执行 probe，当前设备不得替代该证据。
3. 需要真实创建、读取或删除 Keychain/Secure Enclave 条目时单独申请授权；合成 backend 不记为实机资格。
4. lifecycle/material/recovery 与部署证据稳定后才接窄 `ManagerBridge` command/status；满足恢复、授权、撤销、轮换和 backend 门禁后再评估开放用户同步。

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
