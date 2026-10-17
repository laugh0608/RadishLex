# macOS 取消资格证据 reader 合同

本文定义准备取消所需独立程序资格证据的只读 reader，供 Installer、验证与安全边界维护者审查。状态：**2026-10-17 按批准合同实现只读 reader 与合成负向测试；生产审阅目录为空，未接生产 issuer，最终取消门禁保持关闭。** 本文不授权采集证据、运行 source、操作冻结资产或释放 marker。

上游事实以[资格执行方案](macos-source39-qualification-plan.md)和[准备取消兼容合同](macos-preparation-cancellation-compatibility.md)为准。A1 已证明本次仅设置 `CFFIXED_USER_HOME` 的样本未达到路径隔离要求；Q2 仍阻塞。设计 reader 不能消除这个缺口。

## 目标与职责

reader 回答三个问题：证据是否来自明确批准的来源、内容是否完整且未变、覆盖是否与指定 source/schema/环境及取消场景相匹配。它不回答“此刻可以移走 marker”，不产生可保存后重放的资格令牌。

| 层 | 负责 | 不得代替的条件 |
| --- | --- | --- |
| 证据采集与人工操作记录 | 在获准隔离环境中记录精确旧程序的实际行为、合成数据语义和失败样本 | 命令退出、PID 或截图单独不能证明完整主路径 |
| 审阅与可信目录 | 审查原始证据、采集器和覆盖，批准精确证据包摘要及适用域 | hash 只能检验内容，不能自行证明实验真实发生 |
| 只读 reader | 严格解析、核验信任引用和文件实物，计算缺失覆盖及请求匹配 | 不能创建 `CancellationSourceQualification`，不能打开真实业务 DB |
| 未来 issuer | 在当前有效策略下，将匹配证据与本次完整 proof、源 family/schema 及新鲜观察绑定 | 不能把 reader 的完整报告、人工通过文字或结构构造成功直接当作资格 |
| 既有协调器与产品观察 | 持续双 guard、任务授权、真实程序/root/静止、源等价与 marker 事务 | reader 不取代任一现场检查，不持久化或修改旧 v1 回执 |

现有 [qualification 类型与 authority](../../platforms/macos-product/InstallCoordinatorAdapter/src/cancellation_release.rs)只校验结构和精确对象相等；`from_verified_source` 不是验证器。[核心 release](../../crates/ime-product-upgrade/src/cancellation_release.rs)和[marker 持久化检查点](../../crates/ime-product-upgrade/src/cancellation_release_persistence.rs)的既有边界保持。

## 证据来源与信任起点

### 选定的首版设计：随受信产品固定的审阅目录

不新增远程证据服务、运行时公钥下载或自建签名基础设施。首版采用经代码审查和项目所有者批准、随受验证 Installer 产品固定的只读目录；它把精确 pack manifest 摘要与适用 source、覆盖策略及审阅记录关联。这里的“目录”是可信策略记录，不是任意文件夹。

- 产品边界先验证当前 Installer/target 及其封装身份，再向 reader 提供固定的策略对象。未来实现的生产构造入口不能让调用者直接塞入任意目录或把普通 JSON 反序列化为可信策略。
- 目录条目包含 `policy_revision`、`entry_id`、证据 manifest SHA-256、精确 source/环境键、覆盖 profile、采集器版本、审阅记录摘要和显式撤销集合。全部是精确匹配，不支持版本范围、任意 schema 或通配 OS。
- 证据包可存放在明确选择的本地私有根；这个路径只是未信任的查找位置，owner/mode 正确也不使它可信。只有与受信目录中批准的 manifest 完全匹配，才能进入覆盖核验。
- 不从环境变量、用户设置、CLI 的 `trusted=true`、同目录签名/公钥、旁路 sidecar 或网络响应加载信任根。包内自报的批准人、签名和 `passed` 文字均无授权效力。
- 测试目录和合成 provider 只存在于隔离测试配置；生产目录当前无获准的 source 39 条目。测试成功不得生成或提交生产条目。

