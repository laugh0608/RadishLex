# RadishLex 管理端边界

本文档定义 Phase 4 Flutter manager 实现和后续演进必须稳定的职责边界、数据可见性、同步 UI 停止线和第一批功能顺序。读者是后续实现 `apps/radishlex-manager`、`ime-ffi` 管理接口、同步设置页面和审阅隐私边界的开发者。本文不包含 Flutter 页面视觉稿、widget 目录结构、平台输入法壳接入、完整账号系统、OIDC 实现或真实平台私钥 backend 实现。

Phase 4 本地能力是否满足当前退出标准，由 `docs/manager-local-acceptance.md` 记录验收范围、证据入口和停止线；真实同步入口进入 UI / bridge 前的交互边界、错误分类和测试计划见 `docs/manager-sync-entry-boundary.md`；本文只定义长期职责边界。

## 当前定位

Flutter manager 是 RadishLex 的管理界面，不进入输入热路径，不承担候选生成、候选排序、用户词库真相源、同步合并真相源或隐私策略真相源。

管理端的职责是把 Rust core 和本地 userdb 已经具备的能力以可审计、可删除、可解释的方式呈现给用户，并在同步能力具备生产条件前清楚显示不可用原因。

当前仓库已新增 `apps/radishlex-manager/` Flutter macOS 起步工程，通过受控 `ManagerBridge` contract 接入合成 fixture，并在显式配置本地 SQLite userdb 与 `ime-ffi` 动态库时切到真实 Dart FFI bridge。真实远端同步、恢复码和设备授权仍未开放。Phase 4 后续代码继续遵守以下边界：

- 本地 userdb 管理优先于远端同步开关。
- 学习记录摘要优先于 P1 原始事件明细。
- 同步预检、本地 Docker / 本地 HTTPS 联调状态和部署配置检查优先于真实远端启用。
- 恢复码和设备授权 UI 必须等待可用平台私钥 backend。
- 用户可用同步主操作必须等待发布级部署证据和可用平台私钥 backend；非上传的同步入口状态、阻塞说明和本地联调展示可以继续推进。

## 职责范围

Phase 4 第一批管理端功能应覆盖：

- 查看本地用户词条。
- 删除用户词条，并写入 tombstone。
- 导入用户词库，并显示导入检查结果。
- 导出用户词库。
- 查看本地学习状态摘要。
- 查看 ranker explain 的非敏感摘要。
- 查看 sync preflight 摘要。
- 查看 import batches、词条 tombstone 和本地 sync 影响摘要。
- 配置自部署服务端地址和本地连接参数草案。
- 显示同步能力是否可用，以及不可用原因。
- 查看本机设备身份、backend capability 和 production gate 状态摘要。
- 保存非 secret settings draft，并从草案、隐私模式、平台私钥 backend gate 和部署证据来源标签派生 sync gate。
- 预览、复制和导出脱敏诊断摘要。
- 以结构化错误分类展示 bridge 操作失败，不把 native 错误明细透传给 widget 层。

后续同步能力成熟后，管理端可以继续提供：

- 当前设备列表。
- 加入请求审批。
- 设备撤销。
- 恢复码生成、确认保存、轮换和撤销。
- 最近同步对象数量、最近上传时间和最近下载时间。
- 一键停止同步。
- 一键删除本机学习数据。
- 一键从服务端删除当前同步域密文数据。

## 非职责范围

管理端不应：

- 处理每次按键、composition、候选生成、候选排序或文本提交。
- 绕过 Rust core 直接修改 userdb SQLite schema。
- 生成、保存、上传或记录明文同步 payload。
- 展示 P1 原始选择事件、原始负反馈事件、窗口标题、联系人、密码、证件、支付或其他敏感上下文。
- 把恢复码、同步主密钥、设备私钥、wrapped material 明文或 bearer token 写入普通日志、崩溃报告、截图、analytics 或测试 fixture。
- 把 Go server 当作候选排序服务、在线转换服务或明文词库服务。
- 在平台私钥 backend 不可用时提供用户可用同步开关。
- 在发布级目标部署证据不足时把同步状态显示为生产可用。
- 把 OIDC / Radish 产品账号登录提前做成 Phase 4 的前置条件。

