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

M1 已用 Apple Development `build 32` 完成真实 TextEdit/Codex 离线输入、候选交互、生命周期、离线和零残留证据；副屏与 VoiceOver 候选操作仍不是已支持能力。R02L 与 R01B 随后关闭事务化学习、确定性排序、删除/显式恢复、真实重排、进程重启、隐私、unknown、P0 与 secure 系统路由。M2 manager 产品验收和最终回滚已完成，本地个人化 MVP 于 2026-07-18 正式关闭。

## M2 关闭证据

manager 正常产品运行态固定为无环境变量的 `product` mode，从 app bundle 加载 ABI v5 native library，并与 InputMethodKit 共享平台 Application Support userdb；native、ABI、路径、权限或 userdb 失败不会回退 fixture。词库页真实区分 active、suppressed、deleted，导入批次按持久化 `import_batch_id` 关联，恢复只能经独立 explicit restore。文件连接在 schema/integrity SQL 前安装 busy timeout，WAL 首次协商只对锁竞争做有界重试。

实机冻结于 clean HEAD `a7e385e`。Release `Info.plist`、主程序、bundle FFI SHA-256 分别为 `780d2ecbf8d4fd6c615eb7b5cd40e03227fde2280956d005d573bc0087451ce2`、`4ae289fe1df7c1653940ea6e869a626c057dc8ce6b8469a0825fc2279e913bf6`、`4785dd662a918188ad506c0ec1b543a67ef477bafbd479de81e407623a794f0f`；ad-hoc 严格验签通过。Authorization A 已证明真实 v3 -> v4 migration、固定合成词导入审计、delete 防普通导入复活、deleted/suppressed 独立恢复、重启持久化与 rank explain 一致。

Authorization B 使用同一 userdb 完成输入法与 manager 双端验收：输入侧提交后，manager 页头刷新无需重启即可观察聚合变化；manager delete 产生 tombstone 并清除 ranker，输入侧再次选择只增加本地 selection 聚合，不能复活词条；explicit restore 后输入侧新 session 重新学习；manager 与 IMK 精确重启后状态继续一致，未出现非预期 `SQLITE_BUSY`、重复 migration 或损坏降级。隐私模式真实读回为 true 时，普通 TextEdit 提交保持全库聚合零增量；恢复正常模式后，一次提交只产生一次预期 selection/frequency 增量。

secure field 显示 Secure Event Input 已启用。secure 聚焦期间顶部输入法菜单不能切换到 RadishLex 或系统拼音；快捷键可以退回系统拼音，但不能进入 RadishLex；解除聚焦后保持系统拼音。来源监视未记录 secure 期间 RadishLex source，学习聚合保持 `selection_events=7`、固定 `shi/时 frequency=4`。该项只证明“macOS secure 路由旁路，controller secure 分支未由本组实机执行”，不记为 `policy_blocked` 实机通过。

Authorization B/C 最终恢复了系统与数据基线。首次 C 删除在任何数据变化前因 v1 receipt 的 `st_dev` 跨登录漂移安全停止；共享 helper 现只在 receipt 与父目录的旧记录同设备、当前现场也同设备，且两者 inode、owner、mode、link、固定路径和白名单均精确匹配时接受成对 device drift，任一单侧漂移继续拒绝。contract 通过后重跑同一固定入口成功。最终证据为 TIS `matches=0 enabled=0 selected=0`，bundle/Rime/userdb/sidecars/settings/receipts 和 M2 `/private/tmp` 目录 absent，IMK/manager 进程 stopped，隐私键 absent，Application Support 父目录 empty/`0755`。全程未读取 P1 原始行或数据库正文。

## M3 当前边界

仓库已经具备 M3 的实现基础：P2 user term、ranker weight 与 tombstone 可在 Rust 内部组装为加密 envelope；remote client 只接收已加密对象与已签名 manifest；两客户端内存 harness 和短生命周期 Go HTTP 测试已覆盖授权、上传、下载、解密、合并写回、stale conflict 与 v2 重新上传。Go server 已有设备、join request、authorization、recovery record、对象版本、bearer token、SQLite metadata、local blob、备份恢复、外部 TLS 和升级回滚受控证据。

