# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前里程碑、停止线和下一步的维护者与协作者。本文不记录完整历史流水、字段参考或操作步骤；详细事实进入稳定边界、runbook 和 devlog。

## 当前判断

- 复核日期：2026-07-18（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前产品里程碑：M3 端到端加密同步 Beta
- 当前产品主批次：设备签名算法 profile 与 macOS 生产私钥 backend
- 已完成：M0 工程基础、M1 macOS 离线输入 Alpha、M2 本地个人化 MVP；R00、R01A、R02L、R01B、R06A 已退出
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；合成数据、短生命周期服务与受控集成测试可以继续

M1 已完成真实 macOS 离线输入；副屏与 VoiceOver 候选操作仍不受支持。M2 本地学习与 manager 验收、回滚于 2026-07-18 完成。

## M2 关闭证据

manager 产品态已证明 ABI v5、共享 userdb、migration、导入审计、删除恢复、rank explain、双端刷新、并发与重启一致，且失败不回退 fixture。隐私模式零学习；secure field 只证明系统路由旁路。

最终回滚已恢复零基线，详见 manager 验收 runbook。

## M3 当前边界

M3 已具备 P2 envelope、signed manifest、两客户端授权/上传/下载/解密/合并/conflict 测试，以及 Go server 设备、join authorization、对象版本、密文存储、备份恢复、外部 TLS 和升级回滚受控证据。

ADR 0006 已接受 `ecdsa-p256-sha256-v1` 作为与 `ed25519-v1` 共存的生产候选 profile，固定 65-byte SEC1 uncompressed public key、64-byte P1363 signature 和既有 canonical bytes。Rust/Go verifier 已按算法分派并读取同一跨语言 fixture，覆盖两个 profile 正向签名、未知/不一致算法、错误公钥、篡改 canonical bytes、非法编码、签名不匹配和 revoked key。Go 设备与 join request metadata 现显式保存 `signing_algorithm`；历史 schema migration 只把旧行标记为 `ed25519-v1`，新请求缺少算法时失败关闭。

独立 `apple-keychain-p256-v1` 已沿 Apple Security framework、`ime-ffi` 与 manager Release native library 接通。实现只使用 DPK key class 支持的永久、`WhenUnlockedThisDeviceOnly`、tag 和 label 属性，创建、查询、删除均显式选择 DPK，禁止 legacy fallback；DER 只在 native 内转为 P1363，private key、canonical bytes 和 signature bytes 不进入 Dart。经 provisioning-backed Apple Development 产品包实测，创建、重载、签名、Rust/Go 验签、删除、missing 与 cleanup 全部通过，故 `compiled/runtime_available/can_create/can_sign=true`。

评审同时确认普通软件 DPK P-256 私钥可由平台 API 导出，不能继续声明 `exportable=false`。当前 capability 已改为 `exportable=true`，`ensure_production_signing_allowed()` 明确拒绝；`product_qualified/user_sync_enabled=false`。这不是 Secure Enclave、hardware-backed、user presence 或 backup migration 证据。

这些测试仍不是用户可用同步。当前生产阻塞项是：

- `apple-keychain-p256-v1` 的 DPK 软件运行时可用，但私钥可导出，未满足生产 backend 的不可导出条件；locked 矩阵不能改变该资格结论，故本 backend 不进入真实同步。既有 `apple-keychain-v1` 与已测 Android Keystore 环境也仍未证明不可导出 `ed25519-v1` signing key；`test-memory-v1` 只允许测试且无 fallback。
- `apple-secure-enclave-p256-v1` 已由 qualification 产品进程完成创建、重载、签名、Rust/Go 验签、不可导出、删除、missing 与 cleanup，当前报告 runtime/create/sign/hardware-backed；denied/locked/unsupported 和产品资格仍待独立证据。
- 缺少发布级目标部署运行证据，以及真实产品的同步 cursor/orchestration、设备恢复、撤销和 key epoch 全流程。
- `ManagerBridge` 仍无真实同步、恢复码、设备加入、授权、撤销或轮换命令；现有 readiness 只证明关闭态。

M3 当前主批已完成算法协议、跨语言 verifier/vector、普通 DPK 产品评审，以及 Secure Enclave qualification lifecycle。下一证据是逐项授权的 denied/locked/unsupported 失败矩阵；不得提前推进真实同步 orchestration。

## 当前停止线

- M3 退出前不开放真实用户同步，不上传非受控真实 P2 数据，不提供恢复码、设备授权、撤销或轮换的产品成功入口。
- 不把 `test-memory-v1`、普通文件、SQLite、settings、generic password item 或可导出 seed 静默伪装成生产非导出 backend。
- P0 永不学习/同步；P1 原始事件只留本地，不进入 payload、manager、诊断、日志或提交记录。
- 输入热路径继续完全本地；Go server 不解密、不排序、不保存明文用户词或候选偏好。
- M2 已通过第二平台选择门禁，但当前仍集中完成 M3 macOS 同步 Beta，不同时展开第二真实平台主线。
- M4 前不宣称普通用户安装包、最终 librime/schema 分发、App Group 迁移、公证或发布供应链已经完成。

## 下一步顺位

1. 使用已重新冻结的 qualification bundle，逐项授权补 Secure Enclave denied、locked 三段与 unsupported 失败矩阵；脚本不修改系统锁定状态，unsupported 需要不支持该能力的目标环境。
2. 逐字段评审 `product_qualified/user_presence_required/backup_migratable`；只开放产品证据支持的字段，用户同步总 gate 继续关闭。
3. 生产 backend 通过后才建立真实产品 sync orchestration；用户同步总 gate 不随 backend 资格自动开放。orchestration 需覆盖对象发现、hash/签名复验、解密、确定合并、本地 transaction、cursor、上传和 conflict retry，再接 `ManagerBridge`。
4. 最后完成两个真实客户端、恢复/设备授权/撤销/key epoch 与发布级目标部署证据，满足后才评估开放用户同步。

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
- [同步密钥管理](../sync-key-management.md)：设备、恢复、撤销和 key epoch。
- [ADR 0006](../adr/0006-device-signature-algorithm-profiles.md)：Ed25519/P-256 profile、编码、迁移、错误与 Apple backend 边界。
- [平台私钥 Backend 策略](../platform-private-key-backend-strategy.md)：当前证据与算法/backend 决策顺序。
- [Manager 同步入口](../manager-sync-entry-boundary.md)：M3 UI/bridge 与 transient secret 边界。
- [M2 manager 验收 runbook](../runbooks/macos-m2-manager-product-acceptance.md)：关闭证据与回滚流程。
- [macOS 平台边界](../macos-inputmethodkit-boundary.md)：M1/M2 输入与隐私稳定结论。
- [本周周志](../devlogs/2026-W29.md)：完整验证和交接流水。
