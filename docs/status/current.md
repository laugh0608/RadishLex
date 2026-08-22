# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-22（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 format/controller、release-pair与maintenance refresh已完成，六类operation均有独立证据。首个crash已闭合；`install_artifacts_staged`旧clone两次start、v2 clone及新v3 clean clone的首次start均在host失败关闭，未进入guest。v3首次真实只读host诊断又因进程清单超过捕获上限而失败关闭、未读取system log；v2仓库控制已收敛并等待新的单步授权。二十台VM全部stopped，其余七个crash实机case和连续完整L6未闭合。

## 冻结基线与固定边界

- P04 已完成 Debian 13 ARM64 Wayland/X11、多应用输入、隐私、同库学习、Manager、导入导出与重启验收；真实 staging、backup、userdb 和证据不复跑、不清理，也不是 P05 mutation 目标。
- P05 首个载体固定为 Debian 13 ARM64 系统级本地单 package `.deb`，identity `debian-local-deb-v1`；不是公开 repository 或通用 Linux 包。layout 绑定 Manager、双 FFI、addon、RimeData/source/license、desktop/icon 与 product manifest，字体依赖发行版 `fonts-noto-cjk`/`fonts-dejavu-core`。
- 五类 operation 默认对用户 XDG 零写入并保留数据；首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 相同的 artifact。v1 package 不含 RadishLex maintainer scripts，外部 scripts/triggers 不能代表产品 transaction completed。
- actual `.deb`、依赖/版本/dpkg status、receipt/staging/guard、固定 `/usr/bin/dpkg` executor、`/proc/*/maps` 静止与只读 startup gate 的完整合同见 Linux 安装维护边界；current 不重复设计细节。

## 当前证据

- P05A carrier、production relationship、恢复事务、system port/CLI/startup gate、L6 format/controller/pair/refresh均已闭合；合成矩阵不替代真实现场。
- 六类真实operation均有分散证据：第六套形成target completed/S3，独立clone闭合repair、rollback、remove与reinstall；这些不能冒充同一连续session。精确receipt、package、guest/host manifest和磁盘身份进入L6 runbook/周志。
- 旧pair唯一`install_prepared`调用形成prepared checkpoint且未触发dpkg；后续startup因遗漏合法`01777 /run/lock`失败关闭，manifest `5d8c914a…3e42a`与clone `FD24ADFF…17C056`冻结。store/startup现共用权限策略且119项默认/124项L6-feature测试通过，旧case仍不计通过。
- 修复target `d75818f`的handoff record `c74fac12…9849`已冻结。第三台clone `3EC83EB9…593B9`只生成一次operation ID、只调用一次controller与一次production exact resume；checkpoint/crash-state `3fd5df67…4a70`/`e18bde6c…8719`先证明prepared与无dpkg child。terminal postflight/postverify `5319db07…52a0`/`001b5be1…e2cc`再证明source `38-1` installed、receipt `completed`、guard absent、startup `AllowedProduct`、XDG/进程/断网稳定；terminal/关机manifest为`679b7e04…c544`/`036bace8…f3b8`，后者固定config/EFI/qcow2 `62040cc9…eb9`/`f762ee52…e76`/`95e89df3…cb69`、双重qcow2与零句柄。
- repository-only guest-case合同除canonical input/readback/startup预期外，现固定`install_artifacts_staged`的target-only staging、package/dpkg未变、合法guard、XDG/process/network零漂移与resume重验/单次apply，并与matrix交叉校验。首次安装精确Rust回归通过，production语义无需修改。
- 第二个case clone-only/首次start/单次控制retry manifest为`ce430efb…aeb8`/`34c0918d…c8d8`/`337007ff…7ebbb`。retry唯一start在90秒无stdout/stderr后timeout，60次status与terminal list均为stopped，返回10；失败后磁盘未变、十九台全停且无guest/input/operation/transaction。离线差分确认其config除Name/UUID外与已成功启动的第三台及reinstall壳相同，Registry可见结构也一致，未获得可归因的UTM/QEMU根因。
- 后续v2 clone-only从clean `992a307`、`337007ff…7ebbb`、十九台全停及冻结DependencyFrozen开始；唯一`utmctl clone`进程exit 0却在stderr报告OSStatus `-1712`，后置清单仍为原十九台且目标注册/package均absent，因此没有替换磁盘、启动或进入guest。失败目录不可覆盖发布，manifest `65160b12…c1859`逐项通过；这证明返回码不能替代注册与package后置条件，不证明具体UTM根因。
- repository-only `l6_utm_clone_once.py`要求双显式授权并绑定clean head、前序manifest、canonical全停清单、registered source、目标name/package absent与executed-control identity；唯一clone的exit/timeout、64KiB stdout/stderr前缀、完整size/hash、pre/terminal list和package后置状态均持久化。只有命令确定成功、stderr空、注册精确增加一个唯一stopped目标且精确`.utm`目录存在才返回created；零落地失败、前置拒绝和部分/不确定落地分别返回10/11/12，任何终态都不自动retry/delete/start。十一项合成回归已接入默认L6门禁，不调用真实UTM。
- 实机前核对UTM官方CLI后以`b86d72f`修正clone名称为精确`--name`参数；修正前没有消耗真实调用。随后从clean `b86d72f`、`65160b12…c1859`、十九台canonical全停清单与冻结DependencyFrozen开始，唯一clone以exit 0、空stderr、唯一新UUID `5B19AEF1…7DAB` stopped和精确package返回created，manifest `7ce53048…e5b7`逐项通过。只对该新clone原子换入DependencyFrozen EFI/qcow2后，prepared manifest `b065c7ac…32db`固定config/EFI/qcow2 `d04b00e1…1ff4`/`0b797641…1418`/`4967234b…4b18`、`Network=[]`、双重qcow2、source/registration/target零句柄及二十台全停；独立复核再次通过。该批没有start、guest、input、operation ID、transaction、retry或delete。
- v3首次start从clean `27087c3`、clone/prepared manifest与二十台canonical全停清单开始。唯一start在90秒内无stdout/stderr并timeout，60次status及terminal list始终为二十台全stopped，控制返回10与`failed-closed-stopped`；start/failure manifest为`870f56dd…f3a5`/`59c62c14…bba05`。成功门未成立，因此guest断网exec和file pull均为0次，也没有input、operation ID或transaction。独立postverify manifest `8e3d5ced…8386`再次确认config/EFI/qcow2未变、qcow2双重复算、source对照、`Network=[]`、零句柄和二十台全停；该结果仍不能归因具体UTM/QEMU根因。
- 首次真实只读host诊断从clean `a828be2`与start/failure/postverify三份manifest开始，先重验二十台及目标均stopped；随后`/bin/ps -axo pid=,ppid=,uid=,comm=`成功返回95,549 bytes，超过64KiB捕获上限并以`host-process-observation-output-truncated`失败关闭，因此没有执行`/usr/bin/log show`。create-new证据manifest `158fe177…77c`逐项覆盖6个payload，terminal保持`root_cause=unattributed`并明确clone/start/stop/guest/input/operation/transaction/retry/delete均未执行。
- repository-only `l6_utm_launch_diagnostics.py` v2现额外绑定首次不完整诊断manifest及其授权、clean HEAD、固定历史时间窗、二十台数量、旧进程命令、95,549-byte截断和日志未执行终态；主控制与独立binding模块都校验committed identity。后续进程观察改用紧凑、无路径的`/bin/ps -axo pid=,ppid=,uid=,ucomm=`，只保存accounting name、role与PID/PPID/UID，再读取同一有界fixed-predicate unified log；任何漂移、stderr、截断或格式错误继续失败关闭。九项fake-runner/临时目录回归与真实四份冻结manifest的67/14/11/6项只读语义校验通过。

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
- 首次host launch诊断证据`158fe177…77c`须原样冻结，不覆盖、补写或复用输出根；其失败不授权自动retry。v2真实采集仍须限定同一历史时间窗、目标、四份manifest与新的create-new输出根并单独授权；日志事件只能作为观察量，不能凭单条文本归因或放宽v3冻结线。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 下一步（2026-08-22）

