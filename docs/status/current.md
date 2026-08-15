# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-15（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、acceptance controller、release-pair 与 maintenance-only refresh v1 已完成。第五套为`rolled_back`，第六套upgrade为target `completed`；`823afca` handoff的真实repair已形成`repair/same_release/completed`与dpkg同版重装。第三台rollback clone的唯一production调用形成`rollback/target_older/completed`并降级回source `38-1`；独立remove clone只读preflight现为`passed/postflight`，十六台VM全停。

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
- 独立remove clone `5EA2BAA2…27A2`从第三台stopped registered terminal建立；启动前EFI/qcow2等于source、config规范化相同且`Network=[]`。唯一只读attempt terminal `bc65fdb0…fcbc`为`passed/postflight/none`；evidence `35f58ca3…ed12`闭合source package/receipt、依赖/字体、startup、XDG、进程与loopback-only，未创建新operation ID或执行maintenance/acceptance、`dpkg`、remove、Manager/Fcitx及XDG写入。
- guest/host evidence为`14506554…d676`/`ade5743b…9dd8`；关机盘`92568d22…626a`/`4b6292f0…d726`/`91db6f02…6bab`复算一致。v3与S3未漂移，三盘零句柄、十六台全停。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 首台rollback只保留host证据；第二台只作失败关闭取证。第三台`EFD15599…BBDD`为冻结rollback terminal，不直接复用。remove clone `5EA2BAA2…27A2`只允许在新的单步授权下继续真实remove，不清理、恢复或用于其他矩阵。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 明日事项（2026-08-16）

1. 所有failure/mismatch/rolled-back、三台repair clone、前两台rollback现场、S3及证据原样保留，不启动、重试、恢复、清理、复用或混用；首台rollback clone只保留host证据。
2. 取得新的单步授权后，只启动remove clone `5EA2BAA2…27A2`；先复验十六台registered VM中其余全部stopped、`Network=[]`、运行态仅loopback、IPv4/IPv6 main route为空，并重新闭合source package/receipt、依赖/字体、startup、XDG、进程静止与mutation preflight。全部通过后才生成唯一operation ID，并只调用一次production remove。
3. operation后必须以receipt `remove/not_applicable/completed`、package/product tree absent、startup `RemovedProgram`失败关闭、XDG原样保留、进程与网络静止作为terminal postflight；正常关机并冻结guest/host证据、磁盘身份和零句柄。不得自动继续reinstall、八个crash checkpoint、P05C、清理、发布、推送、真实同步或其他平台。

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

上述入口证明 prior-terminal anchor、single-target repair与production-only refresh合同；118项测试覆盖首次健康repair单次apply及proof-backed retry零重复mutation。真实repair、rollback及remove前只读资格现已闭合；remove operation、reinstall、八个crash case、桌面启动、重启、完整L6与公开发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W33.md)
