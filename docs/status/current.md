# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核：2026-08-30；常态分支 `dev`，主线 `master`。
- 里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- 八个crash自动合同与六类operation分散证据已闭合；真实阻塞样本固定三项，前两项已闭合，`upgrade_quiesced`与连续L6未闭合；其余五项真实crash转为hardening。

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
- v4断网、canonical bundle/input transfer、延迟resolution与negative preflight已按manifest `40be3f3f…38383`/`ffc990a3…e0de`/`d1090e8d…dc09`/`7bab8f20…4e4a`/`aba59811…0d7a`依次冻结并闭合`input-ready`、`preflight-ready`；过程不进入operation、dpkg或自动补救，精确阶段与权限见runbook。
- repository-only checkpoint v1绑定25项preflight、前序manifest、source/target与exclusive marker，固定canonical三阶段、operation ID hash-only、`artifacts_staged`/零dpkg mutation及系统静止；14项测试进入L6门禁，未执行真实系统动作。
- clean `7bf6e04`的唯一checkpoint attempt `d75818f-v4-install-artifacts-staged-checkpoint-20260823-v1`通过全部冻结绑定与双回读；guest-local operation ID宿主只存hash `21041a89…111a`。acceptance调用1次并在`artifacts_staged`后SIGKILL完整进程组；checkpoint/receipt为`9cb4acb8…25e0`/`c759b5c6…5d34`，target-only staging、合法未锁guard、package absent、dpkg status/log未变、`ActiveGuard`双startup、XDG/process/network静止。create-new host根49项manifest `3aca0576…0a4f7`逐项通过且raw operation ID扫描为0；未resume、dpkg apply、retry、cleanup、stop、quit或plain list/status/start。
- repository-only exact resume v1逐项绑定49项checkpoint及transaction/boot/dpkg身份，只允许guest重验后单次resume/postflight、host ID hash-only和双回读，漂移即失败关闭；7项host、9项driver及相邻门禁通过，未执行真实系统动作。
- clean `7309313`的唯一exact resume attempt在host `target-handles-preflight`失败关闭：冻结绑定、source、target与process门通过，但相关进程为0且`lsof` exit 1。create-new根7项manifest `a436677c…adc7`逐项通过；file pull/push、guest exec、maintenance resume与postflight均为0，transaction仍为`artifacts-staged-preserved`。该观察只证明当时backend/句柄不满足资格，不证明VM已停止；未调用plain list/status/start、retry、stop或quit。
- backend resolution单次`status`证明registered stopped；reactivation单次`list`与foreground start后出现稳定QEMU句柄，但process合同矛盾，manifest `2a477730…c5c4c`闭合`state-indeterminate`且未进入guest/resume/retry/stop。
- runtime resolution v1/v2曾以唯一`list`观察v4 started、其余20台stopped及PID `61666`三次稳定确认，但两次boot hash均无有效输出；v2的18项manifest `e2e41235…c49e`闭合`state-indeterminate`，不证明boot或后续运行状态。
- clean `dbce3a5`的repository-only boot transport绑定上述18项和全部上游。clean `8f59825`的唯一真实attempt只执行一次`list`，观察21台均registered stopped；target未过inventory gate，PID/probe/guest/file/result均为0，8项manifest `6a1ad09f…5140`闭合`state-indeterminate`。
- clean `38b1bb8`新增boot start控制；clean `f972371`的唯一实机调用在21台全停后start，PID `39591`双句柄三次稳定，但首次guest root遇到`OSStatus -2700`/agent不可用；29项manifest `97959c55…c57d`闭合`state-indeterminate`。
- guest-agent attempt一次readiness即ready，但probe后marker不存在；26项manifest `eb7c42f1…d053`闭合`state-indeterminate`，根因为共享probe scope/root错配，现已修复且旧现场冻结。
- v4 fresh-boot分类、只读恢复资格和resume控制均已冻结；原resume仍为`state-indeterminate`。`202b64d`分开验证prior/current boot并解耦历史/successor driver，没有倒写旧attempt。
- `f05ebd7`排除非UTM QEMU误判；v2首次result缺失后，独立deferred resolution以双份1259-byte结果与completed phase闭合transaction `completed`，21项manifest `7fcef38e…0e3d`已由`7acbbde`递归绑定。
- clean `9485778`只发送一次正常stop，第2轮闭合21台all-stopped及目标句柄/进程absent；28项manifest `096fe01f…e133`已由`ad77c22`递归绑定。
- `9ec3218`/`8e55189`/`2f91d2a`已固定第三个`upgrade_quiesced`的不可变S2、checkpoint/resume语义、专用registration shell创建/冻结控制、六段授权及独立clone前门；v4只形成位置不合规的默认配置部分壳，没有有效壳体证据或真实target。
- registration shell v1-v3三种configuration record均以`-1700`失败关闭且package/evidence absent；三个14项manifest `3e21aa02…e3c34`/`3c9c7ee8…3f604`/`ae3fb81b…4a511`冻结。`17d10a6`改为最小create后typed update并通过13项回归。
- clean `7ab855b`的唯一v4最小create返回UUID `0BAA7355…52A7`及22台全停inventory；授权路径absent使update为0，14项manifest `70251467…fb04`闭合`state-indeterminate`。默认`Documents`中的同名部分壳无句柄但位置不合规，shell evidence与真实target仍不存在。
- `9300ed7`闭合原生Move恢复；prepare v1首门拒绝，v2以一次list闭合22台全停与15项`move-ready` manifest `dc6b0d01…738e`；唯一UI Move将部分壳移至授权路径且三文件身份不变。
- complete v1因跨阶段HEAD误绑零调用失败；`9da8d0f`修复后，clean `e6968bf`的v2以两次全停list和一次update闭合`frozen`。20项manifest `91778f48…c438`、单成员壳体manifest `c37b552e…f715`通过，最终`Network=[]`且EFI/qcow2不变。
## 停止线

