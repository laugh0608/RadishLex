# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-15（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、acceptance controller、release-pair 与 maintenance-only refresh v1 已完成。第五套为`rolled_back`，第六套upgrade为target `completed`；两台旧repair clone分别冻结`aborted_preserved`与`completed_without_package_reapply`。`698fe1f`已让首次健康repair强制单次apply且保留proof-backed retry幂等性；包含该修复的`823afca` ARM64 refresh handoff已在全新S3 clone完成一次真实repair，terminal为`completed`且dpkg确实重装同版package。首台rollback preflight clone已在明确清理授权下删除、host证据保留；第二台在传输根身份断言失败后停止，十四台VM全停。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A carrier、production relationship、恢复事务、system port/CLI/startup gate、L6 format/controller/pair/refresh均已闭合；合成矩阵不替代真实现场。
- 前四套failure/mismatch与第五套`rolled_back`现场原样保留。第六套形成`38-2 completed`与未改写S3；两台旧repair clone冻结为`aborted_preserved`和`completed_without_package_reapply`，精确身份见runbook/devlog。
- `394217A7…B6FB`只调用一次production repair；receipt `4e23198c…133b`为`repair/same_release/completed`，dpkg同版重装、startup/XDG postflight通过。guest/host evidence为`4e46a101…608b`/`116e3f06…5e90f`；没有resume、rollback、第二次调用、acceptance或产品启动。
- 首台rollback clone `9FE6265D…AE28`的failed-closed host manifest `6b7ac641…a51d`仍完整保留；其UTM注册项和package已按明确授权删除。第二台独立clone `BFB3EF09…2720`仍从未改写S3建立、`Network=[]`，文件回读`d7bac2bc…6bd`确认仅loopback、双main route为空、package/receipt与进程静止；canonical pair/source/target身份不变。
- 第二台在私有传输根创建后返回`transfer_setup_error=root-identity`，正式只读preflight脚本尚未传入或执行，attempt marker、operation ID、maintenance/acceptance、`dpkg`与rollback均未发生。冻结脚本的失败断言要求目录`700|root|root|1`；目录link count为1是高概率控制缺陷，但guest实际`stat`未冻结，不能改写成已证实根因或产品漂移。
- 第二台failed-closed host manifest `40dab9b…680e`与关机盘`5258c84a…005`/`a73a3266…9155`/`1d20869b…57b`已冻结；S3仍为`00bac87d…6456`/`0846b1d3…9741`/`6b22499b…b137`且零句柄，十四台stopped。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 首台rollback clone package已删除，仅保留`6b7ac641…a51d` host证据；不得用证据目录冒充可注册或可复用VM。第二台`BFB3EF09…2720`只作传输准备失败关闭取证，不启动、重试、诊断、恢复、清理或复用；不得把高概率目录link-count控制缺陷写成已冻结guest实际值。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步（2026-08-15）

1. 所有failure/mismatch/rolled-back、三台repair clone、第二台rollback clone、S3及证据原样保留，不启动、重试、恢复、清理、复用或混用；首台rollback clone只保留host证据。
2. 再次rollback preflight须另行授权且只从S3建立第三台全新clone。私有传输根只复验目录mode/owner/group与非symlink目录身份；`single-link`只用于归一化后的普通输入文件，再复验`0600`/root/size/hash。正式检查仍须以`O_EXCL`落attempt marker，逐phase记录并原子写terminal result，宿主回拉验真。
3. 生成operation ID和执行rollback仍须再单独授权；remove/reinstall/crash matrix、P05C、发布、推送、清理、真实同步及其他平台继续关闭。

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

上述入口证明 prior-terminal anchor、single-target repair与production-only refresh合同；118项测试覆盖首次健康repair单次apply及proof-backed retry零重复mutation。宿主release-pair verifier也已离线复验当前canonical record；它不能替代新的guest terminal result。真实repair现已闭合；两次rollback preflight均失败关闭，rollback/remove/reinstall、八个crash case、桌面启动、重启、完整L6与公开发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W33.md)
