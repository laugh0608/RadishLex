# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-23（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 controller/pair/refresh与六类operation已有证据，首个crash已闭合。`install_artifacts_staged` v4已消耗唯一foreground transport/transfer/resolution；network v2证明当前boot断网，resolution已把input闭合为`input-ready`。repository-only negative preflight v1控制已闭合但尚未真实调用；仍无operation ID/transaction，其余七个crash与连续L6未闭合。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A carrier、production relationship、恢复事务、system port/CLI/startup gate、L6 format/controller/pair/refresh均已闭合；合成矩阵不替代真实现场。
- 六类真实operation均有分散证据：第六套形成target completed/S3，独立clone闭合repair、rollback、remove与reinstall；这些不能冒充同一连续session。精确receipt、package、guest/host manifest和磁盘身份进入L6 runbook/周志。
- 旧pair唯一`install_prepared`调用形成prepared checkpoint且未触发dpkg；后续startup因遗漏合法`01777 /run/lock`失败关闭，manifest `5d8c914a…3e42a`与clone `FD24ADFF…17C056`冻结。store/startup现共用权限策略且119项默认/124项L6-feature测试通过，旧case仍不计通过。
- 修复target `d75818f`的handoff `c74fac12…9849`已冻结。第三台clone `3EC83EB9…593B9`只生成一次operation ID并闭合prepared checkpoint与exact resume；terminal/关机manifest `679b7e04…c544`/`036bace8…f3b8`证明source `38-1` installed、receipt completed、startup/XDG/进程/断网稳定、磁盘双回读与零句柄，细项见runbook。
- repository-only guest-case合同除canonical input/readback/startup预期外，现固定`install_artifacts_staged`的target-only staging、package/dpkg未变、合法guard、XDG/process/network零漂移与resume重验/单次apply，并与matrix交叉校验。首次安装精确Rust回归通过，production语义无需修改。
- 第二个case旧clone两次start均失败关闭且未改盘/进入guest；v2唯一clone又以exit 0、stderr OSStatus `-1712`但零注册/package落地失败关闭。manifest `337007ff…7ebbb`/`65160b12…c1859`冻结，未获得可归因根因。
- repository-only `l6_utm_clone_once.py`以双授权绑定clean head、前序manifest、全停清单、source与目标absence；只有命令成功、stderr空、唯一stopped注册增量及精确package存在才返回created，失败/拒绝/不确定分别返回10/11/12且不自动retry/delete/start。11项回归已进入L6门禁。
- v3 clone/prepared manifest `7ce53048…e5b7`/`b065c7ac…32db`已冻结；唯一start timeout并保持二十台全stopped，start/failure/postverify `870f56dd…f3a5`/`59c62c14…bba05`/`8e3d5ced…8386`确认磁盘未变且零guest/input/transaction，仍不能归因具体UTM/QEMU根因。
- v1-v6只读host诊断逐步补齐进程、UID、日志上限、空category与finished marker合同；六份manifest见L6 runbook并保持冻结。每次均为二十台全停、`root_cause=unattributed`与零VM/guest/transaction mutation。
- v7只读诊断绑定九份前序manifest，确认二十台全停和相关进程为0；完整脱敏日志闭合到UTM接收start后的AppKit主窗口断言，没有对应reply或QEMU事件。十项manifest `206aa335…7b56c`通过；terminal仍为`root_cause=unattributed`及零VM/guest/transaction动作。
- v4 clone/prepared manifest `f76d1943…ff6e2`/`c40c55d9…144b`曾固定UUID `50B75F88…8038`、`Network=[]`及config/EFI/qcow2 `2de7280b…6195`/`0b797641…1418`/`4967234b…4b18`；21台全停、双重qcow2与零句柄独立复核通过。clean `ea46c9d`后的系统只读门再确认app已不驻留、前后零相关进程，未发送quit。
- 从clean `296a1c5`执行唯一`foreground-applescript-v1`：prepared/live身份、两次descriptor和双进程门均通过；osascript只调用1次、exit 0、空输出。冻结目录`…-v4-Foreground-Launch-Transport-v2`含16项，manifest `6dbbdf40…fc3c`逐项通过且权限为`0700`/`0600`；首轮status与terminal list已见target started，但terminal进程为0，故原控制返回12/`state-indeterminate`，没有quit/stop/retry/delete/guest/input/operation/transaction。
- 后续只读交叉验证始终只有v4登记started；QEMU backend延迟出现并持有EFI/qcow2，活动磁盘的多次变化hash均不是terminal身份。plain list/status后backend可能再次出现，故不再循环查询；修复`2eea62f`只改善未来延迟轮询，不回写本次终态。
- clean `e75509a`的network v1在宿主`utmctl exec`参数解析阶段失败关闭，8项manifest `d5d33212…7348`确认network script/push/pull均为0。修正控制在clean `3286268`逐项绑定launch 16项及v1失败7项后只执行一次v2：脚本push后逐字回读SHA-256 `d0bb5c28…1f28a`，network script仅调用1次，两个301-byte回读均为`2f9abfbe…d0e8e`。结构化证据固定boot `1bcbd795…26ae3`、active仅`lo`、nonloop接口/UP均0、IPv4/IPv6 main route均0；15项manifest `40be3f3f…38383`逐项通过，业务input/operation/transaction/stop/retry/quit及plain list/status/start均未执行。
- canonical input transfer v1强绑定v2 manifest `40be3f3f…38383`、live network双回读和授权source绝对路径/size/SHA-256；同一`O_NOFOLLOW` descriptor前后复验12项USTAR。bundle唯一push、双完整回读后，root installer在私有`0700`staging解包并以`RENAME_NOREPLACE`发布固定input root；host/guest证据create-new，任一漂移失败关闭且不生成operation ID、不进入transaction、不自动retry/stop。10项回归已接入默认L6门禁。
- canonical source bundle已由clean `59eaab3`在create-new根`…-v4-Canonical-Input-v1`冻结：92,825,600 bytes、SHA-256 `7bbeb291…403c`，七文件manifest `ffc990a3…e0de`。12项USTAR、双重bundle hash、owner/mode/link、零xattr与manifest均独立回读通过；terminal固定零`utmctl`、guest、input transfer、operation/transaction及VM启停。
- clean `6a23c37`的唯一attempt `d75818f-v4-input-20260823-v1`通过network/target/process/句柄、installer readback、bundle唯一push/双回读及source postflight。installer调用exit 0且空输出，但同秒首次pull报告`transfer.evidence.json`不存在，第二次result回读未执行，input root状态未知。create-new根`…-v4-Input-Transfer-v1`的19项及manifest `d1090e8d…dc09`通过，权限`0700`/`0600`、single-link且零xattr；terminal为`state-indeterminate`，无list/status/start、断网复跑、operation/transaction、retry/stop/quit。
- clean `ca57dce`的唯一resolution双回读稳定passed result；一次probe证明installer/可疑进程均0、staging absent、final私有且12项inventory逐文件匹配。create-new host根`…-v4-Input-Resolution-v1`的21项与manifest `7bab8f20…4e4a`通过，权限`0700`/`0600`、single-link、零xattr；terminal为`input-ready`，原installer/bundle、operation/transaction、retry/stop/quit均未执行。
- repository-only negative preflight v1已绑定resolution 21项/`7bab8f20…4e4a`及全部前序身份；one-shot probe以exclusive marker、原子phase/no-replace result只读重验input/pair/package/dependency/font/startup/XDG/process/network。8项host与5项probe回归已进入L6门禁；未调用真实`utmctl`或guest，完整边界见runbook。

