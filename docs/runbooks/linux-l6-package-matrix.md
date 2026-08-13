# Linux L6 Debian Package Matrix Runbook

本文定义 M5-P05B 的隔离 Debian 13 ARM64 L6 执行顺序、输入冻结、崩溃恢复、只读探针、证据格式和逐步授权边界。读者是准备或执行 Linux package transaction 验证的维护者。本文不授权创建或修改虚拟机、不提供公开 installer、不允许复用 P04 guest，也不授权运行 `apt`、`dpkg`、维护 CLI、启停 Fcitx/Manager、修改用户 XDG 或清理失败现场。

机器真相源是 [`packaging/linux/l6-matrix.json`](../../packaging/linux/l6-matrix.json)，仓库门禁是：

```bash
./scripts/check-linux-l6-contract.sh
./scripts/check-linux-l6-controller.sh
./scripts/check-linux-l6-release-pair.sh
```

三个入口分别验证 matrix format、compile-isolated controller 与 release-pair 构建/证据合同；入口自身都不连接 guest、不读取真实 `/proc`、不执行 package mutation，也不能单独证明下述真实 pair 或 L6 已通过。

## 当前执行状态

截至 2026-08-13：

- production transaction/startup 代码、actual `.deb` verifier 和 fake command/crash matrix 已完成；
- L6 guest、release pair、六步事务顺序、八个 crash checkpoint、字体/startup/XDG probe 和证据保留规则已由 format v1 固定；
- compile-identity 隔离的 acceptance checkpoint/evidence controller 已完成，八点合成中断/恢复与 canonical 脱敏 envelope 已通过专项门禁；
- 旧 pair 已从 source commit `55351f2` revision 1 与 target commit `e5b6da1` revision 2 的独立 clean root 断网构建、原子发布并独立复验；其 record、两份 ARM64 package 与 production/acceptance executable identity 现作为失败输入保留；
- 独立 L6 guest、专用用户、root-owned handoff 与 `S0-clean-e5b6da1-deff08b1` 已准备；在网络关闭、guest/S0/pair/dpkg/XDG/20 项依赖 preflight 通过后，production maintenance CLI 获授权执行首次 source install；
- 该次执行在写 `prepared` receipt 前因旧 validator 拒绝 Debian 13 默认 `no-debsig` 而失败关闭，operation ID 仅登记 SHA-256 `0a75defd7596a08892a6a526dad1cc59d355d84c02c6a609320e8aa55d14a383`。未调用 `/usr/bin/dpkg`，package、receipt、guard/tmp、operation 与产品/XDG 路径均未改变，仅新增空 state/operations root；
- `bb84d4a` 已规范化发行版默认 `no-debsig` 与固定 `/var/log/dpkg.log` 的空白/等号写法。修复后的 source `55351f2`/target `2fa1b8c` pair 已在独立 builder 断网构建，并由新的 disposable L6 完成断网 preflight；第二次 source install 的 operation ID 只登记 SHA-256 `f11888b054683e5df7224e113fee628e3dab03645241abf11d0e8944a33ecc7a`；
- 第二次执行因 store 拒绝 Debian 标准 `root:root 01777` `/run/lock` 而在 receipt/guard/package mutation 前失败关闭。package 仍 absent、dpkg status hash 未变，receipt、operation、guard/tmp、evidence 与五个 XDG 路径均 absent，仅创建空 state/operations root；`f0415ad` 只额外接受 root-owned 精确 `01777` sticky 共享锁父目录；
- 包含该修复的 source `55351f2`/target `512e8ab` 第三套 pair 已从双 clean root 断网构建并独立复验，随后冻结独立 handoff、guest 与 `S0-clean-512e8ab-84d59494`。第三个 guest 启动后 DHCP 曾重新拉起 `enp0s1`；在 operation ID 生成和任何 mutation 前已将接口关闭并复验路由为空，随后八项 input、dpkg status/config/audit、20 项依赖、字体、state/XDG absent preflight 再次通过。
- production maintenance CLI 已获单步授权执行 source revision 1 install。operation ID 只登记 SHA-256 `79efc4ab6d0f0f452f4f63e7e764282cca3c246a7ed80f0579d75738edea0057`；CLI stdout 为空，未据此推断结果，而是以进程退出、canonical receipt、dpkg 与 inventory postflight 判定。receipt 为 `completed`、SHA-256 `e58b144e2d148a7bc37790045c27ef6746308cfdea2e4e0924aa9ed9afa63ab8`，target proof 精确绑定 source package `09ed1228…bec`/evidence `fe3d6297…cf94`，post-install dpkg status SHA-256 为 `33c4973d4bcccc1932de35b2b326c61140037ee46613c7a925f2cbdbd3d5cff1`。
- `radishlex 26.7.1+38-1` installed，`dpkg -V` 返回 0、audit 为空；manifest、双 FFI、20 项依赖与字体均通过。Manager/Fcitx startup gate 返回 `AllowedProduct + InstalledReceiptVerified + completed`；未启动产品。第三个 guest 已冻结 `S1-source-installed-512e8ab-28328a58` 与公开合成 XDG 的 `S2-source-data-512e8ab-0c2cefd6`。
- 首轮 S2 只读 preflight 误以 Python SQLite `mode=ro`/`query_only` 打开 userdb，SQLite 仍创建 `-wal`/`-shm`。现场立即停止，未运行 upgrade；漂移盘另存为 `S2-preflight-wal-drift-512e8ab-8fe4d9a1`，随后从冻结 S2 原子恢复第三个 guest。第二轮不再打开数据库，只核对固定节点、元数据和内容哈希，package/receipt/dpkg、XDG、依赖、字体、startup 与进程静止全部通过。
- 获得单步 mutation 授权后只执行一次 source→target upgrade。operation ID 仅登记 SHA-256 `f3306a5a0d4a78459fce4277e89bbc7d548b2dc17b1e63b57b13074edc8a9fd1`；production CLI 返回 1，stderr 34 bytes。没有新 operation、receipt、staging、guard 或 dpkg 进程，package 保持 `26.7.1+38-1`，receipt/dpkg status 与三份 XDG 内容哈希精确不变。
- 离线用同一 production Rust parser 复现为 target `.deb` 的 `ProductManifestInvalid`：manifest profile 将合法 package version 错误固定为 `product+build-1`，因而拒绝 target revision 2；旧 release-pair builder 只调用 Python artifact verifier，未让 target production parser 逐侧读取 actual pair。代码现改为接受 canonical positive revision 并精确绑定 product/build，builder 新增 production Rust 双侧验证。旧 target ELF/pair/guest 不热替换或重试，七台注册 VM 全部停止。
- 修复后的第四套 pair 使用 source `55351f2`/target `56dd4de`。builder 从 SHA-256 `3d1bd6db9f42c17219f2922509fda9fbb36aff8f8f838cd9ec891ef51370f475` 的完整 Git bundle，在两套 `umask 0022` clean root 和仅 loopback、无路由的 user/network namespace 中构建；crate 实体、pub cache 与 Flutter bin cache 在构建前后均匹配冻结清单。
- 第四个 record SHA-256 为 `70a394eae293cf95924fa250bca8daf0e11fe45c46342bc8e6f94a03db38139a`。source/target package 分别为 `55fba51b05970e6726e24b7485b65bf608317fd9b086dec4b4a9fe20572c280f`、`58ba35891492a864f36d44df37ab30a9bac5cdee18dd4105ba3a2a56a6452814`，artifact evidence 分别为 `c5a805d24fc0b1e9eaf9f2046221373987f0972fed5988a981e7481f5eb40ad6`、`838afa00554d0e5e9d4c4eb094921a6dc4134ecf7194de49903043fed70fe64e`，production/acceptance ELF 分别为 `3bb2925fca8a88cc7a1c2f107aab7a072faf1f90c68dca1c506040185bcf401c`、`29cb1b1a4c4cab73c80a7dd25803053d9065c07017c897622d8a4d848f69ae76`。target production Rust verifier 对 source/target actual package 均通过，builder 与宿主 verifier、8 项逐哈希及 mode/link/ELF 复验一致；独立 handoff 已原子发布。
- 从冻结 S2 创建的第四套 clone `A3F757B1-CE75-4F23-9509-CAD033260AA1` 已逐哈希写入该 record，并完成断网只读 preflight。source package `26.7.1+38-1`、receipt/dpkg、20 项依赖、字体、双 startup gate、XDG fingerprint `f3df287f…b86b`、WAL/SHM/profile absence 与产品映射均通过；没有新 operation ID、maintenance/acceptance CLI 或 dpkg mutation。clone 关机后八台注册 VM 全部停止。
- 获得单步 upgrade 授权后，只启动第四套 clone 并立即关闭网络。operation ID/CLI 前对照 production `LinuxInstallReceipt::can_replace` 发现：新 operation 的 source artifact 必须精确等于 current terminal receipt 的 installed artifact，但第四套重建 source `55fba51b…280f` 不等于 S2 已安装 source `09ed1228…bec`。因此没有生成 operation ID、运行 maintenance/acceptance ELF、创建 guard/receipt/staging 或调用 dpkg；正常关机后 config/EFI/qcow2 SHA-256 为 `61daca92…9239`/`d32181b0…1960`/`5afb3356…b6d0`，qcow2 零打开句柄，八台 VM 全停。
- repository release-pair contract 已改为 prior-terminal source anchor；第五套只从 clean target `1ebbdab` 构建，精确复用 S2 source package/evidence `09ed1228…bec`/`fe3d6297…cf94`。record `d2661cc0…ed15`、target package/evidence `cdac2f32…7c26`/`7786847c…2d5f` 已通过 target production Rust 双侧解析、builder/宿主 verifier、8 文件 mode/link/hash 与 absent-output 原子 handoff 冻结；未启动 L6 guest或执行 package transaction。
- 从原始 S2 新建第五套 clone `9C5638D7-0F97-4BD8-8A83-ABCDFCADAC6C`，在未启动时精确替换冻结 EFI/qcow2并从磁盘配置移除 Network。首次启动仍观察到 UTM 注册态缓存的源网卡、DHCP 与默认路由；在任何 handoff 写入前立即将 `enp0s1` down并确认 IPv4/IPv6 路由为空，之后才原子保留第三套输入、切入第五套 8 文件并执行正式只读 preflight。
- 第五套 terminal source/target chain、receipt/dpkg、20 项依赖、字体 owner/glyph、manifest/双 FFI、Manager/Fcitx startup `0:1:2:2:6`、XDG `f3df287f…b86b`、WAL/SHM/profile absence 与产品映射均通过。没有生成新 operation ID、运行 maintenance/acceptance CLI 或调用 dpkg mutation。host local evidence `74932b8c…4f51` 已原子冻结；clone 关机后 config/EFI/qcow2 为 `27cbca50…c3c`/`c496eae6…345`/`77c434de…d1f`，qcow2 零打开句柄，九台注册 VM 全停。
- 2026-08-11 mutation 前第五套在 UTM app 冷启动后 unavailable，plain `utmctl` 只枚举其余八台 stopped。Finder/UTM 的“数据丢失”是通用加载错误；空 bookmark 与仅重建 registry 均未恢复。对照 UTM 4.7.5 源码确认 QEMU config 对 `Network` 使用必填 decode，而第五套此前删除该键并只依赖注册态缓存运行。经授权备份 preference/config、移除旧 registry并以唯一差异 `Network=[]` 原子修复后，同一 UUID 由默认 Documents 自动重建注册；config/EFI/qcow2 为 `402a5840…3e9`/`c496eae6…345`/`77c434de…d1f`，九台均 stopped，未启动 VM、生成 operation ID 或执行 guest mutation。
- 修复后只启动第五套一次，启动即仅有 `lo`、IPv4/IPv6 路由为空，未再出现虚拟网卡或 DHCP。record/source/target、receipt/dpkg、20 项依赖、字体、manifest/双 FFI、Manager/Fcitx startup、XDG/WAL/SHM/profile 与产品映射全部通过。guest-agent 对部分命令出现 exit/空输出偏差，因此 package/receipt/status 以 guest file pull 回读，结构检查以可靠 Perl 退出合同，startup 以 guest tmpfs C probe 的正向 `0:1:2:2:6` 和负向 component 9/exit 5 共同证明；probe 已删除并确认 absent。没有新 operation ID、maintenance/acceptance CLI、dpkg mutation、产品进程或 XDG 写入。证据 `9ecd7f54…3089`/`bf2e0591…d09f` 已冻结；关机后 config/EFI/qcow2 为 `402a5840…3e9`/`bcdab060…ce8`/`fc552276…8f10`，qcow2 零打开句柄、九台全停。
- 单步 upgrade 授权后再次通过独立 preflight，并只调用一次 production maintenance。dpkg log 证明 target `38-2` 已 installed，但 production target validation 返回失败；同一 operation 自动恢复 source `38-1`，CLI exit 0/stdout `maintenance_outcome=rolled_back`。receipt `55171bef…6611` 为 `rolled_back`、failure `target_validation_failed` after `package_mutating`、source proof installed、target proof null、manual recovery false；startup `0:1:2:2:11`、dpkg status `33c4973d…ff1` 与 XDG `f3df287f…b86b` 通过。没有重试或其他 operation。证据 `98319405…2378`/`64d5395f…e3d8` 已冻结；关机后 disk 为 `402a5840…3e9`/`cb8a697b…bd65`/`e684f829…8904`，九台全停。
- 离线回归已把 startup manifest 的 revision `1` 重复假设统一至 relationship canonical release 规则。修复 commit `80e49ce` 的第六套 chain-continuous pair 在隔离 builder 中从 prior-terminal source `09ed1228…bec` 与 clean target 构建；record `cda70afa…659b`、target package/evidence `b211d940…d09c`/`2a1132c6…0e1b`，production/acceptance ELF `b060c240…7d81`/`b24d103b…a6c`。target production Rust verifier、builder/宿主 Python verifier、8 文件 mode/link/hash 与 canonical archive 回读一致，host 从 absent `.incoming` 同父目录原子发布；builder package/state 保持 absent。
- 从冻结 S2 建立第六套 clone `193179D5-2595-4628-A063-9EFB73F8EC05`，强制 `Network=[]`，启动后与写入 handoff 前均证明仅 loopback、IPv4/IPv6 路由为空。第六套 8 文件经分块逐哈希、重组和同文件系统原子切换进入 fixed input；原第三套输入独立保留。package/receipt/dpkg、20 项依赖、字体 owner/glyph、manifest/双 FFI、Manager/Fcitx startup 正向 `0:1:2:2:6` 与负向 component 9/exit 5、XDG `f3df287f…b86b`、WAL/SHM/profile absence和产品映射静止全部通过。没有生成 operation ID、运行 maintenance/acceptance CLI、调用 dpkg mutation、启动产品或写用户 XDG。readonly/local evidence `9e84eaf2…e3f`/`dcdd5acb…df36` 已原子冻结；正常关机后 config/EFI/qcow2 为 `00bac87d…456`/`7ba7b6bd…ab17`/`0c0c4aea…8d5f`，十台 VM 全停。
- 另行授权后只启动第六套 clone，重新闭合断网、pair/receipt/dependency/font/startup/XDG/process preflight，再在 guest 内生成随机 operation ID；宿主 evidence 只保存 SHA-256 `d22cad12…1b04`。带不可重复 marker 的 executor 只调用一次 production maintenance，exit 0、stderr 空、stdout `maintenance_outcome=completed`；dpkg log 证明 target `38-2` installed。receipt `ccbc4cd0…1e60` 为 `upgrade/target_newer/completed`、chain 2、target proof `b211d940…d09c`、failure null、manual recovery false。
- target terminal postflight 证明 package `38-2`、dpkg status `c09365b3…cece`、audit/verify clean、20 项依赖、manifest `9f08c7fb…0196`、双 FFI `f51dc0f1…50d3`、startup 正向 `0:1:2:2:6`/负向 component 9、XDG `f3df287f…b86b`、WAL/SHM/profile absence、进程与网络静止全部通过。command/local evidence `044c7a25…64e`/`9cc50665…f695` 已从 absent `.incoming` 原子冻结；正常关机后 config/EFI/qcow2 `00bac87d…456`/`0846b1d3…9741`/`6b22499b…b137`，qcow2 零打开句柄，十台全停。没有 repair 或第二次 maintenance invocation。
- 在后续独立授权中未启动任何 VM，只从第六套 stopped disk 以 APFS clonefile 复制 config/EFI/qcow2 到 absent `S3-target-installed-80e49ce.incoming`，逐项 SHA-256 与 source `00bac87d…456`/`0846b1d3…9741`/`6b22499b…b137` 一致后原子发布。S3 identity 为 `S3-target-installed-80e49ce-6b22499b`；mode `0600` local evidence SHA-256 `79a3a170…494b` 绑定 parent S2、receipt `ccbc4cd0…1e60`、target package/evidence `b211d940…d09c`/`2a1132c6…0e1b`、XDG `f3df287f…b86b` 与磁盘 identity，不含 raw operation ID、raw receipt 或完整 dpkg log。源盘/快照 qcow2 零打开句柄，十台仍全停，repair 未开始。
- 另一次独立授权先确认十台 VM 全停，再由 plain `utmctl clone` 从三项恢复字节与 S3 一致的 registered target-terminal VM 创建 repair clone `A3022255-ED08-4C66-92D3-075A6BB93107`。新 clone 启动前 EFI/qcow 与 S3 逐字节相同，config 除 `Information.Name`/`UUID` 外语义相同且 `Network=[]`；注册数增至十一。第一条 guest 命令即证明只有 `lo`、IPv4/IPv6 路由为空，之后才向 noexec `/run` 写入已哈希的只读脚本与 startup probe。
- 首次直接执行 `/run` stage1 因 noexec 返回 126，target 未运行；改由解释器后 stage1 通过，target 又停在超出正式合同的本地 `4e00-9fff` charset 字符串假设。只读诊断证明 `fonts-noto-cjk 1:20240730+repack1-1` 已安装、owner 与 `Noto Sans CJK SC` family 精确解析，TTC 实际声明 `4e00-9fef`；删除该额外字符串假设后，正式 package/owner/family 合同未放宽。两次诊断均未进入 CLI、dpkg 或产品状态。
- 最终文件回读证明 stage1/target exit 均为 0：pair/input、target package `38-2`、receipt `ccbc4cd0…1e60`、dpkg audit/verify、20 项依赖、字体、manifest/双 FFI、startup 正负向、XDG `f3df287f…b86b`、WAL/SHM/profile absence、进程与断网状态均通过；operation 目录仍为 2、guard absent，未生成 operation ID、运行 maintenance/acceptance CLI、调用 dpkg mutation、启动产品或写用户 XDG。local evidence/accepted bundle SHA-256 为 `aa10e919…69e8`/`180c19ad…0cd`；正常关机后 config/EFI/qcow2 为 `584bf2b5…f59b`/`a73a3266…9155`/`ee6cedf8…5d68`，qcow2 零句柄，十一台全停。
- 后续单步授权重新闭合 mutation preflight 后，在 guest 内生成唯一 operation ID并只调用一次 production maintenance。CLI exit 0、stderr 空、stdout `maintenance_outcome=aborted_preserved`；receipt `ba7a9637…f17c` 为 `repair/same_release/aborted_preserved`、failure `version_relation_invalid` after `artifacts_staged`、chain 3、target-only staging、proof null、manual recovery false。dpkg status/log 与调用前完全相同，startup `0:1:2:2:7`、XDG、网络、进程与映射 postflight 通过，因此没有 package mutation、恢复或第二次 invocation。
- 根因是 production `validate_staged_operation` 对 repair 直接使用物理 staged source；而 repair 按合同只 stage target，host prepare 已把 target 同时作为 effective source。源码现统一该投影并增加 single-target system-port 回归。失败 clone 不 resume、重试或复用；冻结 handoff 的 ARM64 maintenance ELF 仍是旧字节，下一步必须先形成“修复后 ELF + 精确既有 target package/evidence”的新 handoff，再从未改写 S3 创建独立 clone。

