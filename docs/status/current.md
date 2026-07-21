# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、停止线和下一步的维护者与协作者。本文不记录完整历史流水、字段参考或操作步骤；详细事实进入稳定边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-21（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M4 产品发布候选
- 当前产品主批次：M4-P02 Application Support v1 数据升级协调器；M4-P01 双 bundle 产品装配已完成
- 已完成：M0 工程基础、M1 macOS 离线输入 Alpha、M2 本地个人化 MVP、M3 端到端加密同步 Beta；R00、R01A、R02L、R01B、R06A 已退出
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；合成数据、短生命周期服务与受控集成测试可以继续

M4-P02 已完成升级协调器边界、`ime-userdb` 只读 inspection/隔离 migration contract，以及 `ime-product-upgrade` 单向状态机、receipt v1、固定数据根身份校验、原子持久化与跨进程 socket guard。当前实现会拒绝未知状态对象、symlink/hardlink、身份漂移、非法状态替换和中断临时文件，并且只在精确失活 socket 上恢复 guard；SQLite 一致快照、空间预算、双端产品 validation host、原子切换与完整崩溃恢复尚未实现，因此 M4-P02 未退出。

M1 已完成真实 macOS 离线输入；副屏与 VoiceOver 候选操作仍不受支持。M2 manager 已通过共享 userdb、migration、隐私、导入审计、删除恢复、并发和重启验收，并于 2026-07-18 回滚到零基线。

## M3 退出结论

M3 已具备 P2 envelope、signed manifest、Go 密文服务与关闭态 `sync_once`。对象 `change_sequence` 与设备 `lifecycle_sequence` 隔离；userdb schema v9 原子持久化 cursor、journal/outbox、可信公开 lifecycle 和 wrapped ciphertext cache，不保存明文 master key。失败不推进 cursor、不清除 dirty；`409 stale_base_version` 必须重新发现、验签解密、合并并签名。

产品 crypto 装载组合可信 lifecycle、独立 epoch material store 与平台 signing backend。preflight 在读取 secret 前验证本机 active、backend 资格和 public key。独立 P-256 key-agreement backend 解封 wrapped epoch；错误身份/AAD/密文、locked/unavailable、异常 epoch 和 revoked 本机均失败关闭。secret 只短暂存在于 Rust snapshot。

Go metadata schema v9 已落地 signed lifecycle、`profile-sha256-v1` 公钥绑定、signed epoch distribution、recovery v2、recovered-device activation 与 recovery record revocation。Rust 从本地信任锚拒绝乱序、缺口、epoch 跳变、公钥替换、inactive signer、重复恢复和 cursor 分叉。

A/B/C Go HTTP 已证明 B 撤销后仅 A/C 取得 epoch 2，历史/当前 epoch 可验签解封并在重启后恢复。恢复链覆盖 Argon2id possession proof、全新设备 profile、完整 active cohort 分发、单次使用、原子轮换/撤销、activation/revocation 线性化与日志脱敏。

部署 hardening 与本地 HTTPS 子阶段已通过：Compose/Caddy internal TLS、bearer 负向响应、loopback-only、非 root/只读/cap drop、`0700/0600`、symlink 拒绝、冷备份/隔离恢复、日志脱敏和资源清理均有真实门禁。Rust `HttpSyncRemoteTransport` 现支持 rustls HTTPS、Mozilla root 与进程内附加本地 CA，严格校验证书链和主机名；真实门禁已用临时 Caddy CA 完成无 token `401`、有 token `404` 和退出资源清零，不提供 insecure bypass。

Apple signing/key-agreement adapters 已接线。独立 key-agreement ABI、六场景调度与脱敏摘要已落地。ad-hoc denied 返回 missing-entitlement `-34018`；Team/profile 资格 bundle 下 lifecycle 完成 fresh public key、ECDH、wrapped epoch 往返、删除与 missing，设备锁定态返回 `PrivateKeyLocked/-25308`，解锁 cleanup 零残留。普通 DPK 可导出而被拒；两条 Secure Enclave backend 已按一个受支持 macOS 设备的真实主路径评审为 product qualified，unsupported 保留为延期兼容性补测。

Manager ABI v7 保留只读 `radishlex_manager_sync_product_status`，并新增隔离的本地合成资格 start/poll/cancel/free。`ime-sync-runtime` 复用现有 transport/orchestration/userdb/crypto，以唯一合成 domain/device/P2 在临时双客户端中验证冲突恢复与两轮收敛；请求只接受 loopback HTTPS、一次性 token、可选 CA DER 和受限 timeout，不触碰真实 userdb 或平台 key item。Dart 对 enum/flag/成功条件失败关闭，UI 与“启用同步”分区；token/CA 在各层清除，结果不进入 settings/readiness/diagnostics。产品摘要 blocker 仍固定为 `user_sync_closed_current_phase`，用户同步 gate 始终 blocked。