## 停止线

- 未获后续单步授权不得在真实 guest 再运行产品 `dpkg`、写 `/usr`/`/var` 或用户 XDG、修改 Fcitx profile/autostart/systemd、启停 Manager/Fcitx/桌面会话，或执行 upgrade/repair/remove/rollback/reinstall。
- 不重新启动、恢复、清理或复用前四个 stopped L6 failure/mismatch disk；第五套 terminal `rolled_back` 现场只作失败/恢复取证，仍不得启动、重试或复用。各套 evidence 分属不同 config/boot/receipt 身份，不得混用。
- UTM 只使用 `PATH` 中的 plain `utmctl`；任何时刻最多运行一台 VM，启动前必须确认其他注册 VM 全部停止。
- 首台与第二台rollback只保留host evidence；第三台`EFD15599…BBDD`、remove clone `5EA2BAA2…27A2`与reinstall clone `E671DB9C…D465`均为冻结terminal；不得resume、重试、再次调用、清理、恢复、直接复用或用于其他矩阵。
- 旧pair的network失败clone只保留host evidence；真实checkpoint clone `FD24ADFF…17C056`不得resume、替换FFI、补写crash-state evidence、再次执行acceptance或用于后续case。新pair两台retry也只保留持久host evidence，不得据此恢复package或复用。
- 新`d75818f` handoff只允许作为独立clean clone的冻结输入；第三台clone现为stopped source terminal，不得重启、复用、运行下一checkpoint、覆盖、热替换或与旧pair跨套混搭。
- 第二个case clone `B0B826F6…87B3`须保持stopped并冻结为双start失败现场；不得第三次start、进入guest、修补注册/config、复用或与第一case混用。
- v2 clone失败证据`65160b12…c1859`须原样保留；不得沿用本批授权重试clone、重启UTM或把exit 0记为成功。
- 新v3 clean clone `5B19AEF1…7DAB`现冻结为单次start失败现场；不得第二次start、重复物化、替换config/磁盘、传input、进入guest或transaction，也不得把全停结果记为case通过。
- 七次host诊断证据`158fe177…77c`/`8ccdbb9f…dc7`/`f952953a…e5ddf`/`34ae0563…743e`/`9da78f78…08ae`/`44043282…4ae8`/`206aa335…7b56c`均须冻结，不覆盖、补写或复用。v7只定位host UTM/AppKit失败机制，不授权原target retry、`--hide`试跑、GUI动作或把更深根因写成已证实。
- v4 clone/prepared/launch、network v1/v2、bundle、transfer v1与resolution v1证据及UUID `50B75F88…8038`须冻结；不得第二次start、重复断网/transfer/resolution、覆盖guest/host证据或把活动qcow2哈希当terminal。input现只由`7bab8f20…4e4a`证明ready；repository-only negative preflight控制不是guest preflight证据，也不授权operation/transaction。
- repository-only控制与测试不是系统授权。不得重跑installer、bundle push/transfer/probe、自行补证或进入operation/transaction；后续真实动作仍须committed控制、create-new证据与单独精确授权。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步（2026-08-23）

