# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、停止线和下一步的维护者与协作者。本文不记录完整历史流水、字段参考或操作步骤；详细事实进入稳定边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-25（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M4 产品发布候选
- 当前产品主批次：M4-P03 安装载体与发布供应链；M4-P01 双 bundle 产品装配、M4-P02 Application Support v1 数据升级协调器已完成
- 已完成：M0 工程基础、M1 macOS 离线输入 Alpha、M2 本地个人化 MVP、M3 端到端加密同步 Beta；R00、R01A、R02L、R01B、R06A、M4-P01、M4-P02 已退出
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；合成数据、短生命周期服务与受控集成测试可以继续

M4-P02 已闭合只读 inspection、SQLite 一致快照、隔离 migration candidate、settings 副本、receipt/guard、原子切换、最终双端复验、精确 inode 回滚和逐 checkpoint 静止证明。manifest-bound macOS adapter 固定 target preflight、target 双端 candidate/final validation 与 source 双端 rollback validation，并在每次执行前复验 helper 身份、长度和 SHA-256。

隔离产品协调资格已用真实 Manager/InputMethod 装配和 helper 在合成 Application Support 上覆盖完整成功、Manager/InputMethod 端点失败、静止丢失后续跑、切换后回滚、source evidence 暂不可得后的重启恢复和临时根清理。InputMethod 只把锁定 YAML 部署到短生命周期验证 user data；产品 RimeData、candidate 和 Application Support 保持只读。首发 source qualification 具有独立版本、签名和 manifest，但使用当前 native code/schema；它证明 source/target 路由与恢复编排，不冒充尚不存在的历史发布二进制。未来形成实际上一版发布包后，真实跨发布 source bundle 复验必须成为后续升级门禁。

M4-P03 已接受 DMG + 独立用户域 Installer app：Manager、InputMethod 和 install state 固定在 current-user home 下，不请求管理员权限，也不让 `.pkg` script 承担明文数据协调。committed `install-layout.json` 与确定性 `InstallPayloadManifest.json` 已绑定产品 manifest、版本/build、Installer bundle ID、两个安装目标和默认保留数据语义；7 项 contract 覆盖确定性装配、额外文件、内容变更、layout 替换、bundle symlink 和覆盖拒绝。该 payload 仍不是 Installer/DMG，不证明签名、公证或真实安装。

独立 `ime-product-install` 已固定首次安装、升级、修复和默认程序移除的 source/target 关系，以及 `prepared` 到三类终态的外层状态机。严格 receipt 绑定 data-root identity、ProductManifest/bundle tree/code identity hash、source/staged/backup/installed 文件系统身份和 previous-operation chain；原子存储与 Unix socket guard 拒绝中断写、未知对象、身份漂移和并发 operation。两个 component 各自在目标父目录使用 `0700` 私有事务目录，核心按精确 inode 执行 source preserve、Manager/InputMethod 逐端 rename/fsync 和程序 rollback。27 项合成测试覆盖四类终态、首次安装与移除恢复、升级部分提交恢复，以及 preserve/commit/rollback 每个 rename、目标目录 fsync、源目录 fsync 边界；不触碰真实 bundle、Application Support 或用户域安装目标。

独立 `radishlex-macos-product-install` adapter 已内嵌 committed install layout，严格复验 InstallPayloadManifest、ProductManifest、许可证和完整双 bundle tree，并只从 authoritative current-user home 形成固定目标。production code identity 要求 exact Developer ID designated requirement 与 Team ID，经 strict `codesign` 后只把固定字段 hash 交给核心；没有 ad-hoc fallback。staging 使用 metadata-preserving `ditto`，复制前后复验并递归 fsync；完整但未记录的 staged bundle 可恢复 evidence，部分/漂移对象保持现场。9 项合成测试覆盖首次安装、完整升级回滚、source/target/restored 复验、manifest/tree/signature/path 漂移和 staging 中断；没有读取真实用户目录或签名身份。

独立 `radishlex-macos-product-install-coordinator` 已在不合并两个核心的前提下绑定同一 operation ID、Application Support inode、source/target release、双 receipt 与双 guard。M4-P02 每个 quiescence checkpoint 同时复验 installed target 双 bundle；数据 `completed` 映射外层 `data_settled`，数据 `aborted_preserved` / `rolled_back` 则在 source 双程序精确恢复和逻辑身份复验后映射外层 `rolled_back`。12 项合成测试覆盖成功、两段终态、candidate/post-switch 失败、静止丢失、target/source 身份暂不可得、重启续跑、未持久化双 receipt 状态及 operation/release/root 绑定拒绝。