### 当前本地资产登记（非发布证据）

冻结资产宿主根为 `/Users/luobo/VirtualMachines`；第四、第五、第六套与 repair clone package 位于 UTM 默认 Documents 目录。第五套为 `rolled_back` failure/recovery 现场；第六套已形成 target `completed` terminal与未改写 S3，repair clone则是 staged-preflight `aborted_preserved` 现场。plain `utmctl` 枚举十一台且全部 stopped。`Debian13-ARM64-DependencyFrozen.utm` 故意未注册并继续作为只读 COW 来源。前五个 failure/mismatch/rolled-back 现场与当前 repair clone只作取证；第六套原现场、S3 和失败clone均不直接继续 mutation。下一次 guest mutation只能在修复后ARM64 handoff冻结后，从S3创建全新clone并逐步授权。全部 handoff、snapshot 与 host evidence 继续保留且不得混用。UTM 只使用 `PATH` 中的 plain `utmctl`，任何时刻最多运行一台 VM。

| 相对路径 | UTM 状态 | 唯一职责与保留线 |
| --- | --- | --- |
| `RadishLex/VMs/RadishLex-Debian13-ARM64.utm` | 已注册；P04 | P04 验收现场；staging、backup、userdb、导入导出与临时服务原样保留，不复跑或清理 |
| `Debian13-ARM64-CleanBase.utm` | 已注册；rescue | 依赖安装前的纯 Debian 13 救援基线；不是 L6 S0，不写入 |
| `Debian13-ARM64-DependencyFrozen.utm` | 未注册 | 工作 VM 的 dependency-frozen APFS COW 恢复源；不是执行 guest，不启动或改写 |
| `Debian13-ARM64.utm` | 已注册；builder | Flutter/cache/source/build 与真实 release pair 的构建 VM；不执行 L6 package transaction |
| `Debian13-ARM64-L6.utm` | 已注册；旧 L6 stopped | 旧 pre-receipt failure disk；运行内存状态不再保留，package/evidence root 与 controller evidence 仍 absent，不重启、清理或复用 |
| `RadishLex-L6-Snapshots/S0-clean-e5b6da1` | 非 VM；旧 S0 | 未注册、不可启动的 APFS COW 恢复点；绑定旧 pair 的 config/EFI/qcow、guest/dpkg/XDG/handoff baseline，只作取证，不用于修复后重试 |
| `RadishLex-L6-Handoff-e5b6da1` | 非 VM；旧 handoff | host 上冻结的旧 canonical pair 副本；作为失败输入保留，不覆盖、安装或执行 |
| `RadishLex-L6-PairBuilder-2fa1b8c-v2.utm` | 已注册；builder stopped | 2223 隔离 builder；旧输出不覆盖，保留第四套与第六套 build root/失败记录/发布 pair；不执行 package transaction |
| `Debian13-ARM64-L6-2fa1b8c.utm` | 已注册；第二个 L6 failure stopped | 从 DependencyFrozen 独立 COW 创建，2224 转发；仅有空 state/operations root，运行内存状态不再保留，不重启、清理或复用 |
| `RadishLex-L6-Snapshots/S0-clean-2fa1b8c` | 非 VM；第二个 S0 | 未注册、不可启动；identity `S0-clean-2fa1b8c-5683d120`，现只作第二次失败的基线与取证，不用于原地重试 |
| `RadishLex-L6-Handoff-2fa1b8c` | 非 VM；第二个 handoff | host 上独立复验的 8 文件 canonical pair；现作为第二次失败输入保留，不覆盖或执行 |
| `Debian13-ARM64-L6-512e8ab.utm` | 已注册；第三个 L6 failure stopped | source revision 1 terminal installed；upgrade 在新 receipt 前因 target manifest profile 失败关闭。config/EFI/qcow2 SHA-256 为 `5111741c…d2b`/`8f36df35…1ee`/`0cb75b38…f387`，不重启、清理或复用 |
| `RadishLex-L6-Snapshots/S0-clean-512e8ab` | 非 VM；第三个 S0 | 未注册、不可启动；identity `S0-clean-512e8ab-84d59494`，继续作为第三套 clean baseline，不覆盖为 installed 终态 |
| `RadishLex-L6-Snapshots/S1-source-installed-512e8ab` | 非 VM；第三个 S1 | 未注册、不可启动；identity `S1-source-installed-512e8ab-28328a58`，绑定 source terminal completed、package/dpkg/startup/XDG 与 guest identity，作为 S2/upgrade 前恢复点 |
| `RadishLex-L6-Snapshots/S2-source-data-512e8ab` | 非 VM；第三个 S2 | 未注册、不可启动；identity `S2-source-data-512e8ab-0c2cefd6`，绑定公开合成 XDG fingerprint、source terminal package 与单 VM/断网停止线，作为 upgrade 数据保留起点 |
| `RadishLex-L6-Snapshots/S3-target-installed-80e49ce` | 非 VM；第六套 S3 | 未注册、不可启动；identity `S3-target-installed-80e49ce-6b22499b`，local evidence `79a3a170…494b` 绑定 target terminal receipt/package、XDG 与 config/EFI/qcow2；只作为独立 repair/rollback/startup-negative clone 的恢复源 |
| `RadishLex-L6-Snapshots/S2-preflight-wal-drift-512e8ab` | 非 VM；误读漂移取证 | identity `S2-preflight-wal-drift-512e8ab-8fe4d9a1`；保留 SQLite WAL/SHM 副作用现场，不作为恢复或继续执行起点 |
| `RadishLex-L6-Handoff-512e8ab` | 非 VM；第三个 handoff | record `2f2deaed…697`；source install 已消费，但 target 被其 production profile 拒绝，整套只作失败输入，不覆盖或执行 |
| `RadishLex-L6-Handoff-56dd4de` | 非 VM；第四个 mismatch handoff | record `70a394ea…139a`；双侧 actual-package parser 与逐哈希通过，但 source bytes 不等于 S2 terminal installed artifact，不能直接用于该 chain 的 upgrade，也不得与旧 source 跨 pair 混搭 |
| `RadishLex-L6-Handoff-1ebbdab` | 非 VM；第五个 rolled-back handoff | record `d2661cc0…ed15`；source 精确等于 S2 terminal installed artifact，target package `cdac2f32…7c26`；对应 clone 已转为 target-validation failure/recovery 取证，不覆盖或复用 |
| `RadishLex-L6-Handoff-80e49ce` | 非 VM；第六个 canonical handoff | record `cda70afa…659b`；修复 commit `80e49ce`、prior-terminal source `09ed1228…bec`、target `b211d940…d09c`，双 verifier 与 8 文件 mode/link/hash 通过；已作为第六套独立 S2 clone 的唯一 pair 输入 |
| `RadishLex-L6-Preflight-80e49ce` | 非 VM；第六套 host local evidence | readonly/local evidence `9e84eaf2…e3f`/`dcdd5acb…df36`；绑定 loopback-only/零路由、input switch、chain/dependency/font/startup/XDG/process 静止、probe 清理与 postflight disk identity；`0700` 目录、`0600` 单 link 文件，不是 transaction evidence |
| `RadishLex-L6-Upgrade-80e49ce-completed` | 非 VM；第六套 upgrade evidence | command/local evidence `044c7a25…64e`/`9cc50665…f695`；只保存 operation ID hash、单次 invocation、receipt/dpkg/startup/XDG/网络 terminal 摘要与 postflight disk identity，不保存 raw operation ID、raw receipt 或完整 dpkg log；`0700` 目录、9 个 `0600` 单 link 文件 |
| `RadishLex-L6-Repair-Preflight-80e49ce` | 非 VM；repair host local evidence | local/accepted bundle `aa10e919…69e8`/`180c19ad…0cd`；绑定 S3、clone 前后磁盘、断网、pair/package/receipt/dependency/font/startup/XDG/process 与 operation/guard absence；另保留 noexec/charset 两次只读诊断 bundle `39733904…4097`，不含 raw operation ID、raw receipt 或完整 dpkg log；目录 `0700`、3 文件 `0600` 单 link |
| `RadishLex-L6-Repair-80e49ce-Aborted-Preserved` | 非 VM；repair staged-preflight failure evidence | terminal JSON/final bundle `87190852…881f`/`2d300ae6…c8da`；绑定唯一 invocation hash、`repair/same_release/aborted_preserved` receipt、`version_relation_invalid` after `artifacts_staged`、未变 dpkg status/log、startup/XDG/网络/进程 postflight 与关机磁盘；不含 raw operation ID、raw receipt或完整dpkg log；目录 `0700`、4文件`0600` single link |
| `RadishLex-L6-Preflight-1ebbdab` | 非 VM；修复前 host local evidence | local evidence `74932b8c…4f51`，input-switch/readonly evidence `70833344…70f8`/`6e44f027…6e48`；记录 UTM runtime adapter 偏差、断网边界、chain/startup/XDG 与旧 config `27cb…c3c`，不是修复后 config 身份或 canonical session evidence |
| `RadishLex-L6-Preflight-1ebbdab-post-repair` | 非 VM；修复后 host local evidence | readonly/JSON evidence `9ecd7f54…3089`/`bf2e0591…d09f`；绑定 `Network=[]` 冷启动、loopback-only/双路由为空、chain/startup/XDG/进程静止与 postflight disk identity；`0700` 目录、`0600` 文件，不是 canonical session evidence |
| `RadishLex-L6-Upgrade-1ebbdab-rolled-back` | 非 VM；upgrade failure/recovery evidence | text/JSON `98319405…2378`/`64d5395f…e3d8`；CLI、dpkg log、receipt 摘要与 rolled-back startup/XDG postflight 均为 `0600`，只含 operation ID SHA-256；startup revision `1` 根因已离线复现并补门禁，现场仍不得复用 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-56dd4de.utm` | 已注册；第四套 mismatch stopped | UUID `A3F757B1-CE75-4F23-9509-CAD033260AA1`；operation ID/CLI 前确认 artifact chain 不连续并停止。config/EFI/qcow2 SHA-256 为 `61daca92…9239`/`d32181b0…1960`/`5afb3356…b6d0`；不重启、覆盖、恢复或复用 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-1ebbdab.utm` | 已注册；第五套 rolled-back stopped | UUID `9C5638D7-0F97-4BD8-8A83-ABCDFCADAC6C`；target validation 失败后自动恢复 source，terminal receipt/source startup/XDG 通过；config/EFI/qcow2 `402a5840…3e9`/`cb8a697b…bd65`/`e684f829…8904`。不启动或重试 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-80e49ce.utm` | 已注册；第六套 target completed stopped | UUID `193179D5-2595-4628-A063-9EFB73F8EC05`；单次 upgrade 后 target `38-2`、receipt/startup/XDG postflight 通过；config/EFI/qcow2 `00bac87d…456`/`0846b1d3…9741`/`6b22499b…b137`。S3 已冻结，原现场继续保留且不直接 repair |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-80e49ce-repair.utm` | 已注册；repair aborted-preserved stopped | UUID `A3022255-ED08-4C66-92D3-075A6BB93107`；唯一 maintenance invocation 在 staged preflight 以 `version_relation_invalid` 终止，未进入 dpkg。terminal package/startup/XDG通过；关机后 config/EFI/qcow2 `584bf2b5…f59b`/`3b117def…8f0`/`adcc92fd…3a4`。只作取证，不resume、重试、恢复或复用 |

