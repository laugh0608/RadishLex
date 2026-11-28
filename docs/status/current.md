# RadishLex 当前状态

规划见[专题地图](../topics.md)；当前状态见下文。

## 当前判断

- 11-14 收尾：R `65a4147` 固定受审 `40-2` seed。T `fa3b2c5` 通过完整仓库验证，ARM64 `40-3` 新 pair 已完成独立实物核验及宿主回读。macOS仍为09-09 source39恢复，`rolled_back`/`aborted_preserved`，材料保留、双组件停止。常态分支`dev`，主线`master`。
- M5 已完成当前包对的连续 L6 与 `upgrade_quiesced` 恢复。P05C 的 `40-3` 已切到 RadishLex，普通用户启动引擎遇到 `PermissionDenied`，Qt 无候选。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- T 基线已通过八个自动合同。三个代表 crash 实机证据见下文，其余五个真实 crash 留作后续强化。

## 能力与开放状态

里程碑退出记录只表示当时范围；实现、自动验证、实机验证、用户开放与公开发布分别判断。下表中的历史验收不覆盖新发现的验证缺口。

| 能力 | 实现与自动验证 | 实机证据 | 产品开放与发布 |
| --- | --- | --- | --- |
| macOS 离线输入与本地管理 | runtime、FFI、Manager、产品 bundle 与合同测试 | build 38 历史验收；build 39 安装、双端启动及普通选词/学习通过，其余复验待执行 | 已验收范围可供受控本地使用；尚未公开发布 |
| Linux 输入与本地管理 | Fcitx5 addon、共享 XDG/FFI、Manager | P04 Wayland/X11 与同库验收 | 受控本地验收；尚未公开发布 |
| Linux 安装维护 | `.deb`、事务、startup gate、八个 crash 自动合同 | 当前包对的连续六步、依赖、字体、冷启动和 `upgrade_quiesced` 恢复。两个历史 crash 按共同合同引用 | P05C 待验收，尚未公开发布 |
| 加密同步 | 协议、服务、客户端及受控合成资格链 | macOS key backend 与本地 HTTPS 资格记录 | 真实用户同步关闭；目标生产部署未验收 |
| Android / Windows / iOS | Android 仅有 Keystore 能力桥；其他平台壳未落地 | 不构成完整 IME 验收 | 后续串行计划 |

2026-10-12 已确认[客户端连接规划](../client-server-connection-plan.md)：同包选官方 / 自部署，无需重编译；同步仍关闭。

## 综合审阅跟踪

- [2026-09 产品审阅](../remediation/product-review-2026-09.md)继续开放。REV-01 独占学习/隐私修复与 REV-02 SQLite 升级已有源码和限定产物证据，当前 Linux 候选仍需实际复验；输入质量、工具链及其余未覆盖项不由历史通过自动关闭。
- macOS build 39 曾完成安装、普通选词和学习；后续发现上下文归类问题并修复。build 40 升级中止后，同一 operation 已恢复 source 39，外层 `rolled_back`、数据 `aborted_preserved`；双端停止，原数据与恢复材料保留。当前不重试 40，精确停止线见下文。[联合验收](../runbooks/macos-rev01-rev02-acceptance.md)与[WAL 方案](../remediation/macos-wal-upgrade-recovery-2026-09.md)继续承载剩余任务。
- macOS action `10–13`、WAL 准备/恢复编排已有合成证据，最终取消、source 正常资格及内部故障仍开放；这些支线不作为 Linux M5 前置。当前 Rime 锁不能配给冻结 Linux 旧 pair，新候选按 source-first 合同验收。
- [客户端连接规划](../client-server-connection-plan.md)保持同包官方/自部署方向，真实用户同步仍关闭。性能由新热点触发，输入质量、Manager 易用性和早期反馈按原审阅跟踪。

