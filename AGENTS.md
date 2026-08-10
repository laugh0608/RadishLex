# RadishLex 协作约定

本文件统一约束本仓库的人工与 AI 协作。对话开始或结束总结时称呼用户为 `萝卜SAMA`。

## 项目与当前阶段

- RadishLex（萝卜词核）是 Rust 输入核心、Go 自部署同步后端、Flutter Manager 与平台原生薄壳组成的源代码可见中文输入系统。
- 许可条款以根 `LICENSE` 为准，当前采用 RadishLex Source-Available License。
- 输入热路径必须本地可用；服务端默认不可信，只承担密文同步、备份、设备和包分发。平台壳不承载 userdb、排序、同步或隐私真相源。
- v1 通过稳定 engine adapter 接入成熟引擎，可用 `librime`；不得让其私有模型污染 core，也不提前重写完整拼音引擎。
- 当前为 M5-P05B：macOS build 38 冻结；Linux 已完成 P01-P05A、确定性 `.deb`、actual package relationship、恢复事务、固定系统 observer/executor、mutable port、受控维护 CLI 与共用只读 startup gate。
- production 代码、L6 format v1、acceptance controller 与 release-pair verifier 已闭合。前三套真实推进暴露两个 Debian 环境缺口与 revision profile 缺口；第四套又在 operation ID/CLI 前确认重建 source 不等于 S2 terminal installed source，零 mutation 停止。第五套已以 prior-terminal source `09ed1228…bec`/`fe3d6297…cf94` 与 clean target `1ebbdab` 形成 chain-continuous ARM64 pair，record `d2661cc0…ed15`、target package `cdac2f32…7c26` 经 builder/宿主逐项复验并原子冻结；未启动 L6 guest。四个 stopped L6 现场与全部恢复/取证资产保留，八台注册 VM 全停。v1 package 不含 RadishLex maintainer scripts。
- 真实用户同步、公开发布、tag/Release 与远端推送保持关闭。平台顺序为 macOS、Linux Fcitx5、Android、Windows、iOS，每次只推进一条主线。

## 文档真相源

默认入口：`README.md`、`docs/status/current.md`、`docs/technical-plan.md`、`docs/roadmap.md`、`docs/repository-layout.md`、`docs/privacy-sync.md`。按任务补读：

- 阶段与顺位读 `current`、`roadmap`；架构与目录读 `technical-plan`、`repository-layout`；同步与敏感数据读 `privacy-sync`。
- Linux 当前安装工作读 `docs/linux-installation-maintenance-boundary.md`，平台/Manager 分别读 `docs/linux-fcitx5-boundary.md` 与 `docs/linux-manager-local-acceptance.md`。
- 优先更新既有文档。入口只保留当前判断、停止线和索引；设计进专题，流水进 `docs/devlogs/YYYY-Www.md`，历史段不回写成新事实。
- 架构、协议、隐私、平台、目录、里程碑或验证口径变化必须同步文档；每周重要推进追加 Asia/Shanghai 周志。
- 新增或大改文档开头说明用途、读者和不包含内容。兄弟项目只写项目名和在线 URL，不写本机路径。
- `AGENTS.md` 与 `CLAUDE.md` 保持逐字一致，目标均小于 14k 字节；`current` 目标 8k，Guide/Runbook 15k，Boundary 25k-30k。普通活跃 Markdown 接近 500 行优先拆职责。

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

## 隐私与数据

- P0 数据永不同步：密码、支付、证件、secure text 与隐私模式输入也永不学习；P1 原始选择/上下文默认只本地；P2 用户词、摘要、配置只作为端到端加密对象；P3 可公开下载。
- 删除必须以 tombstone 或等价语义同步，防旧设备/备份复活。新设备由已有设备或恢复码授权；撤销后轮换后续对象密钥。
- 日志、fixture、截图、诊断与 golden 禁止真实输入历史、联系人、密码、证件、支付或密钥；使用合成词、虚构 App/设备与脱敏聚合。
- 同步测试覆盖多设备、base version、冲突、离线、恢复、撤销、轮换；删除覆盖 tombstone、旧状态、冲突和备份；ranker 覆盖正负反馈、recency、frequency、context 与 explain。

## Linux P05B 边界

