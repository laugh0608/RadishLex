# RadishLex 技术方案

本文档是 RadishLex 当前技术方向的入口摘要，读者是需要快速判断架构边界、阶段顺序和后续开发重点的维护者与协作者。本文不包含完整 trait 字段、SQLite migration、同步协议细节、平台安装流程、长期推演或验证流水；这些内容应放在对应专题文档、runbook 或 devlog 中。

## 当前阶段

RadishLex 当前处于 Phase 3 自部署同步起步阶段：

- `ime-core` 已建立平台无关输入会话、候选模型、提交模型和 engine trait。
- `ime-engine-rime` 已接入真实 `librime` adapter，并通过本机隔离 Rime smoke 复验 `compose -> candidates -> commit`，同时在 `native-rime` feature 测试中覆盖必需 Rime API 缺失映射。
- `ime-userdb` 已落地本地 SQLite 用户词库、选择事件、负反馈、删除 tombstone、用户词库导入导出、同步前置计数、`dictionary.user_terms` / `ranker.weights` / `dictionary.deleted_terms` P2 plaintext payload 只读迭代器、解密后 P2 JSON 到 merge input 的解析入口，以及合并结果写回真实 userdb 的事务执行器，并通过 `ime-sync::SyncEnvelopeAssembler` 接入本地加密 envelope 装配链路；两客户端 integration test 已覆盖 A 端加密上传、B 端下载解密、合并写回、stale conflict 和 v2 重新上传，真实 Go HTTP 两客户端测试已覆盖设备授权、三类 P2 对象上传下载、客户端解密写回和 v2 重新上传。
- `ime-ranker` 已提供可解释候选重排。
- `ime-sync` 已提供同步 payload 来源分类、P2 envelope 组装边界、加密对象外壳草案、同步域、设备状态、加入请求、授权包、撤销记录、对象版本冲突草案模型、客户端解密后合并模型、设备授权签名、设备撤销签名模型、远端对象版本 client 边界和 std-only `http://` HTTP transport；当前 remote client 只接收已加密 object 与 signed manifest，验证 JSON / base64 DTO、密文对象上传下载路径、stale conflict latest metadata 映射、`unauthenticated` 错误映射、可选 bearer access token header、真实 HTTP request / response 传递和错误脱敏，并已被 userdb 两客户端 harness 与真实 Go HTTP 两客户端测试复用；短生命周期 integration test 已覆盖 Rust HTTP transport 直连 Go server，不启动长期运行 server。
- `ime-crypto` 已落地本地加密 crate，覆盖 XChaCha20Poly1305、HKDF-SHA256、SHA-256 ciphertext hash、Argon2id recovery KDF、Ed25519 设备签名、test-memory signing key store、platform backend id / capability metadata、unavailable backend 明确失败、key 撤销后阻断签名 / 导出、key role、object envelope、AAD 绑定、nonce 重复检测、篡改失败、device key descriptor、device wrapping key / record、recovery material、signed sync object manifest、signed recovery record，以及 userdb P2 payload 本地加密 / 解密 / sync draft 派生测试；`apple-keychain-v1` 平台 backend runbook 已固定，macOS Keychain backend 已在 `apple-keychain` feature 下接线并编译验证，真实 Keychain smoke 已执行但阻塞于 `ed25519-v1` 创建；Apple 平台签名策略 ADR 已固定保留 `ed25519-v1` 协议、不把 seed 存储 fallback 混入 `apple-keychain-v1`，并让该 backend status 在 smoke 通过前阻断生产签名；`android-keystore-v1` 平台 runbook、`android-keystore` feature、不可用状态门禁、Rust bridge wrapper、bridge contract、合成 bridge 单测、ignored smoke 入口、仓库内 Kotlin bridge source、Android Gradle library harness、`@JvmStatic` facade、gated instrumented smoke 和 smoke 记录模板已固定，当前已补 Rust raw JNI glue；本机未安装 Android Rust target，尚未运行 Android target build；Android Gradle harness 已在 Pixel 9 Pro API 35 AVD 上执行真实 smoke，结果为 `unsupported_signature_algorithm`，不解除生产签名门禁。
- `server/sync-server` 已起步 Go module，覆盖配置默认值、API request / response / error DTO、storage interface、SQLite metadata migration 文本、storage conformance tests、内存 metadata store、SQLite-backed metadata repository、local object storage staged transaction、metadata transaction 与 blob transaction 接线、Ed25519 签名验证抽象、签名篡改拒绝测试、device wrapping encrypted key bytes 承载 / 读取测试、recovery wrapped material 读取测试、recovery latest handler、domain / device / join request metadata handler、authorization handler、encrypted object version 上传 / metadata 读取 / payload 下载 handler、单用户 bearer access token 门禁、request id、panic recovery、非持久审计 hook、SQLite audit_events 写入、`cmd/radishlex-sync-server`、runtime 配置装配、HTTP timeout、对象大小门禁、脱敏 audit logger、本机 smoke runbook、短生命周期 HTTP smoke、短生命周期备份恢复 smoke、短生命周期外部 TLS 反代 smoke、短生命周期升级回滚 smoke、Dockerfile、Docker Compose 本地 / 部署态入口、Compose runbook 和生产部署边界 runbook；本地 compose 通过 Caddy internal TLS 提供 `https://localhost:7319`，部署态 compose 只暴露 HTTP 上游 `http://127.0.0.1:7319`，两者使用同一个对外端口，并提供 Nginx 外部 TLS 终止示例。当前验证服务端可见 metadata、设备状态、签名、版本冲突、Rust envelope hash / 长度、blob ref 路径安全、staged write / commit / cleanup、启动装配、运行复验路径、访问 token 失败响应和错误语义，并已覆盖第二设备授权后的跨设备 object 版本链、Rust HTTP transport 直连 Go server 的跨语言对象上传 / 下载 / stale conflict、Rust userdb 两客户端真实 Go HTTP 同步、Docker Compose 本地 HTTPS / 部署态 HTTP 容器实际启动 smoke、SQLite metadata + encrypted blob dir 成对备份恢复后的 domain / device / recovery latest / object payload / stale conflict 复验、HTTPS client -> TLS reverse proxy -> HTTP upstream 下的 bearer header 透传、TLS 版本、对象上传下载、Go 对象大小门禁和日志脱敏，以及短生命周期升级 / 回滚演练中的 idempotent migration 重启、升级后 v2 写入、恢复升级前备份后 v2 不可见、v1 payload / stale conflict 和日志脱敏。
- `docs/sync-key-management.md` 已固定真实同步前的同步密钥、设备授权、恢复码、设备撤销、key epoch、服务端可见元数据和冲突边界；`docs/sync-server-api-storage.md` 已固定 Go sync server API、SQLite metadata、对象存储、版本冲突、恢复 / 撤销记录、错误语义和验证口径；`docs/production-recovery-flow.md` 已固定生产恢复流程、恢复记录轮换 / 撤销、新设备恢复加入、失败限速和停止线；`docs/sync-server-oidc-roadmap.md` 已固定 OIDC / Radish 产品账号体系为后续专题，当前不改变单用户 bearer access token 门禁；`docs/adr/0002-recovery-code-kdf.md` 已固定恢复码 Argon2id KDF、格式、恢复记录字段和验证口径；`docs/adr/0003-device-signing-key-storage.md` 已固定 Ed25519 设备签名、签名对象、私钥存储抽象和验证口径；`docs/adr/0004-platform-private-key-storage-backend.md` 已固定平台私钥存储 backend、capability metadata、错误语义和停止线；`docs/adr/0005-apple-platform-signing-strategy.md` 已固定 Apple 平台签名策略；`docs/runbooks/apple-keychain-signing-backend.md` 已固定 Apple Keychain backend 首个平台验证边界；`docs/runbooks/android-keystore-signing-backend.md` 已固定 Android Keystore backend 验证边界。
- `ime-ffi` 已提供 C ABI 起步验证，覆盖 ABI contract、opaque handle、session owner-thread policy、session options、Rime session options、默认 unavailable 门禁、`native-rime` feature 下真实 Rime session smoke、engine kind 门禁、错误对象、UTF-8 buffer、结构化 snapshot / candidate view、normalized key event、learning status 只读摘要、sync preflight 状态摘要、userdb add / delete / list、dictionary inspect / export / import、import batches 只读查询、平台绑定式 view copy / release host smoke、释放函数 panic 边界、demo engine host smoke 和 FFI 调用 runbook。
- `radishlex-ime-cli` 已提供 `demo`、`rime`、`dict`、`learn status`、`learn select/suppress`、`rank explain`、`rime --rank-db` 和 `sync preflight` 复验入口。

