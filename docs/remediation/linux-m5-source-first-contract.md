# Linux M5 source-first 交付基线合同

本文供 Linux 产品、构建和验收维护者审阅，定义新 source 的建立、`26.7.1+40-2 → 26.7.1+40-3` pair、证据连接与验证计划。合同于 2026-11-01 获批，`fde0ecb`/`8800142` 实现仓库准入、装配和 seed 归集，该批完整常规及 22 项可靠性已通过。11-07，`c038602` 的 ARM64 source handoff 完成独立产物核验；11-13 已完成真实 seed 封存、独立审阅及所有者确认，正式 review 与 baseline 随本次 R 提交固定，新 pair 尚未形成。上游结论见[基线审计](linux-m5-delivery-baseline-audit.md)，执行优先级与冻结边界见 [current](../status/current.md)。本文不授权构建、账户/VM/网络/工具链操作、安装、清理、完整验收或发布。

## 目标与边界

- 首个新 source 从 build40 的真实产品包及一次正常首次安装终态建立，不借用旧38包或手写 receipt。
- target 只消费该精确 source，来自其后继 clean commit，保持完整同数据合同；默认保留用户 XDG、既有 production transaction/startup gate、receipt 编码与 FFI ABI 不变。
- 不支持 `38 → 40` 跨 Rime 合同迁移，不改旧 pair/refresh/partial case，不把 macOS Q2/F0/W0 作为前置，不新增签名或远端策略服务。
- 新 source seed 使用通过 fresh-absent 前门的获准 Debian13 ARM64 合成验收环境，优先复用通用 guest；“全新”约束本次安装/数据初态，不强制新建 VM。不扫描真实 home，不导入 P04/旧 snapshot 数据，不复用已冻结的 guest 或 operation。

原设计审阅基线为干净 `dev / 8ab4095`，只表示设计文档来源；实际 S 为 `c038602`，R 为首次提交受审 baseline 的本次提交，T 尚未形成。

## 当前收口与验收出口（2026-11-13）

