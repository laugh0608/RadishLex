# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-15（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、acceptance controller、release-pair 与 maintenance-only refresh v1 合成合同已完成。第五套形成 `rolled_back`；第六套 upgrade 形成 target `completed`。独立 repair clone 只调用一次旧 production maintenance，但在 staged preflight 以 `version_relation_invalid` 进入 `aborted_preserved`，未调用 dpkg；源码根因与回归测试已修复。refresh 合同现能可信表达冻结 target artifact 与较新 production maintenance ELF，但 ARM64 refresh handoff 和新 S3 clone 尚未形成，十一台 VM 全停。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A deterministic carrier、production actual-package relationship、五类恢复状态机、advisory guard、system observer/port、opaque authorized CLI、startup gate、L6 format/controller、release-pair 与 maintenance-only refresh verifier均已闭合；合成矩阵不替代以下真实现场。
- 前三套暴露 dpkg config、guard parent 与 revision profile 缺口；第四套暴露 source chain不连续；第五套 upgrade在target validation失败后自动恢复source，terminal为`rolled_back`。这些现场原样保留，精确身份见runbook/devlog。
- 第六套 upgrade形成 `38-2 completed`；receipt `ccbc4cd0…1e60`、target package/evidence `b211d940…d09c`/`2a1132c6…0e1b`、startup/XDG postflight通过。S3 `S3-target-installed-80e49ce-6b22499b` 已冻结且未改写。
- repair clone `A3022255…3107` 在完整 mutation preflight 后只调用一次旧 production ELF，receipt `ba7a9637…f17c` 为 `repair/same_release/aborted_preserved`、failure `version_relation_invalid` after `artifacts_staged`。dpkg status/log逐字节未变，startup `0:1:2:2:7`、XDG `f3df…b86b`、网络/进程postflight通过；evidence `87190852…881f`/`2d300ae6…c8da` 已冻结。
- 根因是 staged repair只物理保存target，但system port未像host prepare一样把target投影为effective source。源码与single-target system-port回归已修复；116 tests和clippy通过。冻结ARM64 maintenance ELF仍是旧字节，不能热替换或复用失败clone。
- maintenance-refresh v1 已锚定旧 record、target artifact/ELF 与 `b0197f5`；26 项合成测试拒绝 package rebuild、旧 ELF、acceptance、身份漂移和覆盖式/非原子发布，尚无真实 ARM64 输出。
- 十一台注册VM当前全部stopped；repair clone关机磁盘为`584bf2b5…f59b`/`3b117def…8f0`/`adcc92fd…3a4`。完整资产、hash、guest-agent取证边界与历史流水只在L6 runbook和本周周志维护。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步（2026-08-15）

1. 原样保留第五套 `rolled_back` clone、operation staging/receipt、三套本地 evidence 与修复备份；不得启动、重试或与第六套证据混用。
2. 原样保留第六套 target-completed clone、handoff、旧 input、preflight/upgrade evidence与冻结 S3；repair clone 作为 staged-preflight `aborted_preserved` 现场保留，不 resume、重试、恢复或复用。十一台均 stopped。
3. maintenance-only refresh v1 合同、verifier、builder 源码边界和合成门禁已闭合；继续保持旧 release-pair record、target package/evidence 与第六套 handoff只读，不把宿主合成测试写成 ARM64 handoff 证据。
4. 下一步须另行授权隔离 ARM64 builder，以冻结三输入、包含 `b0197f5` 的 clean root和absent output形成production-only handoff；完整复验并冻结后，再另行授权从未改写S3创建新clone和只读preflight。真实repair及其余矩阵继续关闭。

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

上述入口证明 prior-terminal anchor、target-only builder、canonical revision、single-target repair effective-source，以及冻结 target artifact 上 production-only maintenance refresh 等合成合同。第三套证明 source install，第四套暴露 chain mismatch，第五套证明 target apply/failure/recovery；第六套证明 target terminal，并以 repair 的 mutation 前失败关闭暴露、修复 production staged verifier 缺口。修复后 ARM64 handoff、真实 repair 及其后主序列、八个真实 crash case、桌面启动、重启、完整 L6 与公开发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W33.md)
