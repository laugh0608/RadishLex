# 平台私钥 Backend 策略

本文档整理 RadishLex 设备签名私钥 backend 的当前证据、决策顺序、继续调查条件和停止线。读者是后续实现平台 backend、管理 UI 同步设备页、评估签名算法演进和审阅隐私边界的开发者。本文不替代 ADR 0003 至 ADR 0006，也不包含 Android、Apple、Windows 或 Linux 的逐步 smoke 操作。

## 当前结论

M2 已于 2026-07-18 关闭，RadishLex 当前进入 M3。ADR 0006 已接受 `ecdsa-p256-sha256-v1` 作为与 `ed25519-v1` 共存的生产候选 profile；Rust/Go 验签、历史 metadata migration、共享跨语言 vectors 与独立 Apple backend 已在仓库内落地。Apple P-256 基础生命周期 gated smoke 已取得真实证据，但当前证据仍不足以解除生产门禁：

- `test-memory-v1` 只用于测试和 fixture，不能进入生产同步。
- `unavailable` 是默认失败 backend，不允许静默回退。
- `apple-keychain-v1` 已接线并运行真实 smoke，但阻塞于 `UnsupportedSignatureAlgorithm { algorithm: "ed25519-v1" }`。
- `apple-keychain-p256-v1` 已接入 manager Release native library 并完成 DPK 产品生命周期。普通 DPK 软件 P-256 key 可由平台 API 导出，当前如实声明 `exportable=true`；运行时可创建、重载和签名，但不符合生产 backend 的不可导出条件，`product_qualified=false`。
- `apple-secure-enclave-p256-v1` 已完成独立 repository、FFI、manager native、qualification lifecycle 与 ad-hoc denied；实测支持 runtime、不可导出和 hardware-backed，并确认 missing entitlement 失败关闭，产品资格仍等待 locked/unsupported。
- `android-keystore-v1` 已有 Kotlin / Gradle harness、JNI glue、gated smoke 和 provider diagnostics；Pixel 9 Pro API 35 AVD 与 Pixel 10 Pro API 37 AVD 均返回 `unsupported_signature_algorithm`。
- `windows-cng-v1`、`linux-secret-service-v1` 仍只是能力边界标识，未进入实现。

没有新的 Android 真机或不同系统镜像时，不应继续把“真机矩阵”作为当日硬阻塞。普通 DPK P-256 的评审结论是“软件运行时可用、生产资格拒绝”。独立 Secure Enclave P-256 产品 lifecycle 已支持 runtime、不可导出与 hardware-backed；当前等待独立授权完成 denied/locked/unsupported 失败矩阵，发布级目标部署运行证据仍保留为正式发布前门禁。

## 策略目标

- 保持服务端默认不可信：Go server 只验证设备公钥、签名和设备状态，不接触私钥材料。
- 保持签名 key usage 单一：设备签名 key 只用于签名对象、授权、撤销和恢复记录，不用于对象加密或 key wrapping。
- 保持 backend id 语义清楚：一个 backend id 只表达一种存储与风险模型，不在内部静默降级。
- 保持用户可用同步停止线明确：没有可用生产签名 backend 时，远端同步可以继续做实现级 smoke，但不能开放真实用户同步。
- 保持后续选择可审查：如果要换算法或引入软件保护 backend，必须先补 ADR / runbook，再改 Rust、Go 或平台代码。

## 当前 Backend 证据表

