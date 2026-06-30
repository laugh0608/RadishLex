# Sync Server OIDC 未来接入规划

本文档定义 RadishLex sync server 未来接入 OIDC / Radish 产品账号体系时的规划边界。读者是后续实现认证网关、客户端登录、sync server 访问控制和部署 runbook 的开发者。本文不定义当前必须实现的 OIDC 代码，不改变现有单用户自部署 bearer access token 临时门禁，也不包含 Radish 项目的本地实现路径或源码迁移方案。

## 当前结论

当前 Phase 3 仍保留 `RADISHLEX_SYNC_ACCESS_TOKEN` 作为单用户自部署的临时访问门禁。它只用于阻止未授权请求进入 sync server，不是 Radish 产品账号登录体系。

OIDC 应作为后续专题推进，目标是让 RadishLex 能接入 Radish 产品体系或兼容 OIDC 的自部署身份提供方，同时继续保持以下边界：

- sync server 不成为账号系统。
- sync server 不保存用户密码、refresh token、OIDC client secret 或 Radish 产品会话。
- OIDC 身份只解决部署访问与账号归属，不替代设备签名、设备授权、恢复码、端到端加密或客户端解密合并。
- 服务端仍不能解密 userdb payload，不能看到明文用户词、input code、reading、P1 事件或候选偏好。

## Radish 参考经验

Radish 当前可作为架构经验参考，但不得把其代码或目录结构迁入 RadishLex：

- 独立 `Radish.Auth` 作为 OIDC 认证中心，基于 OpenIddict 提供授权码流程、token、session 和客户端配置。
- `Radish.Gateway` 作为统一外部入口，将 `/connect/*` 转发到认证服务，将 `/api` 转发到业务 API。
- 部署公开入口、OIDC issuer、CORS 和客户端回调地址通过统一公开 URL 对齐，避免回调地址漂移。
- OIDC signing / encryption 证书必须持久化并支持轮换；资源服务验证 token 时应考虑 JWKS / signing key 轮换。
- Claim 语义应收口在协议边界和统一身份转换层；业务代码不应散落解析 `sub`、`scope`、`role` 或兼容 claim。

这些经验在 RadishLex 中应转化为行为规格、接口约束和测试口径，而不是复制实现。

## 目标形态

未来 OIDC 接入建议分两层：

1. 外部入口层：Radish 产品 Gateway、Radish Auth 或兼容 OIDC 的自部署 IdP 负责登录、授权码流程、refresh token、session、MFA 和账号生命周期。
2. sync server 资源层：Go sync server 只做 access token 验证和 scope / audience / subject 检查，继续处理密文对象、设备签名、版本冲突和恢复记录。

sync server 可接受两类部署形态：

- Radish 产品接入：`issuer` 指向 Radish Auth，`audience` / `scope` 固定给 RadishLex sync server。
- 高级自部署接入：部署者提供兼容 OIDC issuer / JWKS / audience / scope 配置。

## 认证策略演进

当前策略：

- `shared-token`：由 `RADISHLEX_SYNC_ACCESS_TOKEN` 配置，适合单用户自部署、内网验证和早期生产前 smoke。

未来策略：

- `oidc-jwt`：sync server 验证 OIDC access token 的 issuer、audience、expiry、signature、scope 和 subject。
- `external-auth-gateway`：外部网关完成 OIDC 验证后，只把受信任的身份头传给内网 sync server；该模式必须有明确的网络边界和 header 防伪策略。

实现前应先把 Go handler 中当前 token 校验收敛为认证接口，例如 `AccessAuthorizer`。业务 handler 不应直接依赖 shared token、OIDC JWT 或网关 header 的具体解析方式。

## Radish 接入前与测试账号处理

在正式接入 Radish Auth / Gateway 前，RadishLex 不实现账号注册、密码登录、用户资料、会话或多账号管理。账号语义应先收敛为认证层返回的访问主体，而不是提前落成账号表。

当前和过渡期按以下规则处理：