- 首个载体是 Debian 13 ARM64 系统级本地单 package，identity `debian-local-deb-v1`，不是公开 repository 或通用 Linux 包。
- CJK/Latin 字体使用发行版 hard dependency：`fonts-noto-cjk`、`fonts-dejavu-core`；payload 只允许固定 Material Icons 图标字形，不注册字体或调用 `fc-cache`。
- package 绑定 Manager、两份同 hash/不同 inode FFI、addon、完整 RimeData/source/license、desktop/icon 与 product manifest。
- `install`、`upgrade`、`repair`、`remove`、`rollback` 默认对用户 XDG 零写入并保留数据；首批升降级要求 ABI/schema/XDG/settings/privacy/Rime contract 完全相同。
- actual `.deb` 校验必须同一流计算 identity，严格解析三成员 ar、canonical USTAR、仅 `control`/`md5sums` 的 control，并交叉完整 payload inventory/manifest/evidence、canonical md5 inventory 与 `Installed-Size`；依赖、版本和 dpkg status 由同域 pure relationship 另行验证，不得退回 detached 字节声明。
- 每次 mutation/retry 重新验证私有 staged relationship，并消费 move-only quiescence permit。首次安装恢复必须保留 recovery target。
- state 使用 root-owned receipt、`receipt.json.tmp`/stage tmp 恢复、精确 current required slots、原子 mode 与父目录 `fsync`；guard 是 mode `0600`、零长度、单 link regular file 上的 advisory exclusive lock，不是 Unix socket。共享锁父目录只接受非 world-writable 或 root-owned 精确 `01777` sticky mode。
- 旧 operation v1 只保留结构与 pair metadata，不存历史 hash proof，也不用于当前恢复。任何未知、半配置、身份/owner/mode/link/hash 漂移均失败关闭并保留现场。
- production executor 只接受固定 `/usr/bin/dpkg`、typed argv、清空环境、null stdin、超时和有界诊断；system observer/port 复验 root identity、actual staging、依赖/版本、`/proc/*/maps` 静止与完整安装结果。外部 scripts/triggers 不能代表 transaction completed。
- 维护 command 是 opaque 类型，CLI 要求精确 operation ID、root-owned 同名 package/evidence 与双显式授权。startup 连接 terminal actual package/dependency；L6 固定独立 guest、相邻 revision、六步主序列和八个 crash checkpoint。新 operation 的 source artifact 必须与前一 terminal receipt 的 installed artifact 精确相同；同版本重建字节不能替代 chain anchor。release-pair builder 只冻结 contract 指定的 prior-terminal source package/evidence，只从单一 clean target 构建，并由 target production Rust verifier 逐侧解析。真实 source install、dpkg、字体与 startup 已取证；剩余主序列与 crash/retry 尚未执行。
- 不要把 RadishLex 做成云端实时输入法 API，也不要让同步后端进入按键热路径。

## 实机与系统边界

- 可直接读取/修改仓库，运行只读 Git、现有格式化、静态检查、测试和 smoke，并做范围清楚的小型本地提交。
- 安装依赖、下载 SDK/模型/数据、改变全局工具链、启动长期服务或 GUI 前先告知；网络或沙盒导致关键验证失真时，只为构建/测试申请受限提权。
- 修改系统输入法、权限、Keychain、`/usr`/`/var`、dpkg、systemd、Fcitx profile/autostart、会话、证书或全局配置必须取得明确授权。不得自动 kill/restart、合成按键或点击冒充人工验收。
- P04 guest staging、backup、userdb、导入导出文件和临时服务保持原样，不复跑或清理。任何 L6/P05C 使用独立 clone/snapshot 或另一台 guest，并逐步授权系统写入、进程/会话和人工输入。
- UTM 只通过 `PATH` 中的 plain `utmctl` 操作，不直接调用 app bundle 可执行文件；任何时刻最多运行一台 VM，启动前先用 `utmctl list` 确认其他注册 VM 全部停止。

## 实现、文件与验证

- 代码优先直观命名、早返回、明确类型与浅层职责；不写空转 wrapper、晦涩 factory、动态字符串状态、异常吞噬或无目的 fallback。抽象只为稳定边界、真实重复或明显降复杂度。
- 单源码原则上不超过 1500 行，接近 1000 行优先拆分；`src/` 与 `scripts/` 使用浅层职责目录；committed 相对路径默认不超过 180 字符。
- 仓库文本 UTF-8 无 BOM、LF、末尾换行；非 Markdown 无尾随空格。不得为过门禁批量改写第三方或保留原格式资料。
- Rust：`cargo fmt --check`、`cargo check`、相关 `cargo test`/`clippy`；Go：`gofmt`、`go test ./...`；Flutter：`dart format`、`flutter analyze`、相关 test；平台壳覆盖 build/smoke，真实行为留人工证据。
- 涉及 docs/入口需检查术语、链接、许可证与 AGENTS/CLAUDE 同步；文本至少跑 `git diff --check`。阶段性或高风险交付补 `./scripts/check-repo.sh`，并说明已验证项与未验证风险。

## 当前顺位

1. 保留四个 stopped L6 failure/mismatch disk、前三套失败 pair/handoff/S0、第三套 receipt/staging/S1/S2、第四套 handoff/input/preflight 与 WAL-drift snapshot；不热替换、覆盖、恢复或清理。
2. 第五套 chain-continuous handoff 已冻结；下一步另行授权从原始 S2 建立新的独立 clone、写入第五套 pair并完成单 VM/断网/package/receipt/XDG/startup/process preflight，不执行 upgrade。
3. preflight 冻结后仍需新的单步 upgrade 授权；L6 闭合后再排 P05C。公开发布、推送、旧资产清理、真实同步及其他平台保持关闭。