两个在 QEMU 引导前因缓存 2222 转发失败、从未运行 guest 的旧 PairBuilder clone 已在单独授权后从 UTM 注册表和磁盘删除；它们不含 package transaction 或 canonical evidence。当前资产仍各有独立职责，不因 UTM 面板是否显示而删除。L6 闭合后可另行授权评估剩余 builder、DependencyFrozen 与 host handoff 的保留期；P04、CleanBase、四个 failure/mismatch L6 和任何 S0/S1/S2/S3 恢复点仍按各自停止线保留。该表只登记本机运维角色，不进入 canonical pair/checkpoint/session evidence。

## 1. 环境身份

L6 只能使用新建 guest、P04 guest 的独立 clone，或同等的可丢弃 guest。环境必须满足：

- Debian 13 `trixie`、ARM64、multiarch `aarch64-linux-gnu`；
- 专用用户固定为 `radishlex-l6`，home 固定为 `/home/radishlex-l6`；
- `radishlex` 初始 dpkg 状态为 `not-installed`，`/var/lib/radishlex/install-v1` 初始不存在；
- P04 staging、backup、userdb、导入导出文件和临时服务均不在该 guest；
- dependency 与取证工具安装完成后冻结 base snapshot，正式 matrix 阶段关闭网络；
- hypervisor、guest image digest、CPU/内存、磁盘、内核、`/etc/os-release`、dpkg status hash 和 snapshot ID 进入会话记录。