审阅目录的可信度来自受信产品及实际证据审阅，不来自 SHA-256 的“自认证”。攻击者若能替换整个受信 Installer 或控制当前进程，已超出 reader 的文件完整性防护范围；不能宣称本设计防御了该情形。

### 证据进入目录的条件

一次审阅必须同时看到：独立运行环境与数据/会话隔离准入、精确原 source 身份、采集器代码与命令、每个覆盖行的原始材料及结构化断言、失败/重试记录和清楚的适用范围。审阅记录须可追溯到明确批准，且绑定最终 pack manifest 摘要；不会因为某次矩阵自报全绿就自动加入目录。审阅记录在 pack 外独立保存，由受信目录同时固定它和 manifest 的摘要；manifest 不反向包含该审阅记录文件或摘要，避免循环绑定。

原始记录只使用合成身份和数据。脱敏发生在归档审阅前，脱敏规则与对应原始证据的受控核对过程进入审阅记录；不能在 reader 验证后临时删改证据。缺乏可审查原始依据时不准入。

reader 在 manifest 内验证结构化事件、预期/实际断言及样本链，附件按闭合清单验证字节摘要和实物身份；它不把任意日志、视频或附件脚本自动解释成行为事实。结构化记录与原始附件之间的真实性核对属于独立审阅，随后由固定 manifest/附件摘要保全；测试目录不能替代这次审阅。

| 材料 | 可支持的结论 | 能否单独进入完整资格来源 |
| --- | --- | --- |
| 精确 source 的正常 Manager/InputMethod 实际运行材料 | 指定环境下的双 gate、真实 runtime、客户端学习与重启 | 仍须完整矩阵、原件关联和审阅 |
| 严格实物/hash/codesign 记录 | 程序身份和完整性 | 否，不证明运行行为 |
| 合成协调器测试与故障矩阵 | 事务和拒绝规则 | 否，不证明旧程序正常运行 |
| validation host、FFI smoke、路径探针 | 各自定义的局部行为；A1 是负向可行性结果 | 否，不提升为正常双程序资格 |
| 人工文字、截图、PID、退出码 | 补充上下文或特定人工步骤 | 否，缺少独立语义及完整链时拒绝 |
| 历史已冻结现场的分散记录 | 对应历史事实 | 不拼接为本次独立连续实验，不重新读取或复跑它们来补齐本设计 |

## 输入、输出与只读边界

以下为协调层 Rust 内部合同，不是新 FFI ABI 或产品命令。实现入口为 [`qualification_reader::review_evidence`](../../platforms/macos-product/InstallCoordinatorAdapter/src/qualification_reader.rs)，请求由 `CancellationEvidenceRequest::new` 从严格核心 proof 派生。

输入分为四组：受信策略及产品绑定、明确指定的证据根、待匹配的精确 source/运行环境、从既有严格核心 reader 得到的本次 proof 描述。缺少 proof 时可以只做证据审计，但不能输出“本次场景匹配”。

reader 返回 `EvidenceReview`：安全读取完成后包含策略/条目/pack 摘要、文件身份观察、source/schema/环境键、逐行覆盖及请求绑定；覆盖缺失保留逐行结果，其余错误在最先失败处返回固定原因，不暴露不可信内容。报告状态只有 `rejected`、`reviewed_unbound`、`reviewed_matching_request`；后两者均不叫 `qualified`。实现不提供报告序列化、可信目录的公开构造或资格转换入口，旧报告不能进入后续授权判断。

reader 不创建目录、guard、日志或修复文件，不执行包内命令，不打开 SQLite、不装载 dylib、不启动 source，不修改偏好、输入源、receipt、index、marker 或任务期限。输入包不存在时报告缺失，不扫描其他路径寻找“最近一次通过”，也不联网补齐。必要日志由上层输出固定原因码和非敏感计数，不输出真实路径、proof 内容、DB 内容或凭据。