## 数据可见性

管理端可以展示的数据：

- 本地用户词条、权重摘要和删除 tombstone 的用户可读视图。
- 导入批次结果、导入错误分类和被跳过条目计数。
- 学习状态摘要、候选 explain 摘要和 sync preflight 摘要。
- 服务端地址、连接状态、HTTP 错误分类和认证缺失状态。
- 设备 ID、backend id、backend capability、production gate 状态和不可用原因。
- 最近同步对象数量、对象类型、版本号、上传 / 下载时间和错误分类。
- settings draft 中的非 secret 草案字段和部署证据来源 allowlist 标签。
- 脱敏诊断摘要字段索引、状态来源、停止线、聚合计数和脱敏策略。

管理端不得展示或持久化的数据：

- P0 数据。
- P1 原始事件明细。
- 明文同步 payload。
- encrypted payload bytes、signature bytes、wrapped key bytes、recovery wrapped material bytes。
- 恢复码明文、KDF 输出、同步主密钥、设备私钥和 bearer token。
- Go server 内部 `blob_ref`、本机真实绝对部署路径、真实 token、证书私钥或用户账号 secret。

如果管理端需要导出诊断信息，默认只能导出非敏感摘要；任何包含本地词条的导出都必须是用户显式触发，并清楚标注导出内容。

## 用户可见行为约束

- 词库导入必须先展示导入检查摘要，再由用户确认写入；普通导入不得复活 tombstone，dry run 不写入 userdb。
- 词库导出只导出用户显式请求的 P2 用户词条视图，不作为诊断报告的一部分混入。
- 学习页、同步页和诊断报告只能展示聚合计数、状态码、来源标签和解释性摘要，不展示 P1 原始事件或明文同步 payload。
- 设置页保存的是本地草案；`retain_sync_config`、`server_endpoint`、`privacy_mode`、`diagnostics_export` 和部署证据来源只用于派生 UI 状态，不启用真实上传。
- 诊断报告预览的 section 筛选和关键字筛选只影响当前对话框字段列表，不改变 `ManagerDiagnosticsReport` 数据模型、脱敏文本、剪贴板内容或导出内容。
- 复制诊断摘要必须复制完整脱敏文本；导出诊断摘要必须保持同一份脱敏摘要语义，不因当前筛选状态输出字段子集。

## FFI 与 Bridge 边界

管理端应通过 `ime-ffi` 或后续受控 bridge 调用 Rust 能力，不直接读写 Rust 内部结构。

当前 Flutter 工程已抽出 `ManagerBridge`，UI 只依赖 snapshot 加载、词条删除、词库导入检查、词库导入、词库导出、设置草案保存、诊断报告预览和诊断报告导出这组受控方法。现有 `FixtureManagerBridge` 只使用合成数据验证调用边界、UI 状态更新、词库搜索 / 空态、词条 key / source / import batch / tombstone / sync 分类审计详情、词库页导入历史筛选 / 排序 / 批次联动审计、本地 sync preflight 影响摘要、学习状态聚合摘要、rank explain 筛选和候选贡献项详情、同步页 gate 状态来源 / 本地 P2 对象分类 / 设备 backend 门禁审计、设置页 gate 草案预览、部署证据来源标签、诊断报告字段分组 / 筛选 / 脱敏文本复制、诊断报告 gate source / stop line、删除确认、导入检查对话框、导入 / 导出结果反馈、操作失败分类提示、设置草案和脱敏诊断报告；真实 Dart FFI bridge 已覆盖本地 userdb list / delete、dictionary inspect / import / export、import batches、learning status、rank explain、sync preflight 摘要、非 secret settings JSON 草案持久化和脱敏诊断报告导出。Dart 绑定层必须复制 Rust view 后释放 handle，不得把 Rust 内部指针、未脱敏错误字符串或明文同步 payload 透传给 widget 层；widget 层只展示结构化错误码、错误分类和非敏感配置来源诊断。