不能以容器中的成功结果替代 VM：容器可复验 package lifecycle，但不能覆盖真实 procfs、桌面 session、Fcitx daemon、system font cache、重启与用户域隔离。P05C 仍使用另一个独立 guest，不把 L6 故障现场提升为日常输入验收环境。

## 2. Snapshot 拓扑

至少保留以下不可混用的 snapshot：

| Snapshot | 现场 | 用途 |
| --- | --- | --- |
| `S0-clean` | dependency 已冻结；RadishLex package/state/XDG 均 absent | 首次安装、remove 后对照和 crash case 起点 |
| `S1-source-installed` | source terminal completed；未启动产品 | upgrade/crash 起点 |
| `S2-source-data` | source installed；专用用户已有合成 XDG fixture | 主序列数据保留起点 |
| `S3-target-installed` | target terminal completed；合成 XDG 未变 | repair、rollback 与 startup 负向 |

每个 crash scenario 必须从声明的 snapshot clone 开始，完成取证后恢复或丢弃该 clone。不得让 crash receipt、dpkg half-state、operation staging 或测试注入继续进入主序列。

旧 `S0-clean` identity 为 `S0-clean-e5b6da1-deff08b1`。恢复目录中的 config、EFI、qcow2 SHA-256 分别为 `d3d7fb4946361b0c9a87a2a0611ef9ed085b3635c4331e254f5cbe359ecf88c6`、`35fa4cdbbd72ba81c00da179cd4327cccaf30007eedc871d37b701c02f8dcafb`、`deff08b1da61043a838f71474d350f83998a34d8de59a42091a99005af070e22`；disposable 只读启动冻结 dpkg status SHA-256 `2c31c35c262b2b2761055fa12a55361f3d47ebfa1923dbb3cc3691499d2ce572` 与五个 XDG absent。`local-snapshot.evidence.json` SHA-256 为 `50dcc48bfba7f65fce56184b2a21183f8e263611eb0f86d62cdafc6f306b1f08`，明确属于本机恢复记录而非 canonical L6 session evidence。它继续保留为旧 pair 基线与失败取证，不用于之后的修复重试；每个修复后的 pair 必须另建独立 handoff、guest 与 S0。

