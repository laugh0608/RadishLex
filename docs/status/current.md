# RadishLex 当前状态

规划见[专题地图](../topics.md)；当前状态见下文。

## 当前判断

- 11-14 收尾：R `65a4147` 固定受审 `40-2` seed。T `fa3b2c5` 通过完整仓库验证，ARM64 `40-3` 新 pair 已完成独立实物核验及宿主回读。macOS仍为09-09 source39恢复，`rolled_back`/`aborted_preserved`，材料保留、双组件停止。常态分支`dev`，主线`master`。
- 11-22，固定 Debian 13 ARM64 专用候选已依次完成六步维护事务，终态为 `40-3 / completed / install / chain=6`，六步脱敏证据已持久归档。依赖、字体和重装终态跨 boot 对照仍待验收。[实机周志](../devlogs/2026-W47.md#2026-11-22-连续-l6-六步实测)
- 里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate，本批完整自动通过，V3–V5另批。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- 八个crash自动合同与六类operation分散证据已闭合；真实阻塞样本固定三项，前两项已闭合，`upgrade_quiesced`与连续L6未闭合；其余五项真实crash转为hardening。

## 能力与开放状态

里程碑退出记录只表示当时范围；实现、自动验证、实机验证、用户开放与公开发布分别判断。下表中的历史验收不覆盖新发现的验证缺口。

| 能力 | 实现与自动验证 | 实机证据 | 产品开放与发布 |
| --- | --- | --- | --- |
| macOS 离线输入与本地管理 | runtime、FFI、Manager、产品 bundle 与合同测试 | build 38 历史验收；build 39 安装、双端启动及普通选词/学习通过，其余复验待执行 | 已验收范围可供受控本地使用；尚未公开发布 |
| Linux 输入与本地管理 | Fcitx5 addon、共享 XDG/FFI、Manager | P04 Wayland/X11 与同库验收 | 受控本地验收；尚未公开发布 |
| Linux 安装维护 | `.deb`、事务、startup gate、八个 crash 自动合同 | 当前候选六步连续实机证据及两个历史代表 crash 终态 | L6 待依赖、字体、重装后冷启动对照和第三个代表 crash，P05C 待验收 |
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

- 当前 `40-2 → 40-3` 包对已在同一候选上完成六步维护。S2 冷启动后的升级、repair、回退、移除和重装均在同一离线 boot 完成，七节点 XDG 指纹逐项一致。移除返回双端 `RemovedProgram`，重装恢复双端 `AllowedProduct`。持久归档清单 `bd7c8d3a…df5d` 含 13 项已核验材料。[事务与证据摘要](../devlogs/2026-W47.md#2026-11-22-连续-l6-六步实测)
- Linux 输入与 Manager 已有 P04 历史实机记录。当前交付物的真实离线输入、隐私、个人化及独立 P05C 仍待验收。
- `install_prepared` 与 `install_artifacts_staged` 已取得代表 crash 恢复终态；第二场景 transaction 权威为 `7fcef38e…0e3d/completed`，停止权威为 `096fe01f…e133/stopped-verified`。
- `upgrade_quiesced` 的旧 `move-ready` 和 `fa6e7335…33ea5` 作为历史证据冻结。旧 registration shell、partial 与 S2 磁盘已退役，旧场景无法原样续跑，第三个代表 crash 仍未闭合。
- 前五批已退休 17 个 raw bundle。11 月 8 日另退休 4 份旧载体及 S2/S3 磁盘，保留快照配置、EFI、handoff、清单和失败证据。
- 原入口中的精确证据流水已原样移入[本周周志的入口归档](../devlogs/2026-W36.md#2026-09-05入口历史证据归档)。这是文档迁移，不是现场复验；操作前仍需按 runbook 重新满足资格与授权。

## 停止线

- [R 记录](../devlogs/2026-W46.md#2026-11-13-source-seed-所有者审阅与-r-锚点)固定 S `c038602` 和源材料。通用 Debian 保持源包 `40-2` 已停机。专用候选的最后 guest 终态为重装后的 `40-3 / chain=6`。保留 S0/S1/S2、固定包对、事务暂存和持久归档，下一真实操作另行授权。
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

六步维护事务实测已归档。下一段先补当前候选的依赖、字体与重装终态冷启动对照，再推进代表恢复边界和当前产品行为，最终进入独立 P05C。M5 候选与 B1–B6 见[基线审计](../remediation/linux-m5-delivery-baseline-audit.md)。

| 顺位 | 必须继续 | 必要阻塞 / 关闭标准 |
| --- | --- | --- |
| 1 | 补当前候选的依赖、字体和冷启动对照 | 先恢复专用候选唯一运行。复核固定 `40-3` 的 20 项依赖及字体 family、glyph、包归属，以 `chain=6`、28 项文件、双端门禁和 S2 指纹作为冷启动前基准。正常关机及冷启动另行授权，新 boot 复验同一终态、XDG 和断网 |
| 2 | 验证代表恢复边界并准备当前产品行为验收 | 绑定当前包对和合格前态，设计 `upgrade_quiesced` 的真实中断及恢复，单独审阅前两个代表样本的适用性。准备离线输入、隐私与普通输入恢复、学习、删除/tombstone、Manager 同库及重启的合成场景，分开记录维护零写入与产品学习证据 |
| 3 | 独立 P05C 与 M5 退出评审 | 在资源上限内串行安排独立 guest，实测当前产品路径，关闭严重数据、隐私和安装缺陷，登记残余限制，并完成退出评审 |

V1/V2 与 V3 seed 已完成，R 固定 seed `d0bd5b7c…c820b`。T 的新 pair 见[日终记录](../devlogs/2026-W46.md#2026-11-14-t40-3-日终回顾与批次结果)。[合同](../remediation/linux-m5-source-first-contract.md)与[前门](../runbooks/linux-source-first-assembly.md#v3-seed-只读前门)继续适用；guest 已安装，不清理后冒充 fresh。

旧 `upgrade_quiesced` 现场及相关载体已退役，保留“历史未完成”，不安排原现场续跑。该恢复边界在新 pair 取得实际证据；旧两项 crash 只在明确审阅适用性后引用，不自动平移。连续 L6 和 P05C 的独立性要求与专机配额继续适用。

出口沿[路线图](../roadmap.md#m5linux-fcitx5-离线输入与个人化产品)：固定 Debian13 ARM64 本地 `.deb`，当前输入/个人化/隐私/持久化通过，连续 L6、八个 crash 自动合同、三个代表实际恢复边界和独立 P05C 闭合。不增加全部 Linux 平台、macOS 遗留清零或公开发布为 M5 条件。

后置：macOS WAL 新升级及 Q2/F0/W0/最终取消、其余五个真实 crash hardening、新 VM 管理/clone/迁移框架、Android/Windows/iOS、真实用户同步，以及无新热点的性能/CI 极限优化。macOS action `10–13` 与已有自动证据保留，冻结现场不重试真实 40；完整发布顺序不变。

继续使用固定 `40-2 → 40-3` 包对。先推进表中的现成验收路径，只有当前候选出现直接阻塞退出的实际缺陷时，才增加针对根因的修复和回归。环境受阻时记录具体 guest、起点与恢复条件。真实维护操作、VM 和完整门禁分项授权。

## Linux M5 系统操作停止线

日常复用 `Debian13-ARM64` / `Windows11-ARM64`，干净基线只作备份。11-22 重装终态时，专用 `RadishLex-Debian13-ARM64-L6-40-3` 唯一运行、配置无网卡，guest 只有 loopback。随后只读前门发现 `Windows11-ARM64` 也在运行，未进入 guest。恢复条件为 `Windows11-ARM64` 正常停机，专用候选唯一运行并通过新的只读资格核验。P04 专机已退役。[当前候选记录](../devlogs/2026-W47.md#2026-11-22-连续-l6-六步实测)

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

上述入口覆盖八个crash合同和现有L6控制，但不替代实机。release-pair 门禁验证冻结声明及合成合同，不表示当前 build 40 满足旧 pair 的 target 资格；实际构建仍需通过独立资格检查。`upgrade_quiesced`、连续L6与发布未闭合；其余五个真实crash转为hardening。

## 阅读索引

- [Linux M5 交付基线审计](../remediation/linux-m5-delivery-baseline-audit.md)
- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [Linux L6 收敛与本地资产生命周期](../runbooks/linux-l6-asset-lifecycle.md)
- [本周周志](../devlogs/2026-W47.md)
