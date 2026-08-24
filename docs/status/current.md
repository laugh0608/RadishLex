# RadishLex 当前状态

本文是维护者判断当前里程碑、证据、停止线和下一步的短入口；设计细节进入专题文档，历史流水进入周志。

## 当前判断

- 复核日期：2026-08-24（Asia/Shanghai）；常态分支 `dev`，稳定主线 `master`。
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品；当前主批次 M5-P05B package transaction/startup gate。
- 已退出 M0-M3、M4 macOS build 38 单版本产品验收、M5-P01-P05A。Linux P05B 已有确定性 `.deb`、实际载体流式关系校验、恢复型事务核心、固定系统 observer/executor、concrete mutable port、受控维护 CLI 与 Manager/Fcitx 共用只读 startup gate。
- L6 controller/pair/refresh与六类operation已有证据，首个crash已闭合。`install_artifacts_staged` v4已消耗唯一foreground transport/transfer/resolution/negative preflight/checkpoint；单次真实checkpoint已闭合`artifacts_staged`且未触发dpkg，仍待exact resume后才计完整case。其余七个crash与连续L6未闭合。

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
- clean `e75509a`的network v1在宿主`utmctl exec`参数解析阶段失败关闭，8项manifest `d5d33212…7348`确认network script/push/pull均为0。修正控制在clean `3286268`逐项绑定launch 16项及v1失败7项后只执行一次v2：脚本push后逐字回读SHA-256 `d0bb5c28…1f28a`，network script仅调用1次，两个301-byte回读均为`2f9abfbe…d0e8e`。结构化证据固定boot `1bcbd795…26ae3`、active仅`lo`、nonloop接口/UP均0、IPv4/IPv6 main route均0；15项manifest `40be3f3f…38383`逐项通过，业务input/operation/transaction/stop/retry/quit及plain list/status/start均未执行。
- canonical input transfer v1强绑定v2 manifest `40be3f3f…38383`、live network双回读和授权source绝对路径/size/SHA-256；同一`O_NOFOLLOW` descriptor前后复验12项USTAR。bundle唯一push、双完整回读后，root installer在私有`0700`staging解包并以`RENAME_NOREPLACE`发布固定input root；host/guest证据create-new，任一漂移失败关闭且不生成operation ID、不进入transaction、不自动retry/stop。10项回归已接入默认L6门禁。
- canonical source bundle已由clean `59eaab3`在`…-v4-Canonical-Input-v1`冻结：92,825,600 bytes、SHA-256 `7bbeb291…403c`，七文件manifest `ffc990a3…e0de`；12项USTAR和文件身份通过，且未调用`utmctl`、guest或VM动作。
- clean `6a23c37`的唯一attempt `d75818f-v4-input-20260823-v1`通过network/target/process/句柄、installer readback、bundle唯一push/双回读及source postflight。installer调用exit 0且空输出，但同秒首次pull报告`transfer.evidence.json`不存在，第二次result回读未执行，input root状态未知。create-new根`…-v4-Input-Transfer-v1`的19项及manifest `d1090e8d…dc09`通过，权限`0700`/`0600`、single-link且零xattr；terminal为`state-indeterminate`，无list/status/start、断网复跑、operation/transaction、retry/stop/quit。
- clean `ca57dce`的唯一resolution双回读稳定passed result；一次probe证明installer/可疑进程均0、staging absent、final私有且12项inventory逐文件匹配。create-new host根`…-v4-Input-Resolution-v1`的21项与manifest `7bab8f20…4e4a`通过，权限`0700`/`0600`、single-link、零xattr；terminal为`input-ready`，原installer/bundle、operation/transaction、retry/stop/quit均未执行。
- clean `8bd9e65`的唯一negative preflight绑定resolution `7bab8f20…4e4a`及全部前序身份；one-shot probe只读重验input/pair/package/dependency/font/startup/XDG/process/network。guest双回读为`passed`，host为`preflight-ready`；create-new根25项manifest `aba59811…0d7a`逐项通过。未执行case/maintenance/acceptance/dpkg/installer、operation/checkpoint/transaction或自动补救/VM动作。
- repository-only checkpoint v1绑定25项preflight及前序manifest、source三元组和v4 target；guest driver以exclusive marker运行canonical `preflight`/`crash`/`inspect-crash`各一次，成功只导出operation ID hash并验证target-only staging、`artifacts_staged` receipt、合法guard、进程组SIGKILL、零dpkg mutation及系统静止。14项测试进入L6门禁；该批无真实系统动作。
- clean `7bf6e04`的唯一checkpoint attempt `d75818f-v4-install-artifacts-staged-checkpoint-20260823-v1`通过全部冻结绑定与双回读；guest-local operation ID宿主只存hash `21041a89…111a`。acceptance调用1次并在`artifacts_staged`后SIGKILL完整进程组；checkpoint/receipt为`9cb4acb8…25e0`/`c759b5c6…5d34`，target-only staging、合法未锁guard、package absent、dpkg status/log未变、`ActiveGuard`双startup、XDG/process/network静止。create-new host根49项manifest `3aca0576…0a4f7`逐项通过且raw operation ID扫描为0；未resume、dpkg apply、retry、cleanup、stop、quit或plain list/status/start。
- repository-only exact resume v1已逐项绑定49项checkpoint、`checkpoint-prepared`及operation/checkpoint/crash/receipt/boot/dpkg身份。guest仅在内部重验raw secret、staging与系统静止条件，再至多执行一次canonical `resume`和一次postflight；host只保存ID hash并双回读证据，任何漂移失败关闭。7项host、9项driver及相邻门禁通过；本批未调用真实`utmctl`、进入guest、resume/dpkg或操作VM。
- clean `7309313`的唯一exact resume attempt在host `target-handles-preflight`失败关闭：冻结绑定、source、target与process门通过，但相关进程为0且`lsof` exit 1。create-new根7项manifest `a436677c…adc7`逐项通过；file pull/push、guest exec、maintenance resume与postflight均为0，transaction仍为`artifacts-staged-preserved`。该观察只证明当时backend/句柄不满足资格，不证明VM已停止；未调用plain list/status/start、retry、stop或quit。
- clean `92e82e2`的唯一backend resolution只调用一次目标`status`并得到canonical `stopped`；10轮`ps`/target `lsof`均静止，30项manifest `0193b435…63e9`通过，仍不推断saved。clean `924118c`的repository-only reactivation v1已绑定这30项及全部上游、v7/prepared和原boot hash，固定一次潜在副作用`list`、至多一次foreground start及一次不暴露raw boot ID的guest hash分流；10项回归和全仓门禁通过，本批未调用真实`utmctl`、查询/启动VM或进入guest。

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
- backend resolution attempt与根已消费并冻结；重新激活、exact resume与受控停止仍是后续独立系统动作。未经授权不得读取raw ID、进入guest、运行maintenance/dpkg、补证或操作VM。
- 不复跑 P04 验收，不清理、reset、覆盖或改写其 guest 资产；不自动清理 operation、receipt、失败材料或 staging。
- 不发布 macOS build 38 或 Linux package，不推送、创建 tag/Release、修改远端设置；不并行推进 Android、Windows 或 iOS。
- 输入热路径保持本地；P0 永不学习/同步，P1 原始事件只本地，P2 只允许端到端加密对象。

