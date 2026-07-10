# RadishLex 项目稳定化整改总专题（2026-07）

本文档是 RadishLex 2026 年 7 月项目审计后的临时执行总专题，读者是维护者、实现者和审阅者。它只负责记录整改目标、推进顺序、停止线、验收证据和退出条件；不替代总体架构、协议、隐私边界、字段参考、runbook、ADR 或开发周志，也不收纳逐日命令输出和长篇历史推演。

## 文档状态

- 状态：生效，临时执行真相源
- 建立日期：2026-07-10（Asia/Shanghai）
- 审计基线：`dev` 分支，提交 `db8ffa78e354`
- 当前主批次：R01 真实输入纵向链（尚未进入代码实现）
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭
- 关闭方式：全部退出条件满足后，将稳定结论回写现有正式文档，再把本文移入 `docs/archive/` 并从当前状态入口移除

## 一、整改结论

RadishLex 已具备 Rust core、Rime adapter、userdb、ranker、crypto、sync client 边界、Go sync server 和 Flutter manager 原型，默认 Rust、Go、Flutter 验证基线也有较完整覆盖。但当前仍没有一条“系统按键进入 -> 本地候选生成与重排 -> 用户选择 -> 文本提交 -> 个人化学习持久化”的真实平台纵向链路，正常构建的 manager 也没有形成包含 Rust FFI、持久化配置和真实数据源的产品运行态。

本轮整改不以继续增加 readiness、evidence、preview、approval 或“尚未导出 symbol”的证明材料为推进证据。阶段进展重新以用户可见链路、数据正确性、安全边界和可重复验证为准。

整体顺序固定为：

1. 先纠正文档真相源和质量门禁。
2. 再打通第一个真实平台的本地输入纵向链。
3. 随后修正 userdb 与 ranker 的正确性基础。
4. 在此基础上治理同步收敛与密码协议。
5. 再补生产同步编排和 manager 产品化。

## 二、整改目标

本专题必须推动以下结果：

- 至少一个真实平台可以离线完成中文输入、候选选择和文本提交。
- 平台能准确获得按键是否消费、即时 commit 和最新输入快照。
- 用户选择、负反馈、删除和显式恢复具备原子性、一致性和可复验语义。
- 候选重排具备有效 recency、受控 frequency、明确 suppress 语义和固定评测基线。
- 多设备合并结果与输入顺序无关，删除、恢复、离线并发和旧设备重放不会破坏本地新状态。
- 真实同步前补齐签名绑定、资源上限、设备生命周期、HTTPS 和客户端编排。
- manager 在正常产品运行态使用真实 FFI 和持久化配置，不静默伪装成 fixture。
- CI 覆盖 Rust Clippy、MSRV、Go race、Flutter、native bundle 和必要 feature 矩阵。
- 当前状态、路线图、架构文档和实现不再互相冲突。

## 三、非目标

整改期间不推进以下事项：

- 第二条真实平台输入法主线。
- Windows、Android、iOS 和 Linux 五平台并行落地。
- 自研完整拼音引擎。
- OIDC、管理控制台、多租户或云端实时转换。
- 在同步协议与平台私钥方案未通过退出标准前开放真实用户同步。
- 为未来功能继续增加新的 gate、approval、readiness、evidence bundle、fake replay 或 no-symbol 证明层。
- 为了通过验收而引入静默 fallback、吞错、默认成功或合成数据伪装。

## 四、全局停止线

以下停止线立即生效：

1. R01 完成前，不新增与真实输入链无关的 future command、native export、migration review、approval review、evidence bundle 或 no-symbol fixture。
2. R03 完成前，不开放真实远端上传、恢复码成功路径、设备授权成功路径或设备撤销执行路径。
3. 第一平台达到可重复日常输入前，不启动第二平台实现。
4. manager 产品模式不得在真实 FFI 加载失败时静默回退为 fixture；整改完成前维持现状只能视为开发模式能力。
5. Clippy、Manager CI 和 native bundle 检查进入门禁前，不形成稳定主线阶段交付。
6. `local_smoke`、合成 fixture、readiness 状态和设计草案不得单独作为产品阶段完成证据。
7. 输入热路径继续保持本地和离线，不得因整改引入网络依赖。

