# Linux L6 Debian Package Matrix Runbook

本文定义 M5-P05B 的隔离 Debian 13 ARM64 L6 执行顺序、输入冻结、崩溃恢复、只读探针、证据格式和逐步授权边界。读者是准备或执行 Linux package transaction 验证的维护者。本文不授权创建或修改虚拟机、不提供公开 installer、不允许复用 P04 guest，也不授权运行 `apt`、`dpkg`、维护 CLI、启停 Fcitx/Manager、修改用户 XDG 或清理失败现场。

机器真相源是 [`packaging/linux/l6-matrix.json`](../../packaging/linux/l6-matrix.json)，仓库门禁是：

```bash
./scripts/check-linux-l6-contract.sh
./scripts/check-linux-l6-controller.sh
./scripts/check-linux-l6-release-pair.sh
```

三个入口分别验证 matrix format、compile-isolated controller 与 release-pair 构建/证据合同；入口自身都不连接 guest、不读取真实 `/proc`、不执行 package mutation，也不能单独证明下述真实 pair 或 L6 已通过。

## 当前执行状态

截至 2026-08-08：

- production transaction/startup 代码、actual `.deb` verifier 和 fake command/crash matrix 已完成；
- L6 guest、release pair、六步事务顺序、八个 crash checkpoint、字体/startup/XDG probe 和证据保留规则已由 format v1 固定；
- compile-identity 隔离的 acceptance checkpoint/evidence controller 已完成，八点合成中断/恢复与 canonical 脱敏 envelope 已通过专项门禁；
- 真实 pair 已从 source commit `55351f2` revision 1 与 target commit `e5b6da1` revision 2 的独立 clean root 断网构建、原子发布并独立复验；
- canonical pair record、两份 ARM64 package 与 production/acceptance executable identity 已冻结，具体 hash 见本 runbook 第 3 节；
- 构建 VM 只新增 per-user SDK/cache、私有源码/build/output；没有执行真实 `dpkg`、`/proc` probe、字体 probe、Manager/Fcitx 启动或系统安装，独立 L6 guest 与 S0 尚未准备。

## 1. 环境身份

L6 只能使用新建 guest、P04 guest 的独立 clone，或同等的可丢弃 guest。环境必须满足：

- Debian 13 `trixie`、ARM64、multiarch `aarch64-linux-gnu`；
- 专用用户固定为 `radishlex-l6`，home 固定为 `/home/radishlex-l6`；
- `radishlex` 初始 dpkg 状态为 `not-installed`，`/var/lib/radishlex/install-v1` 初始不存在；
- P04 staging、backup、userdb、导入导出文件和临时服务均不在该 guest；
- dependency 与取证工具安装完成后冻结 base snapshot，正式 matrix 阶段关闭网络；
- hypervisor、guest image digest、CPU/内存、磁盘、内核、`/etc/os-release`、dpkg status hash 和 snapshot ID 进入会话记录。

不能以容器中的成功结果替代 VM：容器可复验 package lifecycle，但不能覆盖真实 procfs、桌面 session、Fcitx daemon、system font cache、重启与用户域隔离。P05C 仍使用另一个独立 guest，不把 L6 故障现场提升为日常输入验收环境。

## 2. Snapshot 拓扑

至少保留以下不可混用的 snapshot：

| Snapshot | 现场 | 用途 |
| --- | --- | --- |
| `S0-clean` | dependency 已冻结；RadishLex package/state/XDG 均 absent | 首次安装、remove 后对照和 crash case 起点 |
| `S1-source-installed` | source terminal completed；未启动产品 | upgrade/crash 起点 |
| `S2-source-data` | source installed；专用用户已有合成 XDG fixture | 主序列数据保留起点 |
| `S3-target-installed` | target terminal completed；合成 XDG 未变 | repair、rollback 与 startup 负向 |

每个 crash scenario 必须从声明的 snapshot clone 开始，完成取证后恢复或丢弃该 clone。不得让 crash receipt、dpkg half-state、operation staging 或测试注入继续进入主序列。

## 3. Release pair 冻结

source/target 必须是两个不同 commit 形成的真实载体，不允许复制同一 `.deb` 后改名或只手写 evidence。仓库真相源为 [`packaging/linux/l6-release-pair.json`](../../packaging/linux/l6-release-pair.json)：source 固定 `55351f2`/`26.7.1+38-1`，target 是本子批 clean descendant/`26.7.1+38-2`。在获准的 Debian 13 ARM64 构建环境准备两个独立 clean root 后，唯一入口为：