| Backend | 当前状态 | 已有证据 | 下一步条件 |
| --- | --- | --- | --- |
| `test-memory-v1` | 测试可用，生产禁止 | Rust 单元测试、integration test 和签名对象 fixture 已覆盖 | 继续只用于测试，不进入 UI / 生产配置 |
| `unavailable` | 默认明确失败 | Rust capability / status / error 测试已覆盖 | 继续作为能力缺失时的失败路径 |
| `apple-keychain-v1` | 生产不可用 | feature-gated backend 编译通过，真实 smoke 在 Ed25519 创建阶段失败 | 单独补 Apple 原生非导出 Ed25519 支持矩阵，或另起 backend / 算法 ADR |
| `apple-keychain-p256-v1` | DPK 软件运行时可用，生产资格拒绝 | provisioning-backed manager 产品进程已完成创建、重载、签名、Rust/Go 验签、删除、missing 和 cleanup；capability 为 `exportable=true` | 保留为软件保护证据与失败关闭实现，不接真实同步；另建 Secure Enclave backend |
| `apple-secure-enclave-p256-v1` | 产品 runtime、不可导出、hardware-backed 与 denied 已取证，产品资格关闭 | qualification lifecycle 全部通过；ad-hoc missing-entitlement create 返回固定 access-denied，零残留 | 恢复 qualification bundle 后单独授权 locked；另需无 Secure Enclave 环境执行 unsupported，完成后评审 product qualification |
| `android-keystore-v1` | 生产不可用 | Android target build、Gradle harness、API 35 / API 37 AVD diagnostics 和 smoke 记录 | 有新 Android 真机 / OEM / system image 时先跑 diagnostics，再按结果决定 smoke |
| `windows-cng-v1` | 未实现 | 仅有 ADR 0004 backend id | 进入 Windows 主线前补 CNG 签名能力 spike / runbook |
| `linux-secret-service-v1` | 未实现 | 仅有 ADR 0004 backend id | 进入 Linux 同步主线前补 Secret Service / 软件保护能力边界 |

## 决策顺序

后续遇到平台私钥 backend 阻塞时，按以下顺序判断：

1. 先读取设备登记的显式 `signing_algorithm` 与 backend capability，不按平台名或 key 长度猜测算法。
2. 对 Ed25519 设备继续使用原 profile；普通 DPK 与 Secure Enclave P-256 分别使用独立 backend/key identity，任一路径失败都不得尝试另一算法或 backend。
3. 平台失败时记录 API / OS 版本、固定错误分类、cleanup 和日志脱敏结果，不记录 key、signature 或 canonical bytes。
4. 不降级到软件 seed、普通文件、SQLite、SharedPreferences、generic password item 或 `test-memory-v1`。
5. 编译和运行时证据只改变对应能力字段；只有产品环境 smoke 与 capability 评审满足生产合格条件，才允许设置 `product_qualified=true`。真实用户同步还受独立 M3 gate 约束。

这意味着 `apple-keychain-v1` 和 `android-keystore-v1` 都不能因为当前 blocker 而偷偷变成“平台保存 seed，Rust 取出 seed 签名”的方案。那是另一类 backend，风险和 UI 说明都不同。

## 四层状态与门禁

- 编译可用：当前 target 是否把平台 backend 编入实际产物。它只证明符号和平台依赖存在，不访问 Keychain，也不证明 API 在产品进程中成功。
- 运行时能力：`available`、`can_create_signing_keys`、`can_sign` 表达当前实现和已验证 storage domain/host identity 能力，不等于生产资格。`apple-keychain-p256-v1` 的 DPK 产品生命周期已支持这些字段为 true，但 `exportable=true` 使其不能进入生产签名。
- 产品资格：`product_qualified` 只由真实产品 bundle 进程证据和评审改变。repository test、命令行 gated smoke、bundle symbol/status smoke 都不能单独把它改为 true。
- 真实同步 gate：对象 orchestration、设备授权、恢复、撤销、key epoch、部署和 manager Rust bridge 全链满足前继续为 false。backend 产品资格即使通过，也不能自动开放用户同步。

`hardware_backed`、`user_presence_required`、`backup_migratable` 与 Secure Enclave 不是上述四层的别名，各自需要独立实测证据。

## 生产 Backend 合格条件

一个 backend 只有同时满足以下条件，才允许解除生产签名门禁：

- 可以创建不可导出 signing key。
- 可以重新加载同一个 public key。
- 可以对 `radishlex-signature-v1` canonical bytes 生成签名。
- 签名能通过 Rust 和 Go 的 verifier。
- 删除或本地撤销后，后续 `sign` 明确失败。
- `backend_status()` 准确报告 `compiled`、`available`、`can_create_signing_keys`、`can_sign`、`product_qualified`、`exportable`、`hardware_backed`、`user_presence_required` 和 `backup_migratable`。
- locked、access denied、unsupported algorithm、corrupted item 和 unavailable 能被区分。
- Debug、日志、测试 fixture 和文档记录不包含私钥 bytes、seed、完整 alias、canonical bytes 原文、signature bytes、恢复码、token 或用户输入内容。
- 默认仓库测试不会触碰系统 Keychain / Keystore / CNG / Secret Service。
- gated smoke 有 cleanup 证据，失败不能留下不可解释的测试 key。