如后续发现某项工作必须突破停止线，应先在现有架构或 ADR 文档中记录原因、风险和替代方案，经人工确认后再调整本文；不得通过新增一层 gate 模型绕过决策。

## 五、已确认问题基线

以下问题已经由 2026-07-10 全仓审计确认，整改实现无需重复证明“问题是否存在”：

- `ime-ffi` 丢弃 `KeyOutcome` 的 `consumed` 与即时 `commit`，现有 C ABI 无法支撑真实平台按键分流和文本提交。
- 输入 session 未组合 engine、ranker、userdb 和隐私上下文，个人化学习没有进入平台输入热路径。
- userdb 将毫秒时间戳写入 `recency_score`，ranker 又将其裁剪为 `0..1`，导致 recency 不发生有效衰减。
- 选择、负反馈、删除和显式恢复涉及的多条 SQL 没有统一事务边界。
- 同步批次合并没有把本地当前 active state 作为同等输入，远端记录可能覆盖本地新状态。
- 相同 epoch 与时间戳缺少稳定设备级 tie-break，合并结果可能依赖记录顺序。
- 设备授权和恢复记录签名没有完整绑定实际 wrapped ciphertext 或对应 hash，恢复 KDF 参数缺少严格上限。
- Rust HTTP transport 只支持 `http://`，缺少连接超时和响应体上限；Go JSON 请求在完整解码后才执行对象大小检查。
- 正常 manager 构建包没有携带 RadishLex Rust FFI 动态库，未配置环境时默认使用 fixture。
- `AGENTS.md` / `CLAUDE.md` 的快速认知、当前状态和仓库实际实现不一致。
- 当前 CI 不执行 Flutter、Clippy、Go race、MSRV 和 native bundle 门禁。

## 六、整改批次总览

| 批次 | 名称 | 状态 | 前置依赖 | 退出结果 |
| --- | --- | --- | --- | --- |
| R00 | 文档真相源与停止线收敛 | 已完成（2026-07-10） | 无 | 当前入口、阶段口径和推进顺位一致 |
| R01 | 真实输入纵向链 | 下一批，尚未实现 | R00 | macOS 可离线完成真实中文输入 |
| R02 | userdb 与 ranker 正确性 | 待开始 | R01 的 runtime 边界 | 学习、删除和排序有正确性与评测证据 |
| R03 | 同步收敛与密码协议 | 待开始 | R02 数据语义 | 多设备结果确定且协议可安全开放实现 |
| R04 | 生产同步客户端与服务端闭环 | 待开始 | R03 | 两个真实客户端可完成完整同步周期 |
| R05 | Manager 产品化 | 待开始 | R01、R02；同步部分依赖 R04 | 正常安装包使用真实 FFI 与持久化配置 |
| R06 | 质量门禁与代码收敛 | 待开始，可与 R01 并行 | R00 | CI 覆盖真实交付链，已知静态问题清零 |

周期只用于排期参考，不作为降低质量或跳过验证的理由。单人实现的初步预算为：R00 1–2 个工作日，R01 7–10 个工作日，R02 4–6 个工作日，R03 10–15 个工作日，R04 7–10 个工作日，R05 5–8 个工作日，R06 2–4 个工作日。

## 七、R00：文档真相源与停止线收敛

### 目标

把项目从“继续扩展实现前证据模型”切换到“真实纵向链和正确性整改”，并让新会话读取的入口与仓库事实一致。

### 必须完成

- 修正 `AGENTS.md` 与 `CLAUDE.md` 的当前阶段、代码状态和推荐推进顺位，并保持基本复制。
- 压缩 `docs/status/current.md`，只保留当前阶段、最近证据、停止线、下一步和必要索引。
- 将 `docs/roadmap.md` 中的实现流水移出阶段交付和退出标准。
- 复核 `docs/technical-plan.md`、`docs/repository-layout.md`、`docs/privacy-sync.md` 中重复的当前状态，稳定文档不再承担周志职责。
- 给现有 manager sync preview / readiness / gate / review 文档和源码建立“保留、归档、删除、转化为真实 contract”清单。
- 把本专题接入 `docs/status/current.md`，但不加入长期稳定入口清单。

