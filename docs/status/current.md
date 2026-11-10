# RadishLex 当前状态

规划见[专题地图](../topics.md)；当前状态见下文。

## 当前判断

- 规划复核：2026-11-10（无新增实机证据）。macOS现场为09-09 source39恢复，外层`rolled_back`/数据`aborted_preserved`，材料保留、双组件停止，未复验。常态分支`dev`，主线`master`。
- 里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate，本批完整自动通过，V3–V5另批。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- 八个crash自动合同与六类operation分散证据已闭合；真实阻塞样本固定三项，前两项已闭合，`upgrade_quiesced`与连续L6未闭合；其余五项真实crash转为hardening。

## 能力与开放状态

里程碑退出记录只表示当时范围；实现、自动验证、实机验证、用户开放与公开发布分别判断。下表中的历史验收不覆盖新发现的验证缺口。

| 能力 | 实现与自动验证 | 实机证据 | 产品开放与发布 |
| --- | --- | --- | --- |
| macOS 离线输入与本地管理 | runtime、FFI、Manager、产品 bundle 与合同测试 | build 38 历史验收；build 39 安装、双端启动及普通选词/学习通过，其余复验待执行 | 已验收范围可供受控本地使用；尚未公开发布 |
| Linux 输入与本地管理 | Fcitx5 addon、共享 XDG/FFI、Manager | P04 Wayland/X11 与同库验收 | 受控本地验收；尚未公开发布 |
| Linux 安装维护 | `.deb`、事务、startup gate、八个 crash 自动合同 | 六类 operation 分散证据、两个代表 crash 终态 | 第三个代表 crash、连续 L6 与 P05C 未闭合 |
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

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据摘要

