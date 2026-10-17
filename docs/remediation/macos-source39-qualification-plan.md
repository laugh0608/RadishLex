# source 39 独立双程序资格执行方案

本文用于审查准备取消的旧程序资格前置条件、证据与逐步授权，不是当前机器上的启动或释放指令。读者为项目所有者、Installer 与 macOS 验证维护者。状态：**2026-10-17 A1/Q1 已按单步授权执行，路径重定向被拒绝；Q2 仍阻塞，尚无完整运行资格。**

## 当前结论与范围

项目所有者确认没有备用 Mac 或独立 macOS 测试环境，要求保持当前账户与 Agent 会话、不动冻结现场。不得把新增用户、切换登录、VM 或远端设备当作已经存在或默认接受的前提。

在这些约束下，**尚未找到可证明安全、且能覆盖原始 source 39 正常双程序运行的本机隔离路径**。这不是证明 macOS 上绝无实现方式，而是现有证据不足以开放运行。仓库审计和单次短命路径实验已完成；本次仅设置 `CFFIXED_USER_HOME` 未改变任何观测路径，该方案不能作为本机数据隔离前提。只有数据与会话隔离两项都被独立证明后，才可申请精确程序副本的运行授权。

本方案不改变最终 marker 门禁，不新增生产资格提供者、不开放最终取消动作，也不启动 GUI、复制冻结程序、访问真实用户 SQLite 或调整输入源。冻结资产停止线以 [current](../status/current.md) 为准。

## 已知事实与证据边界