外层终态现由四类 operation 共用的两段动作推进：每次先复验当前 receipt/guard、双 `ProgramSwitchStore` 与最终程序结果，再分别持久化 `final_verified`、`completed`；upgrade 组合层还在两段前精确复验 data receipt/guard、Application Support identity、source/target release、data `completed` 和 installed 双 bundle。第二段中断会保留 `final_verified` 并在重启后重新取证续跑。ABI v9 新增独立 `radishlex_product_install_startup_gate`；Manager/InputMethod 均先执行外层 gate，再执行既有数据 gate，之后才允许 Flutter/IMK、settings、userdb 或 Rime 初始化。运行身份由当前 executable 的固定用户域 bundle、Info.plist、完整 tree 与 Developer ID code identity 形成，不接受 UI/settings/`HOME` 或调用方 identity 字段。

M1 已完成真实 macOS 离线输入；副屏与 VoiceOver 候选操作仍不受支持。M2 manager 已通过共享 userdb、migration、隐私、导入审计、删除恢复、并发和重启验收，并于 2026-07-18 回滚到零基线。

## M3 退出结论

M3 已具备 P2 envelope、signed manifest、Go 密文服务与关闭态 `sync_once`。对象 `change_sequence` 与设备 `lifecycle_sequence` 隔离；userdb schema v9 原子持久化 cursor、journal/outbox、可信公开 lifecycle 和 wrapped ciphertext cache，不保存明文 master key。失败不推进 cursor、不清除 dirty；`409 stale_base_version` 必须重新发现、验签解密、合并并签名。

产品 crypto 装载组合可信 lifecycle、独立 epoch material store 与平台 signing backend。preflight 在读取 secret 前验证本机 active、backend 资格和 public key。独立 P-256 key-agreement backend 解封 wrapped epoch；错误身份/AAD/密文、locked/unavailable、异常 epoch 和 revoked 本机均失败关闭。secret 只短暂存在于 Rust snapshot。

Go metadata schema v9 已落地 signed lifecycle、`profile-sha256-v1` 公钥绑定、signed epoch distribution、recovery v2、recovered-device activation 与 recovery record revocation。Rust 从本地信任锚拒绝乱序、缺口、epoch 跳变、公钥替换、inactive signer、重复恢复和 cursor 分叉。

A/B/C Go HTTP 已证明 B 撤销后仅 A/C 取得 epoch 2，历史/当前 epoch 可验签解封并在重启后恢复。恢复链覆盖 Argon2id possession proof、全新设备 profile、完整 active cohort 分发、单次使用、原子轮换/撤销、activation/revocation 线性化与日志脱敏。

部署 hardening 与本地 HTTPS 子阶段已通过：Compose/Caddy internal TLS、bearer 负向响应、loopback-only、非 root/只读/cap drop、`0700/0600`、symlink 拒绝、冷备份/隔离恢复、日志脱敏和资源清理均有真实门禁。Rust `HttpSyncRemoteTransport` 现支持 rustls HTTPS、Mozilla root 与进程内附加本地 CA，严格校验证书链和主机名；真实门禁已用临时 Caddy CA 完成无 token `401`、有 token `404` 和退出资源清零，不提供 insecure bypass。

Apple signing/key-agreement adapters 已接线。独立 key-agreement ABI、六场景调度与脱敏摘要已落地。ad-hoc denied 返回 missing-entitlement `-34018`；Team/profile 资格 bundle 下 lifecycle 完成 fresh public key、ECDH、wrapped epoch 往返、删除与 missing，设备锁定态返回 `PrivateKeyLocked/-25308`，解锁 cleanup 零残留。普通 DPK 可导出而被拒；两条 Secure Enclave backend 已按一个受支持 macOS 设备的真实主路径评审为 product qualified，unsupported 保留为延期兼容性补测。

Manager ABI v9 保留只读 `radishlex_manager_sync_product_status`、隔离的本地合成资格 start/poll/cancel/free 和 ABI v8 数据 startup/validation contract，并增加独立外层 install startup gate。`ime-sync-runtime` 仍只在临时双客户端验证冲突恢复与两轮收敛，不触碰真实 userdb 或平台 key item；产品摘要 blocker 固定为 `user_sync_closed_current_phase`，用户同步 gate 始终 blocked。