- Linux 输入与 Manager 已有 P04 实机记录；P05 的六类 operation 为分散证据，尚不能证明连续 L6。
- `install_prepared` 与 `install_artifacts_staged` 已取得代表 crash 恢复终态；第二场景 transaction 权威为 `7fcef38e…0e3d/completed`，停止权威为 `096fe01f…e133/stopped-verified`。
- `upgrade_quiesced` 的旧 `move-ready` 和 `fa6e7335…33ea5` 作为历史证据冻结。旧 registration shell、partial 与 S2 磁盘已退役，旧场景无法原样续跑，第三个代表 crash 仍未闭合。
- 前五批已退休 17 个 raw bundle。11 月 8 日另退休 4 份旧载体及 S2/S3 磁盘，保留快照配置、EFI、handoff、清单和失败证据。
- 原入口中的精确证据流水已原样移入[本周周志的入口归档](../devlogs/2026-W36.md#2026-09-05入口历史证据归档)。这是文档迁移，不是现场复验；操作前仍需按 runbook 重新满足资格与授权。

## 停止线

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
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38/39/40 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一阶段：Linux M5 交付收口

2026-11-10 所有者确认继续执行已批准的 source-first 合同，不重新设计基线。本轮文档记录见[周志](../devlogs/2026-W46.md#2026-11-10-linux-m5-收口规划)。**下一交付是一个固定 Debian13 ARM64 候选的安装维护、离线输入、个人化与独立验收闭环**。M5 候选与 B1–B6 见[基线审计](../remediation/linux-m5-delivery-baseline-audit.md)；macOS 支线不作为 Linux 退出前置。

| 顺位 | 必须继续 | 必要阻塞 / 关闭标准 |
| --- | --- | --- |
| 1 | 复用通用 Debian13-ARM64，核对 source 安装前门 | `c038602` 的 `40-2` handoff 已核验；只读核对包、receipt、安装状态及指定 XDG。符合 fresh-absent 后才另行安排安装，不重建已合格 source |
| 2 | 取得 source seed 终态并形成 R 锚点 | 真实 production 安装终态、host 交叉核对和独立审阅齐备；无 seed 不填 R，不以手写 receipt 补齐 |
| 3 | 形成同合同 `40-3` target / 新 pair | 沿既有 S→R→T 谱系，锁定依赖与 Rime 合同，逐侧 actual artifact 核验；不混入 `38→40` 迁移或依赖升级 |
| 4 | 连续 L6、代表恢复边界与当前产品行为 | 同一合格 guest 的连续六类 operation，三个代表 crash 的新候选适用证据，当前隐私/普通恢复/学习/删除/tombstone/Manager 同库与重启；维护 XDG 零写入与学习场景分开验收 |
| 5 | 独立 P05C 与 M5 退出评审 | 在资源上限内串行安排独立 guest，关闭严重数据、隐私和安装缺陷，登记残余限制；不是公开发布 |

V1/V2 仓库实现与自动验证已完成；seed、R、新 pair 及后续实机链尚未完成。[source-first 合同](../remediation/linux-m5-source-first-contract.md)与[只读前门](../runbooks/linux-source-first-assembly.md#v3-seed-只读前门)继续适用。前门不符先处理具体冲突，不能自动清理、clone 或重建环境。

旧 `upgrade_quiesced` 现场及相关载体已退役，保留“历史未完成”，不安排原现场续跑。该恢复边界在新 pair 取得实际证据；旧两项 crash 只在明确审阅适用性后引用，不自动平移。连续 L6 和 P05C 的独立性、专机配额与冻结 P04 不变。

出口沿[路线图](../roadmap.md#m5linux-fcitx5-离线输入与个人化产品)：固定 Debian13 ARM64 本地 `.deb`，当前输入/个人化/隐私/持久化通过，连续 L6、八个 crash 自动合同、三个代表实际恢复边界和独立 P05C 闭合。不增加全部 Linux 平台、macOS 遗留清零或公开发布为 M5 条件。

后置：macOS WAL 新升级及 Q2/F0/W0/最终取消、其余五个真实 crash hardening、新 VM 管理/clone/迁移框架、Android/Windows/iOS、真实用户同步，以及无新热点的性能/CI 极限优化。macOS action `10–13` 与已有自动证据保留，冻结现场不重试真实 40；完整发布顺序不变。

停止扩张：只有当前候选真实失败且直接阻塞上述出口，才增加修复与相关回归；环境受阻时登记具体资源决定，不自动扩大矩阵。每项真实安装、VM、完整门禁和发布仍按原授权边界执行，本次只改文档。

## Linux M5 系统操作停止线

日常复用 `Debian13-ARM64` / `Windows11-ARM64`，通用干净基线只作备份。2026-11-08 复验 UTM 6 台全部停机，RadishLex 仅保留冻结的 P04 专机。资产明细见[本机清单](/Users/luobo/VirtualMachines/README.md)与[周志](../devlogs/2026-W45.md#2026-11-08-vm-配额与资产盘点)。

1. 旧证据内容不改写；迁移映射见 `datas/_organization/2026-11-01/`。旧 Move、adopt 和物化控制仅用于审阅历史，后续实机前门按新 source/pair 和实际载体重新建立。
2. 通用测试机之外最多保留一台 RadishLex 专用 VM，按根协作约定计数。现有专用载体超额时暂停新增，按精确清单取得退役授权并核验后再安排专用 guest。
3. 连续 L6 与 P05C 的独立 guest 按顺序安排。更换专机前先确认上一轮证据已保留、旧专机退役已核验。各轮实机动作与完整门禁分别授权。

## 验证入口

- 日常 CI 已合入 `dev`，复测 1 分 43 秒。[CI 精简专题](../remediation/ci-critical-path-optimization.md)

2026-11-01 的 `fde0ecb` 加 22 文件快照通过常规与 22 项可靠性专项。耗时、调度和快照核对见[周志](../devlogs/2026-W44.md)，V3–V5 资格仍按[装配入口](../runbooks/linux-source-first-assembly.md)接续。

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
- [本周周志](../devlogs/2026-W45.md)