settings JSON schema、部署证据来源 allowlist、诊断报告字段索引和脱敏规则见 `docs/manager-settings-diagnostics.md`。

第一批可依赖的接口方向：

- session / dictionary handle 的创建与释放。
- userdb 词条 list / add / delete。
- dictionary inspect / import / export。
- import batches 只读查询。
- learning status 只读摘要。
- rank explain 只读摘要。
- sync preflight 状态摘要。
- settings draft 保存和读取。
- backend capability / production gate 状态摘要。

后续同步 UI 需要新增 bridge 时，应遵循：

- bridge 入参不接受明文同步 payload。
- bridge 返回值不包含 P1 原始事件、wrapped material bytes、signature bytes 或 token。
- 错误必须结构化，至少区分配置缺失、认证失败、平台 backend 不可用、目标部署证据不足、网络不可达、版本冲突和本地数据不一致。
- Rust 错误对象、字符串 buffer、handle 释放和 owner-thread 规则继续遵守 `docs/ffi-boundary.md` 与 `docs/runbooks/ffi-platform-call-contract.md`。

## 同步 UI 状态模型

管理端同步相关 UI 至少应区分以下状态：

- `local_only`：仅本地管理，未配置自部署服务端。
- `preflight_ready`：本地 P2 对象、加密组装、local smoke 来源和 sync preflight 通过，但未启用真实远端。
- `server_configured`：已配置服务端地址和访问 token，但未完成生产条件验证。
- `backend_unavailable`：平台私钥 backend 不可用于生产签名。
- `deployment_unverified`：发布级目标部署运行证据不足，或当前只有本地 `local_smoke`。
- `sync_disabled_by_policy`：隐私模式、用户禁用或策略要求停止同步。
- `ready_for_user_sync`：平台私钥 backend 可用，发布级部署证据、恢复码和设备授权链路齐备，用户明确开启同步。

在 `ready_for_user_sync` 前，UI 可以展示配置检查和不可用原因，但不能提供会把本地 P2 数据上传到用户真实远端的主操作。

设置草案中的目标部署证据只允许保存非敏感来源标签，例如本机 smoke、外部 TLS、备份恢复或升级回滚演练；不得保存日志正文、证书、token、恢复码、路径、请求 / 响应体、payload bytes 或其他运行输出。字段级参考见 `docs/manager-settings-diagnostics.md`。

## 恢复码与设备授权

恢复码、设备授权、设备撤销和真实同步入口状态进入产品实现前，必须先遵守 `docs/manager-sync-entry-boundary.md` 中的进入条件、bridge 边界、诊断脱敏和测试计划。

当前 Flutter manager 已能展示恢复码准备态、设备授权准备态和 join request 状态的只读摘要，默认状态为 `recovery_code_flow_closed`、`device_authorization_flow_closed` 和 `join_request_unavailable`。这些状态只用于解释阻塞和诊断脱敏，不提供恢复码生成 / 输入、join request 创建、授权成功、设备撤销或真实同步上传入口。

恢复码 UI 必须等到以下条件同时满足：

- 可用平台私钥 backend 已通过真实平台验证。
- 恢复记录创建、轮换、撤销和读取已有端到端验证。
- 管理端已实现恢复码只显示一次、用户确认保存和日志脱敏约束。
- 失败原因按非敏感分类展示，不输出恢复码、KDF 输出或 wrapped material bytes。

设备授权 UI 必须等到以下条件同时满足：

- 设备签名和授权包验证链路已稳定。
- 设备列表、join request 列表和 authorization handler 的错误语义已映射到 UI。
- 撤销设备后 UI 能解释历史对象无法被技术上追回的限制。
- key epoch、撤销状态和后续对象签名门禁能被用户理解和复验。