当前下一步仍在同步服务端前置治理内。encrypted object 上传下载、版本冲突 HTTP 语义、runtime 配置装配、对象大小门禁、脱敏 audit logger、本机 smoke runbook、双设备 HTTP smoke、Docker Compose 本地 / 部署态入口、Docker Compose 容器实际启动 smoke、Rust remote client DTO / transport trait、std-only `http://` HTTP transport、Rust HTTP transport 直连 Go server 的短生命周期跨语言测试、Rust 侧两客户端 userdb harness、Rust userdb 两客户端真实 Go HTTP 同步测试、生产部署边界 runbook、单用户 bearer access token 门禁、备份恢复演练运行证据、外部 TLS 反代实现级验证证据、升级回滚演练运行证据、OIDC 未来接入规划、`apple-keychain-v1` 平台 backend runbook、Apple 平台签名策略 ADR、`android-keystore-v1` 平台 backend runbook、`android-keystore` feature 门禁、Android Rust bridge wrapper、bridge contract、仓库内 Kotlin bridge source、Android Gradle library harness、gated instrumented smoke 和 feature-gated macOS backend 编译验证已经补齐；真实 Apple Keychain smoke 已执行但未通过，backend status 已阻断生产签名，Android Keystore 已接不可用状态门禁、bridge contract、合成 bridge 单测、ignored smoke 入口、Kotlin source、Gradle harness 和 gated smoke，并已补 Rust raw JNI glue；本机未安装 Android Rust target，尚未运行 Android target build；Android Gradle harness 已在 Pixel 9 Pro API 35 AVD 上执行真实 smoke，结果为 `unsupported_signature_algorithm`，不解除生产签名门禁。后续继续推进时，优先补 Android target build 证据，并按真实设备 / API / provider 矩阵调查原生非导出 Ed25519 支持；目标部署的真实证书、域名、反向代理配置、目标数据目录备份恢复和升级回滚仍需部署者人工复验；Apple 原生非导出 Ed25519 支持矩阵应单独作为平台 spike；OIDC 不进入当前核心实现，接入前应先补认证策略 ADR 并把 Go handler 认证层收敛为可插拔接口。P1 原始事件、本地审计批次和 FFI 明文 payload 继续不得进入同步路径；现阶段不推进完整平台壳、Flutter manager 主线，也不启动长期运行 server 做客户端上传下载。

