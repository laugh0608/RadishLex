# RadishLex 技术方案

本文档是 RadishLex 当前技术方向的入口摘要，读者是需要快速判断架构边界、阶段顺序和后续开发重点的维护者与协作者。本文不包含完整 trait 字段、SQLite migration、同步协议细节、平台安装流程、长期推演或验证流水；这些内容应放在对应专题文档、runbook 或 devlog 中。

## 当前阶段

RadishLex 当前处于 Phase 2 起步阶段：

- `ime-core` 已建立平台无关输入会话、候选模型、提交模型和 engine trait。
- `ime-engine-rime` 已接入真实 `librime` adapter，并通过本机隔离 Rime smoke 复验 `compose -> candidates -> commit`，同时在 `native-rime` feature 测试中覆盖必需 Rime API 缺失映射。
- `ime-userdb` 已落地本地 SQLite 用户词库、选择事件、负反馈、删除 tombstone、用户词库导入导出、同步前置计数和 `dictionary.user_terms` / `ranker.weights` / `dictionary.deleted_terms` P2 plaintext payload 只读迭代器，并通过 `ime-sync::SyncEnvelopeAssembler` 接入本地加密 envelope 装配链路。
- `ime-ranker` 已提供可解释候选重排。
- `ime-sync` 已提供同步 payload 来源分类、P2 envelope 组装边界、加密对象外壳草案、同步域、设备状态、加入请求、授权包、撤销记录、对象版本冲突草案模型和客户端解密后合并模型，并可从 `ime-crypto` envelope 派生上传草案元数据；不连接后端、不实现网络同步。
- `ime-crypto` 已落地本地加密 crate，覆盖 XChaCha20Poly1305、HKDF-SHA256、SHA-256 ciphertext hash、Argon2id recovery KDF、key role、object envelope、AAD 绑定、nonce 重复检测、篡改失败、device key descriptor、device wrapping key / record、recovery material，以及 userdb P2 payload 本地加密 / 解密 / sync draft 派生测试；签名、真实设备密钥存储和生产恢复流程尚未落地。
- `docs/sync-key-management.md` 已固定真实同步前的同步密钥、设备授权、恢复码、设备撤销、key epoch、服务端可见元数据和冲突边界；`docs/adr/0002-recovery-code-kdf.md` 已固定恢复码 Argon2id KDF、格式、恢复记录字段和验证口径。
- `ime-ffi` 已提供 C ABI 起步验证，覆盖 ABI contract、opaque handle、session owner-thread policy、session options、Rime session options、默认 unavailable 门禁、`native-rime` feature 下真实 Rime session smoke、engine kind 门禁、错误对象、UTF-8 buffer、结构化 snapshot / candidate view、normalized key event、learning status 只读摘要、sync preflight 状态摘要、userdb add / delete / list、dictionary inspect / export / import、import batches 只读查询、平台绑定式 view copy / release host smoke、释放函数 panic 边界、demo engine host smoke 和 FFI 调用 runbook。
- `radishlex-ime-cli` 已提供 `demo`、`rime`、`dict`、`learn status`、`learn select/suppress`、`rank explain`、`rime --rank-db` 和 `sync preflight` 复验入口。

当前下一步仍在 Rust 本地同步加密前置工作内，重点是补签名 / 设备密钥存储设计，以及合并模型与真实 payload / userdb 写回流程的接线。P1 原始事件、本地审计批次和 FFI 明文 payload 继续不得进入同步路径；现阶段不推进平台壳、Go 同步后端或 Flutter manager 主线。

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

真实远端同步不是当前开发主线，但架构需要提前保持边界：

- 输入热路径不得依赖后端。
- 服务端只看到设备 ID、加密对象 ID、密文 blob 大小、对象版本、更新时间和必要同步元数据。
- 新设备加入必须通过已有设备授权或恢复码。
- 单台设备丢失后应允许撤销设备，并在后续对象上轮换同步密钥。
- 冲突合并应按对象类型处理：用户词按词合并，删除使用 tombstone，设置项可 last-write-wins 或显式提示。

当前 `ime-userdb` 可导出 `dictionary.user_terms`、`ranker.weights` 和 `dictionary.deleted_terms` 的 Rust 内部 P2 plaintext payload bytes，并已在测试中通过 `ime-sync::SyncEnvelopeAssembler` 接入 `ime-crypto` envelope 加密、解密和 `ime-sync::EncryptedSyncObjectDraft` 派生。`ranker.weights` 只来自 P1 本地事件压缩后的 P2 权重摘要，不包含原始 selection event、负反馈明细、上下文统计或本地审计批次。`ime-sync` 定义 payload 来源分类、同步对象类型、加密对象外壳校验、P2 envelope 组装边界、设备生命周期、对象版本冲突草案模型和客户端解密后合并模型。它们都不实现网络客户端、签名、生产恢复码、真实 payload 解析或 userdb 写回。

`docs/sync-key-management.md` 已补同步密钥与设备生命周期设计，当前 Rust 侧已落 key epoch、device wrapping、加入请求、授权包、撤销记录、恢复材料模型、恢复码 KDF 模型、P2 envelope 组装边界和客户端合并模型，并覆盖撤销后旧 epoch key 不能解密新对象、授权设备和接收设备都必须 active、版本冲突检测边界、恢复码校验 / KDF / AAD 失败、删除 tombstone 压过旧 user terms / ranker weights、旧 epoch 上传不能复活删除词和显式恢复语义。后续应继续补签名 / 设备密钥存储和真实 payload 接线，再进入 Go server API 设计。

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
- 真实 P2 payload / userdb 写回接线、签名 / 设备密钥存储和生产恢复流程未稳定前，不进入 Go server、远端同步或管理 UI 同步主线。

## 专题文档索引

- [Engine Boundary](engine-boundary.md)：engine trait、核心模型、adapter 职责、错误语义和 clean-room 边界。
- [ime-engine-rime Adapter 设计](engine-rime-adapter.md)：Rime adapter 构建、FFI 生命周期、数据目录和 native smoke。
- [个人化学习设计](personalization-learning.md)：userdb、ranker、学习事件、负反馈、删除 tombstone、导入导出和 CLI 管理入口。
- [隐私与同步设计](privacy-sync.md)：P0/P1/P2/P3 分级、加密对象、设备授权、删除语义和威胁模型。
- [同步 Payload 草案](sync-payload.md)：同步对象类型、P1/P2 来源分类、加密对象外壳和验证口径。
- [ime-crypto 边界设计](crypto-boundary.md)：客户端加密、密钥、envelope、删除同步和验证边界。
- [同步密钥与设备生命周期设计](sync-key-management.md)：同步密钥、设备授权、恢复码、设备撤销、key epoch 和冲突边界。
- [ADR 0002: 恢复码 KDF 与同步域恢复边界](adr/0002-recovery-code-kdf.md)：恢复码格式、Argon2id KDF 参数、恢复记录字段和生产实现验证口径。
- [FFI 边界](ffi-boundary.md)：C ABI 职责、所有权、生命周期、错误语义和平台壳停止线。
- [仓库结构草案](repository-layout.md)：crate、server、app、platform、scripts 和 tests 职责。
- [阶段路线图](roadmap.md)：Phase 0 到 Phase 7 的交付物和退出标准。
- [CLI 说明](cli.md)：当前可运行命令、输出字段、错误语义和安全边界。
- [Rime Native Smoke Runbook](runbooks/rime-native-smoke.md)：本机隔离 `librime` smoke 和 rank smoke 操作步骤。