## 日志、截图与测试数据

管理端日志允许记录：

- 页面名称、操作类型、非敏感错误码、对象类型、对象数量、时间和耗时。
- backend id、capability status、production gate status。
- 服务端 host 的用户可见配置摘要。

管理端日志禁止记录：

- 用户输入历史、候选明细、联系人、窗口标题、恢复码、token、私钥、签名 bytes、wrapped material bytes、encrypted payload bytes。
- 导入文件原文、导出文件内容或完整 userdb dump。
- Go server 请求体、响应体或内部 `blob_ref`。

测试 fixture 应使用合成词、虚构设备、虚构服务端地址和合成错误码。截图测试不得包含真实用户词、真实账号、真实 token、真实域名证书细节或真实设备序列号。

## Phase 4 起步顺序

1. 已固定本文档，并同步路线图、技术计划、仓库结构和周志。
2. 已创建 `apps/radishlex-manager/` Flutter macOS 工程骨架，当前通过 `ManagerBridge` contract 接入合成 fixture。
3. 已验证词库搜索 / 空态、词条审计详情、导入历史筛选 / 排序 / 批次联动审计、本地 sync preflight 影响摘要、学习状态聚合摘要、rank explain 筛选和候选贡献项详情、同步页 gate 状态来源 / 本地 P2 对象分类 / 设备 backend 门禁审计、设置页 gate 草案预览、部署证据来源标签、诊断报告 gate source / stop line、词条删除确认、词库导入检查、词库导入后刷新、词库导入 / 导出结果摘要和操作失败分类提示经由 fixture bridge 完成受控调用。
4. 已补第一批真实 Dart FFI bridge：显式配置本地 SQLite userdb 与 `ime-ffi` 动态库后，可接入 userdb 词条 list / delete、用户词库 inspect / import / export、import batches、learning status 摘要、rank explain 摘要和 sync preflight 摘要。
5. 已新增 `scripts/check-manager-ffi-smoke.sh`，构建 `radishlex-ime-ffi` 动态库并使用临时 SQLite userdb、合成 TSV 和导出文件复验真实 Dart FFI bridge 的本地 list / delete / import / export、import batches、learning status、rank explain 和 sync preflight 摘要。
6. `rank explain` 区域已通过专用 `ime-ffi` ABI 读取单候选贡献项，Flutter 只展示复制后的非敏感摘要，不持有 Rust view 指针。
7. 已补设置页配置来源诊断、sync gate 草案预览、部署证据来源标签、设置草案保存、脱敏诊断报告分组预览 / 筛选 / 复制 / 导出和 bridge 失败结构化错误分类展示；UI 不透传 native 错误明细。
8. 同步配置页继续保持真实上传按钮禁用，状态由设置草案、隐私模式、平台私钥 backend gate 和部署证据来源草案派生，可显示 `local_only`、`sync_disabled_by_policy`、`backend_unavailable`、`deployment_unverified` 或 `preflight_ready`。
9. 下一步可以接 sync entry state helper / UI gate 的非上传实现；待可用平台私钥 backend、恢复 / 授权实现测试和发布级部署证据齐备后，再接设备授权成功路径、恢复码和用户可用同步。

## 停止线

- 没有可用平台私钥 backend 前，不提供用户可用同步开关、恢复码创建 UI 或设备授权成功路径。
- 没有发布级目标部署运行证据前，不把远端同步展示为生产可用；本地 Docker / 本地 HTTPS 联调状态可以作为非生产证据展示。
- 没有 FFI / bridge 明确错误语义前，不让 Flutter 直接解析 Rust 内部错误字符串。
- 任何会展示、记录、上传或导出 P0、P1 原始事件、恢复码、token、私钥或明文同步 payload 的设计都必须停止并回退。
- 如果 UI 需要新增 Go server API，必须先更新 `docs/sync-server-api-storage.md` 或对应 ADR，不能让管理端绕过现有 encrypted object / metadata 边界。
