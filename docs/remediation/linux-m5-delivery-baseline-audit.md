# Linux M5 交付基线与剩余门槛审计

本文面向 Linux 产品、构建与验收维护者，记录候选、源码约束与 B1–B6 的关闭依据。11 月 1 日的审计起点及只读结果按历史保留，下方阻塞清单已同步 11 月 22 日取证。执行优先级由[当前状态](../status/current.md)维护，长期退出标准见[路线图](../roadmap.md#m5linux-fcitx5-离线输入与个人化产品)。本文不授权修改冻结 pair、扩大 VM 预算、启动或移动 guest、安装、发布或下载依赖。

## 审计结论

11 月 1 日，项目所有者同意回归 Linux M5，先审交付基线与剩余门槛。起点为干净 `dev / 55325bafcbb06a10b7ae6c5f7b4967c9b0ad1c9a`，领先本地 `origin/dev` 11 个提交，未 fetch。以下四项保留该审计起点的判断。

1. M5-P01–P04、P05A 的历史结果保留；P05B 已有包/事务/startup gate 和自动合同，但第三个代表 crash、连续 L6 与独立 P05C 未闭合。
2. 当前产品元数据为 `26.7.1+40-2`；冻结 L6 是 `38-1 → 38-2`。不能用新代码直接接旧 pair，也不能把旧 pair 的历史结果当作 REV-01/REV-02 新候选资格。
3. 审计起点尚无新 pair 生成入口（后续实现见执行顺序）：旧 builder 固定 source commit、包与 evidence 字节；maintenance refresh 只更新维护程序，禁止产品元数据变化。新基线需要先设计 source 首次建立与终态锚定，再形成同合同相邻 revision pair。[生产 maintenance CLI](../../platforms/linux-product/src/system/host.rs)的 `install` 接受 target-only `.deb`/evidence，不依赖 pair record；可用此既有事务建立 source，避免“必须先有完整pair才能取得source锚点”的循环。这里只确认源码入口存在，未执行安装。
4. macOS Q2、F0、最终取消、WAL 内部故障宿主及 W0 不作为 Linux M5 的默认前置；保留其未关闭状态、guard/gate 和冻结现场。共享隐私与 SQLite 修复则必须由新的 Linux 实际载体复验。

## 候选与兼容边界

| 对象 | 可核实的仓库身份 | 当前用途与缺口 |
| --- | --- | --- |
| 历史 L6 source | `55351f2`，`26.7.1+38-1`；包 `09ed1228…69bec`、evidence `fe3d6297…fcf94`，完整值在[冻结声明](../../packaging/linux/l6-release-pair.json) | S2 prior-terminal 的精确已安装源；只支持原历史链。本轮未访问原包、磁盘或外部 evidence |
| 历史 L6 target/case | 声明 target 为 `26.7.1+38-2`，[第三 crash case](../../packaging/linux/l6-upgrade-quiesced-case.json)绑定 `d75818f` 历史 handoff | partial/S2 及相关载体已退役，保留历史未完成记录。新包对的恢复结果见 B3 |
| 当前 Linux source 与 target | S `c038602` 为 `26.7.1+40-2`。T `fa3b2c5` 的 [product.json](../../packaging/linux/product.json)为 `26.7.1+40-3`，两者均为 Debian13 ARM64、ABI9、schema9、XDG v1、settings/privacy v1 | S 的通用机源安装已封存，T 实物已用于连续 L6 与独立恢复验收 |
| 当前同合同 pair | R `65a4147` 固定受审 baseline 与正式 seed，D 绑定 S/R/T 及精确 source 字节 | 后续前门使用同一批[已回读材料](../devlogs/2026-W46.md#2026-11-14-t40-3-日终回顾与批次结果)。合同实质变化时重新审阅适用域 |

新 pair 选相邻 Debian revision，目的是复验当前受支持的同合同安装维护主路径，不新增 `38 → 40` 跨合同迁移支持。生产 [artifact relationship](../../platforms/linux-product/src/debian/relationship/version.rs)及[事务身份](../../platforms/linux-product/src/model.rs)均要求相同 data contract；其中包含 Rime 锁。即使 schema9、ABI9 相同，Rime 锁改变也不能豁免。

本次比较旧 pair 的全部 `shared_contract` 字段，仅发现以下两项不同；这不等于已证明二进制或运行行为兼容：

| 字段 | 冻结 pair | 当前 product |
| --- | --- | --- |
| `build_number` | `38` | `40` |
| `rime_data_lock_sha256` | `5d5a2959f7d40256ae8b530e9bcaada57cc40d5c2608c49104b2c4a804f6759d` | `5722b2f31769758d392db395e0c367f2a30e189f25e7cc23bf05a3e3385698a1` |

当前 Rime 锁实物 SHA-256 与当前 metadata 相等；schema 的 `enable_user_dict: false`、runtime 隐私修复及锁定 SQLite 升级的仓库结果见 [REV-01/REV-02](product-review-2026-09.md)。当前 Linux 包仍须实测配置、双 FFI 实际链接 SQLite 身份和隐私路由，不从 macOS 产物或 Cargo.lock 推导 Linux 产物通过。

## 11 月 1 日有界只读核验

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

| 编号 | 11 月 22 日证据 | 后续要求 |
| --- | --- | --- |
| B1 新交付链 | 受审 S/R 与后继 T 已固定，`40-2` 安装终态为 `completed`/chain1 | 保持精确 source、receipt、seed、review 与 Git 关系，实机前门重新核对 |
| B2 实际载体与后继构建 | D 的两份 ARM64 `.deb`、两端 FFI、实际 SQLite、执行器及交接清单已核验 | 后续 case 绑定 D 的闭合清单与实物，同一 T 无需为该前门重建 |
| B3 guest 资源与第三 crash | 当前包对的独立 S2 副本已闭合 `upgrade_quiesced`、单次生产 resume 和正常停机。前两个历史 crash 的适用性限于共同安装恢复合同。[恢复记录](../devlogs/2026-W47.md#2026-11-22-upgrade_quiesced-恢复与停机) · [适用性审阅](../devlogs/2026-W47.md#2026-11-22-两个历史-crash-的适用性) | 保留恢复机与证据，独立 P05C 优先复用通用 Debian。更换专机须符合配额并逐项授权 |
| B4 连续 L6 | 同一候选与当前包对完成 install→upgrade→repair→rollback→remove→reinstall，包含实际 dpkg、双端门禁、依赖和字体、断网、XDG 与重装后冷启动。[六步实测](../devlogs/2026-W47.md#2026-11-22-连续-l6-六步实测) · [冷启动](../devlogs/2026-W47.md#2026-11-22-连续-l6-冷启动对照与留存) | 保留终态与资料，向 M5 出口评审提交同一连续证据链 |
| B5 当前 Linux 产品行为 | P04历史实测早于REV修复，当前候选真实隐私/学习/质量复验不足 | 对实际新产品覆盖Wayland主路径、X11适用回归、跨上下文隐私与普通恢复、多session、候选/commit、本地学习/删除/tombstone、Manager同库/重启；只用合成数据，不把安装维护的XDG零写入场景混作学习场景 |
| B6 P05C与退出 | 独立实机方案已形成，通用 Debian 离线启动前门待批准。[P05C 方案](../runbooks/linux-p05c-acceptance.md) | 独立 guest 授权验收、适用的常规与专项验证、残余限制与 M5 退出审阅 |

REV-06 的根 Rust 1.80 声明与近期依赖 MSRV 不一致仍开放。新 builder 应固定实际可用工具链及复现条件，不能自报1.80通过；若需要工具链/依赖变更再单独确定范围。当前只确认源码约束，没有测旧工具链或修改支持声明。

## 执行顺序

以下为既定证据依赖。source 构建、seed、R 和 target pair 的记录见[装配 runbook](../runbooks/linux-source-first-assembly.md)及[日终周志](../devlogs/2026-W46.md#2026-11-14-t40-3-日终回顾与批次结果)。各步已取得的证据见上表，实际接续顺位见[当前状态](../status/current.md#下一阶段linux-m5-交付收口)。

1. 固定新 pair、包和执行器的精确身份，准备只读实机前门。先区分通用 Debian 的已安装 `40-2` 状态与连续 L6 所需的 fresh 起点，确定可使用的环境和操作边界。
2. 前门与新 case 的身份绑定可审阅后，另行申请实际 VM 与首项操作授权。
3. 按固定新 pair 审定三类代表 crash 的覆盖与旧证据适用域；旧 upgrade_quiesced 现场不可续跑，该边界在新 pair 实测。完成同一 guest 连续六类 operation 的 L6，不以旧分散结果拼接。
4. 在配额内安排独立 P05C，逐批授权并保留前轮证据。实测当前候选的 Wayland/X11 输入、多 session、隐私与普通恢复、学习/删除/tombstone、Manager 同库与重启。安装维护零写入与学习写入分别取证。无法取得独立环境时记录具体阻塞。
5. 汇总独立 P05C、适用门禁与残余限制，关闭阻塞退出的严重缺陷后评审 M5 退出。

当前交付范围止于固定 Debian13 ARM64 本地 `.deb`。macOS 支线、其余五个真实 crash、额外平台/同步/公开发布及无新热点的性能优化后置；不扩建 VM 或通用验收框架。只有新候选真实失败直接阻塞出口，才新增修复及相关回归，不重设计已通过合同。旧 evidence、pair、JSON 和失败材料不改写，本次规划维护没有重新执行以上任何步骤。

## 验证与停止线

11 月 1 日只读审计的范围限于上述检查及文档、文本和 diff 检查。`90e7717` 的完整自动通过作为仓库历史结果；当前 T 的常规与可靠性专项见[11 月 14 日周志](../devlogs/2026-W46.md#2026-11-14-t40-3-日终回顾与批次结果)，实机结果见 B3/B4。

后续实施先执行与实际变更匹配的格式/静态/定向检查；新pair的actual包与ARM64 ELF需真实目标环境。每次完整门禁、完整L6与P05C分别给出命令、预算、资源影响并取得新授权，失败先诊断、不自动重跑。保留所有既存失败和冻结材料，不触碰真实用户XDG/DB或macOS 39/40现场；不推送或发布。