本合同继续作为唯一 source-first 交付路径，机器格式、S/R/T、配额与授权条件不变。S `c038602` 的真实安装为 `completed`/chain1，seed `d0bd5b7c…c820b` 已经独立审阅及所有者认可；[baseline](../../packaging/linux/source-first-baseline.json)固定正式 review 和 portable 附件摘要。后续 T/pair→代表 crash/连续 L6/当前输入行为→独立 P05C 另批授权；本次不进入 T，不以 R 运行要求独立 T 的 `preflight-pair`。材料与两次临时校验失败的保留记录见[周志](../devlogs/2026-W46.md#2026-11-13-source-seed-所有者审阅与-r-锚点)。

旧 upgrade_quiesced 载体已退役，原历史未完成状态保留，不再恢复旧现场；新 pair 对应边界另取实际证据。旧两项 crash 只在显式适用性审阅后使用，不自动继承。安装维护 XDG 零写入与产品学习/删除写入分开验证，独立 P05C 不由同一连续 L6 session 替代。

退出仍需当前候选完整产品行为、连续维护链、八个自动 crash 合同、三个代表真实恢复边界与独立 P05C，具体按[路线图](../roadmap.md#m5linux-fcitx5-离线输入与个人化产品)。macOS 遗留、其余五个真实 crash、额外平台和公开发布不是本合同出口。新增任务限当前真实失败直接阻塞该出口；无此依据不扩张环境/验收框架。

## 三个提交角色与证据顺序

| 角色 | 必须固定的内容 | 进入下一步的条件 |
| --- | --- | --- |
| S：source 构建提交 | 经批准的 seed 构建/证据工具，完整源码与锁定依赖，product 为 `26.7.1+40-2`；构建前 tracked/untracked 均干净 | 在批准 builder 生成 source 包与 evidence，经独立 actual artifact 核验；再在批准 clean guest 取得 source 安装终态及 host 交叉检查 |
| R：source 锚点审阅提交 | S 的包/evidence身份、seed证据清单及独立审阅记录摘要；S为R严格祖先，R仍为`40-2`元数据 | 项目所有者审阅真实材料后，提交新命名的 baseline 声明；没有真实材料时不存在“已批准空锚点” |
| T：target 构建提交 | R为T严格祖先；target为`26.7.1+40-3`，与S不同；新case/工具通过匹配验证 | source复验成功后才构建target，逐侧actual `.deb`核验、验证执行器身份、形成新pair证据并独占发布 |

先后关系为 **S源码 → source包 → source安装/host证据 → 审阅记录 → R锚点 → T源码/target包 → pair证据**。source构建证据不包含将来的R/T SHA，审阅记录不包含自己的摘要或R SHA；R只固定既已完成材料的摘要，pair构建结果再记录实际S/R/T谱系，避免循环绑定。

目标提交可以包含同合同产品缺陷修复；不能为了凑不同commit重签、改名复制source包或编造evidence。target仅修改Debian revision也可形成真实独立构建，但应如实注明没有新业务行为。target存在新缺陷时保留失败，不覆盖source，不自动重建成功样本。

## 候选身份与同合同规则

| 项 | source | target |
| --- | --- | --- |
| Product/包 | `radishlex-linux` / `radishlex` | 相同 |
| Product/build | `26.7.1` / `40` | 相同，不改变macOS/Flutter build |
| Debian revision/version | `2` / `26.7.1+40-2` | `3` / `26.7.1+40-3` |
| 包文件 | `radishlex_26.7.1+40-2_arm64.deb`及同名`.evidence.json` | `radishlex_26.7.1+40-3_arm64.deb`及同名`.evidence.json` |
| 平台/载体 | Debian13/trixie、arm64、`aarch64-linux-gnu`、`debian-local-deb-v1` | 相同 |
| 数据边界 | ABI9、userdb schema9、`xdg-v1`、`debian-system-v1`、settings/privacy v1、完整Rime identity | 相同 |
| Rime锁 | `5722b2f31769758d392db395e0c367f2a30e189f25e7cc23bf05a3e3385698a1` | 相同；重新散列实物并核对完整配置/资源清单 |

除 `debian_revision`、`package_version` 外，S/T 的 Linux product metadata 字段、键集合均须相同；manifest/layout格式、依赖与默认移除语义也须核对，不能仅比较ABI/schema。首版保持Cargo.lock、Flutter锁及Rime锁字节一致，更新依赖不夹带在本批。产品版本、合同、工具链或锁定依赖需变化时重新评审适用域，不在执行中放宽。

T的revision联动product metadata和Debian artifact配置的package/evidence文件名及现有派生校验；不只修改一个Version字符串，不修改Manager/macOS的build40镜像。新profile严格区分candidate、已审baseline与已发布pair，任意profile/格式交叉加载均拒绝。

actual package 的 control Version/arch、product manifest、Manager/Fcitx版本、两份FFI和完整payload必须各自一致。source/target 包、artifact evidence、product manifest摘要须分别不同；依赖清单身份保持一致。source/target均用相同受支持构建profile取证，实际SQLite版本/source id须核对锁定3.51.3来源，不能由Cargo.lock或macOS库代替。

## Source 建立与准入

### 1. Source 载体先建立

seed artifact 构建复用已有 [Manager](../../scripts/build-manager-linux-product.sh)、[addon stage](../../scripts/build-linux-product-addon-stage.sh)、[rootfs](../../scripts/linux-product/rootfs.py)、[Debian artifact](../../scripts/build-linux-deb-artifact.sh)及 actual verifier，不调用旧pair builder来生成source。

构建输入限定为S的干净源码、已冻结工具/依赖与一个明确absent输出根；记录实际Debian/CPU、工具版本、环境profile和构建配方。复验L1–L5适用层：完整payload/ELF/RPATH/依赖/字体/Rime/license、双FFI一致、实际`.deb`关系及production/verifier身份。构建入口不安装包、不调用maintenance、不打开用户DB，也不自动安装依赖。

调用现有脚本本身不证明依赖离线；实际申请须核对Cargo/Flutter/native缓存、依赖冻结及网络隔离，保证构建不自动下载SDK或依赖、不改锁文件。缺项在编译前停止；不为通过新入口改全局配置或静默联网补齐。

完成后按basename/size/SHA-256冻结source包、artifact evidence、product manifest、production maintenance和actual verifier身份；库中完整性校验不得以同版本重建物替代。变换绝对构建根的bit-repeat不是已有保证；重建结果不同即保留差异并停止认领旧source。

### 2. 单包首次安装，不依赖完整 pair

现有[production maintenance](../../platforms/linux-product/src/system/host.rs)的 `start/kind=install`只需要target `.deb`/evidence。seed安装时该target角色承载未来pair的source包；不把它误记成`40-3`，不新增operation kind、不用acceptance程序伪装生产入口，也不手工调用dpkg代替事务。

系统申请须先固定真实guest/boot、专用合成用户、精确输入目录、preflight、执行器、单一operation、预算和保留方式。guest初始package为not-installed，安装state root、目标程序和指定XDG路径均满足fresh-absent预期；未知旧receipt、历史、半配置、guard或运行中程序均拒绝，不能清理后继续冒充fresh。

Fresh-absent 由实际安装前状态判定。已使用过的通用 guest 只要通过上述前门，就可承担首次安装。常规安装和测试优先复用通用 Debian，确有无法满足的隔离需求才评估新增 VM。[只读前门](../runbooks/linux-source-first-assembly.md#v3-seed-只读前门)

一次授权只允许一次production安装调用。成功准入必须同时满足：

1. mutation前后重新核对实际source包/evidence、staged关系、输入根owner/mode/link与同一执行器；真正的系统观察来自固定production port，人工静止说明不替代进程/映射核验。
2. canonical receipt严格解析，`install/not_applicable/completed`、chain长度1、initial package absent、source artifact/proof为空、target为完整source artifact且proof为已安装；failure为空、manual recovery为false。receipt/root和实际staging/产品保持既有关系，未知材料拒绝。
3. dpkg数据库为同一package/version/arch且完全installed，audit无异常；完整product inventory/manifest/MD5及独立SHA-256关系匹配source；直接读取实际包管理状态，不只信CLI退出或receipt终态。
4. Manager/Fcitx只读startup均为预期`AllowedProduct`；无产品业务进程或包管理mutation残留，guard文件/锁状态符合既有正常终态。不得为观察创建新guard或把probe副作用当作原状态。
5. dependency/font family/glyph/owner及断网证据成立；安装前后指定XDG/profile fingerprint均为约定fresh-absent。这里不启动产品或写学习fixture；正常输入/学习另行验证。
6. guest-agent结果与固定结果/证据文件回读一致；host对唯一guest与正常停止作交叉检查，并确认source输入及保留材料身份稳定。退出码0、detached声明或一次空输出均不足以通过。

seed失败不重跑、恢复snapshot、修复receipt或自动卸载。若安装已完成而host收证失败，保留实际终态并标记证据不完整；后续只读收证须另行授权，不补写原attempt为完整通过。新seed需要新attempt及授权，不能覆盖失败根。

### 3. 形成受审 source 锚点

source锚点同时绑定 **构建来源、精确artifact、真实安装后置、host收证和独立审阅**。受审声明只固定摘要，不包含包正文、用户数据、raw receipt/operation ID、PID、宿主路径或dpkg全文；摘要只证明字节，信任起点仍是明确审阅真实材料后提交的仓库声明。

R声明至少固定下列对象，均须非空且严格匹配；实际材料缺失时不生成声明，不用全零SHA、`null`、`approved=true`或测试fixture补齐：

| 组 | 固定内容 |
| --- | --- |
| 身份 | 新baseline/profile、source完整S SHA、metadata与构建配方摘要、平台和工具环境证据摘要 |
| Artifact | source包/evidence的basename、size、SHA-256；manifest、依赖身份；production/verifier的S来源、ELF/profile、size/hash |
| Seed | seed采集协议版本、独立attempt标识、guest/boot/停止证据的脱敏引用、operation ID SHA-256、receipt SHA-256及完整目标artifact/终态摘要、actual postflight摘要 |
| 收证与审阅 | 严格闭合的guest/host材料清单摘要、审阅记录摘要；记录通过哪些原始观察核对receipt/root/staging、dpkg、startup、XDG和正常停止，以及失败/补收证关联 |

raw receipt（现有限额64KiB）和必要系统原文仅在获准的私有现场保存供审阅；portable seed envelope不导出它们，不能用未经核对的摘要替代审阅。环境私有根与原始路径在执行申请中明确，不进入portable record。审阅记录位于seed包之外，不把其自身或未来R提交反向塞入seed manifest。

## 新声明、pair 与旧合同隔离

首版采用**显式新profile + 新命名的仓库声明**，保留旧38 pair、`SOURCE_COMMIT/SOURCE_CHAIN_ANCHOR`、refresh及历史case原样。不把旧工具放宽为接受任意外部JSON/public key，也不自动寻找“最新通过”的seed。

- candidate声明（`packaging/linux/source-first-candidate.json`）只定义本合同的版本/平台/seed要求，允许源包构建与准入材料采集；它没有source锚点，不能进入pair target构建。
- baseline声明（`packaging/linux/source-first-baseline.json`）只在R阶段、真实材料审阅后产生，含上述source锚点及目标相邻revision约束；这是首版唯一可供新pair入口采用的来源。两类声明分别严格解析，不能将缺anchor当作首次路径。
- 新profile及portable格式分别为 `debian13-arm64-source-first-v1` / `radishlex-linux-source-seed-evidence-v1` / `radishlex-linux-source-first-pair-evidence-v1`；仓库已实现严格解析，格式存在不代表生产许可。

上述baseline profile与既有matrix profile分开：复用`debian13-arm64-ephemeral-v1`的六operation、八自动crash、路径/字体/startup合同及acceptance evidence编码，不改其格式。新case另绑定baseline、真实pair摘要和精确执行器；digest未产生前不填占位case，旧case不能因共享matrix profile而认领新pair。

T构建前复验S/R/T严格祖先链、干净源码、R声明的committed字节、S/T metadata与锁、source包/evidence及seed收证引用；失败在target构建前停止。source仅做exclusive copy和完整核验，保持精确文件字节；source原始构建根可以不在builder上，需从Git取S的committed输入摘要，不能要求重建S产品。

新pair builder只构建T的target包、production maintenance/verifier与编译隔离的acceptance；source seed使用S执行器，后续pair的系统操作使用T的production执行器，两者身份分别记录。T actual verifier逐侧读取source/target实际`.deb`与evidence，核对完整关系；raw源seed材料由独立审阅/清单核验，不由Rust artifact verifier假称验证历史安装曾发生。

pair证据绑定S/R/T、R声明/seed/审阅摘要、双侧artifact身份、shared contract、目标构建环境与T执行器身份。继承现有AArch64 loader/生产无acceptance标记/acceptance含编译身份的检查，执行器彼此不同；这证明角色分离，不替代真实进程行为。

发布只进入明确absent的私有staging，逐项fsync、重哈希闭合inventory后no-replace原子发布，父目录同步成功才报告完成。失败现场保留，不覆盖已有handoff，不让现有builder成功清理策略顺带删除本轮失败材料。host证据目录0700/文件0600；handoff内发行artifact沿用0644/single-link规则并由私有根保护。相同字节inode替换、链接、owner/mode漂移、未知条目或目录冲突均失败关闭，不自动retry。

## 证据与资源上限

source seed和新pair的portable JSON沿用canonical UTF-8 pretty JSON加末尾换行、严格未知/重复键拒绝，每份最多1MiB；附件相对路径安全且不超过180字节，不允许绝对路径、`..`或链接。清单限制512项，总计最多16MiB的脱敏结构化附件；包和ELF分别独立核验，沿用512MiB/256MiB上限，不作为JSON附件。原始现场材料不因超限自动截断或删除，超限先审合同，不拆包绕过。

证据清单记录输入前后identity/hash，闭合全部必需文件，不扫描未知根补齐成功材料。不在record保存输入内容、密钥、真实用户资料、raw operation ID、PID、proc/dpkg全文或构建路径。没有新增签名权威、策略新鲜度服务、Keychain或外部存储；审阅提交不能当作任何系统mutation授权。

资源遵循[本机README](/Users/luobo/VirtualMachines/README.md)、现有managed预算及最多一台运行VM串行安排；干净基线只备份。目录整理后旧partial控制先审迁后路径/身份绑定，再逐步申请处理与后续disposable配额；seed guest的正常停止/证据冻结不自动允许退休，退休另批。保留seed证据与artifact后，未来验收可在另一fresh guest重新安装同一source包，不要求seed guest常驻注册；不得据此删除其原始材料。builder、seed、crash、连续L6、P05C角色逐项安排，不能默认多占guest或重用冻结/P04 guest。

## 验证计划与准入产物

| 批次 | 覆盖与通过证据 | 预算/授权边界 |
| --- | --- | --- |
| V1：仓库定向合同 | candidate无anchor不能构建target；合法合成seed/锚点/pair链；S/R/T错序、同commit、dirty/untracked、metadata/锁/依赖变化、错版本/非相邻revision、缺终态或只CLI成功、错误执行器/guest/receipt绑定全部拒绝 | 实施后先列精确测试名与命令，首批纯解析/编排建议最多5分钟、串行一次。合成成功不生成R生产声明；本批精确入口和测试名见下文 |
| V2：文件与失败保留 | 非canonical/重复键/超限、symlink/hardlink/inode替换、相同版本不同字节、混包、未知文件、已有输出、fsync/rename/发布后父目录同步失败、panic/部分初始化均保留可诊断现场；非执行入口mutation计数为0 | 只用预先登记的新合成根，精确触发一次，不改系统权限/服务或旧现场；不为本批测试扫描清理历史残留 |
| V3：source真实准入 | 获准builder的L1–L5适用检查、实际SQLite/Rime/双FFI身份；source seed首次安装六项条件及host回读/停止；实际材料独立审阅后才提交R | 构建工具/路径/缓存与耗时未知，不能借用历史20/45分钟授权；在环境落实后另给精确命令、预算、资源/收尾范围，未定不执行 |
| V4：target与pair | T干净后继、source输入前后不变、target L1–L5、双侧actual verifier、production/acceptance区分、证据完整与安全发布 | 先定向合同，再单独批准必要完整常规/适用可靠性及ARM64真实构建；缺依赖不自动下载。只读字节核验与真正构建分别报告 |
| V5：M5产品链 | 新pair计划各一次三类代表crash：`install_prepared`、`install_artifacts_staged`、`upgrade_quiesced`；另以一台fresh guest形成连续六operation，随后独立guest P05C和当前候选隐私/学习/删除/重启 | 三类以外五个真实crash继续hardening，不扩大为八项实机全跑；完整专项、系统mutation/进程终止分别申请。旧两项真实通过不冒充新pair通过，旧第三场景的资产收口也不替代新pair资格 |

V5的crash样本可各自使用获准的新snapshot/guest并保留各自失败；不把它们拼进连续主序列。连续L6是一个明确的验收session，六operation之间不还原snapshot、不换guest；计划内reboot允许新boot ID，但须连续绑定前后receipt/package/XDG证据。source seed不是该session的第一个operation，主序列仍从fresh install_source开始。产品学习会改变XDG，只能在单独授权的产品行为步骤取证，不能抹掉安装维护零写入基线。

## 首批实现范围与明确停止点

已批准的仓库实现范围只包括：新candidate/seed/baseline严格验证与有界合成用例；一个调用既有产品构建步骤的单artifact装配入口；source seed只读证据校验；新命名pair入口及target有界编排。可复用 [source copy原语](../../scripts/linux-product/l6_source_anchor.py)，但其现有完整实现必须经过新风险用例核对，不能因摘要相同而跳过读取窗口身份检查。

具体文件分工由实施批固定在`scripts/linux-product/`浅层和少量`scripts/`入口；保留旧接口与常量，不预建通用profile registry或空转wrapper。新命名的candidate配置可随实现提交；包含真实source摘要的baseline配置必须等V3完成，不能先提交测试值。原receipt/事务/data contract/FFI、默认数据保留、锁和production/acceptance隔离均不更改，无新依赖或SDK下载。

原设计批仅交付合同和状态同步。2026-11-01 已批准恢复 V1/V2 仓库实现；这不等于批准V3–V5实机动作。第一实施批只允许仓库实现与有界定向检查，完整验收另行申请。候选编号、环境、目标或协议范围实质变化时重新审阅；失败先诊断、不自动重跑、不推送。该实施批当时尚无source锚点、pair或M5退出证据；当前资格以上节为准。

## V1/V2 首批实施记录（2026-11-01，fde0ecb）

本节保留首批范围与当时结果；后续装配/采集/target仓库实现已在`8800142`提交，详见下节及[装配runbook](../runbooks/linux-source-first-assembly.md)。两次提交均不自动分配S/R/T身份。

本批交付独立的 [candidate](../../packaging/linux/source-first-candidate.json)、[严格格式](../../scripts/linux-product/source_first_contract.py)、[只读准入](../../scripts/linux-product/source_first.py)、[读取与发布](../../scripts/linux-product/source_first_io.py)和[source精确拷贝](../../scripts/linux-product/source_first_stage.py)。不更改旧38入口/常量、refresh、matrix、receipt或production行为，也不创建生产baseline或新case。

- CLI只有`validate-candidate`、`verify-seed`、`preflight-pair`，没有构建、采集、安装或生成审阅锚点的命令。pair准入只读取明确R提交的`packaging/linux/source-first-baseline.json`，拒绝外部baseline和candidate冒充。复验S/R/T严格祖先、R字节在T保留、完整metadata/三份锁、S配方、artifact文件名联动、私有材料及source包/evidence读取前后身份；tracked/untracked/submodule及既有忽略构建输出均拒绝。
- portable seed首版严格闭合12个角色附件，每个含相同attempt、guest/boot/operation摘要、source和S production执行器绑定。receipt摘要保留完整target artifact/proof及fresh/chain1/completed条件；dpkg/inventory/startup/guard/dependency/font/offline/XDG/readback/正常停止分别检查，guest/host结果与清单摘要须一致。baseline另绑定seed、inventory、receipt、guest结果与独立review；review不包含自己的摘要或未来R。
- `verify-seed`只核对脱敏结构化摘要的格式、绑定和文件字节，不读取私有raw receipt/proc/dpkg、调用Rust actual verifier或证明历史安装确曾发生。摘要中的通过值和digest必须在V3独立核对raw材料，提交R才形成受审信任起点；合成fixture仅在新临时根内，不进入生产声明。
- 新读取路径使用有界descriptor、no-follow、single-link/owner/mode、路径与descriptor前后identity/hash检查；相同字节inode替换也拒绝。source只exclusive copy，失败保留部分文件及`failure.json`。发布闭合文件/目录inventory、逐文件/目录fsync、重读、no-replace rename和父目录fsync；同步或发布失败保留staging或已发布目录，不报告完成、不清理/重试。尚未调用于真实handoff。
- 本切片只完成V1/V2的解析、准入和文件主路径。单artifact装配入口、真实环境/离线依赖前门、seed采集与raw receipt核验、actual双侧verifier以及target构建编排仍是进入V3/V4前的后续仓库工作；不存在可执行的真实构建承诺。完整验收与VM/系统动作各自另批。

本轮精确命令（串行一次，5分钟测试预算）：

```bash
./scripts/run-low-impact.sh ./scripts/check-linux-source-first.sh
```

该入口先检查candidate，再执行`test_source_first.py`中的`SourceFirstTests`。预登记新临时根和精确用例（现共41项），保存plan、每例目录、tests.log/result；超时只中止当前测试调用自身，无会话进程扫描或外部kill。所有合成材料保留，不扫描或清理历史根。后续完整仓库门禁已接入该入口，本轮不执行完整门禁。

精确测试名如下：

```text
test_baseline_absent_candidate_cannot_admit_target
test_bool_cannot_replace_count_or_size
test_candidate_cannot_be_baseline
test_copy_input_replaced_after_first_read_preserves_output
test_copy_panic_preserves_first_file_and_diagnostic
test_copy_partial_initialization_preserved
test_dirty_tracked_rejected_without_mutation
test_exact_source_copy_and_publication
test_existing_output_is_preserved
test_fsync_failure_preserves_partial_file
test_git_roles_equal_reversed_or_unrelated
test_hardlink_and_mode_owner_drift_rejected
test_host_guest_readback_mismatch
test_ignored_build_output_rejected
test_initial_directory_sync_failure_preserves_directory
test_metadata_locks_dependency_and_revision_drift
test_missing_terminal_cli_only_and_incomplete_postflight
test_mixed_evidence_rejected_before_output
test_noncanonical_duplicate_bom_and_oversized_json
test_observation_guest_boot_operation_and_executor_binding
test_pair_evidence_roles_dependencies_and_distinct_target
test_publish_conflict_never_overwrites_destination
test_publish_file_sync_failure_preserves_staging
test_publish_parent_fsync_failure_preserves_published
test_publish_rename_failure_preserves_staging
test_publish_same_byte_inode_replacement_rejected
test_r_baseline_cannot_be_changed_in_t
test_receipt_artifact_and_source_proof_binding
test_replacement_during_read_rejected
test_review_and_receipt_digest_cannot_be_substituted
test_same_byte_inode_replacement_rejected
test_same_version_different_bytes_rejected
test_seed_unknown_file_and_empty_directory
test_static_candidate_gate_accepts_source_and_target_without_anchor
test_symlink_file_and_parent_rejected
test_target_artifact_filename_drift_rejected
test_unknown_fields_profiles_and_zero_digests
test_unsafe_attachment_paths_counts_and_sizes
test_untracked_rejected_without_cleanup
test_valid_git_lineage_readonly_and_no_build
test_valid_seed_and_baseline_summaries
```

结果：2026-11-01 原40项串行一次全部通过，result计时3.913秒（unittest报告3.912秒），未重跑。计划、逐例合成根、日志和result保留在`/private/var/folders/dp/rzjx58m54kng6tn5k5gjw71h0000gn/T/radishlex-source-first-v12.U5uVyj/`。入口报告nice优先级调整被沙盒拒绝，不能声称调度降级已应用；测试正常完成。没有VM/系统动作、真实构建、生产baseline、pair或完整门禁结果。

收尾发现静态candidate检查只接受40-2会阻塞未来合法40-3。已修正为严格接受既定source或target metadata，静态检查仍不授予target准入；合成S fixture也在真实T版本检出时固定回source配置。新增`test_static_candidate_gate_accepts_source_and_target_without_anchor`首次独立执行一次通过（result计时1.004秒，unittest报告1.003秒），覆盖T版本检出下的合成S/R/T全链，原40项未重跑；新结果位于`/private/var/folders/dp/rzjx58m54kng6tn5k5gjw71h0000gn/T/radishlex-source-first-target-gate-e6iryg63/`。执行方式为`./scripts/run-low-impact.sh env PYTHONDONTWRITEBYTECODE=1 python3 -`，只装载上述一个`SourceFirstTests`用例、预登记新根、300秒自身调用预算和结果回读。该次也报告nice受限。当前41项全集尚未在最终快照重跑；完整验收仍待单独批准。

## 装配、采集与target编排实施批

2026-11-01 项目所有者批准提交V1/V2首批并继续补齐三条入口；首批已提交为`fde0ecb`，不将其认领为S或R。[实施runbook](../runbooks/linux-source-first-assembly.md)固定单artifact与target入口、离线环境、私有capture/Rust receipt桥、失败保留和本轮新增用例计划。旧版本/pair/receipt持久格式和production mutation不变；真实执行、完整验收与VM动作未获本轮授权。

本实施批结果：Rust5项和定向Clippy通过；Python首项因测试临时根断言位置错误失败，断言已修正但未重跑，原其余25项未执行，连同新增Git来源拒绝项现27项待另批复验。失败根/日志保留，具体命令、证据和未验证范围见实施runbook；本轮无VM、真实构建/seed安装、新锚点、完整验收或推送。

2026-11-01另行批准的27项复验在第5项停止：4过、1失败、22未执行，1.082秒。失败来自夹具多层mkdir中间materials目录0755，collector在注入前正确拒绝；已改逐层私有创建并补回调计数/精确异常断言，未重跑，不放宽生产规则。新失败根为 /private/tmp/radishlex-source-first-pipeline-review-20261101-78nxopac/，两批现场保留，仍待27项另批复验。

2026-11-01再次获批的27项复验在新根 /private/tmp/radishlex-source-first-pipeline-review2-20261101-a1uyuhkz/ 串行一次全部通过，7.847秒，无失败/遗漏、未超300秒；被测21文件与上轮摘要匹配。夹具权限、实际注入/拒绝原因、source/target装配、私有seed归集及发布失败保留取得本轮合成证据；不替代Linux实际构建或V3–V5。两批失败材料继续保留，未重放旧41项/Rust/Clippy，未操作VM、执行完整验收或推送。

2026-11-01完整验收经另行批准执行一次：常规109.77秒失败，Linux product库122过/9 startup失败，exit101，可靠性未启动。监护启动为保护文件设置的umask077被门禁继承，0644创建请求变为0600，被原startup读取前门拒绝；独立新文件诊断确认此机制，恢复基线022后是否通过仍待定向验证。nice19/并发1正确落实，源码快照前后相同、无超时/残留进程，不自动重跑。证据根 /private/tmp/rlxgate-source-first-20261101-5k0b7p3u/ 保留日志/快照/采样；9项原startup夹具被既有Drop清理，未重建。下一步先修正监护子进程umask并补panic夹具保留，再单独申请9项有界验证，不盲目重放完整门禁。

2026-11-01 所有者明确批准修正监护、补失败夹具保留及9条exact测试，总300秒。仅startup测试TestSite Drop增加panic时保留目录和路径输出，正常清理/生产权限不变；监护077与gate子进程022隔离并预检。9条串行各一次全部通过，总5.927秒，每条1 passed/123 filtered out，快照前后相同、无超时/遗留进程。证据根 /private/tmp/rlx-startup-targeted-20261101-115ixjwp/，失败原材料不重建；panic保留分支已编译，但本轮9项通过未触发panic注入。最终全仓/统一feature图及可靠性仍待新一轮授权，未运行完整门禁、VM或推送。

2026-11-01 新一轮经明确批准，在修正umask监护前门下，两条完整命令串行各一次均通过：常规723.57秒、可靠性1044.31秒（2/16/4共22项），总1767.88秒；HEAD为fde0ecb加22文件，前后diff/status/untracked摘要一致，无超时/监护错误/遗留进程，无重跑。证据根 /private/tmp/rlxgate-source-first-corrected-20261101-58tjbih5/，常规包含41项基础和27项新pipeline。新Linux实际构建/SQLite/双FFI、真实seed/R锚点、pair及V3–V5未执行；自动验证不能替代它们。本批仅结果文档收尾或提交不重跑完整门禁，后续开发按风险定向，稳定阶段再集中验收。