ADR 0006 已接受 `ecdsa-p256-sha256-v1` 作为与 `ed25519-v1` 共存的生产候选 profile，固定 65-byte SEC1 uncompressed public key、64-byte P1363 signature 和既有 canonical bytes。Rust/Go verifier 已按算法分派并读取同一跨语言 fixture，覆盖两个 profile 正向签名、未知/不一致算法、错误公钥、篡改 canonical bytes、非法编码、签名不匹配和 revoked key。Go 设备与 join request metadata 现显式保存 `signing_algorithm`；历史 schema migration 只把旧行标记为 `ed25519-v1`，新请求缺少算法时失败关闭。

独立 `apple-keychain-p256-v1` repository capability spike 已接入 Apple Security framework：创建永久 P-256 `SecKey` 时声明不可导出和仅本机解锁可用，public key 只导出公开 SEC1 点，Apple DER signature 在 backend 内严格转为 P1363；locked、denied、missing、corrupted 与 unsupported 映射为结构化错误。普通 feature 测试只验证编译、编码转换、错误映射与关闭态，没有访问 Keychain。gated smoke 已覆盖计划中的创建、跨 store 重载、签名、Rust/Go 验签、删除/撤销和 cleanup，但尚未获授权执行，因此当前没有真实 Keychain 成功证据。

这些测试仍不是用户可用同步。当前生产阻塞项是：

- `apple-keychain-p256-v1` 尚未执行真实 gated smoke，仍报告 `available=false`、`can_create_signing_keys=false`、`can_sign=false`；不能宣称 production-ready、Secure Enclave、hardware-backed、user presence 或 backup migration。既有 `apple-keychain-v1` 与已测 Android Keystore 环境也仍未证明不可导出 `ed25519-v1` signing key；`test-memory-v1` 禁止进入生产。
- 缺少发布级目标部署运行证据，以及真实产品的同步 cursor/orchestration、设备恢复、撤销和 key epoch 全流程。
- `ManagerBridge` 仍无真实同步、恢复码、设备加入、授权、撤销或轮换命令；现有 readiness 只证明关闭态。

M3 第一主批的协议与仓库实现阶段已经落地，当前立即停止线是 Apple P-256 实机证据。不得因 repository capability spike 存在就开放产品 gate，也不得在现有 backend 内保存或导出 Ed25519 seed。若真实平台 spike 失败，先记录固定错误分类和 cleanup，再评审独立的软件保护 backend；该路径必须使用新 backend id 和明确风险等级。

## 当前停止线

- M3 退出前不开放真实用户同步，不上传非受控真实 P2 数据，不提供恢复码、设备授权、撤销或轮换的产品成功入口。
- 不把 `test-memory-v1`、普通文件、SQLite、settings、generic password item 或可导出 seed 静默伪装成生产非导出 backend。
- P0 永不学习/同步；P1 原始事件只留本地，不进入 payload、manager、诊断、日志或提交记录。
- 输入热路径继续完全本地；Go server 不解密、不排序、不保存明文用户词或候选偏好。
- M2 已通过第二平台选择门禁，但当前仍集中完成 M3 macOS 同步 Beta，不同时展开第二真实平台主线。
- M4 前不宣称普通用户安装包、最终 librime/schema 分发、App Group 迁移、公证或发布供应链已经完成。

## 下一步顺位

1. 经单独实机授权运行 `apple-keychain-p256-v1` gated smoke，记录创建、跨 store 重载、签名、Rust/Go 验签、删除后 missing 和 cleanup；任何失败保持 production gate 关闭。
2. 在受控矩阵中补 locked / denied 的真实错误分类；没有证据时继续保留结构化映射测试，不通过人为改写系统状态冒充验证。
3. 只有 backend 真实证据和 capability 评审通过后，才更新 production status；Secure Enclave、hardware-backed、user presence 与 backup migration 分别保持独立门禁。
4. 随后建立真实产品 sync orchestration：对象发现、hash/签名复验、解密、确定合并、本地 transaction、cursor、上传和 conflict retry；再接 `ManagerBridge`，不让 Flutter 复制协议或密钥逻辑。
5. 最后完成两个真实客户端、恢复/设备授权/撤销/key epoch 与发布级目标部署证据，满足后才评估开放用户同步。

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
- [manager 本地验收](../manager-local-acceptance.md)：M2 产品证据映射。
- [macOS 平台边界](../macos-inputmethodkit-boundary.md)：M1/M2 输入与隐私稳定结论。
- [本周周志](../devlogs/2026-W29.md)：完整验证和交接流水。
