# Linux P05C 独立实机验收方案

本轮在 Debian 13 ARM64 的独立 guest 验收固定 `40-3` 本地 `.deb` 的输入、隐私、个人化和持久化。连续 L6 和 `upgrade_quiesced` 的维护证据另行留存。测试范围沿用 M5 的 [B5/B6](../remediation/linux-m5-delivery-baseline-audit.md)、[Fcitx5 平台边界](../linux-fcitx5-boundary.md)和 [Manager 同库验收](../linux-manager-local-acceptance.md)。

## 载体与固定输入

- 首选通用 `Debian13-ARM64`，UUID `755199B1-1C18-4441-8A1E-D423C8DE0022`，实际载体 `/Users/luobo/VirtualMachines/utms/Debian13-ARM64.utm`。原 A 批前检确认当时五台 VM 全停，候选 `Network=[]`、无共享目录且共享剪贴板关闭，配置、EFI、qcow2 均为独立文件且零句柄。其 qcow2 无 backing file，与当时恢复专机磁盘的文件身份不同。A 批前的系统包为 `40-2`，B 批升级到 `40-3`。[通用 Debian 原始 seed 停机记录](../devlogs/2026-W46.md#2026-11-14-t40-3-日终回顾与批次结果)
- `upgrade_quiesced` 专机已退役，配置、EFI 和终态磁盘已封存。通用 Debian 保留升级现场。从已验 S0 创建的 `40-3` 首装专机 UUID 为 `435CE261-36B6-4EC2-AB1C-7B5B1F198DFE`，磁盘独立，网卡和共享关闭。首次启动已核实离线、严格未安装及合成账号初态。四件固定材料已在同一 boot 的 `/var/tmp/radishlex-l6-inputs` 逐件回读，首次安装另行确认。[首装前检](../devlogs/2026-W48.md#2026-11-28-首装专机首次启动与只读前检) · [暂存与回读](../devlogs/2026-W48.md#2026-11-28-首装材料离线暂存与回读) 原连续 L6 和旧 P04 专机均已退役，冻结 S2 继续保留。[退役记录](../devlogs/2026-W48.md#2026-11-28-恢复专机退役)
- 使用 S `c038602`、R `65a4147` 和 T `fa3b2c5`。固定 pair SHA-256 `671c9633848a1a27176f978bbf28d8ecccb4ed5b8b5bdd44a85794509482507c`，生产维护执行器 `5d6c220ee7e9e0a174bd5a038c42c5bd39ce6aa0fdba002983646bf519b6ae82`。宿主已复算 pair、执行器、双侧 `.deb` 和 evidence。批次开始时逐项按 pair 再验来源、实际字节与 mode/link，保留 R 封存材料。S2 合成词库和既有账号的 XDG 不作为本轮测试输入。[固定输入归档](/Users/luobo/VirtualMachines/datas/RadishLex/source-first/target-02-d-rebuild-01/readback/pair.evidence.json)

## 分批审批与命令

### A　只启动通用 Debian 并复核资格

预计 8–12 分钟。宿主先复核全部登记 VM 停机、UUID/路径、无共享、磁盘独立和文件零句柄。只有此批获准且前门通过，才执行下列唯一启动。启动会写入该机正常的 OS 日志、EFI 或磁盘状态；其余 VM 保持停机。

```bash
utmctl list
utmctl start 755199B1-1C18-4441-8A1E-D423C8DE0022
utmctl list
```

启动后只读前门用 guest agent 观察并核对以下命令的结果。启动当前 `40-2` 的 Manager 或选中 Fcitx 输入法不属于此批。agent 回读必须有可验证的有效载荷和同一 boot。若使用既有文件回读模式，临时报告仅写入本轮新建、root 私有的 `/run` 路径，双读比对后在宿主保存脱敏结果。旧回读根不复用，空回读或超时立即停下。

```bash
sha256sum /proc/sys/kernel/random/boot_id
dpkg --print-architecture
ip -brief link
ip -4 route show table main
ip -6 route show table main
dpkg-query -W -f='${Package}\t${Version}\t${Status}\n' radishlex
dpkg --audit
dpkg --verify radishlex
getent passwd radishlex-p05c
getent group radishlex-p05c
test ! -e /home/radishlex-p05c && test ! -L /home/radishlex-p05c
```

另核对 Debian 13、固定 source receipt 与 28 项文件、双端只读门禁、包管理静止、20 项依赖和发行版字体。只用已知路径和软件包清单核对显示管理器、Wayland/X11 会话、Fcitx5 前端和本地测试应用。合成账号、同名组及 home 必须均不存在。若已有开机启动的产品进程、运行属于其他用户的会话会影响升级资格、缺测试前端，或 trace 无法在合成账号内隔离，记录缺口并另行确定方案。缺失离线依赖单独报批，当前计划不启用网卡或下载软件。

### B　固定目标包升级

预计 15–25 分钟。先独立确认 A 的 boot、仅 loopback、R 的封存材料、actual pair、source completed receipt、staging 和进程静止。用经逐字节回读的私有输入，为这一批生成一个 guest 内 operation ID，执行一次下列命令。实际 guest 绝对路径、输入文件 mode/owner、执行器和证据摘要在本批审批时锁定。

```bash
"$verified_maintenance" start --authorized-system-mutation --preserve-user-data \
  --operation-id "$operation_id" --kind upgrade \
  --source-deb "$source_deb" --source-evidence "$source_evidence" \
  --target-deb "$target_deb" --target-evidence "$target_evidence"
```

影响为通用 Debian 的系统包与维护 receipt/staging 由已核验的 `40-2` 前态推进至 `40-3`。核对单次 dpkg 升级、`completed / upgrade`、原链加一、28 项安装文件、双端 `AllowedProduct`、断网和合成账号 XDG 仍为空。以已批准产品检查方法核对两份实际安装 FFI 的 SQLite 3.51.3 source ID 与 T 归档，并检查已安装 Rime schema 的 `enable_user_dict: false`。版本检查基于具体载荷，SQLite 系统 CLI 版本另无资格意义。本批出现状态、哈希、进程或回读差异即停，保持证据，不追加修复或再次升级。

### C　真实桌面与合成账号

预计 60–90 分钟，含人工输入。单独审批创建空 home 的 `radishlex-p05c`、该账号内的桌面登录和 Fcitx profile 准备。仅在这个账号打开 product Manager、输入法和离线夹具，保留正常桌面进程启动和实际 FFI 映射的证据。经本批批准才允许该账号的 Fcitx profile、设置和 XDG 发生测试写入，记录预期写入及 owner/mode。用本地空文档、合成词与离线 HTML 表单，由操作者实际输入。应用缺失或身份不符时标记对应测试阻塞。

| 验收项 | 操作与通过条件 |
| --- | --- |
| Wayland 主路径与 X11 对照 | 确认真实会话类型、前端与 Fcitx input panel。覆盖 GTK、Qt、Electron、Firefox ESR 及终端的适用路径。复验 composition、方向键和翻页、数字/空格/鼠标选词、commit、取消、reset、未消费按键透传和输入法切换。至少在两个应用之间带候选往返，验证独立 session 与焦点/生命周期 |
| REV-01 隐私与普通恢复 | 普通字段建立已知学习基线，切入 password、可验证 `Sensitive`、terminal/unknown 和显式隐私模式。password/sensitive 与 unknown 采用各自现行策略，确认受保护路径无个人化读写，隐私模式零学习写入。中途切换时确认未提交选择无泄漏，切回普通字段只对新的显式选择产生一次预期学习。GTK4 `PRIVATE` 仅记录实测 capability 和 unknown 策略，不推定 `Sensitive`。合成 Rime 用户目录无自有学习库 |
| 本地学习与 Manager 同库 | 普通场景选择合成词，核对候选变化、rank explain 与 Manager 刷新后的聚合。用两个实际进程核对同一合成库 identity、WAL/schema 和无异常错误。删除后旧、新输入 session 与普通再导入均不能复活；验证 dry run 零变化、显式恢复、恢复后仅一次新学习。显式导出只含 active 合成 P2 词条 |
| REV-02 持久化 | 在原 schema 上验证两个产品进程交错读取合成词库、Manager/Fcitx 与桌面会话受控重启后仍读取同一库。独立检查聚合、删除和恢复状态及 `integrity_check=ok`。运行态观察以逻辑值为准，维护批次的 XDG 文件指纹单独记录 |

诊断只留身份摘要、聚合、布尔结果、版本、阶段和错误码。合成测试输出与证据各用独立私有位置。同步保持关闭，真实用户账号的库、输入、剪贴板或日志正文不进入验收。

### D　正常关机、冷启动和终态

预计 15–25 分钟。先复核 `40-3`、同一运行 boot、receipt 和合成数据的语义摘要；正常关机与稍后冷启动按获准批次分别各执行一次。关机后复核全部登记 VM 停机及目标文件零句柄。启动前重新复核停机与身份。

```bash
utmctl stop 755199B1-1C18-4441-8A1E-D423C8DE0022 --request
utmctl list
utmctl start 755199B1-1C18-4441-8A1E-D423C8DE0022
utmctl list
```

新离线 boot 复验 `40-3` receipt、28 项文件、双端实际产品启动、合成设置/隐私位、候选排序、tombstone、导入导出聚合与 SQLite 完整性，并记录会话差异。最终停机与证据清理仍各按授权处理。所有批次遇到身份、网络、产品状态或数据差异及回读超时均保留现场、停止推进，VM 和产品动作无自动重试。

## 归档与退出判定

独立记录 guest/boot、固定输入、维护零写入、桌面行为矩阵、持久化和终态。未覆盖的应用、前端或输入路径注明原因和剩余影响。验收结论提交给 M5 退出评审，核对 T 的八个 crash 自动合同、连续 L6 与三个代表恢复边界、适用门禁和仍开放的严重缺陷。需要重跑完整可靠性或全仓门禁时，单独报命令、耗时与资源影响。[当前状态](../status/current.md#下一阶段linux-m5-交付收口)