| 事实 | 依据 | 对执行的约束 |
| --- | --- | --- |
| 精确 source 是 release 装配 26.7.1-39 | [W41 只读资产审计](../devlogs/2026-W41.md#2026-10-05-后续旧程序资格资产与会话边界审计) | 候选输入为 `target/macos-release/26.7.1-39/Product`，历史 manifest SHA-256 为 `de1bf1ea937f742242975ad05a1d639e1d3899454e9acfb41eb28e089fe8362f`；同版本普通装配摘要不同，不能互换。本轮未重新核验载体或签名 |
| outer gate 绑定真正运行的程序 | [FFI gate](../../crates/ime-ffi/src/product_install.rs) 的 `running_program_identity` | 从 `current_exe` 和数据根推导同一 home 下的固定双 bundle，再验证程序身份。外置 probe、注入、修改 bundle ID 或重新签名不能代表原 source |
| 正常程序先过双 gate，再进入业务路径 | [Manager](../../apps/radishlex-manager/macos/Runner/AppDelegate.swift)、[InputMethod](../../platforms/macos-imk/Sources/main.m) | Manager 的正常路径与 smoke 分支不同；InputMethod 随后创建 `IMKServer` 并进入 AppKit 事件循环，启动进程不等于已打开 runtime 或完成输入学习 |
| 数据目录由 Foundation 用户域解析 | [Manager 路径](../../apps/radishlex-manager/macos/Runner/MainFlutterWindow.swift)、[InputMethod runtime](../../platforms/macos-imk/Sources/RadishLexRuntime.m) | 本次 Q1 中 `CFFIXED_USER_HOME` 未改变观测路径，`HOME` 保持不变。即使其他样本数据路径重定向正确，也不证明偏好、注册、Keychain、输入源与会话连接隔离 |
| 现有 host 只覆盖指定验证子项 | [host runner](../../platforms/macos-product/UpgradeCoordinatorAdapter/src/runner.rs)、[InputMethod validation host](../../platforms/macos-product/UpgradeValidationHosts/Sources/input_method_main.m) | qualification harness 的固定 home 只用于既有 host；不得扩展解释成正常双程序支持的隔离模式 |
| 资格对象不是证据本身 | [取消释放组合层](../../platforms/macos-product/InstallCoordinatorAdapter/src/cancellation_release.rs) | `from_verified_source` 仅检查结构；生产 issuer 必须独立核验 source/schema/覆盖范围，并逐检查点绑定本次 proof、复验程序与静止 |

上述源码为当前仓库定位，结合 W41 对旧程序的审计使用；不能用当前 HEAD 重新构建的二进制冒充精确 source 39。本轮没有反汇编、执行或重新散列冻结载体。

Apple 对 [IMKServer](https://developer.apple.com/documentation/inputmethodkit/imkserver/init%28name%3Abundleidentifier%3A%29?language=objc) 的说明指出连接名称用于建立输入客户端通信；[登录会话说明](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPMultipleUsers/Concepts/SystemContexts.html)说明用户服务与登录上下文的关系。因此“目录隔离不能单独推出会话隔离”是本方案的审计判断。旧系统说明不证明当前系统任何具体私有 bootstrap 操作安全，不据此执行 `launchctl` 会话改造。

## 本机路线与停止判据

### Q0：固定输入与矩阵——本轮已完成

- 固定以上历史 source 身份，区分当前源码、历史只读证据和未来精确副本运行证据。
- 固定下文资格矩阵、证据项、授权包和资源边界。
- 将“完整资格已具备”与“方案/局部验证完成”分开记录；最终取消仍阻断。

### Q1：路径解析的短命可行性实验——已执行，重定向被拒绝

目标仅为回答 Foundation 是否在当前系统按指定临时 home 返回同一组路径；实验不加载 RadishLex dylib、不调用 gate、不建立 IMKServer，也不证明正常启动。

仓库探针准备与静态验证完成后，项目所有者单独批准 A1，按以下固定范围执行了一次实验：

1. 在新建 `0700` 的临时实验目录内编译一个最小 Foundation 命令行程序，只使用现有系统工具链。程序只打印合成目录下的解析结果和布尔校验；不读真实文件、偏好、SQLite 或账户凭据。
2. 串行检查默认环境和仅设置 `CFFIXED_USER_HOME` 的子进程环境；保留 `HOME`、`CODEX_HOME` 和父进程环境，不覆盖系统保留变量。已有 `CFFIXED_USER_HOME` 时拒绝实验，不隐式清除。默认值仅作为字符串对照，输出只保留路径分类，不输出真实 home；本批不覆盖 `HOME` 单独重定向或两变量组合。
3. 核对 Application Support、Rime/userdb/settings 的推导是否都位于同一合成根，及外层 gate 期望的双 bundle 路径形状。所有样本串行，单个子进程必须短命；不通过复制 source 来试探。
4. 重定向样本不一致、解析落回真实用户域或未知结果即记录拒绝并停止。默认对照在合成根之外是预期结果。结果一致只能记为“路径字符串实验通过”，未打开这些业务路径，不能据此声称排除了其文件系统链接或证明实际写入隔离；Q2 仍关闭。

编译与执行均由 `./scripts/run-low-impact.sh <具体命令>` 启动，命令、实验根和所生成文件清单在执行申请中固定。工具缺失时停止，不下载 SDK 或安装依赖。源文件、二进制与输出原样保留，删除另授权。工具为 [Foundation 探针](../../scripts/macos-product/source39-path-probe.m)及[串行 runner](../../scripts/probe-macos-source39-paths.py)；没有 `--execute` 时不编译或运行。它不含 GUI、正常旧程序资格、会话操作或自动 kill/清理。

本次已执行的精确命令（仓库根执行；该目录现已存在并保留，命令不得复跑）：

```bash
./scripts/run-low-impact.sh python3 scripts/probe-macos-source39-paths.py \
  --execute --run-root /private/tmp/radishlex-source39-paths-20261017-q1-a1
```

该命令仅使用现有 Python/Clang/Foundation；在新 `0700` 根内保留 `source39-path-probe.m`、`source39-path-probe`、`module-cache/`、`compiler-tmp/`、`synthetic-home/`，以及 `compile` / `baseline` / `cffixed` 三阶段的 `.pid`、`.stdout.log`、`.stderr.log`，通过校验的 `baseline.json` 和汇总 `result.json`。`cffixed` 未通过校验，只有原始脱敏输出，没有生成 `cffixed.json`。编译子进程的 `TMPDIR` 指向 `compiler-tmp/`，两次探针的环境仅相差 `CFFIXED_USER_HOME`。本次无重试、kill 或清理，后续包无自动授权。

本次结果：runner 退出码 `1`，`status=rejected`、`reason=path_resolution_rejected`，`completed=[compile, baseline]`。编译与两个短命探针均正常返回，三个 stderr 均为空；`cffixed` 的 8 项路径与默认对照完全相同，全部为 `real_user_domain`，`in_synthetic_home=false`。合成 home 为空。路径返回不符合隔离前提，拒绝属于实验的有效负向结果，不改宽校验或重跑寻找通过。

已只读交叉核对源副本与仓库源一致、SHA-256 与结果记录一致，四个私有目录为当前用户所有且 `0700`，输出、PID 与拒绝结果齐全。源 SHA-256 为 `a182e62558a9af0cfd01135c125825be7ab65a9752e87354ae46a08fa7e9d696`，`result.json` SHA-256 为 `9a010da5c6aefd7ddd762c452932e2ea679d4c4d1b1da625fff181dd6bed4828`。这只描述本次最小 Foundation 样本，不推广为所有 macOS 环境或精确 source 二进制的行为。

下一步判断：停止该变量方案的本机运行路线，不进入 A2 复制或 A4 双程序启动。项目所有者随后批准[资格证据 reader 合同](macos-cancellation-qualification-reader-contract.md)及其只读实现/负向测试；生产目录为空，未接 issuer。reader 不能补出缺失的真实运行证据或解除最后 marker。

### Q2：会话隔离准入——当前阻塞

必须在运行任何精确 source 副本之前同时证明：

- 文件写入、偏好和所有业务路径仅落在私有合成根；不触及真实用户数据、冻结 bundle/receipt/staging，也不以链接或临时替换真实固定路径实现。
- 原始 bundle ID、连接名、签名与程序内容保持；输入服务注册、LaunchServices、IMK 连接和客户端选择不会进入或改变当前工作会话。
- Manager 与 InputMethod 能走正常 UI/客户端路径，实际完成打开、再学习和进程退出后重开；不能仅观察 PID、窗口、退出码或调用外置 FFI。
- 失败后能停止本次实验自己的程序并保留材料，无需停止用户进程、切换登录、退出 Codex 或改动冻结现场。

当前没有符合以上要求的已验证机制。因此不提供 `open`、`launchctl bootstrap/asuser/bsexec`、输入源注册/选择或运行旧程序的命令，也不把临时 home、后台调度或自造 Mach namespace 当作隔离证明。Q1 成功不能自动进入 Q3。

**在当前约束未变且 Q2 未被证明前，完整资格执行到此停止。** 后续可以继续仓库证据 reader 的设计与拒绝路径测试；不能靠增加测试次数、默认合格 issuer 或伪造 proof 消除这个阻塞。若未来发现可用机制，应先提交其数据/会话边界证据与具体操作，再申请范围变更；本方案没有要求用户现在新增设备、账户或环境。

### Q3：精确副本资格矩阵——仅在 Q2 成立并单独授权后执行

1. 只读复验历史 source manifest、完整文件树和双程序签名；固定本次 source manifest/双 tree/可执行文件及架构签名标识，不仅检查版本号。漂移即停止，不重新签名或修复原件。
2. 另行获准后复制该精确载体到唯一新实验根，复制前后验证内容与签名；保持相对 bundle 布局，记录副本的新物理身份。禁止使用硬链接、克隆真实用户库或将旧 receipt 中的真实 inode/root 原样移植到实验。
3. 由仓库事务原语和受审 fixture 生成器在合成根生成新 operation/root 绑定的场景；不改活动真实状态。生成器及命令尚需实现评审，不能假称现有脚本已支持全部场景。
4. 逐场景先核对两层 gate 的预期，再正常启动两个精确旧程序；在单独授权的人工输入客户端中完成合成输入、选择与学习，正常退出后重启两个程序。不默认自动点击、合成按键或强制 kill。
5. 每行均须得到下表证据。失败行及未知结果保留，不复跑覆盖；后续场景只能在已证明双端静止且 fresh 授权仍有效时开始。

## 最小完整资格矩阵

以下是覆盖要求，不代表现有工具已能运行。每个维度都必须在覆盖清单中点名，不把单一 happy path 泛化到全部状态。

| 场景/维度 | 正向或负向要求 | 必需证据 |
| --- | --- | --- |
| 维护前取消与维护后完成等价收尾 | 无新 data receipt；原 outer 合法终态，已释放时活动 data 槽为空 | 两个原始程序的正常 gate、实际 runtime 打开、再学习和重新启动；维护前后逻辑内容对照 |
| outer 三类旧终态 | `completed`、`aborted_preserved`、`rolled_back` 分别验证 | 受审合成 receipt，合法程序身份/root 绑定；不能伪造失败原因 |
| 前驱组合 | 无 data 前驱、outer/data ID 不同、旧 v1/v2 历史、重复取消 | 新 reader 精确识别 lifecycle 与 legacy/data 前驱；原件与索引保持 |
| 中间状态 | 准备 marker、取消 marker、部分封存、未知临时对象 | 原 source 按其旧合同拒绝不允许的活动状态，且拒绝前没有业务 DB 写入 |
| 证明/索引错误 | 缺失、错 hash/inode、错 source/schema、错 proof/root、祖先漂移 | 新 Installer/issuer 拒绝；不要求旧 source 理解它从未支持的取消 proof 格式 |
| 旧 runtime 数据语义 | 合成选词/WAL、新词学习、删除/tombstone、配置、同步投影 | 旧 runtime 正常读取/学习前后与重启后的语义断言；同步仅合成离线数据，不连接服务 |
| 后续升级链 | 取消后再准备→新 v1→真实数据终态释放 | 先前取消证明、旧 receipt、保护快照与历史原件不被覆盖；不重用冻结 operation |

先建立隔离的兼容性后置状态样本，验证精确旧程序能正常运行；此样本属于资格试验输入，**不是取消事务成功证据**。不能为了启动旧程序，先给真实取消协调器塞入合成合格 issuer 并释放 marker，再用启动成功反证 issuer 有效。随后才设计可信 issuer 将兼容性覆盖与本次完整 proof/family 关系关联，并在独立合成事务中验证最终释放。这两个阶段都不能代替真实用户操作的授权。

## 证据与失效条件

每次运行应有独立 run ID 和不可覆盖的清单，至少含：

- 仓库工具提交、source release/manifest、两个 bundle 与 dylib/资源树摘要、签名标识、CPU 架构、macOS build 和实际测试环境身份。
- 实验根与程序/数据/历史槽的物理身份、owner/mode/link 校验；合成数据生成规则与 schema 的独立实测来源，不能从 target schema 猜 source schema。
- 每行完整命令、实际启动路径/PID/阶段、前后 gate 结果、正常客户端操作证据、正常退出及新进程重开证据、有限且无真实用户内容的输出。
- DB 学习前后逻辑摘要、tombstone 与配置/同步投影断言；原历史私有材料的 inode/hash 保留检查。合法学习改变 DB 字节，不用旧 DB 物理 hash 封死后续运行。
- 当前 proof、source family/schema 与资格覆盖行的匹配规则、证据存放与信任来源。人工文本 `passed`、目录中的自报 JSON 或类型构造成功不能作为信任根。

source/签名/架构/schema/OS 覆盖条件变化、缺行、证据被替换或不可验证均视为 unavailable。生产 issuer 的存储格式、可信生成/读取方式和有效期尚未批准，不在本批实现；逐检查点仍须 fresh 核验当前 proof、真实产品、静止、root 和最多 300 秒的内存任务授权。不能把长资格矩阵塞进这 300 秒任务内，也不能自动续权。

## 授权清单

授权逐包生效，不将“同意方案”解释成全部运行授权。每包执行前必须写出精确路径、命令、环境、影响、停止条件和保留方式；参数未定的阶段不得执行。

| 包 | 操作与影响 | 当前状态/授权需求 |
| --- | --- | --- |
| A0 | 仓库代码/文档只读审计、方案文档、降载文档检查 | 本轮已授权并完成；不产生程序资格 |
| A1 | Q1 最小 Foundation 命令行探针的编译与短命执行，仅新实验根与子进程环境 | 已单独批准并执行一次，重定向拒绝，授权已消费、材料保留；不含任何旧程序/库、GUI、输入注册、Keychain 或会话操作，不复跑 |
| A2 | 只读复验精确 release 源的 metadata/hash/codesign；独立复制整份载体 | 只读核验本身不构成系统 mutation；复制冻结载体必须单独授权。源与目的地、保留项、容量和核验命令须先固定；本轮两者均未执行 |
| A3 | Q2 可能需要的会话/服务/输入命名空间操作 | **未有可批准的具体方案，不执行。** 不请求笼统授权，不新增用户/切换登录，不以 VM 为默认后备 |
| A4 | Q3 正常 Manager/InputMethod 启动、选择输入源、人工输入、正常退出与重启 | Q2 通过后才可单独申请；只限实验自己的进程与合成数据。自动输入、强制终止、超时清理须另列明确操作，不能隐含在 harness 中 |
| A5 | 证据生成器、可信 issuer/reader 与最终取消产品入口的仓库实现 | 只读 reader 与合成负向测试已获准并实现；生产目录为空。writer、issuer、策略新鲜度及最终入口仍须单独批准，禁止默认合格实现；实机最终 marker 释放仍需独立操作授权 |
| A6 | 实验副本、合成 DB、日志或服务的清理 | 默认原样保留；删除/停止只能针对登记的实验对象逐项授权，绝不清理冻结材料 |

## 降载与本轮验证

编译、测试和检查均使用 `./scripts/run-low-impact.sh`；Cargo/libtest/Go 预算为 1，macOS 后台调度，不并行运行重型任务。该入口不提供 CPU/RSS 硬配额。Q1 只允许短命程序；完整资格矩阵开始前须另给资源预算、每批规模与停止方案，不能让降载后超时被当作通过或自动放宽现有任务期限。

工具静态检查与 runner 拒绝路径完成后，本批已在单独授权下串行完成 A1，并只读复核结果，实际记录见[本周记录](../devlogs/2026-W42.md)。没有启动 source 或取得程序资格。A1 排除了本次变量方案，A3/Q2 仍是完整资格的阻塞，A2/A4 未放行；A5 只批准上述仓库 reader 范围，不扩张到生产接线或真实操作。
