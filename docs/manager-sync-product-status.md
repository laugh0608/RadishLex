# Manager 同步产品状态参考

本文定义 ABI v7 保留的 `radishlex_manager_sync_product_status` 结构、常量、调用规则和隐私边界，面向维护 Rust FFI、Dart binding 与 Manager 状态映射的开发者。本文不包含资格 smoke 操作、同步命令设计、页面布局、阶段进度或用户同步开放决策；通用 ABI 所有权和错误规则见 [FFI Boundary](ffi-boundary.md)。

## 调用契约

`radishlex_manager_sync_product_status` 接收一个调用方分配的 `RadishLexManagerSyncProductStatus*`，成功时写入 status schema v1。它无输入参数，不创建、读取、使用或删除平台 key item，只允许读取 backend 的 metadata-only capability。结构中所有 flag 都使用 `u32` 的 `0/1`，不能按 C/Rust `bool` 布局解释。

调用方必须先校验 ABI contract v7、status symbol 和 `version`，再按 C header 常量映射字段。传入空 `status_out` 返回 `RADISHLEX_STATUS_INVALID_ARGUMENT`；`error_out` 的读取和释放遵循通用 FFI 错误所有权规则。该 status schema 仍为 v1；ABI v7 只在保留此入口的同时增加隔离的本地合成资格 run。

## 字段

| 字段 | 语义 |
| --- | --- |
| `version` | 固定为 `RADISHLEX_MANAGER_SYNC_PRODUCT_STATUS_VERSION = 1` |
| `signing_backend` | `UNAVAILABLE = 0` 或 `APPLE_SECURE_ENCLAVE_P256_V1 = 1` |
| `signing_algorithm` | `UNAVAILABLE = 0` 或 `ECDSA_P256_SHA256_V1 = 1` |
| `signing_compiled`、`signing_runtime_available` | signing backend 是否编译和在当前 runtime 可用 |
| `signing_can_create`、`signing_can_sign` | backend capability，不表示调用本 status 时执行过 create/sign |
| `signing_exportable`、`signing_hardware_backed` | 私钥导出与硬件保护属性 |
| `signing_user_presence_required`、`signing_backup_migratable` | 用户在场与备份迁移属性 |
| `signing_product_qualified` | signing 独立资格结果 |
| `key_agreement_backend` | `UNAVAILABLE = 0` 或 `APPLE_SECURE_ENCLAVE_P256_V1 = 1` |
| `key_agreement_compiled` | key-agreement backend 是否编译 |
| `key_agreement_runtime_qualified` | 独立真实 runtime 资格结果，不继承 signing flag |
| `key_agreement_product_qualified` | key-agreement 独立产品资格结果 |
| `product_qualified` | signing 与 key-agreement 两条资格链的合取结果 |
| `user_sync_enabled` | 独立产品策略开关；资格通过不能自动把它置为 `1` |
| `blocker` | 按固定优先级派生的首个阻断原因 |

## Blocker 优先级

```text
NONE = 0
SIGNING_NOT_COMPILED = 1
SIGNING_UNAVAILABLE = 2
SIGNING_QUALIFICATION_REQUIRED = 3
KEY_AGREEMENT_NOT_COMPILED = 4
KEY_AGREEMENT_RUNTIME_QUALIFICATION_REQUIRED = 5
KEY_AGREEMENT_QUALIFICATION_REQUIRED = 6
USER_SYNC_CLOSED = 7
```

`blocker` 只报告按上述顺序遇到的第一个原因，不是所有失败的集合。未知 enum/blocker 必须失败关闭或显示“不支持的状态”，不得当作 `NONE`；`product_qualified = 1` 也不能替代 `user_sync_enabled = 1`。

## 隐私与执行边界

status 结构只允许 schema 值、enum 和 boolean flag，不得扩展以下内容：

- device ID、key ID 或 application tag；
- public key、canonical bytes 或 signature；
- CFError 文本、OSStatus 或平台查询参数；
- wrapped material、shared secret、master key、nonce 或 ciphertext；
- token、endpoint、userdb path 或用户数据计数。

需要执行资格 smoke 时只能走独立原生 gated validation ABI，不能给 status-only symbol 增加场景参数或副作用。Dart binding 和 `ManagerBridge` 只消费本结构，不直接绑定 validation smoke symbol，也不能从 capability flag 推导或执行 create、sign、derive、delete 或真实同步。

## 绑定检查

修改本结构时必须同步更新并验证：

- `crates/ime-ffi/include/radishlex_input.h` 的常量与 C struct；
- Rust `RadishLexManagerSyncProductStatus` 的 `repr(C)` 布局和 blocker 优先级测试；
- Dart `_RadishLexManagerSyncProductStatus` 布局、native DTO 与 mapper；
- C11 / Objective-C header contract、dynamic library symbol smoke 和 Manager widget 状态映射。

结构增删字段或改变常量值属于 ABI 变更，必须升级 ABI contract 或 status schema，不能只改一侧 binding。
