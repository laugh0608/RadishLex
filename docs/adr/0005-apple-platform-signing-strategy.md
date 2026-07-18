# ADR 0005: Apple 平台签名策略

本文档固定 `apple-keychain-v1` 真实 smoke 阻塞后的 Apple 平台签名推进策略，读者是后续实现 `ime-crypto` 平台 backend、管理 UI 设备页面和审阅同步签名协议的开发者。本文不包含新的平台 SDK 实现、FFI 接口、输入法安装步骤或真实用户同步开放流程；设备签名协议见 `docs/adr/0003-device-signing-key-storage.md`，平台私钥 backend 边界见 `docs/adr/0004-platform-private-key-storage-backend.md`。

## 状态

Accepted

## 与后续决策的关系

本 ADR 对 `apple-keychain-v1` 的结论继续有效：Ed25519 原生 `SecKey` 路径保持生产不可用，也禁止用可导出 seed 冒充非导出 backend。ADR 0006 已按本 ADR 的进入条件接受独立 P-256 profile 和普通 DPK backend；ADR 0007 又固定独立 Secure Enclave backend。后续 P-256 进展不回写或放宽 `apple-keychain-v1`，三种 backend identity 与证据必须分开。

## 背景

RadishLex v1 设备签名协议已固定为 `ed25519-v1`。Rust、Go server 和跨语言 smoke 当前都按 32-byte Ed25519 public key、64-byte signature 和 `radishlex-signature-v1` canonical bytes 验证对象版本、设备授权、设备撤销和恢复记录。

2026-06-30 已在沙盒和提权真实环境运行 `apple_keychain_smoke`，两次都阻塞于 macOS Keychain Ed25519 key 创建阶段，错误为：

```text
UnsupportedSignatureAlgorithm { algorithm: "ed25519-v1" }
```

该结果说明当前 `SecKeyCreateRandomKey` + Ed25519 私钥常驻 Keychain 的实现不能作为已验证 backend，也不能继续通过 `backend_status` 声明可用于生产签名。

## 决策

RadishLex 继续保留 `ed25519-v1` 作为 M3 设备签名协议，不因为 Apple 单个平台 backend 的当前阻塞改动全链路签名算法。

`apple-keychain-v1` 继续表示“Apple 原生非导出 signing key backend”方向，但在真实创建、加载、签名、删除 smoke 通过前，必须在代码和文档中视为生产不可用：

- `backend_status` 不得声明 `available = true`、`can_create_signing_keys = true` 或 `can_sign = true`。
- `ensure_production_signing_allowed()` 必须阻断该 backend。
- gated smoke 可以继续作为平台调查入口，但失败不得写成通过。

暂不把 Ed25519 seed 作为 generic password / data item 存入 Keychain 后再取回 Rust 签名，也不把这种方案塞进 `apple-keychain-v1`。该方案会让签名私钥 bytes 进入进程内可导出路径，突破当前“非导出平台 signing key”停止线。后续如确需软件保护 fallback，必须新增独立 backend id 和 capability / 风险口径，例如单独标记为 software-protected、not hardware-backed、not production-eligible，不能复用 `apple-keychain-v1`。

本 ADR 当时决定不直接把 Apple 平台切到 P-256；该进入条件随后已由 ADR 0006 满足并接受独立 P-256 profile，ADR 0007 又固定 Secure Enclave backend。这里保留为历史决策，不再表示当前禁止 P-256；独立算法/backend、跨语言 verifier、迁移与无 fallback 要求继续有效。

## 推进顺序

以下顺序是 `apple-keychain-v1` 阻塞后的决策链。第 5 项已由 ADR 0006/0007 完成，其余规则继续约束 Ed25519 backend 和任何后续 fallback 讨论。

1. 先让 `apple-keychain-v1` 的 status 明确阻断生产签名，保留 smoke 作为调查命令。
2. 记录当前平台阻塞和不可用状态，避免后续管理 UI 或同步上传误认为 Apple backend 可用。
3. 后续如继续调查 Apple 原生非导出 Ed25519，应补 macOS / iOS 版本、App Sandbox、Keychain entitlement 和 Security framework 行为矩阵。
4. 如决定引入软件保护 fallback，应先补新的 backend ADR / runbook，再接代码；不能在 `apple-keychain-v1` 内静默降级。
5. 如决定引入 P-256，应先补签名算法 ADR 和跨语言协议迁移计划，再改 Rust / Go 实现。

## 验证口径

当前必须验证：

- 默认测试和启用 `apple-keychain` feature 的普通测试不访问系统 Keychain。
- `apple-keychain-v1` capability metadata 仍保留为非硬件、非 user-presence、非 backup-migratable 的平台方向说明。
- `apple-keychain-v1` store status 在策略未解决前阻断生产签名。
- gated smoke 失败时记录错误、可复验命令和本机环境边界，不伪造成通过。

真实用户同步前必须重新验证：

- 创建 key 后能重新加载同一 public key。
- 签名结果能通过 Rust 和 Go 的 Ed25519 verifier。
- 删除或撤销后不能继续签名。
- Keychain locked / access denied / unsupported algorithm / corrupted item 错误可区分。
- 日志不包含私钥 bytes、seed、Keychain query、canonical bytes 原文、signature bytes、用户词或本机路径。

## 后果

收益：

- 不为了单个平台阻塞牵动已验证的 Ed25519 协议和 Go server 验签链路。
- 避免把“Keychain 保存 seed、Rust 取出签名”的软件保护方案伪装成非导出平台 backend。
- 后续管理 UI 和真实同步入口能根据 status 正确阻断 Apple backend。

代价：

- Apple 平台仍没有完成最终资格评审的生产 signing backend：普通 DPK P-256 因可导出被拒绝，Secure Enclave P-256 已有运行时、不可导出、hardware-backed、denied 与 locked 证据但仍待 unsupported 和产品资格评审。
- 后续需要单独投入平台 spike，或明确接受新的软件保护 backend / 新签名算法 profile 的实现成本。