## 设计原则

- 本地优先：输入热路径、候选生成、候选重排和学习必须离线可用。
- 隐私优先：服务端默认不可信，不保存明文输入历史、明文用户词库、明文候选偏好或明文上下文片段。
- 引擎可替换：v1 可接 `librime`，但 Rust core 不依赖 Rime 私有对象或内部评分。
- 平台薄壳：平台端只处理系统输入法生命周期、按键接收、候选窗展示和文本提交。
- 可解释学习：用户能查看输入法学到了什么，并能删除、导出、暂停或限制学习。
- 自部署同步：后端只做设备管理、密文 blob 存储、版本历史、备份恢复和包分发。

## 总体架构

```text
Platform IME Shell
  Windows TSF / macOS InputMethodKit / Linux Fcitx5
  Android InputMethodService / iOS Keyboard Extension
        |
        v
Rust Core
  ime-core      input session, composition, candidates, commit
  ime-ranker    rerank and explain
  ime-userdb    local dictionary, learning events, tombstones
  ime-sync      sync payload boundary and planned sync client
  ime-crypto    client-side encryption boundary
  ime-ffi       C ABI boundary and planned platform bridge
        |
        v
Engine Adapter
  ime-engine-rime in v1
  native Rust engine in later phases
        |
        v
Local Storage
  SQLite userdb
  encrypted profile data
  local schema/model packages

Management UI
  Flutter desktop/mobile manager
        |
        v
Go Self-host Backend
  device registry
  encrypted blob storage
  version history
  backup / restore
  package distribution
```