```bash
./scripts/build-linux-l6-release-pair.sh \
  --source-root /absolute/clean/source \
  --target-root /absolute/clean/target \
  --output /absolute/absent/release-pair
```

该命令只构建、验证并原子发布私有 handoff 目录，不安装 package、不运行 maintenance/acceptance CLI、不创建/打开 VM，也不读写用户 XDG。运行前仍需单独准备构建环境；本 runbook 不授权下载依赖或修改全局工具链。构建规则为：

1. source 与 target 使用相同 `productVersion`、Flutter build number、ABI v9、userdb schema v9、XDG/settings/privacy/RimeData contract；
2. source 使用 Debian revision `N`，target 使用相邻 revision `N+1`，Debian version 比较必须证明 target 大于 source；
3. 两个 commit 都必须已经包含 production startup gate、system port 和受控维护 host；
4. 每个 commit 独立从干净源码构建 Manager、system-profile addon、rootfs、`.deb` 和 evidence，并分别通过 L1-L5；
5. 两个 package SHA-256、evidence SHA-256、product manifest SHA-256、control version、文件大小、构建 commit 与构建环境进入 L6 input record；
6. target maintenance 与 acceptance executable 分别记录 compile identity、commit、size、SHA-256、ELF architecture/loader 和 root-owned handoff identity；二者都不是 package payload或公开 installer；
7. 两个 artifact pair 在 guest 固定进入 `/var/tmp/radishlex-l6-inputs`，复制后改为 root ownership，再由 production verifier 重新打开和取证。

builder 先分别调用各自 commit 的 metadata、Manager、addon、rootfs、layout 与 deterministic `.deb` 门禁；随后仅从 target clean root 以 `--no-default-features` 构建 production maintenance ELF，并另行构建链接 acceptance feature 的 controller ELF。record 阶段再次调用各 root 自有 actual artifact verifier，解析两个 ELF 的 ELF64/AArch64 与 `/lib/ld-linux-aarch64.so.1`，要求 production 不含 acceptance markers、acceptance 同时含 build identity 与授权 marker；最后重哈希发布目录中的 package、artifact evidence、build-environment 和两个 executable。任一 root 不干净、commit 不符、revision 不相邻、contract 漂移、hash 相同、ELF/mode/link/marker 或 canonical JSON 不符均失败关闭且不发布输出。

pair envelope format 为 `radishlex-linux-l6-release-pair-evidence-v1` 对应的 format v1/profile v1 组合；只保存 commit、revision/version、package/evidence/manifest/dependency 摘要、无路径 tool version，以及 executable build profile/ELF/size/SHA-256。它不保存源码/构建/staging 绝对路径、operation ID、PID、proc maps、dpkg 原文或用户数据。source/target 的 build number 相同不表示两者是同一 package：Debian revision、manifest、control、package/evidence hash 必须不同。该 pair 只证明首版 Linux package 事务兼容，不宣称跨数据 schema 升级或公开发行兼容。

2026-08-08 的真实 Debian 13 ARM64 record 使用 Rust/Cargo 1.85.0、CMake 3.31.6、Flutter 3.44.0，target 为 `e5b6da1`。canonical record SHA-256 为 `a9bcf35762b460a23ad9bc062611f8d5edb57e7303861bbcb99e1efb40703dfd`；source/target package 分别为 `b41e32db76388ad18cdeb60e4b40fb8e28710556df87d53bfa5b275ff2ce028c`、`8209c0161609fde3b798628e5c3460e6237c8618f2d26f1452063540c7541295`；production/acceptance executable 分别为 `037199abe73559e2cd10013f0930f1f44cf9126ac7987169da11f2933a706fc1`、`c4f6282341c6f68f997b1f5d8d2d1b5dec2d96b60e2f387717594d5a0a9523f4`。两份 package 依赖摘要相同，package、artifact evidence 与 product manifest identity 均不同；独立 verifier 复验发布 inventory、mode/link、AArch64 loader 和全部 hash 后通过。该 record 是后续 root-owned handoff 的唯一 pair 身份，不授权复制、安装或执行其中任何文件。

## 4. 证据 envelope

每个 crash case 先由 controller 产生一个 `radishlex-linux-l6-checkpoint-evidence-v1` canonical JSON envelope；完整 L6 session 再把八份 checkpoint envelope 与主序列/probe 结果收敛进唯一 session envelope。session 至少绑定：