第二个 `S0-clean` identity 为 `S0-clean-2fa1b8c-5683d120`，不是从旧 S0 恢复，而是从未注册 DependencyFrozen 另建 COW guest 并写入第二个 handoff 后冻结。config、EFI、qcow2 SHA-256 分别为 `ae4807ca590815329fd251a868bcd9d088f0ce4113bf3d8968bf82c2f0192043`、`35fa4cdbbd72ba81c00da179cd4327cccaf30007eedc871d37b701c02f8dcafb`、`5683d1205871e21eceaa0aae63216544a0056e2cbef72b01bfb10b9f3205bec5`；disposable 复验后 source/S0 三项 hash 仍一致，package/state/evidence 与五个 XDG 路径 absent，dpkg status hash 未变。local evidence SHA-256 为 `d741d07b90b554bd77b4b2a66447fe142c95b635064bbd9aa46b04e333d23f7c`。第二次失败后再次重哈希，source 与 S0 的三项 hash 及 evidence hash 均未漂移；该 S0 现只作取证，不用于原地重试。

第三个 `S0-clean` identity 为 `S0-clean-512e8ab-84d59494`，同样从未注册 DependencyFrozen 独立 COW 创建，不恢复前两个 S0。config、EFI、qcow2 SHA-256 分别为 `5111741c54a49068dbbacfd131891b0990a531001769db21942eb88ac6767d2b`、`35fa4cdbbd72ba81c00da179cd4327cccaf30007eedc871d37b701c02f8dcafb`、`84d59494e13058d2f3a33311d70fd72170f98f211773c5730b55663fc7a5170a`，local evidence SHA-256 为 `5946b1ecb4c50f32ecaee595d747c89d627ea352ad7f65354b006889585e7e8b`。disposable 断网 preflight 后 source/S0 三项 hash 仍一致且 qcow 打开句柄为零；dpkg status/config SHA-256 分别为 `2c31c35c262b2b2761055fa12a55361f3d47ebfa1923dbb3cc3691499d2ce572`、`fead43b89af3ea5691c48f32d7fe1ba0f7ab229fb5d230f612d76fe8e6f5a015`，package/state/evidence/五项 XDG absent。它已作为本次首次 install 的 clean 起点，继续保留为原始基线；installed 终态没有覆盖 S0。

第三个 `S1-source-installed` identity 为 `S1-source-installed-512e8ab-28328a58`。它只从第三个 guest 的关机、qcow 零打开句柄终态创建，未恢复 S0、未启动 guest 或产品；config、EFI、qcow2 SHA-256 分别为 `5111741c54a49068dbbacfd131891b0990a531001769db21942eb88ac6767d2b`、`eb94763ab95bdc17afdca66e812d9fe6bf953ab9c55f9258adc7b03b82d3b3ef`、`28328a58ba12249e378db02b2fe8828c7f676cf9cd3dccb6e52be4a8e1fc331d`。mode `0600` 的 local evidence SHA-256 为 `a759e4135db265398d3beba95ad4635c30dab78463c988f4d63db0062cfa1613`，只保存 operation ID hash，并绑定 receipt `e58b144e…ab8`、post-install dpkg status `33c4973d…ff1`、package/dependency/font/startup/XDG terminal 结果；它仍是本机恢复记录，不是 canonical L6 session evidence。

第三个 `S2-source-data` identity 为 `S2-source-data-512e8ab-0c2cefd6`。S2 mutation 前先停止两个旧 failure VM，确认七台全停，再只以持久模式启动第三个 guest；任何 XDG 写入前网卡 down、路由为空、package/receipt/dpkg 与 S1 一致、固定 XDG absent、产品映射为零。fixture 只含一条公开合成 active term、合法 v1 settings/privacy、四个 `0700` 产品根与三个 `0600` 文件；userdb schema v9、quick_check、1 active/0 deleted、WAL/SHM absent，Fcitx profile 继续 absent。完整 XDG fingerprint SHA-256 为 `f3df287fa0f1da1d5f1fb3169fe607a84ad7fb9b60aab1308ba2eeb410d0b86b`。关机后 config、EFI、qcow2 SHA-256 分别为 `5111741c54a49068dbbacfd131891b0990a531001769db21942eb88ac6767d2b`、`365b5a170dca95bdf07c0e5e940fafd4580a353e43c141abede71c95f91261bc`、`0c2cefd6b63420adf143e1f1d6e4e70f59841bf1ba56cb54e86d2e7a226f3eea`；mode `0600` 的 local evidence SHA-256 为 `d164de0f5e6e88afbfa76ccd1c7d321d7064bbfd3ee1e275d3d4842d0dfa5c74`，仍不是 canonical L6 session evidence。

第六套 `S3-target-installed` identity 为 `S3-target-installed-80e49ce-6b22499b`。它只从第六套 target-completed guest 的 stopped disk 创建，没有启动、挂载或注册 snapshot；创建前后十台 VM 全停，source 与 snapshot qcow2 均为零打开句柄。APFS clonefile 先写 absent `.incoming`，config/EFI/qcow2 的 size/SHA-256 分别为 `2970`/`00bac87d…456`、`655360`/`0846b1d3…9741`、`10139598848`/`6b22499b…b137`，完整复验后同父目录原子发布。local evidence SHA-256 为 `79a3a170fa9e0c60a2de954cf8129e3fed5578a3bffb69d3f77ea42010e7494b`，绑定 completed receipt、target package/evidence、manifest/FFI/dpkg/startup与不变 XDG fingerprint；它仍是本机恢复记录，不是 canonical L6 session evidence。后续 repair、rollback 与 startup 负向必须从它建立各自独立 clone，不能直接改写 S3 或第六套 terminal 原盘。