- 资产账本只固定建议处置，不授权清理；历史停止线在逐项prepare并取得删除授权前继续有效。候选不得直接delete、注销、搬移或purge。
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
- v4 launch至exact resume失败证据与UUID `50B75F88…8038`须冻结，不得复跑、覆盖、循环query或主动拉起backend。`3aca0576…0a4f7`只保留为resume前检查点；transaction权威是21项`7fcef38e…0e3d`闭合的`completed`，VM终态权威是28项`096fe01f…e133`闭合的`stopped-verified`。旧attempt/root已消费，不得重试或复用。
- 旧boot、恢复、fresh两阶段及结果消歧根全部冻结，不复跑、覆盖、补拉或清理。`recovery-qualified`只证明只读恢复前门，不授权resume；无新授权不得query/start、resume、retry/stop/quit。
- fresh-boot resume attempt `d75818f-v4-install-artifacts-staged-fresh-boot-resume-20260827-v1`与host/guest根已消费并冻结；不得补拉、复跑、重建secret、retry/cleanup、再次resume/postflight或据`state-indeterminate`修补现场。调用后不得无授权追加query/start/stop/quit。
- transaction-state v1/v2、deferred-result及terminal-stop根冻结。21项结果只证明该boot的transaction为`completed`，28项stop结果只证明同一授权调用内目标已正常停止并完成host交叉检查；不得补拉、复跑probe、query/start/stop、resume/dpkg/retry/repair/cleanup或改写现场。
- registration shell v1-v3 control根与manifest `3e21aa02…e3c34`/`3c9c7ee8…3f604`/`ae3fb81b…4a511`冻结；不得覆盖、复用或解释为已创建。
- v4、prepare v1/v2、complete v1/v2与`Evidence-v5`均冻结；专用壳位于既定外部package且默认路径absent。注册项收敛到预算前不得进入clone前门；不得改写壳体、start/delete/retry或进入guest。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步事项（2026-08-30）

1. registration shell已以`91778f48…c438`/`c37b552e…f715`闭合`frozen`；22台全停、`Network=[]`、磁盘身份和零句柄通过。
2. 已冻结23个VM/7个snapshot账本；17个旧VM与5个旧snapshot进入候选。首批4台的repository-only allowlist、只读prepare与fake-runner已闭合，仍未授权删除。
3. 下一步只对`first-four-v1`执行一次真实只读prepare；它至多调用一次plain list和两轮`lsof`，不启动、进入、移动、克隆、注销或删除VM。只有`prepared`及manifest复核通过后才另行列出删除授权请求。
4. 收敛后只新增一个`upgrade_quiesced` target；退休后再以一台新guest完成连续L6。P05C仍使用独立guest。

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

上述入口覆盖八个crash合同和现有L6控制，但不替代实机。`upgrade_quiesced`、连续L6与发布未闭合；其余五个真实crash转为hardening。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [Linux L6 收敛与本地资产生命周期](../runbooks/linux-l6-asset-lifecycle.md)
- [本周周志](../devlogs/2026-W35.md)
