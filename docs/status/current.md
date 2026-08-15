# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-15（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、acceptance controller、release-pair 与 maintenance-only refresh v1 已完成。第五套为`rolled_back`，第六套upgrade为target `completed`；两台旧repair clone分别冻结`aborted_preserved`与`completed_without_package_reapply`。`698fe1f`已让首次健康repair强制单次apply且保留proof-backed retry幂等性；包含该修复的`823afca` ARM64 refresh handoff和全新S3 clone断网只读preflight均已形成，十三台VM全停。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A deterministic carrier、production actual-package relationship、五类恢复状态机、advisory guard、system observer/port、opaque authorized CLI、startup gate、L6 format/controller、release-pair 与 maintenance-only refresh verifier均已闭合；合成矩阵不替代以下真实现场。
- 前三套暴露 dpkg config、guard parent 与 revision profile 缺口；第四套暴露 source chain不连续；第五套 upgrade在target validation失败后自动恢复source，terminal为`rolled_back`。这些现场原样保留，精确身份见runbook/devlog。
- 第六套 upgrade形成 `38-2 completed`；receipt `ccbc4cd0…1e60`、target package/evidence `b211d940…d09c`/`2a1132c6…0e1b`、startup/XDG postflight通过。S3 `S3-target-installed-80e49ce-6b22499b` 已冻结且未改写。
- repair clone `A3022255…3107`只调用一次旧ELF，receipt `ba7a9637…f17c`为`repair/same_release/aborted_preserved`、failure `version_relation_invalid` after `artifacts_staged`；dpkg/XDG零漂移，evidence `87190852…881f`/`2d300ae6…c8da`已冻结。根因与single-target effective-source回归已修复；该clone不得复用。
- maintenance-refresh v1锚定旧record/target artifact并要求`698fe1f`祖先。新`823afca` handoff record/ELF/archive/local evidence为`b3852845…a146`/`1f37b6f9…cca7`/`e917f1a9…6ad7`/`e581dec4…315f`，target package/evidence仍为`b211d940…d09c`/`2a1132c6…0e1b`；旧`b891ed1`只作历史取证。
- `BE3579E0…37F8`唯一调用形成receipt `75c2152f…af21`，但dpkg status/log/mtime未变，分类为`completed_without_package_reapply`；local/guest evidence `655f19f4…b428`/`a331fdf1…3481`已冻结，clone不得复用。
- 全新clone `394217A7…B6FB`从S3建立，`Network=[]`并切入`823afca` production-only input。只读preflight闭合package/receipt、MD5 inventory、依赖/字体、startup、XDG、operation count 2、进程与断网；未生成operation ID、运行CLI、调用`dpkg`或写XDG。host/guest evidence为`878056b2…32f4`/`79cdf8d6…e406`；关机盘`818d8725…7425`/`a73a3266…9155`/`111aaf09…8a6`稳定且S3未漂移，十三台均stopped。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步（2026-08-15）

1. 原样保留第五套 `rolled_back` clone、operation staging/receipt、三套本地 evidence 与修复备份；不得启动、重试或与第六套证据混用。
2. 原样保留第六套 terminal/S3与旧 repair failure clone；后者不 resume、重试、恢复或复用。
3. `BE3579E0…37F8`已形成`completed_without_package_reapply`终态；不得resume、重试、恢复或复用，也不得把receipt completed写成repair闭合。S3、旧pair/package与用户XDG未改写。
4. `394217A7…B6FB`是下一次真实repair的唯一候选，当前只完成`823afca` handoff断网只读preflight并保持stopped。下一步须另行单步授权：复验十三台只运行该clone、运行态断网和mutation preflight后，才生成新operation ID并只调用一次production repair；本轮证据不能写成repair或package mutation已完成，其余矩阵继续关闭。

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

上述入口证明 prior-terminal anchor、target-only builder、canonical revision、single-target repair effective-source与production-only refresh合同。Linux product现有118项测试直接覆盖首次健康repair单次apply与已有target proof的repair retry零重复mutation；refresh合同只接受包含`698fe1f`的clean descendant。单次真实repair、其后主序列、八个crash case、桌面启动、重启、完整L6与公开发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W33.md)