硬件保护、user presence 和备份迁移能力需要单独证明。基础签名 smoke 通过不等于 `hardware_backed = true`，也不等于可以把同步开放给真实用户。

截至 2026-07-18，`apple-keychain-p256-v1` 已在 application identifier、默认 Keychain access group 与 provisioning profile 一致的 manager 产品进程中完成 DPK 生命周期。评审确认标准 DPK 软件私钥可导出，因此该 backend 只开放编译和运行时字段，产品资格与真实同步保持关闭；继续补 locked 不能改变不可导出条件不满足的结论。

## 可选后续路径

### 路径 A：继续调查原生非导出 Ed25519

适用条件：

- 能获得新 Android 真机、不同 OEM、不同 system image、不同 security patch，或能补 Apple 平台版本 / entitlement / sandbox 行为矩阵。
- 当前目标仍是保留 `ed25519-v1`，不改 Go / Rust verifier。

执行规则：

- Android 先跑 provider diagnostics；只有 diagnostics 证明 `AndroidKeyStore` 产出 Ed25519 public key，才跑 gated smoke。
- Apple 先明确 macOS / iOS 版本、Security framework 调用路径、entitlement 和 sandbox 条件，再跑 gated smoke。
- 所有记录都必须写入对应 runbook / device matrix，不把 AVD 结论写成 OEM 真机结论。

### 路径 B：新增签名算法 Profile

当前状态：ADR 0006、跨语言协议、普通 DPK 软件 backend 与产品评审已完成。P-256 协议本身可复用；Secure Enclave 必须使用独立 backend id、handle/capability 与产品证据，不能把普通 DPK key 原地升级成硬件 key。以下条目继续作为后续新增 profile 或 backend 的通用进入条件。

适用条件：

- 目标平台明确不支持非导出 Ed25519，但支持另一种非导出 signing key，例如 P-256 / ECDSA。
- 项目愿意承担协议迁移、跨语言 verifier 和历史设备兼容成本。

进入任何其他算法实现前必须先补新的 ADR，至少回答：

- 新算法标识、public key 长度、signature 编码和 canonical bytes 是否复用。
- Rust `ime-crypto`、Go server verifier、HTTP API 字段约束和测试 fixture 如何迁移。
- 旧设备 Ed25519 与新设备算法如何共存。
- 设备授权、撤销、恢复记录和 object manifest 如何绑定算法。
- 管理 UI 如何展示不同算法 backend 的能力和风险。

不得把 P-256 或其他算法直接接进现有 `ed25519-v1` 字段。当前 P-256 使用 ADR 0006 固定的 `ecdsa-p256-sha256-v1` 和独立 Apple backend id。

### 路径 C：新增软件保护 Backend

适用条件：

- 项目明确接受某些平台没有非导出 signing key，只能用系统 secret storage / OS 文件保护保存软件私钥。
- 该能力只作为低保护等级或用户显式开启路径，不冒充硬件或非导出 backend。

进入实现前必须先补新的 backend ADR / runbook，至少回答：

- backend id，例如 `apple-keychain-software-v1` 或 `linux-secret-service-software-v1`，不得复用 `apple-keychain-v1` / `android-keystore-v1`。
- `exportable`、`hardware_backed`、`backup_migratable` 和恢复 / 撤销风险如何声明。
- 私钥 bytes 是否会进入进程内存、FFI、崩溃报告或备份。
- 用户可用同步 UI 如何提示保护级别。
- 该 backend 是否允许生产同步，还是只允许开发 / 单用户受控环境。

在该 ADR 完成前，不允许把 seed 存储 fallback 混入现有平台 backend。

### 路径 D：继续本地同步产品开发

适用条件：

- 当前没有新 Android 真机或 Apple 平台调查条件。
- 仍希望推进 M3 manager 同步入口和本地联调能力。

可推进内容：

- 使用本地 Docker / 本地 HTTPS 复验 sync server 到达性、bearer token 失败响应、密文对象路径和日志脱敏。
- 在 manager 中实现 sync entry state helper / UI gate，只展示本地联调来源、阻塞原因和下一步，不上传真实用户 P2 数据。
- 为 settings draft、backend gate、部署证据来源、恢复码状态和设备授权状态补 Dart helper / widget / 诊断脱敏测试。
- 保持正式发布前再补真实证书 / 域名 / 外部反代、目标数据目录备份恢复、升级回滚和日志策略复验。

