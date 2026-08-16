# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-16（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、acceptance controller、release-pair 与 maintenance-only refresh v1 已完成。第五套为`rolled_back`，第六套upgrade为target `completed`；`823afca` handoff的真实repair已形成`repair/same_release/completed`与dpkg同版重装。真实rollback已降级回source `38-1`，连续remove已形成`remove/not_applicable/completed`；独立reinstall clone的absent-terminal只读资格已通过并冻结，真实reinstall尚未开始，十七台VM全停。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A carrier、production relationship、恢复事务、system port/CLI/startup gate、L6 format/controller/pair/refresh均已闭合；合成矩阵不替代真实现场。
- 前五套failure/mismatch/`rolled_back`与两台旧repair现场原样保留。第六套形成`38-2 completed`与S3；`394217A7…B6FB`的单次真实repair完成同版重装，第三台rollback的单次production调用完成`38-2 → 38-1`。精确身份见runbook/devlog。
- remove clone `5EA2BAA2…27A2`的one-shot单次调用形成receipt `770a27b7…b40e`：`remove/not_applicable/completed`、chain 4。28项payload/product tree均absent，startup为`RemovedProgram`，XDG零漂移；guest/host evidence为`529ee42c…9b8e`/`be498439…97a1`。
- 独立reinstall clone `E671DB9C…D465`从上述stopped remove terminal建立；启动前EFI/qcow2与source相同、config仅Name/UUID不同且`Network=[]`，运行态仅`lo`且双main route为空。唯一只读attempt的terminal `2858f0ed…562f`为`passed/postflight/reason=none`；证据`8ed9d43a…86b25`固定package、28项payload、product tree与dpkg info均absent，remove receipt chain仍为4，依赖/字体、`RemovedProgram` startup、XDG、进程与网络均通过。未创建operation ID，未执行maintenance/acceptance、`/usr/bin/dpkg`或reinstall。
- guest archive/host manifest为`31855e48…80782`/`a98dbec6…5006a`；正常关机后clone config/EFI/qcow2为`db1e59ae…13b90`/`d9bd0aa3…97463`/`cdf0ee06…72dd`，qcow2两次复算一致。remove、rollback v3与S3未漂移，四盘零句柄，十七台VM全停。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 首台rollback只保留host证据；第二台只作失败关闭取证。第三台`EFD15599…BBDD`为冻结rollback terminal，不直接复用。remove clone `5EA2BAA2…27A2`已是冻结terminal；reinstall clone `E671DB9C…D465`只冻结只读资格。两者均不得resume、重试、再次调用、清理、恢复或用于其他矩阵；后续真实reinstall只能在新的单步授权下启动后者。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 明日事项（2026-08-17）

1. 所有failure/mismatch/rolled-back、repair/rollback/remove terminal、reinstall preflight、S3、handoff与证据原样保留，不启动、重试、恢复、清理、复用或混用；先从clean仓库基线复验十七台VM全停、四盘零句柄与reinstall preflight host manifest `a98dbec6…5006a`。
2. 取得新的单步授权后，只启动stopped reinstall clone `E671DB9C…D465`。启动前复验关机盘与`Network=[]`，启动后重新固定单VM、loopback-only、双main route为空，并在mutation preflight重新闭合canonical pair/target artifact、remove receipt chain 4、package/product tree absent、direct dpkg状态、依赖/字体、`RemovedProgram` startup、XDG和进程静止；通过前不生成新operation ID。
3. mutation preflight通过后才可生成唯一operation ID并由one-shot单次调用production `reinstall_target`；仍不得运行acceptance、手工dpkg、Manager/Fcitx或写用户XDG。八个crash checkpoint、桌面启动/重启、完整L6、P05C、清理、发布、推送、真实同步及其他平台继续关闭。

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

上述入口证明 prior-terminal anchor、single-target repair与production-only refresh合同；118项测试覆盖首次健康repair单次apply及proof-backed retry零重复mutation。真实repair、rollback与默认remove已闭合，reinstall的absent-terminal只读资格也已冻结；真实reinstall、八个crash case、桌面启动、重启、完整L6与公开发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W33.md)