- 单用户自部署生产 / smoke：`shared-token` 通过后，访问主体视为部署本机的 `deployment-owner`。这是部署级 owner，不是 Radish 产品账号，不跨部署稳定，不写入 object id、blob path、encrypted payload hash、恢复材料或普通日志。sync domain 内的真实写入权限仍由 device authorization、device state、key epoch 和签名校验决定。
- 本地开发和单元测试：允许关闭 access token 或使用合成 token；如测试认证抽象需要主体，使用内存中的 `test-owner` / `test-subject` 这类合成值，不使用真实邮箱、手机号、Radish 用户 ID、第三方账号 ID 或开发者个人信息。
- OIDC 代码进入真实 Radish 前的集成测试：只能使用本地 mock issuer / JWKS / audience / scope 和短生命周期测试 token。fixture 中的 `iss`、`sub`、`aud`、`scope` 必须是合成值；不得调用真实 Radish Auth，不提交真实 token、client secret、refresh token、用户 profile 或 JWKS 私钥。
- Flutter manager / 客户端在 Radish 接入前只配置 sync server URL、access token 和设备授权 / 恢复流程；不要出现用户名密码登录，也不要把“Radish 账号”作为 UI 前提。需要展示身份时，只能展示本地 server profile / deployment label 之类的非账号概念。

后续抽象 `AccessAuthorizer` 时，可让不同策略返回统一的 `AccessPrincipal`：

- `shared-token`：返回部署本地 owner 主体。
- `test-disabled` / `test-token`：返回只在测试进程内使用的合成主体。
- `oidc-jwt`：返回 issuer + subject + scope 派生的主体。
- `external-auth-gateway`：返回外部网关已验证并经过防伪边界保护的主体。

业务 storage 不应直接解析这些策略来源。只有进入多账号或 Radish 产品接入阶段后，才允许在 ADR 固定规则后保存 account subject 到 sync domain 的绑定 metadata。

## Identity 映射

OIDC subject 与 RadishLex 同步域必须分层：

- `sub` / account id：表示登录用户或 Radish 产品账号。
- `domain_id`：表示客户端加密同步域。
- `device_id`：表示同步域内设备身份。
- `signing_public_key`：表示设备签名验签材料。

OIDC account 可以拥有一个或多个 sync domain，但不能解密这些 domain 的对象。服务端最多保存必要的 account-subject 到 domain 绑定 metadata，用于访问控制、审计和删除整域密文数据；同步主密钥、恢复码、平台私钥和 userdb plaintext 仍只存在客户端。

## Scope 草案

后续 ADR 应固定最终 scope 名称。当前只保留草案：

- `radishlex.sync.read`：读取 domain state、object metadata、encrypted payload 和 recovery metadata。
- `radishlex.sync.write`：创建 join request、提交授权、上传 encrypted object version 和 recovery record。
- `radishlex.sync.admin`：执行设备撤销、同步域级清空密文数据、部署管理操作。

scope 不替代设备状态。即使 token 具备写 scope，上传对象仍必须通过 active device、key epoch、object manifest signature、payload hash / length 和版本冲突校验。

## 停止线

进入 OIDC 实现前必须先补 ADR 或同等专题设计，至少固定：

- issuer / audience / JWKS / key rotation / clock skew / cache 规则。
- scope 与 API route 的映射。
- account subject 与 sync domain 的绑定、解绑、删除和审计语义。
- OIDC token 验证失败的错误码、日志脱敏和测试 fixture。
- 客户端登录、token 刷新和 token 存储边界。
- 外部 gateway 模式下身份 header 的防伪网络边界。

不得做：

- 不在 sync server 中实现完整账号注册、密码登录、refresh token 存储或 Radish 产品 session 管理。
- 不把 OIDC `sub`、邮箱、昵称、手机号或第三方账号字段写入 object id、blob path、encrypted payload hash 或普通日志。
- 不让 OIDC 登录状态绕过设备授权、设备撤销、恢复码、key epoch 或 object manifest 签名。
- 不为了 OIDC 接入引入多租户数据库复杂度，除非对应阶段已明确从单用户自部署升级。

## 验证口径

未来实现至少应覆盖：

- 无 token、错误 issuer、错误 audience、过期 token、错误签名和缺失 scope 均返回结构化 `unauthenticated` 或 `forbidden`，且不进入业务 storage。
- OIDC token 内容、signature、raw JWT、refresh token 和 user profile 不出现在响应、audit event、runtime log 或 panic message 中。
- signing key / JWKS 轮换可复验，旧 token 与新 token 的过渡行为明确。
- OIDC token 通过后，revoked / pending / unknown device 仍不能写入对象。
- account subject 与 domain 绑定错误时，即使设备签名有效也不能访问不属于该账号的 domain。

## 推进建议

短期继续使用 shared token 完成 sync server 备份恢复演练、外部 TLS 验证和平台私钥 backend 停止线。

中期先新增认证策略 ADR，并把 Go server 的认证层抽象成可插拔边界；不急于接入真实 OIDC provider。

长期在 Radish 产品集成阶段接入 Radish Auth / Gateway 或兼容 OIDC provider，并把 Flutter manager / 移动客户端登录流程与 Radish 现有授权码实践对齐。