## 组件职责

### Rust Core

Rust 是跨端复用和输入热路径的核心层，负责：

- 输入会话状态机、composition、candidate 和 commit 模型。
- Engine trait 与底层 engine adapter 边界。
- 用户词库、选择事件、负反馈、删除 tombstone。
- 候选重排、排序 explain 和后续学习摘要。
- 后续同步客户端、端到端加密和 FFI 边界。
- CLI / smoke 工具。

Rust core 不负责注册系统输入法、强行统一系统候选窗 UI、托管云端实时转换，或在 v1 阶段从零实现完整中文输入引擎。

### Engine Adapter

`ime-engine-rime` 负责把 `librime` 的输入、候选、composition 和 commit 转换为 RadishLex 稳定模型。Rime 相关概念必须停留在 adapter 内部，不向 `ime-core`、`ime-userdb`、`ime-ranker` 或平台壳泄漏。

v1 采用 `librime` 作为成熟底层引擎。长期可以加入 Rust 自研 engine，但不能抢占当前 Rust core、userdb、ranker、同步前置验证和真实平台落地的优先级。

### ime-userdb 与 ime-ranker

`ime-userdb` 保存本地学习数据和用户可管理词条。当前已覆盖：

- `user_terms`
- `selection_events`
- `negative_feedback`
- `deleted_terms`
- `ranker_weights`
- `import_batches`

`ime-ranker` 只消费 RadishLex candidate、userdb summary 和 deleted tombstone summary，不访问 SQLite、不访问 Rime、不读取平台私有生命周期。排序结果必须输出 explain，说明 engine 顺序、用户词提升、频次、近期、上下文、负反馈、suppressed 和 deleted 的贡献。

### Go Backend

Go 后端属于后续阶段。它只负责：

- 用户和设备注册。
- 设备公钥登记。
- 加密 blob 存储。
- 同步版本号、冲突检测和版本历史。
- 备份恢复、审计日志和包分发。
- 默认单用户 SQLite 自部署模式。

Go 后端不参与每次按键，不做候选排序，不做云端实时转换，不保存明文输入历史或明文用户词库。

### Flutter Manager

Flutter manager 属于后续阶段。它负责设置、词库管理、学习记录可视化、隐私控制台、同步状态、设备管理、后端连接和备份恢复。

Flutter 不进入输入热路径，不作为系统候选窗的跨平台统一实现，也不承担用户词库、同步、排序或隐私策略真相源。

### Platform Shells

平台壳只负责系统输入法接入：

- Windows：TSF 薄壳，后置。
- macOS：InputMethodKit，第一批桌面候选平台。
- Linux：优先 Fcitx5，其次 IBus，适合作为第一批真实平台。
- Android：Kotlin `InputMethodService`，移动端首选。
- iOS：Swift / UIKit Keyboard Extension，默认离线，同步依赖 full access，后置。

候选窗优先使用平台原生机制，不强行统一 Windows、macOS、Linux、Android 和 iOS 的候选窗 UI。

## 隐私与数据分级

RadishLex 按 `docs/privacy-sync.md` 的数据分级推进：

- P0：密码框、支付、证件、secure text entry、隐私模式输入，永不学习、永不同步。
- P1：原始选择事件、负反馈详细事件、应用上下文统计，默认只本地学习。
- P2：用户词库、候选权重摘要、自定义短语、输入方案配置，后续只能端到端加密同步。
- P3：官方词库包、输入方案模板、模型包、UI 主题，可公开下载。