1. 冻结launch/network/bundle/transfer/resolution manifest `6dbbdf40…fc3c`/`40be3f3f…38383`/`ffc990a3…e0de`/`d1090e8d…dc09`/`7bab8f20…4e4a`，不覆盖或复跑；prepared哈希只代表启动前，活动EFI/qcow2不得恢复或宣称clean。
2. 下一步只可另行授权一次真实negative preflight：attempt `d75818f-v4-negative-preflight-20260823-v1`，create-new host根`…-v4-Negative-Preflight-v1`、guest根`/var/tmp/radishlex-l6-v4-negative-preflight-d75818f-v4-negative-preflight-20260823-v1`。只允许committed门禁、冻结network/resolution回读及单一probe push/回读/调用；不得执行`case.sh`、maintenance、acceptance、`dpkg`/`dpkg-query`、installer、transfer、operation/transaction、retry/cleanup/stop/quit/list/status/start。精确绝对host路径与控制范围见runbook。
3. negative preflight闭合为`preflight-ready`后，operation ID、`install_artifacts_staged` checkpoint、exact resume与受控停止仍分批授权。旧v3、其余crash、连续L6、P05C、发布、推送和其他平台不推进。

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

上述入口以合成执行器覆盖单次start/clone、v7诊断、target绑定、foreground transport、延迟backend、guest断网、canonical transfer、消歧与negative preflight。真实resolution已证明当前boot断网且input ready；真实negative preflight、operation、八case、连续L6与发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W34.md)