误用 SQLite 只读 URI 后冻结的 `S2-preflight-wal-drift-512e8ab-8fe4d9a1` 只作副作用取证。其 config、EFI、qcow2 SHA-256 分别为 `5111741c54a49068dbbacfd131891b0990a531001769db21942eb88ac6767d2b`、`c496eae6567e551da959684db8c1533335eeabe5cd1b8cfa35c21433c2494345`、`8fe4d9a14c9c53e0b38a2ca4540d7fedd06a22fcefa0cc8ae01207f580f08d53`，local evidence SHA-256 为 `c16bc62e0fc63472e2e86c5f0cf5f6e95f7b28ec484ec5b306a1cd1d086aa199`。第三个 guest 随后从原始 S2 恢复且宿主三项 hash 精确匹配，后续 preflight 禁止打开 SQLite，只按冻结节点和内容 identity 对照。

## 3. Release pair 冻结

source/target 必须来自两个不同 commit 的真实载体，不允许复制后改名、只手写 evidence 或以相同版本重建物替代 terminal 前态。仓库真相源为 [`packaging/linux/l6-release-pair.json`](../../packaging/linux/l6-release-pair.json)：source 固定为 S2 terminal receipt 已安装的 `55351f2`/`26.7.1+38-1` package/evidence 精确字节，target 是本子批 clean descendant/`26.7.1+38-2`。source package `09ed1228…bec`、evidence `fe3d6297…cf94` 及各自文件名/size 全部进入 committed chain anchor；builder 不再重建 source，只构建 target。

在获准的 Debian 13 ARM64 构建环境中，唯一入口为：

```bash
./scripts/build-linux-l6-release-pair.sh \
  --source-package /absolute/frozen/radishlex_26.7.1+38-1_arm64.deb \
  --source-artifact-evidence /absolute/frozen/radishlex_26.7.1+38-1_arm64.deb.evidence.json \
  --target-root /absolute/clean/target \
  --output /absolute/absent/release-pair
```

该命令只构建、验证并原子发布私有 handoff 目录，不安装 package、不运行 maintenance/acceptance CLI、不创建/打开 VM，也不读写用户 XDG。运行前仍需单独准备构建环境；本 runbook 不授权下载依赖或修改全局工具链。构建规则为：

1. source 输入必须是 committed chain anchor 指定的 package/evidence：absolute canonical single-link `0644` regular file，文件名、size、SHA-256、canonical evidence 与 evidence 内 package identity精确；
2. helper 只向私有 absent staging directory 做 exclusive copy、固定 `0644` 与 file/directory `fsync`；任何同版本重建字节、evidence 漂移或额外文件都在 target 构建前失败；
3. target 使用 source commit 的 clean descendant与相邻 Debian revision `N+1`，并保持相同 product/build、ABI v9、userdb schema v9、XDG/settings/privacy/RimeData contract；
4. 只有 target 从干净源码构建 Manager、system-profile addon、rootfs、`.deb` 和 evidence并通过 L1-L5；source 不运行 Manager/addon/rootfs/package 构建；
5. target commit 同时构建 production maintenance、只读 artifact verifier 与 compile-isolated acceptance ELF；记录 compile identity、commit、size、SHA-256 与 AArch64 loader；
6. target production `radishlex-linux-artifact-verifier` 必须逐侧读取 staged source/target actual `.deb` 与 evidence；两侧都通过后才形成 record并重哈希完整 handoff inventory；
7. artifact pair 后续在新 guest 固定进入 `/var/tmp/radishlex-l6-inputs`，复制后改为 root ownership，再由 maintenance production verifier 重新打开和取证。

builder 先冻结 source anchor，再从 target clean root 依次调用 metadata、Manager、addon、rootfs、layout 与 deterministic `.deb` 门禁；随后以 `--no-default-features` 构建 production maintenance ELF 和只读 actual artifact verifier，并另行构建链接 acceptance feature 的 controller ELF。record 阶段再次验证 target Git identity/谱系/clean 状态、source anchor 与两侧 artifact；Python verifier 解析两个 ELF 的 ELF64/AArch64 与 `/lib/ld-linux-aarch64.so.1`，要求 production 不含 acceptance markers、acceptance 同时含 build identity 与授权 marker，最后重哈希发布目录中的 package、artifact evidence、build-environment 和两个 executable。target root 不干净、commit/anchor/revision/contract 漂移、actual package、ELF/mode/link/marker 或 canonical JSON 不符均失败关闭且不发布输出。

pair envelope format 为 `radishlex-linux-l6-release-pair-evidence-v1` 对应的 format v1/profile v1 组合；只保存 commit、revision/version、package/evidence/manifest/dependency 摘要、无路径 tool version，以及 executable build profile/ELF/size/SHA-256。它不保存源码/构建/staging 绝对路径、operation ID、PID、proc maps、dpkg 原文或用户数据。source/target 的 build number 相同不表示两者是同一 package：Debian revision、manifest、control、package/evidence hash 必须不同。该 pair 只证明首版 Linux package 事务兼容，不宣称跨数据 schema 升级或公开发行兼容。

2026-08-08 的旧 Debian 13 ARM64 record 使用 Rust/Cargo 1.85.0、CMake 3.31.6、Flutter 3.44.0，target 为 `e5b6da1`。canonical record SHA-256 为 `a9bcf35762b460a23ad9bc062611f8d5edb57e7303861bbcb99e1efb40703dfd`；source/target package 分别为 `b41e32db76388ad18cdeb60e4b40fb8e28710556df87d53bfa5b275ff2ce028c`、`8209c0161609fde3b798628e5c3460e6237c8618f2d26f1452063540c7541295`；production/acceptance executable 分别为 `037199abe73559e2cd10013f0930f1f44cf9126ac7987169da11f2933a706fc1`、`c4f6282341c6f68f997b1f5d8d2d1b5dec2d96b60e2f387717594d5a0a9523f4`。它现是失败输入与取证材料，不授权在旧 handoff 中替换 executable 或继续执行。

2026-08-09 的第二个 record 使用相同工具版本，source `55351f2`/target `2fa1b8c`；canonical record SHA-256 为 `a5a0ee0deeb48a1e82e87e0bb9eb848e118d21c83274dc2689fcc136dcb38664`。source/target package 分别为 `08205ad712ea7bde08b19a56e42c42ce0c15440f61fae2efd610d024188c0ad1`、`44e0f3da48502dad7d4ea22eb05e0e87f77dfddf98910c098e8f536128ecca42`，artifact evidence 分别为 `42a4d2135454e0181421fdc42fbcabeccfa53077255a7d516a000596ad3a17f1`、`282e4f150b5c800cb58855f82a8986a8d442bf91490518fd0a8db6987bc796ed`，production/acceptance executable 分别为 `919b55dc46958ca55520e0cb9a0fae5c8080f9b58e1bd9142b662af10b7c48eb`、`29c2007b860e506a4ee1dbfe2608e4e18bd5e37edb5c3bbe8fddd4c3b3e2e867`。builder、host handoff 与 guest input 逐哈希一致；独立 verifier 复验 inventory、mode/link、AArch64 loader 与全部 identity 后通过。它现是第二次 pre-receipt 失败输入，不授权热替换 executable 或继续执行。

2026-08-09 的第三个 record 使用相同冻结工具版本，source `55351f2`/target `512e8ab`；canonical record SHA-256 为 `2f2deaed6c8886cfcc4751ccc56439bda95f311767587dc74103645664258697`。source/target package 分别为 `09ed122804b11767b8ac7cd69c323c1f6eef511fd6ae7284d75756fb60569bec`、`6382003e6932b4be7b171f922179c760947fde58eb01ba7771c9f829f3f62ca5`，artifact evidence 分别为 `fe3d6297c08dccd8cacba13d50aa44dbb1c94b0ca8c2ab4df3a5c605466fcf94`、`f1ca686f96335d2c6b3f85bdd6871cc3904bf746ad86307775c5cebc98cae8f4`，production/acceptance executable 分别为 `f716fad30b6657e108272fd1f7361826773b0ca42e4ac1ce6f78a7ad20aade62`、`c3ac9c1a848a29d1bbe70bcac046164f00060b76a62ff8bd4c0c9a51383f4bc2`。该 record 的 Python verifier、host/guest hash 与 source install 均通过，但 target production parser 在 upgrade 前拒绝 revision 2；因此整个 record/handoff/guest 现只作失败取证。修复后的 Rust parser 已在宿主只读复验这两份冻结 actual package 均通过，但这不能替代新 target ELF、双 clean-root pair 与独立 guest 证据。