该路径不解除平台私钥 backend 门禁，也不解除真实用户同步开放门禁；它用于避免当前产品开发被发布级部署环境长期阻塞。

## 当前不做

- 不把 Android AVD 失败写成所有 Android 真机失败。
- 不为了推进 Flutter manager 或平台壳而跳过生产签名 backend 门禁。
- 不让 InputMethodService、TSF、InputMethodKit、Fcitx5 / IBus 或 Keyboard Extension 持有同步私钥。
- 不把同步签名放进输入热路径。
- 不把 `test-memory-v1`、软件 seed 或普通文件私钥作为生产 fallback。
- 不把服务端 bearer token、OIDC token、恢复码或账号密码当作设备签名替代品。
- 不让 Go server 根据管理 token 直接创建、替换或伪造设备签名。

## M3 推进项

在只有当前 Mac 设备、没有额外 Android 真机时，按以下顺序推进：

1. 已完成 ADR 0006、算法无关 Rust/Go verifier、显式 Go metadata migration 与共享跨语言负向 vectors；`ed25519-v1` 保持兼容。
2. 已完成独立 `apple-keychain-p256-v1` repository spike 与双层门禁；普通测试不访问系统 Keychain。
3. 已在独立授权后完成 ad-hoc denied 与 provisioning-backed manager 产品 DPK 生命周期；native 内完成创建、重载、签名、Rust/Go 验签、删除、missing、失败关闭、cleanup 和固定摘要。Dart 不绑定该 ABI，InputMethodKit 不接入同步密钥职责。
4. 已完成 capability 评审：普通 DPK P-256 key 标记 `exportable=true`，编译/运行时字段如实开放，`product_qualified` 与用户同步 gate 关闭；Secure Enclave、hardware-backed、user presence 和 backup migration 均没有从基础签名成功推导。
5. 已补 Secure Enclave 独立 backend ADR/runbook，并沿 crypto、FFI、manager native 完成 repository 接线、qualification lifecycle、capability 重冻结与 ad-hoc denied；不回退普通 DPK 或 test memory。下一证据是恢复 qualification bundle 后执行 locked，并在无 Secure Enclave 环境执行 unsupported。
6. 只有不可导出 production backend 评审和产品环境 smoke 通过后，才进入真实产品 sync orchestration 与 `ManagerBridge` 命令；恢复码、设备授权、撤销和用户同步入口继续关闭到 M3 全部退出证据成立。

## 验证口径

本策略文档自身是治理与架构收口，至少执行：

```text
git diff --check
./scripts/check-repo.sh
```

如果同时修改 Android harness 或 Rust backend，追加：

```text
./scripts/check-android-target.sh
cargo test -p radishlex-ime-crypto --features android-keystore
```

Apple repository 测试可运行：

```text
cargo test -p radishlex-ime-crypto --features apple-keychain
```

该命令中的 Keychain 集成测试默认 ignored。执行 P-256 gated smoke 必须先获实机授权，并显式设置环境门；不能用普通测试通过替代真实证据。

如果继续跑 gated Android diagnostics / smoke，必须先获得明确授权，因为 diagnostics 会触碰测试设备 Android Keystore 的合成 key。

## 参考入口

- [ADR 0003: 设备签名与私钥存储边界](adr/0003-device-signing-key-storage.md)
- [ADR 0004: 平台私钥存储 Backend 边界](adr/0004-platform-private-key-storage-backend.md)
- [ADR 0005: Apple 平台签名策略](adr/0005-apple-platform-signing-strategy.md)
- [ADR 0006: 设备签名算法 Profile](adr/0006-device-signature-algorithm-profiles.md)
- [ADR 0007: Apple Secure Enclave P-256 Backend](adr/0007-apple-secure-enclave-p256-backend.md)
- [Apple Keychain Signing Backend Runbook](runbooks/apple-keychain-signing-backend.md)
- [Apple Secure Enclave P-256 Backend Runbook](runbooks/apple-secure-enclave-p256-backend.md)
- [Android Keystore Signing Backend Runbook](runbooks/android-keystore-signing-backend.md)
- [同步密钥与设备生命周期设计](sync-key-management.md)
- [Sync Server Production Deployment Runbook](runbooks/sync-server-production-deployment.md)