- matrix format/profile 与仓库 commit；
- guest/snapshot identity；
- source/target/maintenance executable 身份；
- 每步随机生成的小写 32 hex operation ID 的 hash，不记录原值；
- mutation 前后 dpkg package tuple、`dpkg --audit` 分类和 product inventory；
- receipt 的 format、operation kind、state、failure 与 terminal 分类，不复制 receipt 原文和 staging 路径；
- Manager/Fcitx startup decision/error/state；
- dependency/font family/glyph/owner 结果；
- XDG fingerprint、进程静止结果和系统重启边界；
- 执行者授权时间、命令退出分类和停止线事件。

证据不得包含 `.deb` 正文、receipt operation ID、绝对构建路径、PID、`/proc/*/maps` 正文、dpkg stdout/stderr、用户词、选择事件、导入导出内容或真实用户目录内容。

checkpoint envelope 只保存 matrix/build/scenario/checkpoint、repository/guest/snapshot identity、operation ID SHA-256、授权/fault/termination 分类、完整 process group 清空结论和 expected terminal。固定 writer 使用 deny-unknown-fields 的 canonical pretty JSON 与末尾换行；不接受调用方提供输出根。合成门禁验证该格式，但不会写固定 evidence root。

## 5. XDG 对照

package mutation 与用户态产品启动必须分开取证：

1. `S0-clean` 中五个固定 XDG 路径均不存在；首次 install terminal 后、任何产品进程启动前仍必须全部不存在。
2. 首次 install 零写入通过后，另行授权在专用用户域创建合成 fixture，形成 `S2-source-data`。fixture 只能包含公开合成词、合成 settings/privacy 和无敏感内容的数据库状态。
3. fingerprint 对每个固定节点记录相对路径、类型、uid/gid、mode、size 和 SHA-256，不导出文件正文；SQLite 同时纳入 `-wal`/`-shm` 是否存在及一致快照规则。
4. upgrade、repair、rollback、remove、reinstall 的前后 fingerprint 必须逐项相同；Fcitx profile 也必须不变。
5. startup smoke 导致的合法用户态写入只能发生在独立 clone，并与 package mutation 零写入证据分开。

固定范围为：

- `/home/radishlex-l6/.local/share/radishlex`；
- `/home/radishlex-l6/.config/radishlex`；
- `/home/radishlex-l6/.local/state/radishlex`；
- `/home/radishlex-l6/.cache/radishlex`；
- `/home/radishlex-l6/.config/fcitx5/profile`。

## 6. 主事务序列

每一步都使用新的 operation ID，并遵循“只读 preflight → 单步授权 → mutation → 只读 postflight → 记录结果”。不能一次授权整条序列。

| 顺序 | Operation | 前态 | Artifact | 终态 | 必须证明 |
| --- | --- | --- | --- | --- | --- |
| 1 | `install_source` | absent | target=source | source completed | dependency、完整 inventory、XDG absent、startup allow |
| 2 | `upgrade_target` | source | source→target | target completed | version 增序、无重复 mutation、XDG fingerprint 不变 |
| 3 | `repair_target` | target | target=target | target completed | 同版重新应用、程序身份恢复、XDG 不变 |
| 4 | `rollback_source` | target | target→source | source completed | version 降序、source 精确恢复、XDG 不变 |
| 5 | `remove_source` | source | source→absent | remove completed | package/product tree absent、startup failed closed、XDG 保留 |
| 6 | `reinstall_target` | absent | target=target | target completed | target 精确恢复、历史 receipt 追加、XDG 保留 |

每个 terminal 后必须确认 guard 未被持有、`receipt.json.tmp` 不存在、current operation slots 与 receipt 精确匹配。operation staging 和历史 receipt 全部保留；本 runbook 不定义清理成功现场。

## 7. Crash/retry matrix

format v1 固定八个 checkpoint：

| Case | Checkpoint | 预期恢复 |
| --- | --- | --- |
| install prepared | `prepared` | resume 后 completed |
| install staged | `artifacts_staged` | 重验 staging/quiescence 后 completed |
| upgrade quiesced | `quiesced` | 重验 relationship 与静止后 completed |
| upgrade before dpkg | `package_mutating_before_dpkg` | 重取 snapshot 后单次 apply |
| upgrade after dpkg | `target_applied_before_proof` | 识别 target 已安装，不重复 apply |
| rollback pending | `rollback_required` | source restore 后 rolled_back |
| rollback before dpkg | `source_restoring_before_dpkg` | 重验 source/静止后 restore |
| rollback after dpkg | `source_applied_before_proof` | 识别 source 已恢复，不重复 restore |

