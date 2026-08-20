# RadishLex 协作约定

本文件统一约束本仓库的人工与 AI 协作。对话开始或结束总结时称呼用户为 `萝卜SAMA`。

## 适用范围

本文件只保存跨阶段、长期有效的协作约束，不记录当前里程碑、批次、日期、提交、证据编号、实机现场或下一步。项目介绍见 `README.md`，许可条款以根 `LICENSE` 的 RadishLex Source-Available License 为准，文档导航见 `docs/README.md`，当前判断与临时停止线只读 `docs/status/current.md`。

## 文档真相源

默认从 `docs/README.md` 按任务进入；项目定位读 `README.md`，当前阶段读 `docs/status/current.md`。其余规则：

- 阶段与顺位读 `current`、`roadmap`；架构与目录读 `technical-plan`、`repository-layout`；同步与敏感数据读 `privacy-sync`。
- 优先更新既有文档。入口只保留当前判断、停止线和索引；设计进专题，流水进 `docs/devlogs/YYYY-Www.md`，历史段不回写成新事实。
- 架构、协议、隐私、平台、目录、里程碑或验证口径变化必须同步文档；每周重要推进追加 Asia/Shanghai 周志。
- 阶段、现场和顺位变化不得回写 `AGENTS.md` / `CLAUDE.md`；只有长期协作规则变化才同步修改两份文件。临时高优先级约束写入 `current` 或其引用专题。
- 新增或大改文档开头说明用途、读者和不包含内容。兄弟项目只写项目名和在线 URL，不写本机路径。
- `AGENTS.md` 与 `CLAUDE.md` 保持逐字一致并尽量精简；`current` 目标 8k，Guide/Runbook 15k，Boundary 25k-30k。普通活跃 Markdown 接近 500 行优先拆职责。

## 开发节奏与协作

- 开始任务先检查 Git 状态并阅读直接相关文档。若基线与用户给出的 commit、ahead 或 clean 状态不一致，不 reset、覆盖或清理，先报告差异。
- 长期功能、跨语言能力、平台或重要模块先固定边界再实现。小型 bug、文案、纯清理或不改变边界的验证补漏可直接推进。
- 用户未明确要求修改代码时，先说明方案并等待批准；范围清晰且用户要求直接修改时可实施。影响架构、协议、隐私、许可证、平台接入或阶段边界而意图不明时先澄清。
- 从根因、长期维护和系统一致性出发，完整覆盖真实主路径；不得用占位、半成品或“短平快”表述冒充交付。
- 每个可分割步骤做匹配验证；核心、数据一致性、安全、隐私、用户可见行为或阶段交付扩大门禁。fast/quick 不能作为最终门禁。
- 工作区可能含用户改动或并行改动；只修改本批文件，保留无关变更。禁止未授权 `git reset --hard`、checkout 覆盖、force push 等破坏性操作。
- 提交使用当前用户身份和 Conventional Commits，不加 AI 署名。代码、文档、治理按主题拆分；提交前复验范围与必要门禁。推送、PR、Release、tag 或远端设置须另行授权。

## 架构与 Engine 边界约束

- `crates/ime-core`：session、composition、candidate、commit、engine trait 与领域模型。
- `ime-engine-rime`：隔离 Rime 生命周期与类型；`ime-runtime`：组合 engine、ranker、userdb、privacy；`ime-ranker`：确定性重排与 explain；`ime-userdb`：SQLite、学习、tombstone、导入导出和同步投影。
- `ime-sync`/`ime-crypto`：客户端 P2 协议、合并、设备与密钥；`server/sync-server` 只存密文与必要元数据，不参与按键或明文合并。
- `ime-ffi` 必须明确 ABI、所有权、生命周期、线程、UTF-8、释放和错误；不得静默吞掉 crypto/sync/ranker/userdb/FFI 错误。
- Flutter Manager 负责设置、词库、学习视图、隐私、同步状态、设备和备份入口，不进入热路径。平台壳只处理系统生命周期、按键、候选、commit 和 FFI。
- Linux 优先 Fcitx5，候选使用 input panel；Wayland 不自造浮窗协议。`platforms/linux-product` 只承担 Debian product transaction、关系校验与 startup decision，不接管输入业务或 XDG 数据。
- 不要把 RadishLex 做成云端实时输入法 API，也不要让同步后端进入按键热路径。

## 隐私与数据