删除语义必须强于普通降权。被删除词条需要 tombstone 或等价语义，避免旧选择事件、旧导入、旧设备或旧备份复活。

## 同步方向

用户可用远端同步不是当前开发主线，但架构需要提前保持边界：

- 输入热路径不得依赖后端。
- 服务端只看到设备 ID、加密对象 ID、密文 blob 大小、对象版本、更新时间和必要同步元数据。
- 新设备加入必须通过已有设备授权或恢复码。
- 单台设备丢失后应允许撤销设备，并在后续对象上轮换同步密钥。
- 冲突合并应按对象类型处理：用户词按词合并，删除使用 tombstone，设置项可 last-write-wins 或显式提示。

当前 `ime-userdb` 可导出 `dictionary.user_terms`、`ranker.weights` 和 `dictionary.deleted_terms` 的 Rust 内部 P2 plaintext payload bytes，并已在测试中通过 `ime-sync::SyncEnvelopeAssembler` 接入 `ime-crypto` envelope 加密、解密和 `ime-sync::EncryptedSyncObjectDraft` 派生。`ranker.weights` 只来自 P1 本地事件压缩后的 P2 权重摘要，不包含原始 selection event、负反馈明细、上下文统计或本地审计批次。`ime-sync` 定义 payload 来源分类、同步对象类型、加密对象外壳校验、P2 envelope 组装边界、设备生命周期、对象版本冲突草案模型、客户端解密后合并模型、远端对象版本 client 边界和 std-only `http://` HTTP transport；`ime-userdb` 已能把已解密 P2 JSON 解析为该合并模型需要的记录，并把被接受的 user terms、deleted tombstones 和 ranker weights 写回本地 SQLite。当前 remote client 只接收 `AssembledSyncObject` 与 `SignedSyncObjectManifest`，通过 transport trait 和 HTTP transport 验证服务端 JSON / base64 DTO、metadata 读取、binary payload 下载和错误映射；Rust userdb 两客户端真实 Go HTTP 测试已复验设备授权、三类 P2 对象上传下载、客户端解密写回、stale conflict 和 v2 重新上传。不提供 plaintext 上传入口，也不提供生产恢复 UI / API 或平台私钥存储 backend 实现。

`docs/sync-key-management.md` 已补同步密钥与设备生命周期设计，当前 Rust 侧已落 key epoch、device wrapping、加入请求、授权包、撤销记录、恢复材料模型、恢复码 KDF 模型、P2 envelope 组装边界、客户端合并模型、userdb 写回执行器、平台私钥存储 backend capability / unavailable 模型、feature-gated macOS Keychain backend、feature-gated Android Keystore 不可用门禁、Rust bridge wrapper 和 bridge contract、remote client DTO / transport trait、HTTP transport、两客户端 userdb harness 和真实 Go HTTP 两客户端测试，并覆盖撤销后旧 epoch key 不能解密新对象、授权设备和接收设备都必须 active、版本冲突检测边界、恢复码校验 / KDF / AAD 失败、删除 tombstone 压过旧 user terms / ranker weights、旧 epoch 上传不能复活删除词、显式恢复语义、测试 backend 不能用于生产签名、unavailable backend 不回退、Apple Keychain 默认能力不声明硬件保护、Android Keystore 未验证前不声明可用、Android bridge request / error code / response 校验、Android 合成 bridge 创建 / 签名 / 删除语义、远端上传请求不包含 plaintext 字段、stale conflict latest metadata 映射、payload length mismatch 拒绝、HTTP transport 错误脱敏、bearer access token header 脱敏和客户端解密写回边界。签名 / 设备密钥存储边界、生产恢复流程、Go server API / storage 边界、生产部署边界、Apple Keychain backend 平台验证边界、Apple 平台签名策略和 Android Keystore backend 验证边界已由专题文档固定；`platforms/android-ime/keystore-bridge` 已补 Kotlin / Gradle harness、`@JvmStatic` facade、gated instrumented smoke 和 smoke 记录模板。Go 侧已起步 metadata / storage / API 验证模型，并已补 SQLite-backed metadata repository、local blob transaction、签名验签、device wrapping 密文承载、recovery 密文读取、recovery latest handler、domain / device / join request metadata handler、authorization handler、encrypted object version handler、bearer access token 门禁、request id、panic recovery、非持久审计 hook、SQLite audit_events 写入、runtime 装配、脱敏日志、双设备 HTTP smoke、Rust HTTP transport 直连 Go server 的短生命周期跨语言测试、Rust userdb 两客户端真实 Go HTTP 测试、Docker Compose 本地 / 部署态入口、容器实际启动 smoke、备份恢复 smoke、外部 TLS 反代 smoke 和升级回滚 smoke；后续代码应继续保持不接触 plaintext payload，并在进入真实用户部署或用户可用同步前运行 Android target build，或在获得明确授权后运行真实 API / 设备矩阵 smoke，或补目标部署运行证据。