1. 下一项系统动作须重新单独授权一次v2真实只读host诊断：只针对冻结v3 `5B19AEF1…7DAB`，绑定clean committed head、start/failure/postverify/首次诊断manifest `870f56dd…f3a5`/`59c62c14…bba05`/`8e3d5ced…8386`/`158fe177…77c`，固定UTC `2026-08-22 08:12:00+0000`至`08:18:00+0000`，写入当前absent私有根`RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v3-Host-Launch-Diagnostics-v2`。依次只执行`utmctl list`、目标`utmctl status`、紧凑`ucomm`进程观察和固定predicate `/usr/bin/log show`；不含start、stop、clone、guest exec/file、input、operation ID、transaction、retry或delete。
2. v2结果只记录可验证的UTM/utmctl/QEMU进程、事件和全停状态；即使存在错误文本，缺少独立事实交叉证明时仍保持`root_cause=unattributed`，不预设bookmark、config、磁盘或权限根因，也不得复用或第二次启动v3。首次不完整输出继续独立冻结。
3. 只读诊断闭合后再依据结果决定是否设计并单独授权新的独立clean target。只有未来独立target唯一started、其余VM全stopped且双重文件回读证明仅`lo`/IPv4与IPv6 main route为空，才可再分批进入canonical input与只读/mutation preflight；其余六个crash case、连续完整L6、P05C、发布、推送和其他平台继续关闭。

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

上述入口以合成执行器证明单次start/clone、v2只读host launch诊断、各自确定与失败关闭terminal，以及零越界自动动作。新v3 clone的真实创建、磁盘冻结、单次start与首次诊断失败关闭已有独立host evidence，始终未进入guest；首次诊断在进程清单截断处停止，真实system log尚未读取，八case整体、连续完整L6与发布未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W34.md)