真实 Caddy 门禁已完成 TLS、bearer 负向/授权响应、建域、双设备授权、v1/v2 上传、`conflict_stale_base_version`、A 重发现与 v3 上传、B 合并 v4、第二轮双方零上传、日志脱敏与 Compose 资源清零。跨进程 socket guard 保证同时仅一条资格 run；新进程只清理同用户、精确命名且 marker 匹配的旧工作区，覆盖异常退出后的重启清理，不扫描其他 temp 内容。M3 路线图退出项已闭环，但这些仍是合成/本地资格证据，不开放真实用户同步。

平台 backend 外部资格已不再阻塞当前开发。开发者没有真实 unsupported 环境，现有支持设备不得模拟该证据；有合适目标时再按保留 harness 补测。锁屏链结束后的受限环境 trust 假象已由真实登录会话复核排除，同一冻结 hash bundle 严格验签通过。正式域名、公开证书和目标生产演练按产品决策后移到首版发布后；当前 `product_qualified=true`、`user_sync_enabled=false`。

## M4-P01 退出结论

macOS 产品元数据已统一为 `0.1.0 (35)`、macOS 13.0、FFI ABI v7、userdb v9 和 RimeData manifest v2。`packaging/rime/product-rime-data.json` 固定 `radishlex_pinyin`、Apache-2.0 `pinyin_simp` 词典 commit/hash、`SourceManifest.json` 和逐资产 LICENSE/AUTHORS；首个候选不携带 LGPL `prelude`、`stroke`、笔画反查或扩展符号表。

稳定入口已从 committed RimeData 输入离线装配 Manager/InputMethod 双 bundle、`librime` 传递闭包和 `ProductManifest.json`。真实 `librime 1.17.0` CLI、native FFI smoke、递归 dylib、ad-hoc 签名、RimeData/native manifests 与无构建机绝对路径复验通过；装配目录仍不是普通用户安装包，也没有 Developer ID、公证或 Gatekeeper 发布证据。

## 当前停止线

- 首个正式版本继续关闭真实用户同步，不上传非受控真实 P2 数据，不提供恢复码、设备授权、撤销或轮换的产品成功入口。
- 不把 `test-memory-v1`、普通文件、SQLite、settings、generic password item 或可导出 seed 静默伪装成生产非导出 backend。
- P0 永不学习/同步；P1 原始事件只留本地，不进入 payload、manager、诊断、日志或提交记录。
- 输入热路径继续完全本地；Go server 不解密、不排序、不保存明文用户词或候选偏好。
- M2 已通过第二平台选择门禁，但当前集中完成 M4 macOS 产品发布候选，不同时展开第二真实平台主线。
- M4 退出前不宣称普通用户安装包、Developer ID、公证或发布供应链已经完成；首个候选保持已验证的 Application Support v1，不为形式统一迁入 App Group。

## 下一步顺位

1. M4-P02 下一切面实现 SQLite backup API 一致快照、空间预算与文件系统故障注入，保持原库只读且候选完全隔离；随后实现双端 validation host、原子切换与崩溃恢复。当前没有 App Group 迁移需求，不得静默改变容器。
2. M4-P03 选择并验证安装载体，闭合固定程序路径、Developer ID/Hardened Runtime、notarization、升级回滚和默认保留用户数据的移除语义；真实系统动作另行授权。
3. 普通用户同步、恢复/授权/撤销/轮换继续关闭；在真实不支持 Secure Enclave 的环境可得时再补 unsupported，首版发布后且准备生产同步前再验收正式域名/证书。

## 验证入口

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
./scripts/check-manager-product.sh
./scripts/check-macos-product-metadata.sh
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
- [Manager 同步入口](../manager-sync-entry-boundary.md)：产品关闭态、UI/bridge 与 transient secret 边界。
- [M2 manager 验收 runbook](../runbooks/macos-m2-manager-product-acceptance.md)：关闭证据与回滚流程。
- [macOS 平台边界](../macos-inputmethodkit-boundary.md)：M1/M2 输入与隐私稳定结论。
- [macOS 产品包边界](../macos-product-package-boundary.md)：M4 组件、版本、数据、签名与装配停止线。
- [macOS 数据升级协调器](../macos-data-upgrade-coordinator.md)：M4-P02 状态机、receipt、SQLite 快照、双端验证与回滚边界。
- [本周周志](../devlogs/2026-W30.md)：当前资格执行批次的验证和交接流水。