## 证据包的逻辑格式

首版 reader 接受 `radishlex-cancellation-qualification-evidence-v1`，仅在测试中构造合成包；没有生产 writer 或已准入真实证据。采用严格 UTF-8 JSON manifest 与精确列出的普通文件，拒绝 BOM、重复键、未知字段、错误类型、超界整数及非整数数值。摘要针对原始文件字节，不采用会吞掉未知字段或重复键的“规范化后等价”。manifest 摘要存放在受信目录，避免自摘要循环。

| 字段组 | 必须绑定的内容 |
| --- | --- |
| 包与采集 | 格式、唯一 run ID、采集器 source commit/可执行摘要、协议版本、命令清单摘要、采集环境准入记录；时间仅作记录 |
| 精确程序 | source release、完整 product manifest 摘要、Manager/InputMethod 完整 tree、可执行文件、dylib/资源及各架构签名标识；不只记录版本号或 Team ID |
| 环境 | OS build、CPU 架构及原生/转译运行方式、文件系统类型/大小写语义、隔离方法及获准环境证据；不记录真实用户名 |
| schema | 从该精确 source 与合成源库独立测得的 schema、测量方法和输出引用；target schema 不充当 source schema |
| 样本 | 合成数据配方与摘要、初始状态、预期变化、实际逻辑断言、单次运行链和前后文件身份；不包含真实用户库 |
| 覆盖行 | 唯一 case ID、明确维度、前置条件、预期允许/拒绝、实际结果、原始文件引用与独立断言引用、结束状态 |
| 文件清单 | 受限相对路径、类型、字节数、SHA-256、内容角色；包括审阅所需日志/结构化观察/合成数据材料，缺一不可 |

实验文件 inode/root 只用于证明同一次采集的连续性，不要求部署后的用户 root 与实验 inode 相等。程序内容身份必须相等，部署实物的物理身份由当前产品观察独立验证。禁止将实验 receipt 的 operation/root/inode 复制到真实 proof。

程序内容及启动事件的结构化路径相对于各自 component 的 bundle，例如 `Contents/MacOS/radishlex_manager`；bundle 名和用户目录不进入此相对路径。实际固定路径启动及其来源仍须由原始材料、双 gate 与环境准入审阅证明。reader 只校验这些记录和附件，不沿记录的程序路径访问实物。

首版解析上限固定为：manifest 1 MiB、覆盖行和附件各最多 4096 项、相对路径最多 180 字节、JSON 深度 16、单附件 16 MiB、含 manifest 总计 256 MiB。流式散列、单线程读取，不把全部附件载入内存。超过上限报 `resource_limit`，不静默截断、跳过或自动拆包；如果真实采集材料需要更大预算，先修改和审查合同。大视频等外部引用不算已读取证据，不能以 URL 代替必需文件。

## 覆盖与匹配规则

