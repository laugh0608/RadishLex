# Sync Server Admin Console 远期专题

本文档定义自部署 sync server 未来是否提供 WebUI / 管理控制台时的规划边界。读者是后续实现 Docker 部署运维界面、认证策略、Go server admin API 和审阅隐私边界的开发者。本文不包含当前阶段必须实现的 UI、路由、前端技术选型、账号系统、OIDC 代码、真实部署操作步骤或任何会改变 Phase 4 管理端近期推进顺序的任务。

## 当前结论

当前仓库没有实现 sync server WebUI，也不把它纳入最近开发主线。

近期主线仍是 Flutter manager 通过真实 Dart FFI bridge 接入本地 userdb 管理、学习状态摘要、rank explain 摘要和 sync preflight 摘要；真实远端同步、恢复码和设备授权 UI 继续等待可用平台私钥 backend 与目标部署运行证据。

Sync server admin console 作为远期可选专题保留，适合在以下条件成熟后再评估实现：

- OIDC / admin scope 或等价部署者认证边界已经固定。
- Go server 已具备稳定的 admin API 错误语义和权限模型。
- 目标部署运行证据、备份恢复和升级回滚 runbook 已稳定。
- 明确不会把服务端变成明文用户数据管理入口。

## 定位

Sync server admin console 是自部署运维控制台，不是用户词库管理器，也不是输入法客户端管理端。

它只面向部署者展示服务端可见的非敏感元数据、健康状态、配置状态、备份恢复状态和脱敏审计摘要。用户本机词库、学习记录、候选偏好和同步 preflight 的真实管理入口仍在 Flutter manager / Rust core 侧。

## 可展示范围

未来 WebUI 可以展示：

- 服务版本、启动时间、健康状态和 runtime 配置摘要。
- SQLite metadata migration 状态、对象存储目录状态和存储空间摘要。
- sync domain、device、join request、recovery record 和 encrypted object 的数量、版本、状态与更新时间。
- Docker / 部署态配置检查结果、外部 TLS 反代状态和 bearer / OIDC 模式摘要。
- 备份恢复演练、升级回滚演练和对象大小门禁的最近结果。
- 脱敏 audit log 摘要，包括 request id、route、status、result code、object type、耗时和时间。
- 只包含非敏感字段的诊断导出。

## 禁止展示或保存

未来 WebUI 不得展示、保存、导出或记录：

- 明文用户词库、明文输入历史、明文候选偏好、明文上下文片段。
- P0 / P1 原始事件、窗口标题、联系人、密码、证件、支付信息或其他敏感上下文。
- encrypted payload bytes、signature bytes、wrapped material bytes、recovery wrapped material bytes。
- bearer token、OIDC token、refresh token、client secret、恢复码、KDF 输出、同步主密钥、设备私钥或 TLS 私钥。
- Go server 内部 `blob_ref` 真实路径、宿主机绝对路径、容器 secret 或部署者私有域名证书细节。

如果未来需要提供诊断导出，默认只能导出非敏感摘要；任何导出功能都必须经过权限检查、脱敏测试和文档说明。

## 权限与部署边界

未来 WebUI 默认应关闭，并且只能在部署者明确启用后暴露。

可接受的部署形态：

- 本机 loopback 访问，只用于本机运维。
- 内网访问，并由外部反代完成认证、TLS 和访问控制。
- OIDC / admin scope 固定后，由 sync server 验证具备管理权限的 access token。

不可接受的部署形态：

- 默认随 Docker Compose 对公网暴露。
- 复用用户同步 bearer token 作为长期 admin token。
- 没有 TLS / 反代 / 认证边界时允许危险操作。
- 在 WebUI 内实现完整账号注册、密码登录或 refresh token 存储。

## 危险操作停止线

以下操作即使未来实现，也必须有二次确认、权限检查、审计记录和 runbook：

- 撤销设备。
- 清空某个 sync domain 的服务端密文对象。
- 删除 recovery record。
- 触发备份恢复或回滚。
- 修改访问控制模式。
- 修改对象大小门禁、存储路径或审计配置。

WebUI 不得提供“查看明文数据”“重置用户词库”“服务端修复候选排序”这类会破坏客户端真相源或隐私边界的能力。

## 与 Flutter Manager 的关系

Flutter manager 面向用户和本机数据：

- 本地 userdb 词条管理。
- 学习状态摘要。
- rank explain 摘要。
- sync preflight 摘要。
- 设备授权、恢复码和用户可用同步状态。

Sync server admin console 面向部署者和服务端运维：

- 部署健康。
- 存储与 migration。
- 认证模式。
- 备份恢复。
- 脱敏 audit。
- 服务端可见 metadata。

两者不能互相替代。Flutter manager 不应承载 Docker 运维能力；sync server admin console 不应承载本地词库、学习记录或候选偏好管理能力。

## 验证要求

未来如果进入实现，至少需要覆盖：

- WebUI 默认关闭。
- 未认证访问被拒绝。
- 非 admin scope 访问被拒绝。
- 页面和 API response 不包含禁止字段。
- 脱敏 audit log 不包含 token、payload、signature、wrapped material、恢复码或宿主机 secret。
- 危险操作需要二次确认并写入审计摘要。
- Docker Compose 本地 / 部署态配置不会默认对公网暴露 WebUI。

## 路线位置

该专题属于后端部署治理的远期可选能力，不影响当前 Phase 4 管理端近期计划，也不作为 Phase 3 / Phase 4 退出标准。

建议顺序：

1. 当前继续推进 Flutter manager 真实 Dart FFI bridge。
2. 平台私钥 backend 与目标部署运行证据成熟后，再开放用户可用同步。
3. OIDC / admin scope 或等价认证边界固定后，再评估 sync server admin API。
4. admin API 的权限、错误语义和脱敏测试稳定后，再考虑 WebUI 实现。