2026-08-10 的第四个 record 使用同一冻结工具版本，source `55351f2`/target `56dd4de`；canonical record SHA-256 为 `70a394eae293cf95924fa250bca8daf0e11fe45c46342bc8e6f94a03db38139a`。source/target package 分别为 `55fba51b05970e6726e24b7485b65bf608317fd9b086dec4b4a9fe20572c280f`、`58ba35891492a864f36d44df37ab30a9bac5cdee18dd4105ba3a2a56a6452814`，artifact evidence 分别为 `c5a805d24fc0b1e9eaf9f2046221373987f0972fed5988a981e7481f5eb40ad6`、`838afa00554d0e5e9d4c4eb094921a6dc4134ecf7194de49903043fed70fe64e`，production/acceptance executable 分别为 `3bb2925fca8a88cc7a1c2f107aab7a072faf1f90c68dca1c506040185bcf401c`、`29cb1b1a4c4cab73c80a7dd25803053d9065c07017c897622d8a4d848f69ae76`。两侧 actual package 均由 target production Rust verifier 解析通过，builder/host verifier、guest fixed input 与 8 项逐哈希一致；独立 clone 的 package/XDG/startup preflight 也通过。但该 source 是同版本重建字节，不等于 S2 terminal receipt 的 installed artifact，故 production replacement contract 在 operation ID/CLI 前判定 chain 不连续；record、handoff 与 clone 现只作 mismatch 取证。

2026-08-10 的第五个 record 使用 source `55351f2`/target `1ebbdab` 与相同冻结工具版本；scoped Git bundle SHA-256 为 `f2820ac7e0076e5d78987a52bf5d1e0d50fd04d937d7b15ee61d0b9e589eaed6`，canonical record 为 `d2661cc0b7bc3b2ad85dccd11f589baf704d285d99f5c528bc3fea612ac7ed15`。source/target package 分别为 `09ed122804b11767b8ac7cd69c323c1f6eef511fd6ae7284d75756fb60569bec`、`cdac2f32e7828165b5c3bb3bbb9839df6cd462f965e88b57b4f97a6374567c26`，artifact evidence 分别为 `fe3d6297c08dccd8cacba13d50aa44dbb1c94b0ca8c2ab4df3a5c605466fcf94`、`7786847ce5dea6fe135f8549d78445565605111ae6e7fe07cee9dd48a2cb2d5f`，production/acceptance executable 仍为 `3bb2925fca8a88cc7a1c2f107aab7a072faf1f90c68dca1c506040185bcf401c`、`29cb1b1a4c4cab73c80a7dd25803053d9065c07017c897622d8a4d848f69ae76`。构建在仅 loopback、零路由 namespace 完成，冻结 Cargo/pub/Flutter cache 清单前后通过；target production Rust verifier、builder/宿主 Python verifier 与 8 文件 mode/link/hash 一致，host 从 absent `.incoming` 同父目录原子发布。builder package/state 与 L6 guest 均未触碰。

2026-08-12 的第六个 record 使用 source `55351f2`/target `80e49ce` 与相同冻结工具版本；完整 `dev` Git bundle SHA-256 为 `829c8d252f1e2a78526298661511e0781547b8a7bb2cd6daab5fb88ab9bbadaf`，canonical record 为 `cda70afa89b3f0ee05235b95dcc346eaeea9805c4b87af9d451ce1900f00659b`。source/target package 分别为 `09ed122804b11767b8ac7cd69c323c1f6eef511fd6ae7284d75756fb60569bec`、`b211d9406825515b2ba1c473b5f98069de00fa709505e2ba3ed3cb9b8af7d09c`，artifact evidence 分别为 `fe3d6297c08dccd8cacba13d50aa44dbb1c94b0ca8c2ab4df3a5c605466fcf94`、`2a1132c6fb27d4ca2e5e4bbd76864e3e753749c2bb43cae82287635b1c2d0e1b`，production/acceptance executable 为 `b060c2403424560e9ac0c11f890838894d19d153a89541575ade9afd87fe7d81`、`b24d103b48142df86ef1cf1a582c5d4104bedb035ee93a971a325e22fdc79a6c`。dependency digest 仍为 `2ee2b7e58434b3358e8b1dbc699473f3f2fd9cb341c6bd5afd96050f80ea5743`；target production Rust verifier、builder/host verifier 与 8 项 owner/mode/link/hash 全部通过。canonical USTAR archive SHA-256 为 `f2e975b827d78395541e1be387f8542a9bbf45e19247c3ca03fd70ba65485f59`，经分块回读重组、临时目录复验后从 absent `.incoming` 原子发布到独立 handoff；旧资产未覆盖。2026-08-13 独立 clone 已完成 fixed input、只读 preflight和单次 upgrade，target terminal 为 `completed`。

同一 source 在不同长度 clean-root 下重建时，旧/new canonical md5 inventory 只有 Manager runner 及其 manifest 不同：CMake install RPATH 的动态字符串内容相同，新 ELF 仅多 18 个尾部 NUL 预留，继而改变 PLT relocation 与 Build-ID；只读数据、data、eh_frame、FFI、Rime 与 dependency 摘要一致，且无构建路径字节。该观察不放宽任何验证，也不把跨绝对构建根 payload bit-repeat 写成已有保证；已经形成 terminal receipt 后，后续 pair 必须消费其绑定的精确 source 字节，不能以同版本重建物替代。

## 4. 证据 envelope

每个 crash case 先由 controller 产生一个 `radishlex-linux-l6-checkpoint-evidence-v1` canonical JSON envelope；完整 L6 session 再把八份 checkpoint envelope 与主序列/probe 结果收敛进唯一 session envelope。session 至少绑定：

- matrix format/profile 与仓库 commit；
- guest/snapshot identity；
- source/target/maintenance executable 身份；
- 每步随机生成的小写 32 hex operation ID 的 hash，不记录原值；
- mutation 前后 dpkg package tuple、`dpkg --audit` 分类和 product inventory；
- receipt 的 format、operation kind、state、failure 与 terminal 分类，不复制 receipt 原文和 staging 路径；
- Manager/Fcitx startup decision/error/state；
- dependency/font family/glyph/owner 结果；
- XDG fingerprint、进程静止结果和系统重启边界；
- 执行者授权时间、命令退出分类和停止线事件。

证据不得包含 `.deb` 正文、receipt operation ID、绝对构建路径、PID、`/proc/*/maps` 正文、dpkg stdout/stderr、用户词、选择事件、导入导出内容或真实用户目录内容。

checkpoint envelope 只保存 matrix/build/scenario/checkpoint、repository/guest/snapshot identity、operation ID SHA-256、授权/fault/termination 分类、完整 process group 清空结论和 expected terminal。固定 writer 使用 deny-unknown-fields 的 canonical pretty JSON 与末尾换行；不接受调用方提供输出根。合成门禁验证该格式，但不会写固定 evidence root。

## 5. XDG 对照

package mutation 与用户态产品启动必须分开取证：

1. `S0-clean` 中五个固定 XDG 路径均不存在；首次 install terminal 后、任何产品进程启动前仍必须全部不存在。
2. 首次 install 零写入通过后，另行授权在专用用户域创建合成 fixture，形成 `S2-source-data`。fixture 只能包含公开合成词、合成 settings/privacy 和无敏感内容的数据库状态。
3. fingerprint 对每个固定节点记录相对路径、类型、uid/gid、mode、size 和 SHA-256，不导出文件正文；SQLite 同时纳入 `-wal`/`-shm` 是否存在及一致快照规则。
4. upgrade、repair、rollback、remove、reinstall 的前后 fingerprint 必须逐项相同；Fcitx profile 也必须不变。
5. startup smoke 导致的合法用户态写入只能发生在独立 clone，并与 package mutation 零写入证据分开。

固定范围为：

- `/home/radishlex-l6/.local/share/radishlex`；
- `/home/radishlex-l6/.config/radishlex`；
- `/home/radishlex-l6/.local/state/radishlex`；
- `/home/radishlex-l6/.cache/radishlex`；
- `/home/radishlex-l6/.config/fcitx5/profile`。