### 验收证据

- `AGENTS.md` 与 `CLAUDE.md` 内容同步且不再声称代码尚未落地。
- 新会话从 `docs/status/current.md` 能在短阅读内判断当前整改批次、停止线和下一步。
- roadmap 只描述阶段目标、交付物和退出标准，不继续追加实现日记。
- 文档检查、文本检查与 `git diff --check` 通过。
- 现有 future review 资产均有明确处置结论，没有继续自然增长的默认路径。

### 完成记录

- `AGENTS.md` / `CLAUDE.md` 已同步纠正当前认知，并保持在 14k 字符目标内。
- `docs/status/current.md` 已收敛为约 3.3k 字符的唯一当前状态短入口。
- README、technical plan、roadmap、repository layout 和 privacy-sync 已移除重复的实现流水，分别恢复稳定入口、架构、阶段、目录和隐私职责。
- 本文第十四节已登记 manager sync review 文档、Rust draft、Flutter 模型、fixture 和测试的处置去向。
- 文档预算、文本卫生、协作入口同步、diff 检查和仓库基线结果记录在 `docs/devlogs/2026-W28.md`。

## 八、R01：真实输入纵向链

### 目标

以 macOS InputMethodKit 为第一平台，形成完全离线的真实输入链。平台私钥和远端同步问题不得阻塞本地输入能力。

### 必须完成

- 将按键 ABI 升级为版本化结果对象，至少返回 `consumed`、可选 `commit` 和可读取的最新 snapshot。
- 明确 key result、snapshot、candidate view、error 和字符串的所有权、生命周期与释放责任。
- 建立面向平台的 runtime，组合 engine、ranker、userdb、privacy mode 和学习事件，不让平台壳承担业务真相源。
- 把 librime setup / initialize / finalize 收口为进程级 runtime，session 只管理 librime session 生命周期。
- 补多 session、线程归属、重置、schema 切换、异常释放和进程退出测试。
- 新增 macOS InputMethodKit 薄壳，只负责系统生命周期、按键接收、候选展示、文本提交和调用 Rust FFI。
- 明确开发版安装、启用和移除 runbook；真实安装动作仍需人工授权。

### 验收场景

- 全拼输入能产生 composition 和候选。
- 数字键、空格或平台选中操作能提交正确候选。
- 未消费按键能交还宿主应用。
- Backspace、Escape、Enter、方向键和 schema 切换行为有黑盒测试或人工记录。
- 两个并行输入 session 不重复初始化或破坏 librime 全局状态。
- 断网时全部输入能力可用。
- 重启输入法后用户词库仍在，且真实选择能形成受隐私策略约束的学习记录。

### 退出标准

- 开发者可按 runbook 安装开发版输入法，并在真实应用中完成连续中文输入。
- 自动测试覆盖 KeyOutcome 到平台提交的契约；人工 smoke 记录不包含真实敏感输入。
- 没有通过 fixture、CLI 输出或 manager 页面替代真实平台输入证据。

## 九、R02：userdb 与 ranker 正确性

### 目标

让用户的选择、抑制、删除、恢复和排序反馈具备稳定、可解释、可恢复的语义。

### 必须完成

- 将 add、selection、negative feedback、delete 和 explicit restore 的多表写入改为事务。
- 配置适合输入法与 manager 并发访问的 SQLite WAL、busy timeout 和连接策略。
- 明确本地数据库路径、文件权限、备份和损坏恢复边界。
- 用抗碰撞身份替换 64 位 FNV tombstone hash，或直接使用规范化复合身份。
- 将 recency 改为以时间戳为输入的确定性衰减，不再存储永远被裁剪为 `1.0` 的分数。
- 将 frequency 改为有界或对数增长，避免无限线性权重压过负反馈。
- 明确 suppress、delete 和 manual restore 的优先级，不依赖多层补偿权重偶然生效。
- 建立只含合成或可公开样例的固定排序评测集。