## 当前下一步（2026-08-24）

1. 冻结checkpoint、exact resume失败与backend resolution manifest `0193b435…63e9`，不覆盖、复跑、补拉或再查询VM；`registered-stopped`不等于saved状态已知。
2. 下一真实系统批须另行精确授权固定UUID、attempt `d75818f-v4-install-artifacts-staged-reactivation-20260824-v1`与absent `…-v4-Reactivation-v1`根；只允许一次`list`证明其余20台全停，必要时一次foreground start，再以唯一只读guest hash在任何业务guest动作前区分原boot/新boot。任一歧义不启动、不重试。
3. 原boot分支才可另做全新exact resume控制与授权；新boot分支须另行设计恢复决策，不沿用旧boot假设。受控停止继续独立；不得自动resume/retry/cleanup/stop/quit或推进下一checkpoint，其余crash、连续L6、P05C、发布、推送和其他平台不推进。

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

上述入口以合成执行器覆盖单次start/clone、v7诊断、target绑定、foreground transport、延迟backend、guest断网、canonical transfer、input/backend消歧、reactivation boot分流、negative preflight及checkpoint/resume控制。真实`install_artifacts_staged`已闭合checkpoint但尚未resume；该case、其余七个case、连续L6与发布仍未闭合。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Linux L6 Debian package matrix runbook](../runbooks/linux-l6-package-matrix.md)
- [本周周志](../devlogs/2026-W35.md)
