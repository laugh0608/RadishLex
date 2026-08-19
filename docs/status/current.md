# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-19（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format v1、acceptance controller、release-pair 与 maintenance-only refresh v1 已完成，六类真实operation均有独立证据。首个`install_prepared` checkpoint暴露的startup共享锁目录漂移已修复，新pair与离线guest-case合同已冻结；第三台clean retry现为唯一started VM并已证明运行态断网，连续完整L6与八个crash/retry尚未闭合。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A carrier、production relationship、恢复事务、system port/CLI/startup gate、L6 format/controller/pair/refresh均已闭合；合成矩阵不替代真实现场。
- 六类真实operation均有分散证据：第六套形成target completed/S3，独立clone闭合repair、rollback、remove与reinstall；这些不能冒充同一连续session。精确receipt、package、guest/host manifest和磁盘身份进入L6 runbook/周志。
- 旧pair唯一`install_prepared`调用形成prepared checkpoint且未触发dpkg；后续startup因遗漏合法`01777 /run/lock`失败关闭，manifest `5d8c914a…3e42a`与clone `FD24ADFF…17C056`冻结。store/startup现共用权限策略且119项默认/124项L6-feature测试通过，旧case仍不计通过。
- 修复target `d75818f`的handoff record `c74fac12…9849`、target package `4dd00540…dcec`已冻结并通过guest/host verifier。两台harness失败retry未进入transaction，持久归档后连同两台旧失败clone按授权清理。第三台clean clone `3EC83EB9…593B9`的clone-only manifest为`13f2e3e9…954a2`；现已单独授权启动，network evidence `711f850c…b9d15`经双重回读一致，host manifest `4b7081d5…7291`证明仅`lo`、双main route为空、其余十七台stopped。
- repository-only guest-case合同现固定UTF-8 bytewise canonical input inventory、唯一`build-environment.json`、guest-agent仅作观察量且result/evidence文件回读为真相源，并把fresh absent的`ReceiptMissing/no receipt`与remove terminal的`RemovedProgram/completed`拆成互斥回归。合同已接入L6与`check-repo`门禁，未修改production startup语义或任何guest/system状态。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 首台与第二台rollback只保留host evidence；第三台`EFD15599…BBDD`、remove clone `5EA2BAA2…27A2`与reinstall clone `E671DB9C…D465`均为冻结terminal；不得resume、重试、再次调用、清理、恢复、直接复用或用于其他矩阵。
- 旧pair的network失败clone只保留host evidence；真实checkpoint clone `FD24ADFF…17C056`不得resume、替换FFI、补写crash-state evidence、再次执行acceptance或用于后续case。新pair两台retry也只保留持久host evidence，不得据此恢复package或复用。
- 新`d75818f` handoff只允许作为独立clone的冻结输入；第三台clone当前保持started/断网，未获授权不得传input、生成operation ID、停止或执行其他guest命令，也不得覆盖、热替换或与旧pair跨套混搭。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步（2026-08-19）

1. 下一笔系统动作须另行授权：在同一断网boot先再次回读网络证据，再把冻结handoff投影为canonical input并闭合input switch、fresh-absent只读与mutation preflight。任一predicate失败即保留现场；通过前不生成operation ID或运行acceptance，`install_prepared`仍须下一笔独立授权。
2. 第二批repair历史clone `A3022255…3107`/`BE3579E0…37F8`仍含独立真实operation，只作后续清理候选；没有新的精确授权不得删除。DependencyFrozen、builder、terminal、旧pair真实checkpoint与前四套L6现场继续保留。
3. 连续完整L6、guest reboot、dynamic preload/错误sibling、外部package lifecycle、P05C、桌面启动、进一步清理、发布、推送、真实同步及其他平台继续关闭；不得用六类分散operation证据冒充完整session。

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

上述入口证明 prior-terminal anchor、single-target repair、production-only refresh及共享父目录策略；新pair与第三台clone另由ARM64 builder/宿主证据固定。真实repair、rollback、默认remove与reinstall均已闭合；首个crash仅形成失败关闭证据，八个case、连续完整L6、桌面启动/重启与公开发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W34.md)