- P0 数据永不同步：密码、支付、证件、secure text 与隐私模式输入也永不学习；P1 原始选择/上下文默认只本地；P2 用户词、摘要、配置只作为端到端加密对象；P3 可公开下载。
- 删除必须以 tombstone 或等价语义同步，防旧设备/备份复活。新设备由已有设备或恢复码授权；撤销后轮换后续对象密钥。
- 日志、fixture、截图、诊断与 golden 禁止真实输入历史、联系人、密码、证件、支付或密钥；使用合成词、虚构 App/设备与脱敏聚合。
- 同步测试覆盖多设备、base version、冲突、离线、恢复、撤销、轮换；删除覆盖 tombstone、旧状态、冲突和备份；ranker 覆盖正负反馈、recency、frequency、context 与 explain。

## 安装与系统事务边界

- 安装、升级、修复、移除和回滚默认保留用户数据；任何例外都必须由明确产品合同和用户授权定义。
- 载体、依赖、安装状态、receipt、staging、guard 与运行中程序必须从实际系统状态交叉验证；detached 声明、脚本退出码或外部 trigger 不能单独代表事务完成。
- mutation 与 retry 前重新验证私有 staged relationship 和静止条件；未知状态、半配置、身份/owner/mode/link/hash 漂移均失败关闭并保留现场。
- production executor 只接受固定系统工具、typed argv、最小环境、null stdin、超时和有界诊断；维护 CLI 使用 opaque command、精确 operation identity 与双显式授权。
- 升降级 chain 必须锚定前一终态实际安装产物；同版本重建字节不能替代 source artifact。分散实机证据不得冒充同一连续 session。
- 具体载体、平台 transaction、startup gate、验收矩阵和当前现场以 `docs/README.md` 导向的专题、runbook 与 `current` 为准。

## 实机与系统边界

- 可直接读取/修改仓库，运行只读 Git、现有格式化、静态检查、测试和 smoke，并做范围清楚的小型本地提交。
- 安装依赖、下载 SDK/模型/数据、改变全局工具链、启动长期服务或 GUI 前先告知；网络或沙盒导致关键验证失真时，只为构建/测试申请受限提权。
- 修改系统输入法、权限、Keychain、`/usr`/`/var`、dpkg、systemd、Fcitx profile/autostart、会话、证书或全局配置必须取得明确授权。不得自动 kill/restart、合成按键或点击冒充人工验收。
- 已冻结或作为证据保留的 guest、staging、backup、userdb、导入导出文件和临时服务不得复跑、覆盖或清理；具体资产清单只在 `current` 与 runbook 维护。新系统验收使用独立 clone/snapshot 或另一台 guest，并逐步授权系统写入、进程/会话和人工输入。
- UTM 只通过 `PATH` 中的 plain `utmctl` 操作，不直接调用 app bundle 可执行文件；任何时刻最多运行一台 VM，启动前先用 `utmctl list` 确认其他注册 VM 全部停止。磁盘配置移除 Network 不能替代 guest 运行态证据；每次启动后、写入 input 或生成 operation ID 前都要复验接口 down 且 IPv4/IPv6 路由为空。
- UTM 库条目 unavailable、UUID not found 或 data error 时必须停止并保留 package；不得把通用错误直接归因于 bookmark，也不得用磁盘目录存在替代 registered-stopped 证据。QEMU 配置集合键必须满足目标 UTM 的冷解码合同；无网卡使用 `Network=[]`，不能删除必填键。注册/config 修复须单独授权并在前后复验 UUID、config/EFI/qcow2 identity。
- UTM/guest-agent 的命令返回码或空输出不能单独作为成功证据；关键字节、注册/文件后置条件、结构化检查和 startup 结果须以文件回读、可靠退出合同及正负向对照独立复验。

## 实现、文件与验证

- 代码优先直观命名、早返回、明确类型与浅层职责；不写空转 wrapper、晦涩 factory、动态字符串状态、异常吞噬或无目的 fallback。抽象只为稳定边界、真实重复或明显降复杂度。
- 单源码原则上不超过 1500 行，接近 1000 行优先拆分；`src/` 与 `scripts/` 使用浅层职责目录；committed 相对路径默认不超过 180 字符。
- 仓库文本 UTF-8 无 BOM、LF、末尾换行；非 Markdown 无尾随空格。不得为过门禁批量改写第三方或保留原格式资料。
- Rust：`cargo fmt --check`、`cargo check`、相关 `cargo test`/`clippy`；Go：`gofmt`、`go test ./...`；Flutter：`dart format`、`flutter analyze`、相关 test；平台壳覆盖 build/smoke，真实行为留人工证据。
- 涉及文档入口需检查术语、链接和许可证；只有长期协作规则变化时才检查并同步 `AGENTS.md` / `CLAUDE.md`。文本至少跑 `git diff --check`。阶段性或高风险交付补 `./scripts/check-repo.sh`，并说明已验证项与未验证风险。
