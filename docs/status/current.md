# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核：2026-09-09（macOS 39→40 暂停事务已获单独授权，经独立 Installer 显式中止并恢复 source 39；外层 `rolled_back` / 数据 `aborted_preserved`，原 DB/WAL 与受控资产保留，恢复后双组件未启动）；常态分支 `dev`，主线 `master`。
- 里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
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

## 综合审阅跟踪

- 激活[2026-09 产品审阅与改进跟踪](../remediation/product-review-2026-09.md)，初审基线 `a5345b8`；2026-09-08 完成隔离诊断，随后按批准方案修复 REV-01 并升级 REV-02 Rust SQLite 依赖；新构建的真实输入复验尚未完成。
- REV-01 已落实 RadishLex 独占学习与 composition 最严格策略保留；升级 SQLite 后 native 48 进程存储/旧合成库回归及 12 进程配置回归通过。REV-02 Rust 链已升至 bundled SQLite 3.51.3，WAL/旧库/备份恢复与新 macOS FFI 身份核验通过；Go 依赖和冻结产物未更换。两项的平台复验、输入质量与工具链限制仍开放，结果与关闭条件只在专题维护。
- macOS `26.7.1 (39)` 双组件与本地 ad-hoc Installer 已构建，包内 FFI 合成 smoke、资源/载荷身份、完整仓库门禁及 Manager 99 项测试通过。2026-09-09 首次安装到达 `completed`，双程序身份与无 quarantine 检查通过；Manager 从固定路径启动，显示 `local_only`。项目所有者已手动添加/选择输入源，固定 TextEdit `shi → 时` 两次选择与本地学习增量通过，候选由 2 升至 1。后续按[联合验收入口](../runbooks/macos-rev01-rev02-acceptance.md)验证隐私及其余矩阵；此载体无历史升级源，未制作新 DMG、未发布。
- 项目所有者选择本机现有账户测试；此前 17 个旧目录根已原样归档，545 个节点核对通过。build 39 新验收库未导入旧资料，全程隐私零学习与目标词普通恢复已验证；先前混合输入区间不作为单次精确用例。另行授权的 composition 隐私→普通组观察到零学习，但新普通对照在 TextEdit 提交后记入 `code` 而非 `editor`，已暂停后两组。暂停时隐私键已恢复显式 false、临时基线已消费，输入源选中且 InputMethod 运行；后续仓库修复未再操作该现场。
- `6a55782` 已修复 IMK 客户端身份来源，build 40 升级曾停于 `data_coordinating` / `candidate_verified`。错误呈现、合法 WAL 特征回归及显式恢复入口已实现，原生/全仓门禁与真实程序副本资格通过。项目所有者随后单独确认实际恢复：独立 Installer 对同一 `aad9cf8a…a706` 完成 source 39 双程序原 inode 恢复，外层 `rolled_back`、内层 `aborted_preserved`；原 DB/WAL/SHM、snapshot/candidate/settings backup 的身份、metadata 和摘要保留，target 40 留在 staging。source 双端及数据只读 startup gate 允许，target 双端拒绝；未启动双组件或执行新输入。继续激活 [WAL 升级与恢复方案](../remediation/macos-wal-upgrade-recovery-2026-09.md)：永久 WAL 源库准备仍待独立设计/实施，不能直接重试 40。REV-01/REV-02 继续开放，实际结果见[联合验收入口](../runbooks/macos-rev01-rev02-acceptance.md#切换前中止与-source-39-实机恢复完成2026-09-09)。
- 次要事项为输入回调锁等待、新词召回/评测、删除与事件保留、MSRV/CI；维护成本、Manager 易用性和早期反馈列为后续建议。具体证据、未知项与关闭条件只在跟踪专题维护。
- 当前 Rime 来源锁已变化，不能作为冻结 Linux L6 pair 的新 target；现有构建资格检查继续拒绝混配。旧 pair、M5 系统操作顺位和冻结现场保持原状；新 pair 或平台实机动作仍须单独明确范围。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据摘要

- Linux 输入与 Manager 已有 P04 实机记录；P05 的六类 operation 为分散证据，尚不能证明连续 L6。
- `install_prepared` 与 `install_artifacts_staged` 已取得代表 crash 恢复终态；第二场景 transaction 权威为 `7fcef38e…0e3d/completed`，停止权威为 `096fe01f…e133/stopped-verified`。
- `upgrade_quiesced` 当前为 partial `move-ready`：`7765f1c` 的唯一 prepare 观察 8 台全停，manifest `fa6e7335…33ea5` 冻结；未执行 UI Move 或物化。
- 五批 VM 退休共 17 个 raw bundle 已 absent，旧 snapshot/handoff 与失败证据保留。
- 原入口中的精确证据流水已原样移入[本周周志的入口归档](../devlogs/2026-W36.md#2026-09-05入口历史证据归档)。这是文档迁移，不是现场复验；操作前仍需按 runbook 重新满足资格与授权。

## 停止线

- macOS `aad9cf8ac2dae71b2b659e96a91ea706` 已为外层 `rolled_back` / 数据 `aborted_preserved`。源 39 双程序已恢复，target 40 保留在同 operation 的两个 `staged.app`；原 DB/WAL/SHM、snapshot/candidate/settings backup、独立恢复证据、原 39/40 与恢复 Installer 全部保留。输入源未选中、Manager/InputMethod 停止、privacy false，恢复 Installer 保持终态窗口。实际恢复授权已完成，不点“重新执行”/移除，不手动 checkpoint/删除 sidecar，不清理或启动双组件/输入测试；后续 WAL 新升级合同及系统动作另行确定范围。
- 五批均闭合`deleted`，第五批manifest为`a65abab2…1b106`；授权均已消费，不得复跑。
- 第五批证据、projection与S2冻结，三台bundle已absent；不得恢复、重建、注册或复用。
- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 五批共17个raw bundle已absent，账面157.60 GiB；prepare/delete证据与旧snapshot/handoff保留，其余disk不得启动、恢复、清理或复用。
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
- clone v1/v2根、`Evidence-v5`与默认Documents partial target冻结；不得reclone、start、delete、手工搬移或冒充S2。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38/39/40 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## Linux M5 系统操作下一步（顺位不变，2026-09-05 复核）

1. 只可先另行授权一次UTM原生UI Move；`move-adopt`与物化继续分段授权。
2. start、input-preflight、crash、resume与terminal-stop仍分段授权；第五批与clone不得复跑。
3. 第三场景闭合后，以一台新guest完成连续L6；P05C使用独立guest。

## 验证入口

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

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [Linux L6 收敛与本地资产生命周期](../runbooks/linux-l6-asset-lifecycle.md)
- [本周周志](../devlogs/2026-W37.md)
