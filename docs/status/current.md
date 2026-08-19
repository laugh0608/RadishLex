# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-19（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、acceptance controller、release-pair 与 maintenance-only refresh v1 已完成，六类真实operation均有独立证据。首个`install_prepared`暴露的startup共享锁目录漂移已修复；第三台clean retry已在同一断网boot闭合input/preflight、prepared checkpoint与exact resume terminal并正常停止，当前十八台VM全部stopped。连续完整L6与其余七个crash case尚未闭合。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A carrier、production relationship、恢复事务、system port/CLI/startup gate、L6 format/controller/pair/refresh均已闭合；合成矩阵不替代真实现场。
- 六类真实operation均有分散证据：第六套形成target completed/S3，独立clone闭合repair、rollback、remove与reinstall；这些不能冒充同一连续session。精确receipt、package、guest/host manifest和磁盘身份进入L6 runbook/周志。
- 旧pair唯一`install_prepared`调用形成prepared checkpoint且未触发dpkg；后续startup因遗漏合法`01777 /run/lock`失败关闭，manifest `5d8c914a…3e42a`与clone `FD24ADFF…17C056`冻结。store/startup现共用权限策略且119项默认/124项L6-feature测试通过，旧case仍不计通过。
- 修复target `d75818f`的handoff record `c74fac12…9849`已冻结。第三台clone `3EC83EB9…593B9`只生成一次operation ID、只调用一次controller与一次production exact resume；checkpoint/crash-state `3fd5df67…4a70`/`e18bde6c…8719`先证明prepared与无dpkg child。terminal postflight/postverify `5319db07…52a0`/`001b5be1…e2cc`再证明source `38-1` installed、receipt `completed`、guard absent、startup `AllowedProduct`、XDG/进程/断网稳定；terminal/关机manifest为`679b7e04…c544`/`036bace8…f3b8`，后者固定config/EFI/qcow2 `62040cc9…eb9`/`f762ee52…e76`/`95e89df3…cb69`、双重qcow2与零句柄。
- repository-only guest-case合同固定canonical inventory、唯一`build-environment.json`、文件回读真相源及fresh absent/remove terminal互斥预期，已接入L6与`check-repo`门禁；production startup语义未改。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 首台与第二台rollback只保留host evidence；第三台`EFD15599…BBDD`、remove clone `5EA2BAA2…27A2`与reinstall clone `E671DB9C…D465`均为冻结terminal；不得resume、重试、再次调用、清理、恢复、直接复用或用于其他矩阵。
- 旧pair的network失败clone只保留host evidence；真实checkpoint clone `FD24ADFF…17C056`不得resume、替换FFI、补写crash-state evidence、再次执行acceptance或用于后续case。新pair两台retry也只保留持久host evidence，不得据此恢复package或复用。
- 新`d75818f` handoff只允许作为独立clean clone的冻结输入；第三台clone现为stopped source terminal，不得重启、复用、运行下一checkpoint、覆盖、热替换或与旧pair跨套混搭。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步（2026-08-20）

1. 明日先做repository-only准备：复验Git clean、十八台all-stopped、terminal/关机manifest，再从matrix、production coordinator与现有测试固定第二个crash case `install_artifacts_staged`的精确中断态、恢复预期及host case最小差异；合同和门禁通过前不创建VM。
2. 系统动作继续逐笔授权：只从未改写DependencyFrozen建立独立clean clone，不重启或复用现有terminal；clone、启动/断网/input/preflight、controller checkpoint、resume与关机冻结均保持分批，任一差异即保留现场且不自动进入下一步。
3. 第二批repair清理、连续完整L6、guest reboot、dynamic preload/错误sibling、P05C、发布、推送、真实同步及其他平台继续关闭；不得用六类分散operation证据或首个crash terminal冒充完整session。

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

上述入口证明 prior-terminal anchor、single-target repair、production-only refresh及共享父目录策略；新pair、第三台clone与input/preflight另由ARM64 builder/宿主证据固定。真实repair、rollback、默认remove与reinstall均已闭合；首个crash仅形成失败关闭证据，八个case、连续完整L6、桌面启动/重启与公开发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W34.md)