checkpoint controller 必须是显式 acceptance 构建身份，以不可由 production runtime 环境变量开启的编译边界实现。它在固定 checkpoint 持久化后通知外部 controller 并暂停；controller 只终止该测试的完整 process group，确认没有遗留 dpkg child 后再执行 resume。禁止：

- 轮询 receipt 并依赖竞态发送信号；
- 替换 `/usr/bin/dpkg`、修改 `PATH`、注入 shell/wrapper 或放宽 program identity；
- 使用 `--force-*`、手工改 dpkg status、手工改 receipt 或删除 staging 让恢复“通过”；
- 在同一 clone 连续执行多个 crash case。

实现位于独立 `platforms/linux-l6-acceptance/` crate。production crate 的 `l6-acceptance-checkpoints` feature 默认关闭，production main 只连接 disabled sink，不识别 acceptance 参数；acceptance worker 通过继承 pipe 发送 typed checkpoint 并暂停。controller 创建独立 process group，命中后用固定 `/usr/bin/kill` 发送 `SIGKILL`，等待 worker signal 终止，并连续复验 `/proc/*/stat` 中 group member 为零且没有 `dpkg` child，满足全部条件后才允许写 evidence。`crash` 命令还必须同时具备 `--authorized-l6-crash` 与 production mutation/data-preservation 授权；本节仍不构成运行授权。

## 8. 系统、字体与 startup probe

### Package 与 process

- dpkg status 必须接受 `install|hold ok installed`，拒绝 reinstreq、deinstall、half-*、unpacked 与 trigger 中间态；
- installed control `Depends` 与 terminal actual evidence 精确相同，所有直接依赖满足版本/architecture；
- Manager、addon、双 FFI、RimeData、metadata、desktop/icon/doc 与 manifest 的 owner/mode/link/size/hash 精确；
- mutation 前 Manager/Fcitx 及映射固定产品 inode 的进程均不存在；procfs 不可读、畸形或竞态不确定即停止。

### 字体

- `fonts-dejavu-core` 必须由 dpkg 证明 installed，并由 fontconfig 解析 `DejaVu Sans`；
- `fonts-noto-cjk` 必须由 dpkg 证明 installed，并解析 `Noto Sans CJK SC`；
- font file owner 必须回到对应 Debian package；Latin/数字样例 `RadishLex ABI v9 12345` 与中文样例“萝卜词核中文输入”均具有 glyph coverage；
- payload 仍只能携带固定 Material Icons，不能因 probe 失败复制字体或调用私有 `fc-cache` 修补现场。

### Startup

- installed terminal：Manager 与 Fcitx addon 分别得到 `AllowedProduct`，然后才可进入 Flutter/Engine/XDG/Rime；
- nonterminal/crash：两端均为 `MaintenanceRequired` 且用户域零写入；
- completed remove：两端均失败关闭，不能回退 development staging；
- 错误 sibling 与 preload interposition：必须在业务初始化前失败关闭；
- startup 负向测试使用独立 snapshot，不污染主序列的 XDG 零写入证据。

## 9. 每步授权与停止线

下列动作必须分别取得当次授权：

- 创建/克隆 VM、安装 dependency/取证工具、创建专用用户或写入 artifact input root；
- 执行每一个 install、upgrade、repair、rollback、remove、reinstall；
- 启用 acceptance checkpoint、终止 process group、resume 或恢复 snapshot；
- 启动/停止 Manager、Fcitx、桌面 session，或修改 Fcitx profile；
- 重启、注销、网络切换和任何现场清理。

出现以下任一情况立即停止并保留 clone：guest identity、artifact hash、dpkg config/status、receipt/guard/tmp、operation ID、process quiescence、dependency/font、XDG fingerprint、startup decision 或 expected terminal 不匹配；命令超时、输出溢出、权限不足、未知 lifecycle 也不能自动重试或降级。

## 10. L6 完成与后续

L6 只有在主序列、八个 crash case、字体/dependency、startup 正负向、XDG 零写入/保留和 guest reboot 对照均由同一 release pair 闭合后完成。完成后仍然：

- 不清理 operation staging、receipt、artifact input 或 evidence；
- 不复用 L6 guest 作为 P04/P05C 日常环境；
- 不推送、创建 tag/Release 或发布 apt repository；
- 不自动修改 Fcitx profile或代表用户完成输入交互；
- P05C 仍需独立 guest 与新的逐步授权。
