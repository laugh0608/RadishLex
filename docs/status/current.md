# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-26（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 controller/pair/refresh与六类operation已有证据，首个crash已闭合。`install_artifacts_staged` v4真实checkpoint闭合`artifacts_staged`且未触发dpkg；boot transport实机在stopped inventory失败关闭，boot身份、exact resume、其余七个crash与连续L6仍未闭合。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A carrier、production relationship、恢复事务、system port/CLI/startup gate、L6 format/controller/pair/refresh均已闭合；合成矩阵不替代真实现场。
- 六类真实operation均有分散证据：第六套形成target completed/S3，独立clone闭合repair、rollback、remove与reinstall；这些不能冒充同一连续session。精确receipt、package、guest/host manifest和磁盘身份进入L6 runbook/周志。
- 旧pair的`install_prepared`在dpkg前因startup未接受合法`01777 /run/lock`失败关闭；修复target `d75818f`的第三台clone只生成一次operation ID并闭合checkpoint、exact resume与关机冻结，细项见runbook。
- `install_artifacts_staged`的typed guest合同固定target-only staging、package/dpkg未变、合法guard、XDG/process/network零漂移与resume重验/单次apply；首次安装精确Rust回归通过，production语义无需修改。
- 第二个case的旧clone、v2和v3均在host失败关闭且未进入guest。v1-v7诊断最终只定位到UTM接收start后的AppKit主窗口断言，未证实更深根因；全部失败现场与manifest冻结，不再原地start/clone或试跑`--hide`。
- v4 clone/prepared/foreground launch固定UUID `50B75F88…8038`、`Network=[]`与载体身份；launch manifest `6dbbdf40…fc3c`保持`state-indeterminate`，后续QEMU双句柄与network v2独立证明当前boot运行且仅loopback。plain list/status曾使backend再次出现，故不循环查询，也不把活动磁盘hash当terminal。
- network v1在宿主参数解析失败关闭且guest调用为0；修正后的唯一v2完成脚本push/逐字回读、一次network probe与双结果回读。manifest `40be3f3f…38383`固定boot `1bcbd795…26ae3`、active仅`lo`且无nonloop接口或route；未进入业务input/operation/transaction或自动补救。
- canonical input transfer强绑定network、source descriptor与12项USTAR；bundle唯一push/双回读后只允许私有staging和`RENAME_NOREPLACE`发布，host/guest证据均create-new，漂移即失败关闭且不进入operation/transaction。10项回归进入L6门禁。
- canonical source bundle已由clean `59eaab3`在`…-v4-Canonical-Input-v1`冻结：92,825,600 bytes、SHA-256 `7bbeb291…403c`，七文件manifest `ffc990a3…e0de`；12项USTAR和文件身份通过，且未调用`utmctl`、guest或VM动作。
- clean `6a23c37`的唯一attempt `d75818f-v4-input-20260823-v1`通过network/target/process/句柄、installer readback、bundle唯一push/双回读及source postflight。installer调用exit 0且空输出，但同秒首次pull报告`transfer.evidence.json`不存在，第二次result回读未执行，input root状态未知。create-new根`…-v4-Input-Transfer-v1`的19项及manifest `d1090e8d…dc09`通过，权限`0700`/`0600`、single-link且零xattr；terminal为`state-indeterminate`，无list/status/start、断网复跑、operation/transaction、retry/stop/quit。
- clean `ca57dce`的唯一resolution双回读稳定passed result；一次probe证明installer/可疑进程均0、staging absent、final私有且12项inventory逐文件匹配。create-new host根`…-v4-Input-Resolution-v1`的21项与manifest `7bab8f20…4e4a`通过，权限`0700`/`0600`、single-link、零xattr；terminal为`input-ready`，原installer/bundle、operation/transaction、retry/stop/quit均未执行。
- clean `8bd9e65`的唯一negative preflight绑定resolution `7bab8f20…4e4a`及全部前序身份；one-shot probe只读重验input/pair/package/dependency/font/startup/XDG/process/network。guest双回读为`passed`，host为`preflight-ready`；create-new根25项manifest `aba59811…0d7a`逐项通过。未执行case/maintenance/acceptance/dpkg/installer、operation/checkpoint/transaction或自动补救/VM动作。
- repository-only checkpoint v1绑定25项preflight及前序manifest、source三元组和v4 target；guest driver以exclusive marker运行canonical `preflight`/`crash`/`inspect-crash`各一次，成功只导出operation ID hash并验证target-only staging、`artifacts_staged` receipt、合法guard、进程组SIGKILL、零dpkg mutation及系统静止。14项测试进入L6门禁；该批无真实系统动作。
- clean `7bf6e04`的唯一checkpoint attempt `d75818f-v4-install-artifacts-staged-checkpoint-20260823-v1`通过全部冻结绑定与双回读；guest-local operation ID宿主只存hash `21041a89…111a`。acceptance调用1次并在`artifacts_staged`后SIGKILL完整进程组；checkpoint/receipt为`9cb4acb8…25e0`/`c759b5c6…5d34`，target-only staging、合法未锁guard、package absent、dpkg status/log未变、`ActiveGuard`双startup、XDG/process/network静止。create-new host根49项manifest `3aca0576…0a4f7`逐项通过且raw operation ID扫描为0；未resume、dpkg apply、retry、cleanup、stop、quit或plain list/status/start。
- repository-only exact resume v1已逐项绑定49项checkpoint、`checkpoint-prepared`及operation/checkpoint/crash/receipt/boot/dpkg身份。guest仅在内部重验raw secret、staging与系统静止条件，再至多执行一次canonical `resume`和一次postflight；host只保存ID hash并双回读证据，任何漂移失败关闭。7项host、9项driver及相邻门禁通过；本批未调用真实`utmctl`、进入guest、resume/dpkg或操作VM。
- clean `7309313`的唯一exact resume attempt在host `target-handles-preflight`失败关闭：冻结绑定、source、target与process门通过，但相关进程为0且`lsof` exit 1。create-new根7项manifest `a436677c…adc7`逐项通过；file pull/push、guest exec、maintenance resume与postflight均为0，transaction仍为`artifacts-staged-preserved`。该观察只证明当时backend/句柄不满足资格，不证明VM已停止；未调用plain list/status/start、retry、stop或quit。
- backend resolution单次`status`证明registered stopped；reactivation单次`list`与foreground start后出现稳定QEMU句柄，但process合同矛盾，manifest `2a477730…c5c4c`闭合`state-indeterminate`且未进入guest/resume/retry/stop。
- runtime resolution v1/v2曾以唯一`list`观察v4 started、其余20台stopped及PID `61666`三次稳定确认，但两次boot hash均无有效输出；v2的18项manifest `e2e41235…c49e`闭合`state-indeterminate`，不证明boot或后续运行状态。
- clean `dbce3a5`的repository-only boot transport绑定上述18项和全部上游。clean `8f59825`的唯一真实attempt只执行一次`list`，观察21台均registered stopped；target未过inventory gate，PID/probe/guest/file/result均为0，8项manifest `6a1ad09f…5140`闭合`state-indeterminate`。
- clean `38b1bb8`新增boot start控制；clean `f972371`的唯一实机调用在21台全停后start，PID `39591`双句柄三次稳定，但首次guest root遇到`OSStatus -2700`/agent不可用；29项manifest `97959c55…c57d`闭合`state-indeterminate`。
- clean `1d64916`最终离线binding闭合boot start 29项。唯一真实resolution一次readiness即ready，PID `39591`与双句柄稳定；probe传输/回读和调用返回成功，但marker不存在，26项manifest `eb7c42f1…d053`闭合`state-indeterminate`。日终审阅确认共享probe CLI只接受`boot-transport`根，而本控制传入`guest-agent`根，精确合成argv在marker前返回`control-root-invalid`；随后正常request关机，21台全停且零相关进程/target句柄。
- `25e1608`绑定26项及probe三类scope/root。boot classification允许单次start、有限readiness和probe双回读。离线binding为`state-indeterminate`、计划根absent；未操作VM/guest。

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
- v4 launch至checkpoint及exact resume失败证据与UUID `50B75F88…8038`须冻结；不得复跑、覆盖、循环list/status、主动拉起backend或把无句柄归因为stopped。当前唯一有效transaction仍是manifest `3aca0576…0a4f7`证明的`artifacts_staged`；attempt `d75818f-v4-install-artifacts-staged-resume-20260824-v1`及输出根`…-Exact-Resume-v1`已消费，不得重试或复用。
- backend至guest-agent resolution的attempt、host根和guest根均已消费并冻结；当前21台registered VM均stopped。新boot classification只完成repository-only合同与冻结链binding，不构成真实动作授权；不得补拉、重跑旧probe、复用旧根、list/status/start/resume、retry/stop/quit。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步（2026-08-26）

1. 冻结manifest `eb7c42f1…d053`、旧attempt及旧根；不得倒写旧probe已在guest成功执行。
2. 若继续实机，须另行授权新attempt/absent根、单次全停inventory/start、有限readiness及probe双回读。
3. 当前不resume；新attempt稳定分类后再设计后续。`state-indeterminate`立即停止，受控停止另行授权。

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

上述合成入口覆盖start/clone、launch/network/input、backend/runtime/boot分流、stopped-inventory boot classification及checkpoint/resume控制，但不替代实机。真实`install_artifacts_staged`已闭合checkpoint，尚未识别boot或resume；其余case、连续L6与发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W35.md)
