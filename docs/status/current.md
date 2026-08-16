# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-16（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、acceptance controller、release-pair 与 maintenance-only refresh v1 已完成。第五套为`rolled_back`，第六套upgrade为target `completed`；`823afca` handoff的真实repair已形成`repair/same_release/completed`与dpkg同版重装。真实rollback已降级回source `38-1`；其连续remove clone的唯一production调用形成`remove/not_applicable/completed`，package/product tree absent、startup失败关闭、XDG零漂移，十六台VM全停。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A carrier、production relationship、恢复事务、system port/CLI/startup gate、L6 format/controller/pair/refresh均已闭合；合成矩阵不替代真实现场。
- 前四套failure/mismatch与第五套`rolled_back`现场原样保留。第六套形成`38-2 completed`与未改写S3；两台旧repair clone冻结为`aborted_preserved`和`completed_without_package_reapply`，精确身份见runbook/devlog。
- `394217A7…B6FB`只调用一次production repair；receipt `4e23198c…133b`为`repair/same_release/completed`，dpkg同版重装、startup/XDG postflight通过。guest/host evidence为`4e46a101…608b`/`116e3f06…5e90f`；没有resume、rollback、第二次调用、acceptance或产品启动。
- 首台rollback clone已删除并保留manifest `6b7ac641…a51d`；第二台`BFB3EF09…2720`冻结在transfer `root-identity`失败现场。第三台`EFD15599…BBDD`的唯一production rollback形成receipt `b97c7580…664d`、chain 3；dpkg delta `6b4e8f39…6721`证明`38-2 → 38-1`，guest/host evidence为`879ab337…2b7e`/`fef4ea32…851d`。
- 独立remove clone `5EA2BAA2…27A2`从第三台rollback terminal建立；既有只读preflight manifest `ade5743b…9dd8`与本轮mutation preflight `0be5e2a8…7f8d`均通过。guest生成的唯一operation ID只保存hash `93fc5b83…ef3d`；one-shot仅调用一次production remove，stdout为`maintenance_outcome=completed`，未retry、resume、运行acceptance或手工调用dpkg。
- receipt `770a27b7…b40e`为`remove/not_applicable/completed`、chain 4；dpkg delta `c5dd0066…3ad4`记录完整remove lifecycle，28项payload与product tree均absent，依赖/字体保持。Manager/Fcitx startup均以`RemovedProgram`失败关闭；XDG指纹保持`f3df287f…b86b`，WAL/SHM/profile absent，进程、映射和loopback-only网络静止。
- postflight初版把同hash FFI放在`noexec`的`/run`而失败，失败result/stderr已冻结；改在`/var/tmp`执行同一探针后，terminal evidence `eef62072…e80d`通过，未重跑maintenance。guest archive/host manifest为`529ee42c…9b8e`/`be498439…97a1`；关机盘`92568d22…626a`/`abea62d2…c8fc`/`ad8a6c6b…55fa`复算一致。rollback v3与S3未漂移，三盘零句柄、十六台全停。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 首台rollback只保留host证据；第二台只作失败关闭取证。第三台`EFD15599…BBDD`为冻结rollback terminal，不直接复用。remove clone `5EA2BAA2…27A2`已是冻结terminal，不resume、重试、再次调用、清理、恢复或直接用于其他矩阵；后续只能按新授权从它建立独立reinstall clone。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 明日事项（2026-08-17）

1. 所有failure/mismatch/rolled-back、repair/rollback/remove terminal、S3、handoff与证据原样保留，不启动、重试、恢复、清理、复用或混用；先从clean仓库基线复验十六台VM全停、三盘零句柄与remove host manifest `be498439…97a1`。
2. 取得新的单步授权后，只从stopped remove terminal建立独立reinstall clone；启动前冻结config/EFI/qcow2与`Network=[]`，启动后只做loopback-only、双main route为空、remove receipt chain 4、package/product tree absent、依赖/字体、`RemovedProgram` startup、XDG和进程静止的只读preflight。本步不生成operation ID、不运行maintenance/acceptance或dpkg。
3. 只读资格另行冻结后，真实`reinstall_target`仍须新的独立授权；八个crash checkpoint、桌面启动/重启、完整L6、P05C、清理、发布、推送、真实同步及其他平台继续关闭。

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

上述入口证明 prior-terminal anchor、single-target repair与production-only refresh合同；118项测试覆盖首次健康repair单次apply及proof-backed retry零重复mutation。真实repair、rollback与默认remove现已闭合；reinstall、八个crash case、桌面启动、重启、完整L6与公开发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W33.md)
