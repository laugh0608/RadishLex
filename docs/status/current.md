# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、停止线和下一步的维护者与协作者。本文不记录完整历史流水、字段参考或操作步骤；详细事实进入稳定边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-19（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M3 端到端加密同步 Beta
- 当前产品主批次：macOS signing/key-agreement 产品资格已按单平台主路径闭合；下一批转入本地 HTTPS 下的 Manager 受控资格链
- 已完成：M0 工程基础、M1 macOS 离线输入 Alpha、M2 本地个人化 MVP；R00、R01A、R02L、R01B、R06A 已退出
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；合成数据、短生命周期服务与受控集成测试可以继续

M1 已完成真实 macOS 离线输入；副屏与 VoiceOver 候选操作仍不受支持。M2 manager 已通过共享 userdb、migration、隐私、导入审计、删除恢复、并发和重启验收，并于 2026-07-18 回滚到零基线。

## M3 当前边界

M3 已具备 P2 envelope、signed manifest、Go 密文服务与关闭态 `sync_once`。对象 `change_sequence` 与设备 `lifecycle_sequence` 隔离；userdb schema v9 原子持久化 cursor、journal/outbox、可信公开 lifecycle 和 wrapped ciphertext cache，不保存明文 master key。失败不推进 cursor、不清除 dirty；`409 stale_base_version` 必须重新发现、验签解密、合并并签名。

产品 crypto 装载组合可信 lifecycle、独立 epoch material store 与平台 signing backend。preflight 在读取 secret 前验证本机 active、backend 资格和 public key。独立 P-256 key-agreement backend 解封 wrapped epoch；错误身份/AAD/密文、locked/unavailable、异常 epoch 和 revoked 本机均失败关闭。secret 只短暂存在于 Rust snapshot。

Go metadata schema v9 已落地 signed lifecycle、`profile-sha256-v1` 公钥绑定、signed epoch distribution、recovery v2、recovered-device activation 与 recovery record revocation。Rust 从本地信任锚拒绝乱序、缺口、epoch 跳变、公钥替换、inactive signer、重复恢复和 cursor 分叉。

A/B/C Go HTTP 已证明 B 撤销后仅 A/C 取得 epoch 2，历史/当前 epoch 可验签解封并在重启后恢复。恢复链覆盖 Argon2id possession proof、全新设备 profile、完整 active cohort 分发、单次使用、原子轮换/撤销、activation/revocation 线性化与日志脱敏。

部署 hardening 与本地 HTTPS 子阶段已通过：Compose/Caddy internal TLS、bearer 负向响应、loopback-only、非 root/只读/cap drop、`0700/0600`、symlink 拒绝、冷备份/隔离恢复、日志脱敏和资源清理均有真实门禁。Rust `HttpSyncRemoteTransport` 现支持 rustls HTTPS、Mozilla root 与进程内附加本地 CA，严格校验证书链和主机名；真实门禁已用临时 Caddy CA 完成无 token `401`、有 token `404` 和退出资源清零，不提供 insecure bypass。

Apple signing/key-agreement adapters 已接线。独立 key-agreement ABI、六场景调度与脱敏摘要已落地。ad-hoc denied 返回 missing-entitlement `-34018`；Team/profile 资格 bundle 下 lifecycle 完成 fresh public key、ECDH、wrapped epoch 往返、删除与 missing，设备锁定态返回 `PrivateKeyLocked/-25308`，解锁 cleanup 零残留。普通 DPK 可导出而被拒；两条 Secure Enclave backend 已按一个受支持 macOS 设备的真实主路径评审为 product qualified，unsupported 保留为延期兼容性补测。

Manager ABI v6 的 `radishlex_manager_sync_product_status` 只读固定数值 capability，不访问系统 key item。默认构建显示 signing 未编译，macOS 产品构建显示 signing/key-agreement 与组合 backend 已获产品资格，但 blocker 固定为 `user_sync_closed_current_phase`；畸形或自称开启同步的 native 状态降级为 `native_sync_product_status_invalid`。gate 始终 blocked，底层 Apple validation 不直接进入 Dart，也没有新增同步命令。

平台 backend 外部资格已不再阻塞当前开发。开发者没有真实 unsupported 环境，现有支持设备不得模拟该证据；有合适目标时再按保留 harness 补测。锁屏链结束后的受限环境 trust 假象已由真实登录会话复核排除，同一冻结 hash bundle 严格验签通过。正式域名、公开证书和目标生产演练按产品决策后移到首版发布后；当前 `product_qualified=true`、`user_sync_enabled=false`。

## 当前停止线

- M3 退出前不开放真实用户同步，不上传非受控真实 P2 数据，不提供恢复码、设备授权、撤销或轮换的产品成功入口。
- 不把 `test-memory-v1`、普通文件、SQLite、settings、generic password item 或可导出 seed 静默伪装成生产非导出 backend。
- P0 永不学习/同步；P1 原始事件只留本地，不进入 payload、manager、诊断、日志或提交记录。
- 输入热路径继续完全本地；Go server 不解密、不排序、不保存明文用户词或候选偏好。
- M2 已通过第二平台选择门禁，但当前仍集中完成 M3 macOS 同步 Beta，不同时展开第二真实平台主线。
- M4 前不宣称普通用户安装包、最终 librime/schema 分发、App Group 迁移、公证或发布供应链已经完成。

## 下一步顺位

1. 下一主批是 Manager 本地 HTTPS 同步资格执行：先在 `docs/manager-sync-entry-boundary.md` 固定的边界上完成 Rust-owned run handle、单次运行、start/poll/cancel/free、timeout、transient token/CA 和固定脱敏结果 contract。
2. 资格 runner 必须复用现有 Rust orchestration/crypto/remote/userdb，内部生成隔离合成双客户端和 P2 数据，经真实 Caddy HTTPS 完成发现、验签解密、合并、上传、冲突处理和第二轮收敛；不得接受调用方 payload/key/path，也不得触发系统 key item。
3. 在 Rust/FFI 稳定后接入真实 Dart bridge 与明确标识的 Manager 本地资格交互，覆盖取消、并发拒绝、重启、临时资源清理、错误脱敏和 Release bundle；普通用户成功入口与 `user_sync_enabled` 继续关闭。
4. 在真实不支持 Secure Enclave 的环境可得时补测 signing/key-agreement unsupported；当前设备不得模拟，补测不阻塞上述开发。首个正式版本发布后、准备启用真实生产同步前，再补正式域名/证书和目标 `deployment_evidence.v1`。

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
- [Secure Enclave Key Agreement Runbook](../runbooks/apple-secure-enclave-key-agreement-backend.md)：独立六场景资格、错误与清理边界。
- [ADR 0006](../adr/0006-device-signature-algorithm-profiles.md)：Ed25519/P-256 profile、编码、迁移、错误与 Apple backend 边界。
- [平台私钥 Backend 策略](../platform-private-key-backend-strategy.md)：当前证据与算法/backend 决策顺序。
- [Manager 同步入口](../manager-sync-entry-boundary.md)：M3 UI/bridge 与 transient secret 边界。
- [M2 manager 验收 runbook](../runbooks/macos-m2-manager-product-acceptance.md)：关闭证据与回滚流程。
- [macOS 平台边界](../macos-inputmethodkit-boundary.md)：M1/M2 输入与隐私稳定结论。
- [本周周志](../devlogs/2026-W29.md)：完整验证和交接流水。