### 验收证据

- 故障注入证明每个用户意图要么完整提交、要么完整回滚。
- 两个进程或连接并发读写不会因默认配置频繁产生 `SQLITE_BUSY`。
- 删除、旧设备重放、显式恢复和备份恢复测试形成闭环。
- 排序评测固定记录 Top-K 命中、MRR 或等价指标，并记录候选重排延迟基线。
- explain 输出能对应每个生效因子，且 recency 随时间单调衰减。

## 十、R03：同步收敛与密码协议

### 目标

先证明数据能够确定收敛并且签名真正绑定安全相关材料，再允许进入生产同步实现。

### 必须完成

- 合并时把本地当前 user term、ranker weight 和 tombstone 纳入同一 merge input。
- 定义稳定版本顺序，至少包含 key epoch、逻辑时钟或对象版本、设备 ID 和确定性 tie-break。
- 为 user term、weight、tombstone 和 explicit restore 建立统一冲突表。
- 增加交换律、结合律、幂等性属性测试，以及乱序、重复、离线和同时间戳测试。
- 设备授权签名绑定接收设备密钥材料、wrapping algorithm、nonce 和 wrapped ciphertext hash。
- 恢复记录签名绑定实际密文 hash；KDF、salt、密文和解析字段同时设置最小与最大上限。
- 对主密钥、恢复密钥和临时明文使用明确清零或 secret container 策略。
- 建立 Rust 与 Go 共用的版本化协议 test vector。
- 调整平台签名算法为可演进协议；原生 P-256 与平台封装 Ed25519 seed 的保护语义必须明确区分。

### 验收证据

- 任意输入顺序得到相同合并结果。
- 较旧远端 active 记录不能覆盖本地较新编辑。
- 旧 tombstone 不能删除更新的显式恢复，旧设备也不能复活已删除词条。
- 篡改接收公钥、wrapped ciphertext、nonce、hash、KDF 参数或签名字段均被拒绝。
- 恶意超大 KDF 或密文参数在执行高成本操作前被拒绝。
- Apple 与 Android 的平台算法选择有真实能力证据，不再把 unavailable backend 当作长期产品路径。

## 十一、R04：生产同步客户端与服务端闭环

### 目标

形成不依赖测试手工拼接的生产同步周期：发现远端状态、下载、验签、解密、合并、上传、冲突重试和设备治理。

### 必须完成

- 为对象提供 latest、list 或 change cursor 等可发现接口。
- 在 Rust 中实现明确的 `sync_once` 或等价 orchestration，管理本地 cursor、版本、重试和错误分类。
- 补齐设备列表、wrapped key 获取、加入授权、撤销、恢复记录创建/轮换/撤销等 API。
- 使用维护中的 HTTPS/TLS transport，设置连接、读、写和总超时以及响应体上限。
- Go handler 在 JSON/base64 解码前限制请求体，并限制 ID、key、challenge 和字符串长度。
- 部署态强制非空认证；无认证只允许显式的 loopback 开发模式。
- 恢复读取限速绑定可信身份或服务器策略，避免伪造 header 绕过和无界内存增长。
- 补 graceful shutdown、关键审计写入错误处理和 blob/metadata 持久化顺序验证。

### 验收证据

- 两个真实客户端从空状态开始完成注册、授权、上传、发现、下载、验签、解密和合并。
- 离线双方修改后重连能确定收敛，stale base 能自动进入受控重试。
- 撤销设备无法获得新 epoch 数据，已有设备可继续同步。
- 请求体、响应体、慢连接、错误 TLS、空 token 和限速绕过均有负向测试。
- 服务端日志、诊断和错误响应不包含明文 P1/P2 内容或认证材料。

## 十二、R05：Manager 产品化

### 目标

让 manager 的默认产品运行态使用真实 native library、真实本地数据库和持久化配置；fixture 只保留为显式开发模式。