## 6. 主事务序列

每一步都使用新的 operation ID，并遵循“只读 preflight → 单步授权 → mutation → 只读 postflight → 记录结果”。不能一次授权整条序列。

| 顺序 | Operation | 前态 | Artifact | 终态 | 必须证明 |
| --- | --- | --- | --- | --- | --- |
| 1 | `install_source` | absent | target=source | source completed | dependency、完整 inventory、XDG absent、startup allow |
| 2 | `upgrade_target` | source | source→target | target completed | version 增序、无重复 mutation、XDG fingerprint 不变 |
| 3 | `repair_target` | target | target=target | target completed | 同版重新应用、程序身份恢复、XDG 不变 |
| 4 | `rollback_source` | target | target→source | source completed | version 降序、source 精确恢复、XDG 不变 |
| 5 | `remove_source` | source | source→absent | remove completed | package/product tree absent、startup failed closed、XDG 保留 |
| 6 | `reinstall_target` | absent | target=target | target completed | target 精确恢复、历史 receipt 追加、XDG 保留 |

每个 terminal 后必须确认 guard 未被持有、`receipt.json.tmp` 不存在、current operation slots 与 receipt 精确匹配。operation staging 和历史 receipt 全部保留；本 runbook 不定义清理成功现场。

## 7. Crash/retry matrix

format v1 固定八个 checkpoint：

| Case | Checkpoint | 预期恢复 |
| --- | --- | --- |
| install prepared | `prepared` | resume 后 completed |
| install staged | `artifacts_staged` | 重验 staging/quiescence 后 completed |
| upgrade quiesced | `quiesced` | 重验 relationship 与静止后 completed |
| upgrade before dpkg | `package_mutating_before_dpkg` | 重取 snapshot 后单次 apply |
| upgrade after dpkg | `target_applied_before_proof` | 识别 target 已安装，不重复 apply |
| rollback pending | `rollback_required` | source restore 后 rolled_back |
| rollback before dpkg | `source_restoring_before_dpkg` | 重验 source/静止后 restore |
| rollback after dpkg | `source_applied_before_proof` | 识别 source 已恢复，不重复 restore |

checkpoint controller 必须是显式 acceptance 构建身份，以不可由 production runtime 环境变量开启的编译边界实现。它在固定 checkpoint 持久化后通知外部 controller 并暂停；controller 只终止该测试的完整 process group，确认没有遗留 dpkg child 后再执行 resume。禁止：

- 轮询 receipt 并依赖竞态发送信号；
- 替换 `/usr/bin/dpkg`、修改 `PATH`、注入 shell/wrapper 或放宽 program identity；
- 使用 `--force-*`、手工改 dpkg status、手工改 receipt 或删除 staging 让恢复“通过”；
- 在同一 clone 连续执行多个 crash case。

实现位于独立 `platforms/linux-l6-acceptance/` crate。production crate 的 `l6-acceptance-checkpoints` feature 默认关闭，production main 只连接 disabled sink，不识别 acceptance 参数；acceptance worker 通过继承 pipe 发送 typed checkpoint 并暂停。controller 创建独立 process group，命中后用固定 `/usr/bin/kill` 发送 `SIGKILL`，等待 worker signal 终止，并连续复验 `/proc/*/stat` 中 group member 为零且没有 `dpkg` child，满足全部条件后才允许写 evidence。`crash` 命令还必须同时具备 `--authorized-l6-crash` 与 production mutation/data-preservation 授权；本节仍不构成运行授权。

## 8. 系统、字体与 startup probe

### Package 与 process

- dpkg status 必须接受 `install|hold ok installed`，拒绝 reinstreq、deinstall、half-*、unpacked 与 trigger 中间态；
- installed control `Depends` 与 terminal actual evidence 精确相同，所有直接依赖满足版本/architecture；
- Manager、addon、双 FFI、RimeData、metadata、desktop/icon/doc 与 manifest 的 owner/mode/link/size/hash 精确；
- mutation 前 Manager/Fcitx 及映射固定产品 inode 的进程均不存在；procfs 不可读、畸形或竞态不确定即停止。

### 字体

- `fonts-dejavu-core` 必须由 dpkg 证明 installed，并由 fontconfig 解析 `DejaVu Sans`；
- `fonts-noto-cjk` 必须由 dpkg 证明 installed，并解析 `Noto Sans CJK SC`；
- font file owner 必须回到对应 Debian package；Latin/数字样例 `RadishLex ABI v9 12345` 与中文样例“萝卜词核中文输入”均具有 glyph coverage；
- payload 仍只能携带固定 Material Icons，不能因 probe 失败复制字体或调用私有 `fc-cache` 修补现场。

### Startup

- installed terminal：Manager 与 Fcitx addon 分别得到 `AllowedProduct`，然后才可进入 Flutter/Engine/XDG/Rime；
- nonterminal/crash：两端均为 `MaintenanceRequired` 且用户域零写入；
- completed remove：两端均失败关闭，不能回退 development staging；
- 错误 sibling 与 preload interposition：必须在业务初始化前失败关闭；
- startup 负向测试使用独立 snapshot，不污染主序列的 XDG 零写入证据。

## 9. 每步授权与停止线

下列动作必须分别取得当次授权：

- 创建/克隆 VM、安装 dependency/取证工具、创建专用用户或写入 artifact input root；
- 执行每一个 install、upgrade、repair、rollback、remove、reinstall；
- 启用 acceptance checkpoint、终止 process group、resume 或恢复 snapshot；
- 启动/停止 Manager、Fcitx、桌面 session，或修改 Fcitx profile；
- 重启、注销、网络切换和任何现场清理。

出现以下任一情况立即停止并保留 clone：guest identity、artifact hash、dpkg config/status、receipt/guard/tmp、operation ID、process quiescence、dependency/font、XDG fingerprint、startup decision 或 expected terminal 不匹配；命令超时、输出溢出、权限不足、未知 lifecycle 也不能自动重试或降级。

UTM guest-agent 的传输返回码或空输出不能单独证明 transaction completed。第五套修复后 preflight 已观察到部分命令 exit status、stdout 与 stdin payload 转发不可靠；不得使用 Python/空输出/单一返回码直接判定。关键 package/receipt/status 字节须回读，结构化检查须有独立可靠退出合同，startup 调用须含正负向对照。长命令结束后还要确认 maintenance 进程退出，并以 canonical receipt、dpkg status/audit 与完整 package inventory 判定结果；缺少 receipt 即使 `utmctl exec` 返回 0 也按失败关闭，不推断或补写成功状态。

UTM 磁盘配置使用空 `Network` 数组也不能单独证明 guest 运行态断网；删除整个必填键会使 UTM 4.7.5 冷加载失败，注册缓存仍可能在首次启动挂回虚拟网卡并取得 DHCP。每次启动后、写入 artifact input 或生成 operation ID 前，都必须在 guest 内复验目标接口 down 且 IPv4/IPv6 路由为空；任一网络状态不明立即停止，不把后续断网状态倒推成“从启动起全程离线”。

前三个 L6 分别处于 dpkg config、guard parent 与 target manifest profile 停止线；第四个为 artifact-chain mismatch；第五个在 target validation 失败后自动恢复 source，terminal `rolled_back`。这些现场均停止并原样保留，不得热替换、恢复、跨 pair 混搭或原地重试。第六套已形成 target `completed` terminal与S3；其首次repair在dpkg前因旧production staged verifier缺口`aborted_preserved`，失败clone同样只作取证。源码回归已修复，但下一步先在隔离builder形成绑定既有target artifact的修复后ARM64 maintenance handoff，再从S3建全新clone；真实repair、rollback、remove、reinstall与crash/retry仍需逐次授权。

## 10. L6 完成与后续

L6 只有在主序列、八个 crash case、字体/dependency、startup 正负向、XDG 零写入/保留和 guest reboot 对照均由同一 release pair 闭合后完成。完成后仍然：

- 不清理 operation staging、receipt、artifact input 或 evidence；
- 不复用 L6 guest 作为 P04/P05C 日常环境；
- 不推送、创建 tag/Release 或发布 apt repository；
- 不自动修改 Fcitx profile或代表用户完成输入交互；
- P05C 仍需独立 guest 与新的逐步授权。