整理前逐批摘要原样保存在[本周入口归档](../devlogs/2026-W46.md#整理前综合审阅跟踪)，仅保留历史语境，不恢复当时授权或顺位。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；11-14 所有者批准退役 P04 专机，历史验收结论仍只适用于当时版本。P05 不复用其历史数据。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据摘要

- 固定包对在同一候选完成六步维护。S2 冷启动后的五步均在同一离线 boot 完成，七节点 XDG 指纹一致。移除与重装分别返回双端 `RemovedProgram`、`AllowedProduct`。13 项归档清单 `bd7c8d3a…df5d` 已复验。[六步实测](../devlogs/2026-W47.md#2026-11-22-连续-l6-六步实测)
- 一次正常关机和一次冷启动后，离线 boot `0b73f8fd…2105d` 保持 `40-3 / completed / install / chain=6`。28 项文件、双端 `AllowedProduct`、20 项依赖、两种发行版字体和七节点 XDG 通过。归档 `58b41cae…dde2`。[冷启动对照](../devlogs/2026-W47.md#2026-11-22-连续-l6-冷启动对照与留存)
- 独立 S2 副本在离线 boot `af171935…5296` 完成 `upgrade_quiesced / chain=2` 和单次生产 resume。仅一次 dpkg 升级，终态 `40-3 / completed / upgrade / chain=2`、28 项文件、双端 `AllowedProduct` 与 XDG 均通过。恢复归档 `9a7dc6e4…f530`，正常停机归档 `6a8280fc…19e4`。[恢复与停机](../devlogs/2026-W47.md#2026-11-22-upgrade_quiesced-恢复与停机)
- `install_prepared` 的历史终态 `679b7e04…c544` 和 `install_artifacts_staged` 的历史终态 `7fcef38e…0e3d` 经审阅可引用于 `Prepared` 与 `ArtifactsStaged` 的共同安装恢复合同。实测来自旧 `38-1 → 38-2` 包对，新 `40-2 → 40-3` 的两项 install crash 未重放，旧原始归档本轮未逐件复算。[适用性审阅](../devlogs/2026-W47.md#2026-11-22-两个历史-crash-的适用性)
- Linux 输入与 Manager 已有 P04 历史实机记录。当前交付物的真实离线输入、隐私、个人化和持久化仍待独立 P05C 验收。
- 前五批已退休 17 个 raw bundle。11 月 8 日另退休 4 份旧载体及 S2/S3 磁盘，保留快照配置、EFI、handoff、清单和失败证据。
- 原入口中的精确证据流水已原样移入[本周周志的入口归档](../devlogs/2026-W36.md#2026-09-05入口历史证据归档)。这是文档迁移，不是现场复验；操作前仍需按 runbook 重新满足资格与授权。

## 停止线

- [R 记录](../devlogs/2026-W46.md#2026-11-13-source-seed-所有者审阅与-r-锚点)固定 S `c038602` 和源材料。通用 Debian 保留 B 批 `40-3` 现场、私有输入与 receipt。原连续 L6 和 `upgrade_quiesced` 两台专机均已退役。保留各自的终态副本、S0/S1/S2、固定包对和持久证据。
- macOS `aad9cf8ac2dae71b2b659e96a91ea706` 已为外层 `rolled_back` / 数据 `aborted_preserved`。源 39 双程序已恢复，target 40 保留在同 operation 的两个 `staged.app`；原 DB/WAL/SHM、snapshot/candidate/settings backup、独立恢复证据、原 39/40 与恢复 Installer 全部保留。输入源未选中、Manager/InputMethod 停止、privacy false，恢复 Installer 保持终态窗口。实际恢复授权已完成，不点“重新执行”/移除，不手动 checkpoint/删除 sidecar，不清理或启动双组件/输入测试；后续 WAL 新升级合同及系统动作另行确定范围。
- 五批均闭合`deleted`，第五批manifest为`a65abab2…1b106`；授权均已消费，不得复跑。
- 第五批证据、projection 与 S2 的留存证据冻结，三台 bundle 已 absent；不得恢复、重建、注册或复用。
- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 第三台rollback `EFD15599…BBDD`、remove `5EA2BAA2…27A2`与reinstall `E671DB9C…D465`的bundle均已删除；终态与退休证据冻结，不得恢复、重建、注册或复用。
- 旧pair与新pair失败现场只保留持久证据；已删除的`FD24ADFF…17C056`、`B0B826F6…87B3`、`5B19AEF1…7DAB`及`BE3579E0…37F8`不得恢复package、重新注册、重建或复用。
- 新`d75818f` handoff仅作为冻结输入；第三台clone bundle已删除，不得恢复、重建、注册、复用或与旧pair混搭。
- v2 clone失败证据`65160b12…c1859`须原样保留；不得沿用本批授权重试clone、重启UTM或把exit 0记为成功。
- 七次host诊断证据`158fe177…77c`/`8ccdbb9f…dc7`/`f952953a…e5ddf`/`34ae0563…743e`/`9da78f78…08ae`/`44043282…4ae8`/`206aa335…7b56c`均须冻结，不覆盖、补写或复用。v7只定位host UTM/AppKit失败机制，不授权原target retry、`--hide`试跑、GUI动作或把更深根因写成已证实。
- v4 launch至exact resume证据与UUID `50B75F88…8038`须冻结；bundle已删除，不得恢复、重建、注册或复跑。transaction权威仍是`7fcef38e…0e3d`的`completed`，删除前VM终态权威是`096fe01f…e133`的`stopped-verified`。
- 旧boot、恢复、fresh两阶段及结果消歧根全部冻结，不复跑、覆盖、补拉或清理。`recovery-qualified`只证明只读恢复前门，不授权resume；无新授权不得query/start、resume、retry/stop/quit。
- fresh-boot resume attempt `d75818f-v4-install-artifacts-staged-fresh-boot-resume-20260827-v1`与host/guest根已消费并冻结；不得补拉、复跑、重建secret、retry/cleanup、再次resume/postflight或据`state-indeterminate`修补现场。调用后不得无授权追加query/start/stop/quit。
- transaction-state v1/v2、deferred-result及terminal-stop根冻结。21项结果只证明该boot的transaction为`completed`，28项stop结果只证明同一授权调用内目标已正常停止并完成host交叉检查；不得补拉、复跑probe、query/start/stop、resume/dpkg/retry/repair/cleanup或改写现场。
- registration shell v1-v3 control根与manifest `3e21aa02…e3c34`/`3c9c7ee8…3f604`/`ae3fb81b…4a511`冻结；不得覆盖、复用或解释为已创建。
- clone v1/v2 根与 `Evidence-v5` 冻结。曾位于默认 Documents 的 partial 已退役，不得重建或冒充 S2。
- P04 载体已退役，不恢复或复跑旧现场。不自动清理当前 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38/39/40 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一阶段：Linux M5 交付收口

11-28 专机 `2FA2135A…3093` 离线首装 `40-3`，回执 `completed / install / chain=1`，五台全停。seed `01ca203b…e635` 独立审阅 12/12 通过。[第二轮审阅](../devlogs/2026-W48.md#2026-11-28-第二轮首装-seed-独立审阅)

| 顺位 | 必须继续 | 必要阻塞 / 关闭标准 |
| --- | --- | --- |
| 1 | C 批普通用户启动受阻 | [40-4 候选准入](../runbooks/linux-source-first-assembly.md#从冻结的-40-3-建立-40-4-候选)已实现。待所有者确认第二轮 seed 后生成正式 review、提交 R，再推进新候选与普通用户复验。[实机诊断](../devlogs/2026-W48.md#2026-11-27-qt-普通字段无候选) |
| 2 | C 批桌面实测与 D 批冷启动 | 分别授权。覆盖 Wayland/X11、多应用与多 session、隐私与普通输入恢复、学习、删除/tombstone、Manager 同库及重启持久化 |
| 3 | M5 退出评审 | 核对独立 P05C、代表 crash 适用域和常规及专项验证，关闭阻塞退出的缺陷，登记残余限制 |

原包对的 R 固定 40-2 seed `d0bd5b7c…c820b`。[T 记录](../devlogs/2026-W46.md#2026-11-14-t40-3-日终回顾与批次结果)。[合同](../remediation/linux-m5-source-first-contract.md)与[前门](../runbooks/linux-source-first-assembly.md#v3-seed-只读前门)继续适用。

启动命令、影响和测试矩阵见 [P05C 验收方案](../runbooks/linux-p05c-acceptance.md)。

出口沿[路线图](../roadmap.md#m5linux-fcitx5-离线输入与个人化产品)：固定 Debian13 ARM64 本地 `.deb`，当前输入/个人化/隐私/持久化通过，连续 L6、八个 crash 自动合同、三个代表实际恢复边界和独立 P05C 闭合。不增加全部 Linux 平台、macOS 遗留清零或公开发布为 M5 条件。

后置：macOS WAL 新升级及 Q2/F0/W0/最终取消、其余五个真实 crash hardening、新 VM 管理/clone/迁移框架、Android/Windows/iOS、真实用户同步，以及无新热点的性能/CI 极限优化。macOS action `10–13` 与已有自动证据保留，冻结现场不重试真实 40；完整发布顺序不变。

首轮 seed `4f3ccb17…eaec` 审阅 9/12 项通过，旧机 `435CE261…8DFE` 已封存退役。两轮归档独立留存。实机、装配及完整门禁分项授权。[首轮审阅](../devlogs/2026-W48.md#2026-11-28-40-3-首装-seed-独立审阅)

## Linux M5 系统操作停止线

日常复用 `Debian13-ARM64` / `Windows11-ARM64`，干净基线只作备份。11-26 A 批启动前五台全停，通用 Debian 与恢复专机的配置、EFI 和磁盘均为独立文件且零句柄。P04、原连续 L6 和恢复专机均已退役。后续操作沿用 [P05C 验收方案](../runbooks/linux-p05c-acceptance.md)。

1. 留存的旧证据只用于历史审阅。迁移映射见 `datas/_organization/2026-11-01/`。旧 Move、adopt 和物化控制仅用于审阅历史，后续实机前门按新 source/pair 和实际载体重新建立。
2. 通用测试机之外最多保留一台 RadishLex 专用 VM，按根协作约定计数。现有专用载体超额时暂停新增，按精确清单取得退役授权并核验后再安排专用 guest。
3. 连续 L6 与 P05C 的独立 guest 按顺序安排。更换专机前先确认上一轮证据已保留、旧专机退役已核验。各轮实机动作与完整门禁分别授权。

## 验证入口

- 日常 CI 已合入 `dev`，复测 1 分 43 秒。[CI 精简专题](../remediation/ci-critical-path-optimization.md)

2026-11-14 的 T `fa3b2c5` 已串行通过常规门禁（含 source-first）和完整可靠性专项。命令、耗时和快照见[本周周志](../devlogs/2026-W46.md#2026-11-14-t40-3-日终回顾与批次结果)。

```bash
./scripts/check-linux-product-metadata.sh
./scripts/check-linux-product-layout.sh
./scripts/check-linux-deb-artifact.sh
./scripts/check-linux-package-transaction.sh
./scripts/check-linux-startup-gate.sh
./scripts/check-linux-l6-contract.sh
./scripts/check-linux-l6-controller.sh
./scripts/check-linux-l6-release-pair.sh
./scripts/check-linux-l6-maintenance-refresh.sh
./scripts/check-linux-fcitx5.sh
./scripts/check-manager-linux-product.sh
./scripts/check-repo.sh
./scripts/check-docs.sh
./scripts/check-text-files.sh
git diff --check
```

上述入口覆盖八个 crash 合同和现有 L6 控制。当前包对的实际构建资格、连续 L6 与真实恢复证据见本页证据摘要。独立 P05C 和 M5 退出评审仍开放。

## 阅读索引

- [Linux M5 交付基线审计](../remediation/linux-m5-delivery-baseline-audit.md)
- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [Linux L6 收敛与本地资产生命周期](../runbooks/linux-l6-asset-lifecycle.md)
- [Linux P05C 独立实机验收方案](../runbooks/linux-p05c-acceptance.md)
- [本周周志](../devlogs/2026-W48.md)