### 必须完成

- 将 `radishlex-ime-ffi` 作为 native asset 或受控动态库随应用打包，并在 CI 检查 bundle 内容。
- 将 fixture 模式改为显式开发开关，并在 UI 中持续显示不可混淆的演示标识。
- 产品模式加载真实 FFI 失败时明确失败，不展示合成用户词或同步状态。
- 使用平台 App Support 路径保存数据库与配置，补原子写入、权限和迁移策略。
- 使用系统文件选择器处理词库与诊断导入导出，补 macOS sandbox entitlement 和 security-scoped access。
- 若 macOS manager 与 InputMethodKit 需要共享数据，固定 App Group、锁与数据库所有权边界。
- 统一 Rust、Flutter、native bundle 的版本号和兼容性诊断。

### 验收证据

- 不依赖 shell 环境变量即可启动真实产品模式。
- 构建产物包含并能加载正确版本的 FFI library。
- 设置重启后保持，词库导入导出通过系统文件选择器完成。
- 缺失 FFI、数据库损坏、权限失败和版本不兼容都有明确用户可见错误。
- 发布构建不因 sandbox 权限缺失而使核心本地管理功能不可用。

## 十三、R06：质量门禁与代码收敛

### 目标

让 CI 真实覆盖即将交付的代码，并减少大文件、dead code 和评审模型进入生产源码造成的维护负担。

### 必须完成

- 修正 `ime-ffi` 公开裸指针接口的 `unsafe` 契约与 `# Safety` 文档，使 Clippy 通过。
- PR 与 Release CI 增加 Rust fmt/check/test/clippy、声明 MSRV、Go test/race/vet、Flutter format/analyze/test。
- 增加 macOS manager build 和 FFI bundle presence 检查。
- 为 native-rime 建立可重复 feature job；真实平台 smoke 可放到带环境标签的人工或定时门禁。
- 补依赖安全、许可证和必要的供应链检查。
- 拆分接近 1000–1500 行且继续增长的生产源码，按职责保留浅层模块。
- 将 `manager_sync_command` 中未导出的 review draft 和对应 Dart review fixture 逐项删除、归档为文档，或转化为真实 contract；不允许长期依赖 `#![allow(dead_code)]` 保存审批流程。
- 为 FFI parser、同步 merge 和解密后 payload parser 增加 property/fuzz 或等价边界验证。

### 验收证据