## Clean-room 原则

外部输入法和底层引擎只作为行为规格、接口约束和测试用例来源，不复制实现。

允许：

- 阅读公开文档。
- 观察公开软件行为。
- 总结输入法交互规格。
- 自己设计数据结构、Rust API 和模块边界。
- 使用兼容许可证的库作为可选 adapter。

禁止：

- 复制源码、私有函数结构或有版权风险的词库。
- 从 GPL / LGPL 项目搬实现进核心层。
- 把外部项目实现细节逐行翻译成 Rust。

## 主要风险

- 输入质量风险：短期内依赖成熟底层引擎，RadishLex 先把个人化、可解释、可删除和同步边界做好。
- 平台集成风险：系统输入法接入复杂，每个平台只写薄壳，并优先 Linux / macOS / Android。
- iOS 限制风险：iOS 后置，默认离线可用，同步需要用户显式开启 full access。
- 隐私信任风险：默认不上传明文，提供本地可视化学习记录、禁学名单、删除和导出能力。
- 文档漂移风险：阶段目标、协议、隐私边界和平台策略变化必须同步更新对应专题文档。

## MVP 成功标准

MVP 至少需要证明：

- CLI 能通过成熟底层 engine 输出真实候选。
- Rime candidates 能进入 ranker，并输出可解释排序。
- 用户选择、删除、负反馈能影响后续排序。
- 用户词库能导入、导出，且普通导入不会复活 deleted tombstone。
- 同步设计能保证服务端不接触明文 P2 数据，且服务端可见 hash 只基于 ciphertext 或 ciphertext + AAD。
- 至少一个桌面或移动平台能作为真实系统输入法使用。

## 当前停止线

- userdb schema、删除语义、导入导出和 ranker explain 未稳定前，不接远端同步。
- FFI 所有权、生命周期、错误语义、字符串编码、线程模型和释放责任未明确前，不推进平台壳。
- Rime native smoke 和学习层复验未稳定前，不推进复杂平台候选窗或管理 UI。
- Apple Keychain 真实 smoke 未通过、目标部署 TLS / 备份 / 升级回滚复验证据没有补齐前，不进入管理 UI 同步主线，也不开放给真实用户同步。Go server 当前只能先按专题文档验证 metadata、storage、签名、版本冲突、encrypted object HTTP handler、bearer access token 门禁、runtime 装配、脱敏日志、server smoke、备份恢复 smoke、外部 TLS 反代 smoke、升级回滚 smoke、Docker Compose 本地 / 部署态入口、容器实际启动 smoke、生产部署边界和错误语义边界；Rust remote client 已验证 DTO / transport trait、std-only `http://` HTTP transport、bearer token header、两客户端 userdb harness、直连 Go server 的短生命周期测试和两客户端真实 Go HTTP 同步；OIDC 接入前必须另补认证策略 ADR，不得把账号登录、refresh token 或 Radish 产品 session 放进 sync server；Apple Keychain backend 目前只完成 feature-gated 接线和非 smoke 测试，真实 smoke 阻塞于 `ed25519-v1` 创建，backend status 明确阻断生产签名。

