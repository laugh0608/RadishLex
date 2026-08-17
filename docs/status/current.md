# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-17（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、acceptance controller、release-pair 与 maintenance-only refresh v1 已完成，六类真实operation均有独立证据。首个`install_prepared` crash checkpoint已唯一执行，但startup实机暴露`/run/lock`共享父目录合同漂移；未resume或调用dpkg，旧pair不再继续。连续完整L6与八个crash/retry尚未闭合；十九台VM全停。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A carrier、production relationship、恢复事务、system port/CLI/startup gate、L6 format/controller/pair/refresh均已闭合；合成矩阵不替代真实现场。
- 前五套failure/mismatch/`rolled_back`与两台旧repair现场原样保留。第六套形成`38-2 completed`与S3；`394217A7…B6FB`的单次真实repair完成同版重装，第三台rollback的单次production调用完成`38-2 → 38-1`。精确身份见runbook/devlog。
- remove clone `5EA2BAA2…27A2`的one-shot单次调用形成receipt `770a27b7…b40e`：`remove/not_applicable/completed`、chain 4。28项payload/product tree均absent，startup为`RemovedProgram`，XDG零漂移；guest/host evidence为`529ee42c…9b8e`/`be498439…97a1`。
- reinstall clone `E671DB9C…D465`唯一production调用形成`install/not_applicable/completed` receipt `3eb44171…e274c`、chain 5，target `38-2`完整恢复且startup/XDG/断网postflight通过。guest/host evidence为`4ea296fa…fd3ba`/`54f1e952…6685`；terminal与关机盘已冻结，精确过程见runbook/devlog。
- 首个crash的第一次clone `C0D96C0A…23A1F8`因UTM注册缓存继承网卡，在写入input前停止并只作失败资产。v2 clone `FD24ADFF…17C056`以`Network=[]`和DependencyFrozen absent盘闭合断网/preflight，唯一acceptance调用形成`install_prepared` checkpoint；operation ID宿主只存hash `9429b171…9145a`，dpkg mutation与resume均为0。
- crash-state检查先因`/run`为`noexec`导致同hash startup probe不可映射；复制到`/var/tmp`后未重跑transaction，Manager/Fcitx均返回`0:1:4:11:0`即`FailedClosed/GuardInvalid`。guard本身为`root:root 0600`零长度单链接，父目录为canonical `root:root 01777 /run/lock`，确认startup读路径遗漏已冻结共享父目录合同。源码现让store/startup消费同一权限策略，119项默认与124项L6-feature测试通过；旧pair上的case不计通过。
- host失败关闭目录`RadishLex-L6-Crash-Install-Prepared-80e49ce-Failed-Closed`为`0700`，manifest `5d8c914a…3e42a`逐项通过，summary `dab8cde5…1905`只保存operation hash。v2正常关机后config/EFI/qcow2恢复`cfb5d343…1f2d`/`0b797641…1418`/`4967234b…4b18`且零句柄；十九台VM全部stopped。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 首台rollback只保留host证据；第二台只作失败关闭取证。第三台`EFD15599…BBDD`、remove clone `5EA2BAA2…27A2`与reinstall clone `E671DB9C…D465`均为冻结terminal；不得resume、重试、再次调用、清理、恢复、直接复用或用于其他矩阵。
- 两台`install_prepared` clone也只作失败取证；不得在旧pair上resume、替换FFI、补写crash-state evidence、再次执行acceptance或用于后续case。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步（2026-08-17）

1. startup共享父目录策略修复与回归已由`a6622d4`闭合；旧`80e49ce` pair和两台crash clone继续冻结，不resume、热替换或把失败证据写成通过。
2. 从包含修复的新clean target重建相邻revision release pair并完成production双侧验证；再另建clean absent clone，从头重新执行`install_prepared`。重建、clone、启动、acceptance与mutation仍分别需要授权。
3. 连续完整L6、guest reboot、dynamic preload/错误sibling、外部package lifecycle、P05C、桌面启动、清理、发布、推送、真实同步及其他平台继续关闭；不得用六类分散operation证据冒充完整session。

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

上述入口证明 prior-terminal anchor、single-target repair、production-only refresh及共享父目录策略；Linux product默认119项、L6-feature 124项测试通过。真实repair、rollback、默认remove与reinstall均已闭合；首个crash仅形成失败关闭证据，八个case、连续完整L6、桌面启动/重启与公开发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W34.md)