- `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- CI 能在 Flutter 或 native bundle 失败时阻止 PR 合并。
- MSRV 与 stable 两套 Rust 环境均有明确结果。
- Go race 与 Flutter 全量测试进入常态门禁。
- 生产源码中不存在仅用于证明 future feature 尚未开放的大规模 dead-code review 模型。

## 十四、Manager Sync Review 资产处置清单

本清单只决定现有资产的去留，不批准真实同步，也不创建新的迁移层。处置前允许为修复明确 bug 或完成删除而修改；不得继续增加新的 review、approval、gate、evidence、fake replay 或 no-symbol 分支。

### 文档与 ADR

| 资产 | 处置 | 目标批次 | 说明 |
| --- | --- | --- | --- |
| `docs/manager-sync-entry-boundary.md` | 保留并收敛 | R05 | 保留 UI 职责、secret 与停止线；移除实现流水和 future preview 索引 |
| `docs/manager-sync-bridge-command-contract.md` | 冻结后转化 | R03/R04 | 只提取为真实版本化 Rust command contract，不再扩展设计-only DTO |
| `docs/manager-sync-bridge-command-contract-checklist.md` | 归档 | R04/R06 | 真实 host contract tests 落地后移入历史归档 |
| `docs/manager-readiness-scenarios.md` | 冻结后归档 | R05 | 只保留可转化为真实状态/安全负例的场景 |
| `docs/manager-sync-action-protocol-preview.md` | 冻结后归档 | R05 | 被真实 command/result contract 和 UI 交互替代 |
| `docs/manager-sync-action-acceptance-matrix.md` | 冻结后归档 | R05 | 有效安全断言迁入真实集成测试，其余 preview 矩阵删除 |
| `docs/manager-sync-ffi-command-boundary.md` | 提取后归档 | R04/R06 | 所有权、线程、panic 和 redaction 规则回写 `docs/ffi-boundary.md` |
| `docs/manager-sync-ffi-command-contract-test-plan.md` | 由真实测试替代 | R04/R06 | 测试落地后归档，不继续增加计划层 |
| `docs/adr/0006-manager-sync-c-abi-contract-governance.md` | 标记被替代后归档 | R04/R06 | 稳定 ABI 决策进入真实 contract/FFI ADR，不保留审批状态机 |

### Rust 生产源码

| 资产 | 处置 | 目标批次 | 说明 |
| --- | --- | --- | --- |
| `crates/ime-ffi/src/manager_sync_command.rs` | 删除或以真实实现替换 | R06 后接 R04 | 当前 `#![allow(dead_code)]` review draft 不作为生产基线 |
| `crates/ime-ffi/src/manager_sync_command/admission.rs` | 删除 | R06 | admission/approval 状态属于历史治理，不属于运行时 |
| `crates/ime-ffi/src/manager_sync_command/host_test_gate_review.rs` | 删除 | R06 | host test gate 由 CI 和真实 contract tests 取代 |
| `crates/ime-ffi/src/manager_sync_command/migration_review.rs` | 删除 | R06 | migration review 不在 `src/` 保留 |
| `crates/ime-ffi/src/manager_sync_command/tests.rs` | 拆分处置 | R06 | redaction/panic/ownership 负例迁入真实 FFI 测试，其余 review-only 测试删除 |

真实 manager sync command 尚未进入 R04 前，`ime-ffi` 可以完全不导出该能力；不需要保留一套运行时草案来证明它没有导出。

### Flutter 生产模型与 bridge

| 资产 | 处置 | 目标批次 | 说明 |
| --- | --- | --- | --- |
| `manager_sync_action_preview_models.dart` | 冻结后删除 | R05 | 由真实 command/result 与明确 disabled state 替代 |
| `manager_sync_entry_models.dart` | 保留核心、删除 future preview | R05 | 只保留真实可见状态、阻塞原因和用户操作入口 |
| `manager_sync_models.dart` | 拆分并保留真实领域模型 | R05/R06 | 移除 approval/evidence shape，控制单文件规模 |
| `ffi_manager_sync_readiness_mapper.dart` | 评估后保留或删除 | R05 | 只有映射真实 Rust snapshot 时保留；不再读取设计 fixture |
| `ffi_manager_sync_mapper.dart` | 保留并收敛 | R05 | 只映射真实 FFI 输出，不派生同步业务真相源 |

### Dart fixture 与测试

| 资产 | 处置 | 目标批次 | 说明 |
| --- | --- | --- | --- |
| `sync_ffi_rust_host_contract_review_fixtures.dart` | 删除 | R06 | review package 不进入长期测试资产 |
| `sync_ffi_rust_host_migration_review_fixtures.dart` | 删除 | R06 | approval/migration replay 由真实 contract tests 取代 |
| `sync_ffi_command_boundary_fixtures.dart` | 拆分后删除 | R06 | 只迁移真实 ABI 安全负例 |
| `manager_sync_ffi_migration_review_test.dart` | 删除 | R06 | 不再测试审批状态机 |
| `manager_sync_ffi_command_boundary_test.dart` | 由真实 FFI 测试替代 | R06/R04 | 保留 ownership、status、redaction 断言，删除 no-symbol/review 断言 |
| `manager_sync_ffi_binding_contract_test.dart` | 保留核心、删除 replay | R06 | 保留 dynamic binding 与释放契约 |
| `manager_sync_bridge_command_contract_test.dart` | 由真实 bridge 测试替代 | R04/R05 | 不再验证设计-only command shape |
| `sync_evidence_bundle_fixtures.dart` | 冻结后删除 | R05 | 发布证据由真实安全摘要输入替代 |
| `sync_readiness_bridge_fixtures.dart` | 冻结后删除 | R05 | 由 Rust 真实 readiness snapshot integration fixture 替代 |
| `manager_sync_action_preview_test.dart` | 随 preview 模型删除 | R05 | 真实 disabled/enabled UI 行为进入 widget/integration test |
| `manager_sync_entry_gate_test.dart` | 保留安全断言并重写 | R05 | 保留停止线、隐私模式和失败可见性，不保留 future 状态机 |
| `manager_sync_transient_secret_interaction_test.dart` | 转化为真实 secret 生命周期测试 | R03/R05 | 保留不持久化、不日志化和确认边界 |