## 专题文档索引

- [Engine Boundary](engine-boundary.md)：engine trait、核心模型、adapter 职责、错误语义和 clean-room 边界。
- [ime-engine-rime Adapter 设计](engine-rime-adapter.md)：Rime adapter 构建、FFI 生命周期、数据目录和 native smoke。
- [个人化学习设计](personalization-learning.md)：userdb、ranker、学习事件、负反馈、删除 tombstone、导入导出和 CLI 管理入口。
- [隐私与同步设计](privacy-sync.md)：P0/P1/P2/P3 分级、加密对象、设备授权、删除语义和威胁模型。
- [同步 Payload 草案](sync-payload.md)：同步对象类型、P1/P2 来源分类、加密对象外壳和验证口径。
- [ime-crypto 边界设计](crypto-boundary.md)：客户端加密、密钥、envelope、删除同步和验证边界。
- [同步密钥与设备生命周期设计](sync-key-management.md)：同步密钥、设备授权、恢复码、设备撤销、key epoch 和冲突边界。
- [同步服务端 API 与存储边界](sync-server-api-storage.md)：Go sync server API、SQLite metadata、对象存储、版本冲突、恢复 / 撤销记录、错误语义和停止线。
- [Sync Server OIDC 未来接入规划](sync-server-oidc-roadmap.md)：后续接入 Radish 产品账号体系或兼容 OIDC IdP 的身份边界、认证策略演进和停止线。
- [Sync Server Local Smoke Runbook](runbooks/sync-server-local-smoke.md)：Go sync server 本机启动边界、自动化 smoke 和日志脱敏检查。
- [Sync Server Production Deployment Runbook](runbooks/sync-server-production-deployment.md)：部署拓扑、外部 TLS、认证 / 访问控制、备份恢复、升级回滚和真实用户开放停止线。
- [生产恢复流程设计](production-recovery-flow.md)：恢复记录创建 / 轮换 / 撤销、新设备恢复加入、全部设备丢失、失败限速、日志和停止线。
- [ADR 0002: 恢复码 KDF 与同步域恢复边界](adr/0002-recovery-code-kdf.md)：恢复码格式、Argon2id KDF 参数、恢复记录字段和生产实现验证口径。
- [ADR 0003: 设备签名与私钥存储边界](adr/0003-device-signing-key-storage.md)：设备签名、签名对象、私钥存储抽象、错误语义和测试口径。
- [ADR 0004: 平台私钥存储 Backend 边界](adr/0004-platform-private-key-storage-backend.md)：平台 key backend、capability metadata、FFI 边界、错误语义、迁移和停止线。
- [ADR 0005: Apple 平台签名策略](adr/0005-apple-platform-signing-strategy.md)：`apple-keychain-v1` smoke 阻塞后的 Ed25519 协议、Keychain seed fallback 和生产状态门禁决策。
- [Apple Keychain Signing Backend Runbook](runbooks/apple-keychain-signing-backend.md)：`apple-keychain-v1` 创建、加载、签名、删除、锁屏 / 权限、备份迁移和日志脱敏验证边界。
- [Android Keystore Signing Backend Runbook](runbooks/android-keystore-signing-backend.md)：`android-keystore-v1` Ed25519 创建、加载、签名、删除、锁屏 / 权限、备份迁移、IME 生命周期和日志脱敏验证边界。
- [FFI 边界](ffi-boundary.md)：C ABI 职责、所有权、生命周期、错误语义和平台壳停止线。
- [仓库结构草案](repository-layout.md)：crate、server、app、platform、scripts 和 tests 职责。
- [阶段路线图](roadmap.md)：Phase 0 到 Phase 7 的交付物和退出标准。
- [CLI 说明](cli.md)：当前可运行命令、输出字段、错误语义和安全边界。
- [Rime Native Smoke Runbook](runbooks/rime-native-smoke.md)：本机隔离 `librime` smoke 和 rank smoke 操作步骤。
