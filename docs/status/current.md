# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-15（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、acceptance controller、release-pair 与 maintenance-only refresh v1 已完成。第五套为`rolled_back`，第六套upgrade为target `completed`；两台旧repair clone分别冻结`aborted_preserved`与`completed_without_package_reapply`。`698fe1f`已让首次健康repair强制单次apply且保留proof-backed retry幂等性；包含该修复的`823afca` ARM64 refresh handoff已在全新S3 clone完成一次真实repair，terminal为`completed`且dpkg确实重装同版package。首台rollback preflight clone已在明确清理授权下删除、host证据保留；第二台在传输根身份断言失败后冻结；第三台独立clone的唯一production rollback已形成`rollback/target_older/completed`，真实降级回source `38-1`且XDG零漂移，十五台VM全停。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A carrier、production relationship、恢复事务、system port/CLI/startup gate、L6 format/controller/pair/refresh均已闭合；合成矩阵不替代真实现场。
- 前四套failure/mismatch与第五套`rolled_back`现场原样保留。第六套形成`38-2 completed`与未改写S3；两台旧repair clone冻结为`aborted_preserved`和`completed_without_package_reapply`，精确身份见runbook/devlog。
- `394217A7…B6FB`只调用一次production repair；receipt `4e23198c…133b`为`repair/same_release/completed`，dpkg同版重装、startup/XDG postflight通过。guest/host evidence为`4e46a101…608b`/`116e3f06…5e90f`；没有resume、rollback、第二次调用、acceptance或产品启动。
- 首台rollback clone已删除VM并保留failed-closed manifest `6b7ac641…a51d`。第二台`BFB3EF09…2720`冻结在正式preflight前的传输根`root-identity`断言，manifest `40dab9b…680e`；目录link count是高概率控制缺陷，guest实际值未冻结。两台均未生成operation ID或执行mutation。
- 第三台clone `EFD15599…BBDD`从未改写S3建立，`Network=[]`；唯一只读preflight terminal `64f1b1ab…8ee`为`passed`，证据`4edcbb6b…9f2`闭合pair、artifact、package/receipt、依赖/字体、startup、XDG、进程与断网，host manifest为`0b71c24b…3073`。当时未创建operation ID或执行mutation。
- 后续授权只启动该clone；boot网络证据`629680f8…37d6`与mutation preflight `3c4894fa…6320`通过。guest生成唯一operation ID而宿主只保存hash `663623bc…c8b1`；one-shot只调用一次production rollback。receipt `b97c7580…664d`为`rollback/target_older/completed`、chain 3；dpkg delta `6b4e8f39…6721`证明`38-2 → 38-1`，source payload/startup/XDG/进程/断网由postflight `62e17a05…0427`通过。
- guest/host evidence为`879ab337…2b7e`/`fef4ea32…851d`；关机盘`6295c4ed…29ae`/`3b117def…a8f0`/`9a28509a…995a`复算一致，S3未漂移、两盘零句柄、十五台全停。未执行acceptance/remove/reinstall/crash、Manager/Fcitx或XDG写入。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 首台rollback只保留host证据；第二台只作失败关闭取证，不启动、重试、诊断、恢复、清理或复用。第三台`EFD15599…BBDD`已是`rollback/target_older/completed`终态现场，不resume、重试、再次调用、清理或直接复用；后续连续链只允许在新授权下从其registered terminal建立独立clone。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步（2026-08-15）

1. 所有failure/mismatch/rolled-back、三台repair clone、前两台rollback现场、S3及证据原样保留，不启动、重试、恢复、清理、复用或混用；首台rollback clone只保留host证据。
2. 下一次连续主序列只允许另行授权从第三台`EFD15599…BBDD`的stopped registered terminal建立独立remove clone，并先闭合启动前identity、`Network=[]`、单VM、运行态断网、source package/receipt/XDG/process只读preflight；在新的operation授权前不得生成operation ID或执行remove。
3. reinstall、八个crash checkpoint、P05C、发布、推送、清理、真实同步及其他平台继续关闭。

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

上述入口证明 prior-terminal anchor、single-target repair与production-only refresh合同；118项测试覆盖首次健康repair单次apply及proof-backed retry零重复mutation。宿主release-pair verifier也已离线复验当前canonical record；它不能替代guest terminal result。真实repair与真实rollback现已闭合；remove/reinstall、八个crash case、桌面启动、重启、完整L6与公开发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W33.md)