### 处置验收

- R06 结束时，Rust `src/` 不再包含 review-only manager sync command module。
- R05 结束时，Flutter 产品模型不再包含 action preview、approval 或 evidence bundle 业务层。
- R04/R05 结束时，真实 command/bridge/integration tests 覆盖迁移保留的 ownership、panic、redaction、secret 和 disabled-state 断言。
- 被归档文档从 `README.md`、`docs/status/current.md` 和默认阅读链移除；稳定规则回写现有边界文档。
- 处置过程不得删除仍未迁移的隐私、安全或 FFI 生命周期断言；每次删除前先指出替代测试位置。

## 十五、推荐执行顺序

### 第一段：纠偏与真实输入

1. 完成 R00。
2. 启动 R06 的 Clippy、Manager CI 和 bundle 检查最小门禁。
3. 完成 R01 的 KeyOutcome ABI、runtime 和 macOS InputMethodKit 纵向链。

第一段结束时必须已经能够在真实应用中输入中文；否则不进入同步主线。

### 第二段：本地数据正确性

1. 完成 R02 的事务、SQLite 并发策略和 tombstone 身份。
2. 修正 ranker recency/frequency/suppress。
3. 建立排序评测和输入热路径性能基线。

### 第三段：同步安全与产品化

1. 先完成 R03，再实现 R04。
2. R05 的本地管理产品化可以在 R02 后推进；真实同步 UI 必须等待 R04。
3. R06 持续收敛全部门禁和代码结构。

## 十六、批次更新规则

- 本文只更新批次状态、阻塞项、验收证据链接和退出判断，不追加逐日流水。
- 每次只能有一个主批次处于“进行中”；R06 可作为并行质量工作流，但不得抢占主批次产品目标。
- 每个可分割实现步骤完成后执行匹配验证；只有跨边界变更、阶段交付或发布前才运行全量门禁。
- 验证命令、提交列表和当天事实写入 `docs/devlogs/YYYY-Www.md`。
- 稳定接口、协议、隐私和平台决策回写对应正式文档，不以本文作为长期引用源。
- 如果批次阻塞，必须记录具体缺失能力、已验证替代方案和解除条件，不能用新增 review 模型代替实现。

## 十七、总体验收与退出条件

本专题只有同时满足以下条件后才能关闭：

- macOS 第一平台能离线完成稳定中文输入，并有自动契约测试和人工 smoke 证据。
- 真实输入选择能安全写入 userdb，并参与候选重排。
- userdb 用户意图具备事务性，ranker 具备有效时间衰减和固定评测基线。
- 同步 merge 满足确定收敛，协议签名绑定和资源上限通过负向测试。
- 两个真实客户端能完成完整同步周期，设备撤销和 key epoch 轮换有效。
- manager 默认产品模式携带并使用真实 FFI，配置和数据可持久化。
- PR 与 Release CI 覆盖 Rust、Go、Flutter、Clippy、MSRV 和必要 native 构建。
- `AGENTS.md`、`CLAUDE.md`、README、当前状态、roadmap、技术方案与实际实现一致。
- 所有真实用户同步、平台安装和发布动作仍遵守人工授权边界。

关闭时应把最终阶段判断写入 `docs/status/current.md`，把稳定路线和退出结果写回 `docs/roadmap.md`，把接口与安全决策分别写回对应专题或 ADR；本文仅保留为一次整改的历史索引。
