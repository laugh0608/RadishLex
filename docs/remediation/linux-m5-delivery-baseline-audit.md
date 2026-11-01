# Linux M5 交付基线与剩余门槛审计

本文面向 Linux 产品、构建与验收维护者，记录回归 M5 主线后的候选、源码约束、证据缺口和串行顺序。状态：**2026-11-01 仓库审计完成；新交付基线尚未建立，未构建或实机核验候选。** 本文不授权修改冻结 pair、扩大 VM 预算、启动/移动 guest、安装、发布或下载依赖。执行优先级见 [current](../status/current.md)，长期退出标准见[路线图](../roadmap.md#m5linux-fcitx5-离线输入与个人化产品)。

## 审计结论

项目所有者已明确同意回归 Linux M5：先审交付基线与剩余门槛，同步调整 current。起点为干净 `dev / 55325bafcbb06a10b7ae6c5f7b4967c9b0ad1c9a`，领先本地 `origin/dev` 11 个提交，未 fetch。该提交仅为审计起点，不是未来 source artifact 的冻结提交。

1. M5-P01–P04、P05A 的历史结果保留；P05B 已有包/事务/startup gate 和自动合同，但第三个代表 crash、连续 L6 与独立 P05C 未闭合。
2. 当前产品元数据为 `26.7.1+40-2`；冻结 L6 是 `38-1 → 38-2`。不能用新代码直接接旧 pair，也不能把旧 pair 的历史结果当作 REV-01/REV-02 新候选资格。
3. 当前实现没有通用的新 pair 生成入口：旧 builder 固定 source commit、包与 evidence 字节；maintenance refresh 只更新维护程序，禁止产品元数据变化。新基线需要先设计 source 首次建立与终态锚定，再形成同合同相邻 revision pair。[生产 maintenance CLI](../../platforms/linux-product/src/system/host.rs)的 `install` 接受 target-only `.deb`/evidence，不依赖 pair record；可用此既有事务建立 source，避免“必须先有完整pair才能取得source锚点”的循环。这里只确认源码入口存在，未执行安装。
4. macOS Q2、F0、最终取消、WAL 内部故障宿主及 W0 不作为 Linux M5 的默认前置；保留其未关闭状态、guard/gate 和冻结现场。共享隐私与 SQLite 修复则必须由新的 Linux 实际载体复验。

## 候选与兼容边界

| 对象 | 可核实的仓库身份 | 当前用途与缺口 |
| --- | --- | --- |
| 历史 L6 source | `55351f2`，`26.7.1+38-1`；包 `09ed1228…69bec`、evidence `fe3d6297…fcf94`，完整值在[冻结声明](../../packaging/linux/l6-release-pair.json) | S2 prior-terminal 的精确已安装源；只支持原历史链。本轮未访问原包、磁盘或外部 evidence |
| 历史 L6 target/case | 声明 target 为 `26.7.1+38-2`；[第三 crash case](../../packaging/linux/l6-upgrade-quiesced-case.json)绑定 `d75818f` 历史 handoff | `upgrade_quiesced` 的 partial/Move 路线继续按旧 case；不能把 refresh 的 `80e49ce` base 或当前源码替换进去 |
| 当前 Linux 源码候选 | [product.json](../../packaging/linux/product.json)为 `26.7.1+40-2`，Debian13 ARM64、ABI9、schema9、XDG v1、settings/privacy v1；macOS/Flutter build 同为40 | 元数据一致不证明有合格 `.deb`、双 FFI、addon 或 Linux runtime 证据。本轮未确认新载体存在或合格，不给它填写包摘要或 terminal source 身份 |
| 新的同合同 pair 方向 | 建议从当前 `40-2` 建立真实 source，再由 clean descendant 形成 `40-3` target | 仅为候选编号与来源顺序，未改元数据或批准构建；最终提交、source 安装终态、摘要、工具链与输出位置须独立固定。若构建前版本再变，重新审计，不硬套此编号 |

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
| B2 实际构建/依赖资格未知 | 本轮未进入 Debian13 ARM64 builder；现有本机门禁不能替代目标产物 | 明确现有 builder、离线依赖可用性、Rust/Flutter/CMake/librime/Fcitx 工具版本、双 FFI/addon/Manager/rootfs/ELF/`.deb` L1–L5 及实际 SQLite 身份；缺工具时先报告，不自动安装/下载 |
| B3 guest 资源与第三 crash | 历史账本为5个 managed 锚点加1个 stopped partial，已到6台预算；partial仅为壳，不是S2副本，状态未本轮复验 | 先按现有 case 另批原生 UI Move、adopt、物化、start/input-preflight/crash/exact resume/stop；形成可追溯终态后才按授权退休并取得下一 disposable 配额。不能默认再建guest或重跑clone |
| B4 连续 L6 | 六类 operation 现有分散证据不能拼成连续 session；两项已闭合 crash 也不覆盖新 pair | 同一新 guest、同一获准 pair 的 install→upgrade→repair→rollback→remove→reinstall；包含实际载体、receipt/dpkg/process、双端startup正负、dependency/font、XDG保留、断网与reboot。必须明确旧 crash 证据适用域及新 pair 的代表 crash 覆盖，不自动平移 |
| B5 当前 Linux 产品行为 | P04历史实测早于REV修复，当前候选真实隐私/学习/质量复验不足 | 对实际新产品覆盖Wayland主路径、X11适用回归、跨上下文隐私与普通恢复、多session、候选/commit、本地学习/删除/tombstone、Manager同库/重启；只用合成数据，不把安装维护的XDG零写入场景混作学习场景 |
| B6 P05C与退出 | P05B未收口，独立实机验收未完成 | 另一独立guest授权验收、必要常规和全部适用专项、残余限制与M5退出审阅；不扩张为八个真实crash全跑或公开发布 |

REV-06 的根 Rust 1.80 声明与近期依赖 MSRV 不一致仍开放。新 builder 应固定实际可用工具链及复现条件，不能自报1.80通过；若需要工具链/依赖变更再单独确定范围。当前只确认源码约束，没有测旧工具链或修改支持声明。

## 执行顺序

**最近的仓库任务是新 source-first 基线合同设计，不是直接运行旧 pair builder。** 设计只覆盖 build40 同合同主路径、真实 source 锚定及显式新 case 的有界复用，不扩展跨 Rime/schema 迁移，不新建通用验收框架。

1. **固定新基线方案**：明确 source候选/冻结时点、首次 source 包生成及终态取证方式、新 pair/case 与旧合同的隔离、target相邻revision和谱系、证据引用、失败保留及每步预算。真实 source 证据未产生前不编造 anchor，不写占位哈希给 builder；实施前审阅具体代码/文件范围。
2. **落实环境资格与资源路径**：仓库可先审设计；真正查询或改动 builder/VM须独立授权。现有资产边界下，下一允许申请的系统动作仍只有旧 partial 的原生 UI Move；后续 adopt/物化/启动/旧第三crash/stop/退休分段授权。若该环境链受阻，记录具体 blocker，不自动扩建框架、创建替代clone、删除未终结partial或提高预算。
3. **建立新 source**：在获准现有构建环境形成L1–L5合格的当前合同包，冻结精确字节与构建提交；在获准新合成根/guest取得 source 首次安装终态及身份。执行中无需第二台并发VM；builder与测试guest角色和顺序在实际资源申请中固定。source未合格则不构建pair target。
4. **形成新 pair 并执行 P05B**：source作为不可重建输入；由clean descendant形成相邻revision target、production maintenance/verifier与编译隔离的acceptance，逐侧actual `.deb`核验。新case先做匹配定向合同，完整L6/专项另批；再执行B4与B5，不将旧profile结果标成新候选通过。
5. **P05C与退出**：P05B完整后，另批独立guest验收。关闭必要隐私/数据/安装缺口后审M5退出；Android/Windows/iOS、真实用户同步和公开发布不提前启动。

旧 `upgrade_quiesced` 的收口服务于当前冻结资产与历史证据链，不能单独宣布新build40交付完成。新 pair 的验收覆盖要在第1步显式固定；不得利用“旧两个crash已通过”跳过新产品的行为差异，也不把所有旧样本机械重建为新一轮完整矩阵。

## 验证与停止线

本轮只做上述静态/只读检查、文档/文本/diff检查；未启动全仓、完整Linux专项、容器/VM、GUI、产品构建、签名或系统安装。干净`90e7717`的完整自动通过保留为仓库历史结果，不升级为本轮Linux交付证据。

后续实施先执行与实际变更匹配的格式/静态/定向检查；新pair的actual包与ARM64 ELF需真实目标环境。每次完整门禁、完整L6与P05C分别给出命令、预算、资源影响并取得新授权，失败先诊断、不自动重跑。保留所有既存失败和冻结材料，不触碰真实用户XDG/DB或macOS 39/40现场；不推送或发布。