覆盖 profile 来自受信策略，不能由证据包自选较弱 profile。首版要求[资格矩阵](macos-source39-qualification-plan.md#最小完整资格矩阵)的全部适用行，并按下表明确维度。不是把若干“包含过某个标签”的行当作完整笛卡尔积：profile 必须列出每个适用组合的固定 case ID、前置与断言；尚未枚举清楚的组合视为缺失，不能留 `other` 或 `not_applicable` 通配。

| 覆盖组 | 必需维度与断言 |
| --- | --- |
| 兼容后置状态 | 维护前取消与维护后等价收尾；无新 data receipt；旧 outer 的 `completed` / `aborted_preserved` / `rolled_back` 各合法终态 |
| 历史组合 | 无 data 前驱、outer/data ID 不同、旧 v1/新 v2 生命周期前驱、重复取消；声明不适用须由 profile 固定原因，不能由运行时结果决定 |
| 正常双程序 | 原始 Manager 与 InputMethod 各自正常启动、两道 gate、实际 runtime 打开；正常客户端合成输入/选择/学习，正常退出、新进程重开及重开后的语义断言 |
| 数据语义 | 合成 WAL、新词学习、删除/tombstone、settings、离线同步投影；维护前后等价与随后合法学习分别断言，不要求学习后 DB 字节仍相同 |
| 阻断状态 | 准备/取消 marker、中间封存、未知临时对象；旧 gate 的预期拒绝必须发生在业务写入之前，不能只看错误文本或进程退出 |
| 新 reader/issuer 拒绝 | 缺失/错 hash/inode、source/schema/proof/root/祖先错配、索引错误、混入未审包；旧 source 不被要求理解新增证明格式 |
| 后续链与恢复 | 取消后再准备→新 v1→数据终态释放；保留先前 proof、receipt、保护快照；marker 移动边界中断后重新授权及逐检查点资格复验 |

正常运行材料至少关联启动路径、验证后的可执行身份、进程实例/启动顺序、gate 观察、客户端步骤、前后逻辑状态与退出/重启。PID 可以复用，不能只用 PID 证明新进程；采集器版本、进程启动标识和事件次序须一致。录像或文字描述不替代存储语义断言。

同一 case 的全过程必须来自同一个已绑定样本链；不能将 A 样本 gate、B 样本学习和 C 样本重启拼成通过。多个 case 仅可在同一受审 pack 中按相同 source/schema/环境/profile 汇总，reader 不自动合并多个零散包。保留失败尝试和重试关系，不用最后一次绿色覆盖前面的失败；预期拒绝行实际拒绝可以通过，实际允许则失败。

## 独立资格与本次 proof 的连接

长期兼容性证据证明的是“精确程序在明确场景类别中具备某种行为”，不会预先知道未来用户的 operation/root。reader 只能从已严格解码的本次 `CancellationReleaseReceipt` 派生场景：source、target、维护前后、旧 outer 终态、data 前驱有无及关系、生命周期版本与 source family，随后与 profile 的精确行匹配。

- reader 不接受调用者提供的 `covered=true`、旧版本号字符串或任意 case ID 作为场景判断；取值必须来自验证过的 proof 和产品/环境上下文。声明 source tree 与 manifest 不一致即拒绝。
- 当前 proof 的 canonical bytes/摘要、完整 typed source、data root、旧 outer/data、family 和索引关系留在请求绑定中；不能只按 operation ID 匹配。实验的 operation 不与真实 operation 作相等匹配。
- 有 snapshot schema 时须与证据支持的 source schema 严格相等。维护前无 snapshot 时，不从 target schema 猜测，也不把缺失 schema 当通配；必须有来自当前 exact source family 的受信 schema 观察。该观察如何在不试开真实业务库的前提下取得，尚需 A5 单独落实；缺少时保持 unavailable。
- 源内容等价、当前程序及静止来自既有核心和产品观察，reader 不重新实现源库验证。合法学习后的历史查询不被旧 DB hash 封死；尚待释放的当次 family 则仍受现有 proof 规则约束。
- 为避免循环证明，独立兼容性实验先验证隔离的后置状态样本；不得先用假 issuer 释放真实 marker，再用启动成功为该 issuer 背书。

未来 issuer 即使获得 `reviewed_matching_request`，仍须在 `QualifiedSource` 的每个实际回调重新核验证据、当前策略与当次任务绑定。包括 `Begin`、`SourceVerified`、marker 移动/同步各检查点及 `Released`；恢复路径同样不得复用前次报告。保持外层→内层连续 guard、最多 300 秒且可撤销任务以及 provider 前后的产品/静止复核。资格矩阵在任务外预先采集，不能在这 300 秒内现场运行，也不能自动续权。

## 文件实物与读取一致性

reader 通过目录句柄逐级读取明确根，拒绝根/祖先或条目的符号链接、`..`、绝对路径、路径别名、大小写/Unicode 归一化冲突、包内跨文件系统挂载和特殊文件。根及包内目录必须为当前用户所有、`0700`；普通文件为当前用户所有、`0600` 且 link count 为 1，合成二进制材料作为不可执行附件保存。根外系统祖先按受信系统目录/带 sticky 位的临时父目录规则分别验证，不把 `/private/tmp` 错当作用户 `0700` 目录；附加 ACL 若扩大访问或无法严格判定则拒绝。证据打包器必须在审阅前满足此形状，reader 不改权限或搬动文件。

文件清单严格闭合：未知文件、临时对象、重复路径、缺失附件均拒绝；不读取包内自带的脚本或链接来“补全”结果。每个已打开文件核对 descriptor 对应的 device/inode/owner/mode/type/size，与清单字节数及 SHA-256 对照，读取后再次核对；根和祖先在调用结束重新检查。相同字节的 inode 替换仍是本次读取漂移，失败关闭，不重试到恰好成功。完整性依赖摘要与受信 manifest，metadata 不是内容验证的替代。

上述规则针对新证据包，不授权整理旧实验根。A1 根没有这种生产 pack，也不会被修改成这种形状。writer/采集器输出到 reader 格式的流程与真实实验均需后续另行批准。

## 拒绝分类及映射

reader 的内部原因应比现有四类 qualification 错误更具体，供诊断与测试；未来 issuer 才负责映射。任何 reader 拒绝都不产生资格对象，不自动修复或寻找替代来源。

| 内部原因 | 判据举例 | 未来 authority 映射 |
| --- | --- | --- |
| `untrusted_source` / `missing_evidence` | 无受信目录条目、只有自报通过、没有必需原始记录 | `Unavailable` |
| `coverage_missing` / `environment_unsupported` | 缺行、skip、未证明隔离、OS/架构/profile 不匹配 | `Unavailable` |
| `policy_unavailable` / `revoked` | 当前策略身份不明、明确撤销或新鲜度前提无法满足 | `Unavailable` |
| `schema_unsupported` | 源 schema 无证据、缺独立测量或超出受审集合 | `UnsupportedSchema`；仅待采集的当前 schema 观察缺失可为 `Unavailable` |
| `identity_mismatch` / `evidence_changed` | source/proof/family 错绑、hash/inode/权限/祖先漂移 | `EvidenceChanged` |
| `malformed` / `resource_limit` / `io_failure` | 非法格式、未知/重复键、越界、读取失败 | `VerificationFailed`，保留细分原因 |
| `assertion_failed` / `contradictory_run` | 结果与原始断言矛盾、链不连续、正负预期不符 | `VerificationFailed` |

同一调用出现多个问题时，未建立信任/完整性前不将内容解释为可信覆盖结果；完成安全读取后可以报告全部缺失覆盖。I/O 失败、超限或 deadline 到期不能伪装成“没有证据”后走默认允许。恢复任务无证据时保留当时实物位置；若 marker 已移动后新鲜核验失败，报告已有核心错误及实际状态，不承诺无条件搬回或对用户显示取消成功。

## 新鲜度、撤销与明确未解决项

每次 reader 调用重新核验固定策略身份、撤销集合、包实物和请求，不保存允许布尔值。source、schema、OS build、CPU 执行方式、采集协议、覆盖 profile 任一变化都需要新条目。时间戳仅用于审计；本版不设置一个任意天数 TTL 来冒充安全有效期。

离线固定目录只能执行它已知道的撤销，无法保证旧 Installer 得知后来撤销；本地墙钟或文件 mtime 也不能证明策略是最新。这是生产 issuer 的开放阻塞，不能用目录版本自报值或普通用户文件中的最低版本解除。未来 A5 必须单独批准策略更新/防降级或每次运行的新鲜批准来源；在该机制成立前，reader 最多给出只读报告，不能支撑生产最终取消开放。

当前尚缺三个独立条件：可信且完整的 source 39 正常运行证据、无 snapshot 时对当前源 family 的 schema 观察合同、生产策略新鲜度/撤销合同。reader 的文件解析和覆盖设计完成不代表这些条件完成。

## 实施边界与验收设计

本批已获准实现 macOS 产品协调层的独立只读 reader；生产 provider、目录装配与最终按钮仍留在独立批准批次。通用核心只新增既有前驱分类的只读 getter，不读取产品资格包；AppKit 不直接解析证据。action `10–13`、FFI、v1/v2 回执格式与双 gate 不变。

协调层经单独批准直接引用锁文件中已有的 `serde 1.0.228`、`serde_json 1.0.150`、`sha2 0.10.9` 与 `libc 0.2.186`，没有下载或升级依赖。私有文件读取模块是唯一 POSIX `unsafe` 例外：逐级目录句柄、`openat` / `fstatat` / `O_NOFOLLOW`、descriptor ACL 和每段读取后的身份复核。其余协调层保持 `deny(unsafe_code)`。macOS 实际执行验证；Linux 分支尚未在本批环境执行，其他平台不编译此 reader。

实现采用以下保守约束：包内路径只接受安全 ASCII，拒绝大小写冲突及非 ASCII，避免未实现的 Unicode 归一化猜测；包内与祖先存在任何 ACL 均拒绝，Darwin 无 ACL 的 `ENOENT` 仅在 descriptor 身份复核后接受，其余未知失败关闭。目录扫描最多 8192 个节点、4096 附件加一个 manifest；附件以 64 KiB 块流式 hash，不保留全文。`ReadControl` 使用单调时钟、最多 300 秒并支持撤销，每次扫描/读块前后检查；这不是系统调用超时或 CPU/RSS 硬配额。

签名记录明确区分 `ad_hoc` 与 `developer_id`：前者 Team ID 必须为空，后者必须为合法非空值；不会为 source 39 的 ad-hoc 身份伪造 Team ID。后置数据保持断言必须指向 `Postcondition` 后的最终存储观察，初始绿色状态或中间绿色观察不能掩盖后续漂移；双组件的学习/重开状态按各自进程链核对。真实后置实验、审阅批准和可信目录仍不能由合成 pack 替代。

最小验收必须包含以下可观察行为，不能仅测试字段枚举或构造器：

1. 可信测试目录+完整合成 pack 得到只读匹配报告，树及所有原文件身份不变；该报告无法经过生产路径生成资格或移动 marker。
2. 无目录/伪造目录、自签包、未知条目、改写 manifest、同字节 inode 替换、symlink/hardlink/祖先漂移、重复键与路径别名均拒绝，读取不逃出指定根。
3. 逐项删除必需覆盖、把失败改成通过、替换 source/schema/OS、拼接不同 case 阶段、混包和无依据的 skip 均拒绝；预期拒绝样本可按固定断言通过。
4. 相同 release 但不同 manifest/tree/signature 拒绝；无 snapshot 不能借 target schema 通过；不同 proof/root/family 或仅相同 operation ID 不能复用报告。
5. 文件读取中、provider 前后以及各 marker 检查点模拟漂移/撤销/期限失效；只读 reader 测试不执行 marker 事务，组合层另用既有合成目录验证重新授权与失败保留。
6. 上限边界及超界、短读/权限错误、取消/deadline、诊断脱敏与重复调用均覆盖；测量最坏包的时间/内存，不能靠缓存允许值或削减 hash 检查满足任务期限。

检查继续使用降载入口、单任务串行；reader 首版实现只跑匹配风险的定向测试，进入 issuer/协调器接线时再扩大到完整必要门禁。合成测试只验证合同，不能生成 source 39 的真实证据或自动更改生产目录。
