# Linux M5 交付基线与剩余门槛审计

本文面向 Linux 产品、构建与验收维护者，记录回归 M5 主线后的候选、源码约束、证据缺口和串行顺序。状态：**2026-11-10 收口顺位复核：source-first 合同及 V1/V2 仓库实现已完成；11 月 7 日 `c038602` 的 `40-2` ARM64 handoff 已核验，seed、R、新 pair、连续 L6 与独立 P05C 尚未完成。** 下节审计起点及只读结果保留 11 月 1 日语境，不覆盖后续产物或 11 月 8 日退役事实。 本文不授权修改冻结 pair、扩大 VM 预算、启动/移动 guest、安装、发布或下载依赖。执行优先级见 [current](../status/current.md)，长期退出标准见[路线图](../roadmap.md#m5linux-fcitx5-离线输入与个人化产品)。

## 审计结论

项目所有者已明确同意回归 Linux M5：先审交付基线与剩余门槛，同步调整 current。起点为干净 `dev / 55325bafcbb06a10b7ae6c5f7b4967c9b0ad1c9a`，领先本地 `origin/dev` 11 个提交，未 fetch。该提交仅为审计起点，不是未来 source artifact 的冻结提交。

1. M5-P01–P04、P05A 的历史结果保留；P05B 已有包/事务/startup gate 和自动合同，但第三个代表 crash、连续 L6 与独立 P05C 未闭合。
2. 当前产品元数据为 `26.7.1+40-2`；冻结 L6 是 `38-1 → 38-2`。不能用新代码直接接旧 pair，也不能把旧 pair 的历史结果当作 REV-01/REV-02 新候选资格。
3. 审计起点尚无新 pair 生成入口（后续实现见执行顺序）：旧 builder 固定 source commit、包与 evidence 字节；maintenance refresh 只更新维护程序，禁止产品元数据变化。新基线需要先设计 source 首次建立与终态锚定，再形成同合同相邻 revision pair。[生产 maintenance CLI](../../platforms/linux-product/src/system/host.rs)的 `install` 接受 target-only `.deb`/evidence，不依赖 pair record；可用此既有事务建立 source，避免“必须先有完整pair才能取得source锚点”的循环。这里只确认源码入口存在，未执行安装。
4. macOS Q2、F0、最终取消、WAL 内部故障宿主及 W0 不作为 Linux M5 的默认前置；保留其未关闭状态、guard/gate 和冻结现场。共享隐私与 SQLite 修复则必须由新的 Linux 实际载体复验。

## 候选与兼容边界

| 对象 | 可核实的仓库身份 | 当前用途与缺口 |
| --- | --- | --- |
| 历史 L6 source | `55351f2`，`26.7.1+38-1`；包 `09ed1228…69bec`、evidence `fe3d6297…fcf94`，完整值在[冻结声明](../../packaging/linux/l6-release-pair.json) | S2 prior-terminal 的精确已安装源；只支持原历史链。本轮未访问原包、磁盘或外部 evidence |
| 历史 L6 target/case | 声明 target 为 `26.7.1+38-2`；[第三 crash case](../../packaging/linux/l6-upgrade-quiesced-case.json)绑定 `d75818f` 历史 handoff | 旧 case 仅作历史证据；partial/S2 及相关载体已退役，不能沿 Move 续跑、重建或混入新 pair；该恢复边界须在新 pair 取证 |
| 当前 Linux 源码候选 | [product.json](../../packaging/linux/product.json)为 `26.7.1+40-2`，Debian13 ARM64、ABI9、schema9、XDG v1、settings/privacy v1；macOS/Flutter build 同为40 | 11 月 1 日只审元数据；11 月 7 日 c038602 handoff 已独立核验。产物核验不等于 seed 安装/学习/运行通过，terminal source 仍待实际证据 |
| 新的同合同 pair 方向 | 建议从当前 `40-2` 建立真实 source，再由 clean descendant 形成 `40-3` target | 已批准 source-first 的同合同顺序；R/T 仍未形成。source 安装终态、摘要及真实 target 单独固定；若版本/合同实质变化，重新审阅适用域，不放宽原合同 |

新 pair 选相邻 Debian revision，目的是复验当前受支持的同合同安装维护主路径，不新增 `38 → 40` 跨合同迁移支持。生产 [artifact relationship](../../platforms/linux-product/src/debian/relationship/version.rs)及[事务身份](../../platforms/linux-product/src/model.rs)均要求相同 data contract；其中包含 Rime 锁。即使 schema9、ABI9 相同，Rime 锁改变也不能豁免。

本次比较旧 pair 的全部 `shared_contract` 字段，仅发现以下两项不同；这不等于已证明二进制或运行行为兼容：

| 字段 | 冻结 pair | 当前 product |
| --- | --- | --- |
| `build_number` | `38` | `40` |
| `rime_data_lock_sha256` | `5d5a2959f7d40256ae8b530e9bcaada57cc40d5c2608c49104b2c4a804f6759d` | `5722b2f31769758d392db395e0c367f2a30e189f25e7cc23bf05a3e3385698a1` |

当前 Rime 锁实物 SHA-256 与当前 metadata 相等；schema 的 `enable_user_dict: false`、runtime 隐私修复及锁定 SQLite 升级的仓库结果见 [REV-01/REV-02](product-review-2026-09.md)。当前 Linux 包仍须实测配置、双 FFI 实际链接 SQLite 身份和隐私路由，不从 macOS 产物或 Cargo.lock 推导 Linux 产物通过。

## 本轮有界只读核验

以下每项执行一次，不是完整 L6 或全仓门禁；均未构建、开 SQLite、创建 evidence 根或访问 VM。Python 禁止写入 bytecode 缓存。

| 入口 | 结果 | 解释 |
| --- | --- | --- |
| `env PYTHONDONTWRITEBYTECODE=1 python3 scripts/linux-product/product_metadata.py validate-source` | exit0 | 当前产品源码/元数据合同通过；不是载体资格 |
| `env PYTHONDONTWRITEBYTECODE=1 python3 scripts/linux-product/l6_release_pair.py validate-frozen-contract` | exit0 | 旧声明仍自洽；不确认外部包或 target 当前合格 |
| `env PYTHONDONTWRITEBYTECODE=1 python3 scripts/linux-product/l6_release_pair.py validate-contract` | exit1，`source/target product contract drifted at build_number` | 对当前 build40 的预期拒绝。独立字段比较另确认 Rime 锁也不同；没有放宽或重跑 |
| `env PYTHONDONTWRITEBYTECODE=1 python3 scripts/linux-product/l6_maintenance_refresh.py validate-contract` | exit0 | refresh 声明有效，不代表当前 root 可构建 refresh |
| 以 `PYTHONDONTWRITEBYTECODE=1` 导入 refresh，调用 `verify_refresh_root(Path.cwd(), require_absent_build_outputs=False)` | exit1，`maintenance refresh must not change target product metadata` | 只读核对 clean repository、修复祖先及 base/current metadata 后拒绝；关闭的只是输出目录 absent 检查，没有禁用产品兼容核验 |

源码核对：[pair](../../scripts/linux-product/l6_release_pair.py)固定 `SOURCE_COMMIT`、`SOURCE_CHAIN_ANCHOR`、source/target版本与完整 shared contract；[refresh](../../scripts/linux-product/l6_maintenance_refresh.py)固定 `80e49ce` base，并要求当前 metadata 与 base 完全相等。因此不能通过只改旧 JSON 的 build/hash 来准入新候选，也不能通过 refresh 携带新 FFI/Rime 包。

## 阻塞清单与关闭证据

| 编号 | 阻塞 | 关闭所需证据 |
| --- | --- | --- |
| B1 新交付链未建立 | build40 不是旧 pair target；当前也没有新 source 终态锚点 | 独立的新 source-first 合同、实际 source `.deb`/evidence、已安装 terminal 与精确身份、clean descendant target；旧 JSON、source 常量和原证据保持 |
| B2 实际载体与后继构建 | 40-2 handoff 已核验，target 尚未形成；不得回退成“尚无 source 载体” | 复用精确 source 字节与既有 L1–L5 证据；target 仍核对工具/离线依赖、双 FFI/addon/Manager/ELF/`.deb` 和实际 SQLite 身份。漂移或缺工具时报告，不自动重建 source、安装或下载 |
| B3 guest 资源与第三 crash | 旧 partial/S2 及相关载体已退役，旧 upgrade_quiesced 历史未完成；现仅保留冻结 P04 专机 | 优先通用 Debian 前门；新 pair 取得 upgrade_quiesced 实际恢复证据，旧两个 crash 分别审适用域。连续 L6/P05C 串行满足独立性与配额，不自动恢复、重建、Move/adopt、清理、扩容或 clone |
| B4 连续 L6 | 六类 operation 现有分散证据不能拼成连续 session；两项已闭合 crash 也不覆盖新 pair | 同一新 guest、同一获准 pair 的 install→upgrade→repair→rollback→remove→reinstall；包含实际载体、receipt/dpkg/process、双端startup正负、dependency/font、XDG保留、断网与reboot。必须明确旧 crash 证据适用域及新 pair 的代表 crash 覆盖，不自动平移 |
| B5 当前 Linux 产品行为 | P04历史实测早于REV修复，当前候选真实隐私/学习/质量复验不足 | 对实际新产品覆盖Wayland主路径、X11适用回归、跨上下文隐私与普通恢复、多session、候选/commit、本地学习/删除/tombstone、Manager同库/重启；只用合成数据，不把安装维护的XDG零写入场景混作学习场景 |
| B6 P05C与退出 | P05B未收口，独立实机验收未完成 | 另一独立guest授权验收、必要常规和全部适用专项、残余限制与M5退出审阅；不扩张为八个真实crash全跑或公开发布 |

REV-06 的根 Rust 1.80 声明与近期依赖 MSRV 不一致仍开放。新 builder 应固定实际可用工具链及复现条件，不能自报1.80通过；若需要工具链/依赖变更再单独确定范围。当前只确认源码约束，没有测旧工具链或修改支持声明。

## 执行顺序

2026-11-10 确认继续[已批准合同](linux-m5-source-first-contract.md)，不再把方案设计、装配入口或 source 构建重复列为未完成起点。`fde0ecb`/`8800142` 的仓库实现与自动结果、`c038602` 的实际 handoff 分别保留；具体动作从[装配 runbook](../runbooks/linux-source-first-assembly.md)进入。

1. 只读核对通用 Debian13-ARM64 的精确 source、receipt、安装状态和指定 XDG；fresh-absent 满足后，另行授权同一目标的 production seed 安装与 host 核对。前门不符先记录冲突，不清理后冒充 fresh。
2. seed 实际终态与独立审阅齐备后固定 R；再由 clean descendant 形成同合同 40-3 target 与新 pair。source 是不可重建的精确输入，缺 R 不进入 target。
3. 按固定新 pair 审定三类代表 crash 的覆盖与旧证据适用域；旧 upgrade_quiesced 现场不可续跑，该边界在新 pair 实测。完成同一 guest 连续六类 operation 的 L6，不以旧分散结果拼接。
4. 在当前候选完成 Wayland/X11 适用输入、多 session、隐私与普通恢复、学习/删除/tombstone、Manager 同库与重启。安装维护零写入与学习写入分别取证。
5. 在专机配额内串行安排独立 P05C，保留前轮证据并按原授权处理资源；必要常规/适用专项与严重缺陷关闭后评审 M5 退出。无法取得独立环境时明确阻塞，不降低独立性要求。

当前交付范围止于固定 Debian13 ARM64 本地 `.deb`。macOS 支线、其余五个真实 crash、额外平台/同步/公开发布及无新热点的性能优化后置；不扩建 VM 或通用验收框架。只有新候选真实失败直接阻塞出口，才新增修复及相关回归，不重设计已通过合同。旧 evidence、pair、JSON 和失败材料不改写，本次规划维护没有重新执行以上任何步骤。

## 验证与停止线

本轮只做上述静态/只读检查、文档/文本/diff检查；未启动全仓、完整Linux专项、容器/VM、GUI、产品构建、签名或系统安装。干净`90e7717`的完整自动通过保留为仓库历史结果，不升级为本轮Linux交付证据。

后续实施先执行与实际变更匹配的格式/静态/定向检查；新pair的actual包与ARM64 ELF需真实目标环境。每次完整门禁、完整L6与P05C分别给出命令、预算、资源影响并取得新授权，失败先诊断、不自动重跑。保留所有既存失败和冻结材料，不触碰真实用户XDG/DB或macOS 39/40现场；不推送或发布。