真实 Caddy 门禁已完成 TLS、bearer 负向/授权响应、建域、双设备授权、v1/v2 上传、`conflict_stale_base_version`、A 重发现与 v3 上传、B 合并 v4、第二轮双方零上传、日志脱敏与 Compose 资源清零。跨进程 socket guard 保证同时仅一条资格 run；新进程只清理同用户、精确命名且 marker 匹配的旧工作区，覆盖异常退出后的重启清理，不扫描其他 temp 内容。M3 路线图退出项已闭环，但这些仍是合成/本地资格证据，不开放真实用户同步。

平台 backend 外部资格已不再阻塞当前开发。开发者没有真实 unsupported 环境，现有支持设备不得模拟该证据；有合适目标时再按保留 harness 补测。锁屏链结束后的受限环境 trust 假象已由真实登录会话复核排除，同一冻结 hash bundle 严格验签通过。正式域名、公开证书和目标生产演练按产品决策后移到首版发布后；当前 `product_qualified=true`、`user_sync_enabled=false`。

## M4-P01 退出结论

macOS 产品元数据已统一为 `0.1.0 (35)`、macOS 13.0、FFI ABI v9、userdb v9 和 RimeData manifest v2。`packaging/rime/product-rime-data.json` 固定 `radishlex_pinyin`、Apache-2.0 `pinyin_simp` 词典 commit/hash、`SourceManifest.json` 和逐资产 LICENSE/AUTHORS；首个候选不携带 LGPL `prelude`、`stroke`、笔画反查或扩展符号表。

稳定入口已从 committed RimeData 输入离线装配 Manager/InputMethod 双 bundle、`librime` 传递闭包和 `ProductManifest.json`。真实 `librime 1.17.0` CLI、native FFI smoke、递归 dylib、ad-hoc 签名、RimeData/native manifests 与无构建机绝对路径复验通过；装配目录仍不是普通用户安装包，也没有 Developer ID、公证或 Gatekeeper 发布证据。

## 当前停止线

- 首个正式版本继续关闭真实用户同步，不上传非受控真实 P2 数据，不提供恢复码、设备授权、撤销或轮换的产品成功入口。
- 不把 `test-memory-v1`、普通文件、SQLite、settings、generic password item 或可导出 seed 静默伪装成生产非导出 backend。
- P0 永不学习/同步；P1 原始事件只留本地，不进入 payload、manager、诊断、日志或提交记录。
- 输入热路径继续完全本地；Go server 不解密、不排序、不保存明文用户词或候选偏好。
- M2 已通过第二平台选择门禁，但当前集中完成 M4 macOS 产品发布候选，不同时展开第二真实平台主线。
- M4 退出前不宣称普通用户安装包、Developer ID、公证或发布供应链已经完成；首个候选保持已验证的 Application Support v1，不为形式统一迁入 App Group。

## 下一步顺位

1. M4-P03 下一切面建立隔离双 bundle + 合成 Application Support 的端到端故障恢复门禁，组合程序切换、数据协调、两段终态、双端启动阻断和重启续跑。
2. 完成成功、数据失败、程序恢复、双端启动阻断与重启续跑证据后，才进入 Installer UI、真实用户目录安装与发布供应链；这些动作继续需要独立授权。
3. 普通用户同步、恢复/授权/撤销/轮换继续关闭；在真实不支持 Secure Enclave 的环境可得时再补 unsupported，首版发布后且准备生产同步前再验收正式域名/证书。

## 验证入口

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
./scripts/check-manager-product.sh
./scripts/check-macos-product-metadata.sh
./scripts/check-macos-install-layout.sh
./scripts/check-product-install-core.sh
./scripts/check-macos-install-adapter.sh
./scripts/check-macos-install-coordinator.sh
./scripts/check-macos-imk.sh
./scripts/check-macos-upgrade-coordinator.sh
./scripts/check-macos-upgrade-product-coordination.sh
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
- [macOS 安装载体 ADR](../adr/0008-macos-installation-carrier.md)：M4-P03 用户域 Installer、固定目标、程序事务与移除边界。
- [macOS 程序安装事务](../macos-installation-transaction.md)：外层 operation、产品身份、receipt/guard、状态机与启动门禁。
- [macOS 数据升级协调器](../macos-data-upgrade-coordinator.md)：M4-P02 状态机、receipt、SQLite 快照、双端验证与回滚边界。
- [本周周志](../devlogs/2026-W30.md)：本周 M3 退出、M4-P01 装配与 M4-P02 升级协调器的验证和交接流水。
