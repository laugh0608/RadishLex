# Linux L6 Debian Package Matrix Runbook

本文定义 M5-P05B 的隔离 Debian 13 ARM64 L6 执行顺序、输入冻结、崩溃恢复、只读探针、证据格式和逐步授权边界。读者是准备或执行 Linux package transaction 验证的维护者。本文不授权创建或修改虚拟机、不提供公开 installer、不允许复用 P04 guest，也不授权运行 `apt`、`dpkg`、维护 CLI、启停 Fcitx/Manager、修改用户 XDG 或清理失败现场。

机器真相源是 [`packaging/linux/l6-matrix.json`](../../packaging/linux/l6-matrix.json)，仓库门禁是：

```bash
./scripts/check-linux-l6-contract.sh
./scripts/check-linux-l6-controller.sh
./scripts/check-linux-l6-release-pair.sh
./scripts/check-linux-l6-maintenance-refresh.sh
```

四个入口分别验证 matrix format、compile-isolated controller、release-pair 与 package-preserving maintenance-refresh 构建/证据合同；入口自身都不连接 guest、不读取真实 `/proc`、不执行 package mutation，也不能单独证明下述真实 pair、refresh handoff 或 L6 已通过。

## 当前执行状态

截至 2026-08-17：

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
- 根因是 production `validate_staged_operation` 对 repair 直接使用物理 staged source；而 repair 按合同只 stage target，host prepare 已把 target 同时作为 effective source。源码现统一该投影并增加 single-target system-port 回归。失败 clone 不 resume、重试或复用；release-pair v1 继续要求 executable commit 等于 target release commit并同批构建package。maintenance-only refresh v1 已另行闭合“冻结target package/evidence + 较新production ELF”的可信组合；commit `b891ed1` 的 ARM64 handoff record/ELF `4b41d1c0…fb2cd`/`9a657510…54585` 已冻结，旧 pair/package 未改写。
- 新 clone `BE3579E0-B150-438D-ABE0-53A8D46137F8` 从未改写 S3 建立并通过断网preflight后，只调用一次production maintenance。CLI与receipt返回`repair/same_release/completed`，但dpkg status/log/mtime均未变化；源码复核确认`drive_target`在installed target已匹配且product validation成功时，于`apply_package`前返回，既有repair测试又通过强制`target_valid=false`规避了健康路径。该结果分类为`completed_without_package_reapply`，不满足repair重装合同。clone与证据只作取证，不重试或复用。
- `698fe1f`现只让非repair或已有target proof的恢复采用valid-target快捷路径；首次repair即使产品健康也继续消费quiescence permit并apply一次。直接测试同时固定fresh healthy repair单次apply、proof-backed repair retry零mutation和五类operation既有语义，Linux product 118 tests与clippy通过。`f19cea7`再把maintenance-refresh required ancestor前移到`698fe1f`，旧`b891ed1` handoff不再满足当前构建合同；新`823afca` ARM64 handoff与全新S3 clone断网只读preflight均已冻结。
- clone `394217A7-BFC9-43C8-94E6-539FF3F2B6FB`启动前EFI/qcow2与S3精确相同、config仅Name/UUID不同且`Network=[]`；运行态证据固定仅`lo`与IPv4/IPv6 main route为空。旧第六套input被原子保留，`823afca` canonical USTAR `e917f1a9…6ad7`逐块回读后切入production-only fixed input，acceptance/source absent。只读preflight用`dpkg-query`与直接dpkg database/MD5 inventory复验package `38-2`，未调用`dpkg`；receipt `ccbc4cd0…1e60`、operation count 2、依赖/字体、manifest/双FFI、startup正负向、XDG、进程/映射及postflight均通过。host manifest/guest archive `878056b2…32f4`/`79cdf8d6…e406`已冻结；正常关机后config/EFI/qcow2 `818d8725…7425`/`a73a3266…9155`/`111aaf09…8a6`，qcow2复算一致、零句柄，S3复算未漂移，十三台VM全停。没有生成operation ID、运行maintenance/acceptance CLI、package mutation、产品启动或用户XDG写入；真实repair未开始。
- 后续单步授权重新闭合运行态断网与mutation preflight `9d379758…acca`后，guest生成唯一operation ID，宿主只保存hash `9a701cf3…ae0`。不可重复wrapper只调用一次production maintenance：exit 0、stderr空、stdout `maintenance_outcome=completed`；receipt `4e23198c…133b`为`repair/same_release/completed`、chain 3、target proof installed、failure null、manual recovery false。dpkg log新增1,652 bytes，受限delta `cca2898e…178`明确记录同版`38-2` upgrade/configure/installed；dpkg status与完整payload MD5 inventory保持不变，证明不是旧completed-noop路径。
- strict postflight `bce81a6e…f165`再次闭合20项依赖、字体、manifest/双FFI、startup正负向、XDG `f3df287f…b86b`、WAL/SHM/profile absence、进程/映射与断网；用户XDG零漂移。guest archive/host manifest `4e46a101…608b`/`116e3f06…5e90f`已冻结；正常关机后config/EFI/qcow2 `818d8725…7425`/`3b117def…8f0`/`79adac93…03f6`，qcow2复算一致且零句柄，S3仍为`00bac87d…6456`/`0846b1d3…9741`/`6b22499b…b137`，十三台VM全停。没有resume、rollback、第二次maintenance invocation、acceptance或产品启动；该clone不得再次调用或复用。
- 首台rollback clone `9FE6265D-2F3C-40C2-8830-19AA7641AE28`从S3对应registered terminal建立；启动前EFI/qcow2精确等于S3，config仅Name/UUID不同且`Network=[]`。文件回读网络证据`a18e1ab9…217f`证明Debian 13 ARM64、仅`lo`、IPv4/IPv6 main route为空、target package/receipt baseline与进程静止；宿主release-pair verifier也通过record `cda70afa…659b`及source/target artifact identity。
- 完整只读preflight脚本`49225412…b467`与startup probe`68f559ad…e1f5`在guest/host字节一致，但guest-agent只返回exit 0和空stdout/stderr，必需result文件不存在；按返回码不可信边界失败关闭，未重试、诊断、生成operation ID、运行maintenance/acceptance CLI、调用`dpkg`、执行rollback、启动产品或写XDG。host manifest `6b7ac641…a51d`已冻结；关机盘config/EFI/qcow2 `fdfc0502…23a5`/`a73a3266…9155`/`2b323e27…bb4a`稳定且零句柄，S3未漂移，十四台VM全停。旧证据确认push初始mode可为`0666`，而本次probe要求`0600`却未先归一化；这只是高概率触发点，精确guest predicate仍unknown。确定的wrapper缺口是失败时没有持久化phase与terminal result。
- 首台rollback clone随后在明确清理授权下由plain `utmctl delete`移除注册项与package，host manifest `6b7ac641…a51d`继续保留。plain `utmctl clone`再从S3对应registered terminal建立第二台`RadishLex-Debian13-ARM64-L6-80e49ce-rollback-v2`，UUID `BFB3EF09-2F13-4F75-9909-1F3EE2432720`；启动前EFI/qcow2等于S3、config仅Name/UUID不同且`Network=[]`。文件回读`d7bac2bc…6bd`再次证明仅`lo`、双main route为空、target package/receipt baseline、guard和产品/dpkg进程静止。
- 第二台先创建私有传输根，却因setup脚本要求目录mode/owner/group/link精确为`700|root|root|1`而返回`transfer_setup_error=root-identity`。正式只读preflight脚本未传入或执行，attempt marker、operation ID、maintenance/acceptance、`dpkg`、rollback、Manager/Fcitx与XDG写入均未发生；没有修复、重试或guest诊断。目录link count为1是高概率wrapper缺陷，但guest实际`stat`未冻结，不能写成确定根因或产品漂移。host manifest `40dab9b…680e`和关机盘`5258c84a…005`/`a73a3266…9155`/`1d20869b…57b`已冻结；S3未漂移、两盘零句柄，十四台VM全停。
- 第三台独立clone `EFD15599-7177-4D55-BF17-173EE1F0BBDD`仍由plain `utmctl clone`从S3对应registered terminal建立；启动前config `6295c4ed…29ae`去除Name/UUID后与S3语义相同，EFI/qcow2等于S3且`Network=[]`。文件回读网络证据`42e7da47…f454`证明仅`lo`、IPv4/IPv6 main route为空、target package/receipt baseline及产品/dpkg进程静止。
- 修正后的setup只对私有`0700`根验证root owner/group、directory与non-symlink；普通输入再归一化为root-owned `0600` single-link regular file，host/incoming/final guest副本逐字节一致。唯一一次正式只读preflight写入`O_EXCL` attempt并到达`postflight`，terminal `64f1b1ab…8ee`为`passed/postflight/reason=none`；证据`4edcbb6b…9f2`闭合canonical pair、source/target artifact、package/receipt、20项依赖、字体、startup正负向、XDG、WAL/SHM/profile absence、进程/映射及断网。没有创建新operation ID或运行maintenance/acceptance、`dpkg`、rollback、Manager/Fcitx与XDG写入。
- 正常request关机后第三台只读资格现场的config/EFI/qcow2为`6295c4ed…29ae`/`a73a3266…9155`/`34733d78…3b10`，qcow2复算一致；host manifest `0b71c24b…3073`覆盖其余26项。S3未漂移、两盘零句柄，十五台VM全停。
- 后续单步授权只启动第三台clone；文件回读网络证据`629680f8…37d6`与mutation preflight `3c4894fa…6320`再次固定仅`lo`、双main route为空、canonical pair、source/target artifact、target package/receipt、direct dpkg database与MD5 inventory、依赖/字体、startup、XDG和进程静止。此时仍没有新operation ID、maintenance/acceptance调用、dpkg mutation或XDG写入。
- guest随后生成唯一operation ID，宿主只保存hash `663623bc…c8b1`；one-shot只调用一次production maintenance，exit 0、stderr空、stdout `maintenance_outcome=completed`。receipt `b97c7580…664d`为`rollback/target_older/completed`、chain 3、target proof installed、failure null、manual recovery false；package回到`38-1`，dpkg status `33c4973d…cff1`，1,652-byte delta `6b4e8f39…6721`明确记录`38-2 → 38-1`与installed，source MD5 inventory完整通过。
- strict postflight `62e17a05…0427`通过source manifest/双FFI、20项依赖、字体、startup正负向、XDG `f3df287f…b86b`、WAL/SHM/profile absence、进程/映射与断网。guest archive/host manifest `879ab337…2b7e`/`fef4ea32…851d`已冻结；正常关机后config/EFI/qcow2 `6295c4ed…29ae`/`3b117def…a8f0`/`9a28509a…995a`，qcow2复算一致且零句柄，S3三项未漂移、十五台VM全停。该clone已是terminal rollback现场，不resume、重试、再次调用、清理或直接复用。
- 从clean `f8db10b`、origin/dev ahead 14与十五台VM全停开始，plain `utmctl clone`只从第三台rollback registered terminal建立独立remove clone `5EA2BAA2-B9A1-46CC-B496-B37826DD27A2`。启动前config去除Name/UUID后与source语义相同，EFI/qcow2精确等于source，且`Network=[]`、source/S3/clone零句柄；只启动该clone，运行态network evidence `a21afee8…6c3d`证明Debian 13 ARM64、仅`lo`、IPv4/IPv6 main route为空、source `38-1` package/rollback receipt与进程静止。
- 私有传输根、incoming脚本回读和root-owned `0600` single-link归一化均通过；正式只读preflight只执行一次，attempt/phase/terminal `5cd96f54…d079`/`f783172b…c532`/`bc65fdb0…fcbc`固定`passed/postflight`，完整证据`35f58ca3…ed12`闭合canonical pair、source/target artifact、`38-1` package、`rollback/target_older/completed` receipt、依赖/字体、startup正负向、XDG、进程/映射与断网。未创建新operation ID或执行maintenance/acceptance、dpkg、remove、Manager/Fcitx及用户XDG写入。guest archive/host manifest `14506554…d676`/`ade5743b…9dd8`已冻结；正常关机后config/EFI/qcow2 `92568d22…626a`/`4b6292f0…d726`/`91db6f02…6bab`，qcow2复算一致、三盘零句柄，rollback source与S3未漂移，十六台VM全停。
- 后续单步授权只启动该remove clone；运行态断网与mutation preflight `0be5e2a8…7f8d`通过后，guest生成唯一operation ID而宿主只保存hash `93fc5b83…ef3d`。one-shot只调用一次production remove，形成receipt `770a27b7…b40e`：`remove/not_applicable/completed`、chain 4；dpkg delta `c5dd0066…3ad4`记录完整remove lifecycle，package、28项payload、product tree与dpkg info absent，依赖/字体及XDG指纹保持。
- startup按`RemovedProgram`失败关闭，进程/映射与loopback-only网络静止。postflight v1只因同hash FFI位于`noexec`的`/run`失败，失败控制保留；移至`/var/tmp`后terminal evidence `eef62072…e80d`通过且未重跑maintenance。guest archive/host manifest `529ee42c…9b8e`/`be498439…97a1`已冻结；正常关机后config/EFI/qcow2 `92568d22…626a`/`abea62d2…c8fc`/`ad8a6c6b…55fa`，v3/S3未漂移、三盘零句柄且十六台VM全停。该clone现为冻结remove terminal，后续只另行授权从它建立独立reinstall clone并先做只读preflight。
- 独立reinstall clone `E671DB9C-5E2C-447B-9425-8D91D2CFD465`的absent-terminal只读资格由evidence `8ed9d43a…86b25`与host manifest `a98dbec6…5006a`冻结；remove receipt chain 4、package/product tree absent、`RemovedProgram` startup、XDG、依赖/字体、进程与断网均通过，未创建新operation ID或执行maintenance/dpkg。
- 后续boot的network/transfer/mutation-preflight `11c00403…be2`/`db9b3c59…3e50`/`fca225a7…3a01`通过后，guest生成唯一operation ID，宿主只保存hash `fafb05ad…79c8`。one-shot只调用一次production `reinstall_target`，receipt `3eb44171…e274c`为`install/not_applicable/completed`、chain 5；1,512-byte dpkg delta `b0338d25…116a`、完整payload、startup、XDG零漂移与strict postflight `b4bf57ba…09f93`通过。guest archive/host manifest为`4ea296fa…fd3ba`/`54f1e952…6685`；关机盘`db1e59ae…13b90`/`76753750…a1c3`/`feb2ba7e…aaa5`稳定，remove/rollback v3/S3未漂移、四盘零句柄且十七台VM全停。该clone现为冻结reinstall terminal，不resume、重试、再次调用、清理或复用。
- 首个`install_prepared` crash准备新增两台clone。`C0D96C0A…23A1F8`虽有磁盘config `Network=[]`，UTM注册缓存仍继承网卡，故在input与operation ID前停止并只作失败资产。v2 `FD24ADFF…17C056`从无网卡registered terminal取得注册身份，再换入DependencyFrozen absent EFI/qcow2；断网、package/state/XDG absent、输入回读与mutation preflight均通过。
- v2只调用一次acceptance controller，checkpoint envelope记录`install_prepared/prepared`、完整process group `SIGKILL`和无dpkg child；operation ID宿主仅存hash `9429b171…9145a`。首次crash-state探针因`/run`为`noexec`不能映射FFI，保留失败控制后将同hash probe复制到`/var/tmp`，未重跑transaction。第二次检查得到Manager/Fcitx `0:1:4:11:0`，独立证据确认合法guard位于canonical `root:root 01777 /run/lock`；这是startup只读validator与store共享父目录合同漂移。
- 该case在resume前失败关闭：dpkg mutation、resume、postflight均未执行，crash-state成功证据未产生，不能计为通过。host manifest `5d8c914a…3e42a`与summary `dab8cde5…1905`冻结失败/诊断控制且不含raw operation ID；v2关机盘恢复prestart `cfb5d343…1f2d`/`0b797641…1418`/`4967234b…4b18`并零句柄，十九台VM全停。`a6622d4`已让store/startup消费同一权限策略；旧pair与两台clone不得resume、热替换或复用，该步当时停止在“重建pair后另建clone”。
- 修复target `d75818f`的新pair在唯一2223 builder中只构建一次。完整dev bundle `a03ac739…b901`、冻结source package/evidence `09ed1228…bec`/`fe3d6297…cf94`和三套cache先后通过；外层接口down/双路由为空，namespace内loopback down、零路由且Cargo offline。record/target package/target evidence/production/acceptance/canonical USTAR为`c74fac12…9849`/`4dd00540…dcec`/`9e646c86…3f17`/`422a5080…7f3`/`5ca804e6…c7a6`/`bbc96507…b4db`。guest Python与双production Rust、宿主Python和8文件mode/link/hash均通过，handoff从absent incoming以no-replace rename发布。builder未安装package或执行transaction，关机盘`1f509c73…d4f`/`ecaefd2a…0699`/`84314201…573d`稳定且零句柄。
- 新pair首台retry `45D8205F…F2B4`在input switch前暴露host inventory/path缺陷。第二台`0BB24317…307C`完成断网input switch，但case把fresh absent误写成remove-terminal startup预期；Manager/Fcitx实际`0:1:4:15:0`为正确`ReceiptMissing`，故preflight失败关闭。两套证据持久归档manifest为`ae4d1da2…ab8d`/`d99cbfad…324c`；两台package随后与旧network/rollback transfer失败clone一起按授权删除。离线guest-case合同提交后，第三台retry `3EC83EB9…593B9`依次完成clone、断网、canonical input、fresh-absent双preflight、prepared checkpoint与exact resume terminal；host manifest `679b7e04…c544`证明source `38-1` installed、receipt completed、guard absent、`AllowedProduct`及XDG零写入。
- checkpoint授权只生成一次operation ID，宿主仅存hash `55af5046…1828`，并只调用一次controller。checkpoint `3fd5df67…4a70`证明prepared通知后完整process group经SIGKILL清零、无dpkg child；crash-state `e18bde6c…8719`固定receipt `install/not_applicable/prepared/chain-1`、guard有效已解锁、status/log未变及Manager/Fcitx `MaintenanceRequired/ActiveGuard`。73项持久manifest `a36b0cbc…8396`证明该时点尚未resume或执行dpkg mutation；随后独立授权的单次exact resume才到达上述source terminal。
- terminal manifest逐项复验后只发出一次正常host stop。plain清单前后均为十八台all-stopped；config/EFI/qcow2冻结为`62040cc9…eb9`/`f762ee52…e76`/`95e89df3…cb69`，qcow2严格串行复算两次一致，三文件零打开句柄。关机manifest `036bace8…f3b8`覆盖控制脚本、terminal复验、清单与磁盘证据；terminal后guest命令、maintenance与next case调用均为0。
- 第二个case `install_artifacts_staged`先完成repository-only typed合同，再取得clone-only授权。只读preflight从clean `5c278ed`复验十八台all-stopped、handoff `c74fac12…9849`、terminal/stopped manifest `679b7e04…c544`/`036bace8…f3b8`、DependencyFrozen与零句柄；首次plain UTM list在应用沙盒边界exit 134并以`8c8e4fdf…298f`失败关闭，未执行clone，随后只读提权重验通过。
- 控制脚本静态入口只有一次plain `utmctl clone`，没有start/stop/delete/exec/file、dpkg或operation ID入口。新clone `B0B826F6-D7D3-433C-8987-E2D6993A87B3`借用冻结reinstall terminal注册壳，随后仅把新clone EFI/qcow2替换为未改写DependencyFrozen APFS clonefile；config/EFI/qcow2为`038274cb…af32`/`0b797641…418`/`4967234b…b18`且`Network=[]`，双重qcow2复算一致、三域零句柄，十九台全部stopped。
- clone-only evidence从absent incoming以`renameatx_np(RENAME_EXCL)`发布为`RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-Clone-Prepared`；manifest `ce430efb…aeb8`覆盖14项控制、失败/成功清单、snapshot、最终config与postverify，目录/文件为`0700`/`0600`、单link、零xattr并逐项通过。该批没有启动VM、传input、进入guest、生成operation ID或运行acceptance/maintenance/dpkg；下一步必须另行授权启动并先闭合运行态断网。
- 首次start授权从clean `58e33af`与上述manifest开始；控制脚本静态命令面只有一次start、status/list、第一项guest断网exec与file pull，没有stop/clone/delete/push、input、preflight、transaction或operation ID入口。两份10GB级clone/source qcow2前置复哈希通过后，`utmctl start`在事件层报告OSStatus `-1712`，连续status仍为stopped，控制以`clone-did-not-start`退出；任务根没有产生started清单、exec输出或pull文件，证明未到达guest命令。
- 失败后只读冻结再次确认十九台all-stopped、config/EFI/qcow2仍为`038274cb…af32`/`0b797641…418`/`4967234b…b18`、`Network=[]`、qcow2双重复算一致且source/clone零句柄。八项持久证据（含manifest）从absent incoming不可覆盖发布为`RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-Start-Failed`，manifest `34c0918d…c8d8`逐项通过；不得把`-1712`直接归因为bookmark或磁盘问题，也不得自动retry。
- repository-only `l6_utm_start_once.py`把再次start固定为单次可判定状态机：双显式授权，绑定clean 40位head、executed-control固定路径/regular single-link identity、上述失败manifest及其`0700`/`0600`单link条目；output必须absent且位于repo/旧证据之外。控制先要求精确VM数、全停和target独立status stopped，再只调用一次plain start；stdout/stderr以64KiB前缀、完整size/hash、timeout/exit落盘，每次status各有独立JSON，最后另取完整list。status与list同时确认仅target started才返回0；started始终未观察且terminal全停返回10，前置拒绝返回11，其他状态返回12。证据文件为`0600`、root为`0700`并逐文件`fsync`，末尾生成manifest；所有terminal均写明未自动stop/retry、未进入guest、未传input或生成operation ID。八项合成测试覆盖成功、`-1712`全停、start timeout后started、清单/status不一致、并发VM拒绝、证据及控制身份约束，不调用真实UTM。
- 真实单次控制从clean `053018d`、首次失败manifest与十九台all-stopped开始。唯一start在90秒内无stdout/stderr并timeout，60次status与terminal list仍全为stopped，按合同返回10；guest断网分支、input、operation与transaction均未到达。postverify确认三项磁盘未变、qcow2双重复算一致、source/clone零句柄；包含全部poll JSON的持久manifest为`337007ff…7ebbb`。离线config/Registry/限定统一日志差分未取得可归因根因，因此该clone冻结为双start失败现场，不得第三次调用、修补或复用；下一台须从DependencyFrozen独立创建且clone-only/start仍分批授权。
- 后续v2 clone-only从clean `992a307`、前序失败manifest、十九台all-stopped和冻结DependencyFrozen开始，只调用一次plain clone。进程exit 0但stderr为OSStatus `-1712`，紧随其后的list仍与preclone相同，目标注册和`.utm` package均absent；控制在数量后置条件失败关闭，未替换磁盘、启动或进入guest，也未retry/delete。source/reinstall/旧clone磁盘与零句柄复验通过，失败manifest `65160b12…c1859`不可覆盖发布。repository-only clone-once控制已闭合，真实clone与start继续分批授权。
- 实机前按UTM官方CLI把clone名称修正为精确`--name`参数并提交`b86d72f`；修正前没有消耗真实调用。随后唯一clone从clean `b86d72f`、`65160b12…c1859`、十九台canonical all-stopped和冻结DependencyFrozen开始，以exit 0、空stderr、唯一新UUID `5B19AEF1…7DAB` stopped及精确package返回created，manifest `7ce53048…e5b7`逐项通过。只对该新v3 package原子换入DependencyFrozen EFI/qcow2，prepared manifest `b065c7ac…32db`固定config/EFI/qcow2 `d04b00e1…1ff4`/`0b797641…1418`/`4967234b…4b18`、`Network=[]`、双重qcow2与三域零句柄；独立复核确认二十台全部stopped。该批没有start、guest、input、operation ID、transaction、retry或delete；启动与首条guest断网仍须新授权。
- 取得单次start与成功后首条guest断网授权后，从clean `27087c3`重新绑定clone/prepared manifest、二十台canonical all-stopped、target/source config/EFI/qcow2、`Network=[]`与零句柄。一次性外层控制SHA-256为`d12c75ee…358b1`，只调用committed `l6_utm_start_once.py`一次；只有其确认target唯一started才允许一次guest断网exec与两次file pull，不含stop/retry/delete、input、operation ID、preflight、acceptance、maintenance或dpkg。
- 唯一start在90秒内无stdout/stderr并timeout；60次status与terminal list均未观察到started，二十台始终全stopped，控制返回10与`failed-closed-stopped`。start manifest `870f56dd…f3a5`及外层失败manifest `59c62c14…bba05`逐项通过，后者精确记录guest exec与file pull均为0次。成功门未成立，因此没有写guest网络证据、传input、生成operation ID或进入transaction，也没有自动stop/retry/delete。
- 独立只读postverify重新验证上述四份前序manifest、target stopped与二十台all-stopped，config/EFI/qcow2仍为`d04b00e1…1ff4`/`0b797641…1418`/`4967234b…4b18`，target qcow2双重复算一致、DependencyFrozen三项对照一致、`Network=[]`且source/target零句柄；manifest `8e3d5ced…8386`逐项通过。v3现冻结为单次start失败现场，不第二次start、修补、删除或复用；该观察不证明具体UTM/QEMU根因，下一步先在repository-only范围收敛host launch诊断边界。
- 首次真实只读host诊断绑定clean `a828be2`与上述三份manifest，在UTC 08:12–08:18窗口先确认二十台及target均stopped；`ps ... comm=`随后成功产生95,549-byte stdout并超过64KiB上限，控制以`diagnostics-incomplete`停止，未调用`log show`。六项证据manifest `158fe177…77c`完整通过且`root_cause=unattributed`，没有任何VM、guest或transaction动作。v2控制改为无路径的紧凑`ucomm`并强绑定该不完整manifest，下一次真实采集仍须新授权和新输出根。
- v2真实只读诊断绑定clean `5ab391d`、上述三份manifest和首次诊断`158fe177…77c`，再次确认同一二十台及target stopped。`ps ... ucomm=` exit 0、stderr空、stdout 31,570 bytes且未截断，但标识解析以`host-process-identifier-invalid`失败关闭，仍未调用`log show`；六项manifest `8ccdbb9f…dc7`完整通过且零mutation。诊断控制v3允许合法PID 0系统行、继续拒绝负值和重复PID，并强绑定两次不完整证据；下一次真实采集须再次新授权和新输出根。
- v3真实只读诊断绑定clean `86304b3`及前五份manifest，仍确认同一二十台及target stopped。`ps ... ucomm=` exit 0、stderr空、stdout 31,080 bytes且未截断，但标识解析再次以同一reason失败关闭，仍未调用`log show`；六项manifest `f952953a…e5ddf`完整通过且零mutation。原始清单未持久化，不能证明具体拒绝行；本机macOS `nobody`账户的精确UID `-2`是高可信合同缺口假设。诊断控制v4只允许该单一legacy UID，继续拒绝其他负值、非精确PID 0与重复PID，并强绑定三次不完整证据；下一次真实采集仍须新授权和新输出根。
- v4真实只读诊断绑定clean `bf42768`及前六份manifest，再次确认二十台与target stopped。`ps ... ucomm=` exit 0、stderr空、stdout 31,605 bytes且未截断；UID `-2`兼容使解析完成，脱敏结果中的UTM/utmctl/QEMU进程为0。固定predicate的`log show`随后exit 0、stderr空且未超时，但stdout 3,916,860 bytes、SHA-256 `0dbed63a…e9c`超过旧64KiB捕获上限，未生成`unified-log.json`并以日志输出截断失败关闭。八项manifest `34ae0563…743e`逐项通过，root cause仍未归属，所有VM/guest/transaction动作均未执行。
- repository-only v5额外绑定v4八项manifest与精确截断终态；list/status/process保留64KiB上限，只有固定system log命令显式使用8MiB捕获。日志仍要求完整NDJSON `finished`、最长15分钟窗口、同一predicate、非零/timeout/stderr/truncation失败关闭，并新增16,384事件、16KiB单行、32KiB消息、1,024字符脱敏prefix与16MiB脱敏序列化总量；原始stdout正文不写入证据。下一次真实采集须重新单独授权并使用新v5输出根。

### 当前本地资产登记（非发布证据）

冻结资产宿主根为 `/Users/luobo/VirtualMachines`；第四、第五、第六套、三台 repair clone、第三台rollback、独立remove/reinstall、旧pair真实checkpoint、第三台clean retry source terminal及第二个case新旧clone位于 UTM 默认 Documents 目录。第五套为 `rolled_back` failure/recovery 现场；第六套已形成 target `completed` terminal与未改写 S3；其余terminal与失败现场均不得复用。两台rollback失败clone、旧network失败clone与两台新pair harness失败clone已在持久归档后按授权删除，只保留host evidence。plain `utmctl` 枚举二十台且全部stopped；第三台clean retry已冻结为source terminal，第二个case旧clone两次start均未进入guest，新v3 clean clone的首次start也失败关闭且未进入guest。`Debian13-ARM64-DependencyFrozen.utm` 故意未注册并继续作为只读 COW 来源。全部 failure/mismatch/rolled-back/terminal/crash clone、handoff、snapshot与host evidence只作各自真相源，不原地重试、清理或混用。UTM只使用`PATH`中的plain `utmctl`，任何时刻最多运行一台VM。

| 相对路径 | UTM 状态 | 唯一职责与保留线 |
| --- | --- | --- |
| `RadishLex/VMs/RadishLex-Debian13-ARM64.utm` | 已注册；P04 | P04 验收现场；staging、backup、userdb、导入导出与临时服务原样保留，不复跑或清理 |
| `Debian13-ARM64-CleanBase.utm` | 已注册；rescue | 依赖安装前的纯 Debian 13 救援基线；不是 L6 S0，不写入 |
| `Debian13-ARM64-DependencyFrozen.utm` | 未注册 | 工作 VM 的 dependency-frozen APFS COW 恢复源；不是执行 guest，不启动或改写 |
| `Debian13-ARM64.utm` | 已注册；builder | Flutter/cache/source/build 与真实 release pair 的构建 VM；不执行 L6 package transaction |
| `Debian13-ARM64-L6.utm` | 已注册；旧 L6 stopped | 旧 pre-receipt failure disk；运行内存状态不再保留，package/evidence root 与 controller evidence 仍 absent，不重启、清理或复用 |
| `RadishLex-L6-Snapshots/S0-clean-e5b6da1` | 非 VM；旧 S0 | 未注册、不可启动的 APFS COW 恢复点；绑定旧 pair 的 config/EFI/qcow、guest/dpkg/XDG/handoff baseline，只作取证，不用于修复后重试 |
| `RadishLex-L6-Handoff-e5b6da1` | 非 VM；旧 handoff | host 上冻结的旧 canonical pair 副本；作为失败输入保留，不覆盖、安装或执行 |
| `RadishLex-L6-PairBuilder-2fa1b8c-v2.utm` | 已注册；builder stopped | 2223 隔离 builder；保留第四套、第六套、`823afca` refresh与`d75818f` pair build root/控制证据，不覆盖旧输出。最新关机后config/EFI/qcow2为`1f509c73…d4f`/`ecaefd2a…0699`/`84314201…573d`且qcow2复算一致、零句柄；不执行package transaction |
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
| `RadishLex-L6-Handoff-d75818f` | 非 VM；startup-lock修复后的canonical handoff | record `c74fac12…9849`；target commit `d75818f`包含`a6622d4`，source仍为prior-terminal `09ed1228…bec`，target package/evidence `4dd00540…dcec`/`9e646c86…3f17`。guest/host verifier、双production Rust、8文件mode/link/hash与canonical USTAR `bbc96507…b4db`通过并no-replace原子发布；retry input已使用但未生成operation或transaction |
| `RadishLex-L6-Maintenance-Refresh-b891ed1` | 非 VM；历史 package-preserving refresh handoff | record `4b41d1c0…fb2cd`、production maintenance ELF `9a657510…54585`；精确复用 target package/evidence `b211d940…d09c`/`2a1132c6…0e1b`，local evidence `a495e6a2…1cbb`。它缺少`698fe1f` ancestry要求，只作completed-noop根因取证，不再作为transaction输入 |
| `RadishLex-L6-Maintenance-Refresh-823afca` | 非 VM；当前 package-preserving refresh handoff | record `b3852845…a146`、production maintenance ELF `1f37b6f9…cca7`、canonical USTAR `e917f1a9…6ad7`；精确复用 target package/evidence `b211d940…d09c`/`2a1132c6…0e1b`并要求`698fe1f`祖先。guest/host verifier与mode/link/hash通过，local evidence `e581dec4…315f`；未重建package、启动transaction guest或产出acceptance executable |
| `RadishLex-L6-Repair-Preflight-b891ed1` | 非 VM；refresh repair host/guest evidence | local/guest evidence `7db3a16d…7502`/`4a92116f…1c62`；绑定 S3/refresh handoff、断网 input switch、target package/receipt/dependency/font/startup/XDG/process静止、operation/guard absence与关机磁盘身份；目录`0700`、文件`0600` single link，不是repair transaction evidence |
| `RadishLex-L6-Repair-b891ed1-Completed-Noop` | 非 VM；repair completed-noop evidence | local/guest evidence `655f19f4…b428`/`a331fdf1…3481`；receipt `75c2152f…af21`为`repair/same_release/completed`、chain 3，但dpkg status/log/mtime未变，classifier为`completed_without_package_reapply`。绑定唯一invocation hash、startup/XDG/网络/进程静止和关机磁盘；不是repair transaction成功证据 |
| `RadishLex-L6-Repair-Preflight-823afca` | 非 VM；当前repair host/guest只读证据 | host files manifest/guest archive `878056b2…32f4`/`79cdf8d6…e406`；绑定未改写S3、`823afca` handoff、断网input switch、target package/receipt/MD5 inventory/dependency/font/startup/XDG/process静止、operation count 2与关机磁盘身份。目录`0700`；manifest覆盖其余25个文件并另有自身hash；不是repair、operation或package mutation证据 |
| `RadishLex-L6-Repair-823afca` | 非 VM；真实repair completed evidence | host files manifest/guest archive `116e3f06…5e90f`/`4e46a101…608b`；绑定唯一invocation hash `9a701cf3…ae0`、receipt `4e23198c…133b`、同版dpkg重装delta、startup/XDG/网络/进程terminal postflight与关机磁盘。目录`0700`、文件`0600`，manifest覆盖其余29个文件；不含raw operation ID或完整dpkg log |
| `RadishLex-L6-Rollback-Preflight-33140db-Failed-Closed` | 非 VM；首台rollback只读preflight失败关闭证据 | host manifest `6b7ac641…a51d`覆盖13项；绑定未改写S3、canonical pair、clone身份、断网文件回读、guest/host脚本字节、必需result absent、关机磁盘与十四台stopped。VM package已授权删除，证据继续保留；精确guest predicate未知，不得把高概率mode mismatch写成确定根因，也不是rollback preflight通过证据 |
| `RadishLex-L6-Rollback-Preflight-8d21449-Root-Identity-Failed-Closed` | 非 VM；第二台rollback传输准备失败关闭证据 | host manifest `40dab9b…680e`覆盖其余14项；绑定未改写S3、canonical pair、第二台clone身份、断网文件回读、传输准备`root-identity`失败、正式preflight未开始、关机磁盘与十四台stopped。冻结脚本暴露目录link-count-one高概率控制缺陷，但guest实际`stat`未冻结，不是package/receipt漂移或preflight通过证据 |
| `RadishLex-L6-Rollback-Preflight-0611b86-Passed` | 非 VM；第三台rollback只读preflight通过证据 | host manifest `0b71c24b…3073`覆盖其余26项；绑定未改写S3、canonical pair/source/target artifact、第三台clone身份、loopback-only/双main route为空、target package/receipt、依赖/字体、startup、XDG与进程静止、唯一attempt和`passed/postflight` terminal、关机磁盘及十五台stopped。未创建新operation ID或执行mutation；不是rollback operation证据 |
| `RadishLex-L6-Rollback-80e49ce` | 非 VM；真实rollback completed evidence | host manifest/guest archive `fef4ea32…851d`/`879ab337…2b7e`；绑定唯一invocation hash `663623bc…c8b1`、receipt `b97c7580…664d`、`38-2 → 38-1` dpkg delta、source payload/startup/XDG/网络/进程terminal postflight与关机磁盘。目录`0700`、38文件`0600`，manifest覆盖其余37项；不含raw operation ID或receipt原文 |
| `RadishLex-L6-Remove-Preflight-80e49ce` | 非 VM；独立remove clone只读preflight通过证据 | host manifest/guest archive `ade5743b…9dd8`/`14506554…d676`；绑定rollback source terminal、canonical pair/source/target artifact、loopback-only/双main route为空、source package/receipt、依赖/字体、startup、XDG与进程静止、唯一attempt及`passed/postflight` terminal、关机磁盘与十六台stopped。未创建新operation ID或执行remove/package mutation；不是remove operation证据 |
| `RadishLex-L6-Remove-80e49ce` | 非 VM；真实remove completed证据 | host manifest/guest archive `be498439…97a1`/`529ee42c…9b8e`；绑定唯一invocation hash、receipt `770a27b7…b40e`、remove dpkg delta、package/product tree absent、`RemovedProgram` startup、XDG/网络/进程terminal postflight与关机磁盘。保留`/run` noexec失败控制与`/var/tmp`成功复验，不含raw operation ID、raw receipt或完整dpkg status/log |
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
| `UTM Documents/RadishLex-Debian13-ARM64-L6-b891ed1-repair.utm` | 已注册；repair completed-noop stopped | UUID `BE3579E0-B150-438D-ABE0-53A8D46137F8`；唯一production maintenance invocation hash `615d768a…b528`形成receipt `75c2152f…af21`，但健康product validation在dpkg前提前成功，status/log/mtime未变。关机后config/EFI/qcow2 `b06691d1…0ea9`/`3b117def…8f0`/`3ddd82a8…4ddf`；只作取证，不resume、重试、恢复或复用 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-823afca-repair.utm` | 已注册；repair completed stopped | UUID `394217A7-BFC9-43C8-94E6-539FF3F2B6FB`；唯一production invocation形成`repair/same_release/completed`且dpkg确实同版重装，operation count 3，startup/XDG postflight通过。关机后config/EFI/qcow2 `818d8725…7425`/`3b117def…8f0`/`79adac93…03f6`，qcow2复算一致且零句柄；不resume、重试、恢复、再次调用或复用 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-80e49ce-rollback.utm` | 已删除；host evidence retained | UUID `9FE6265D-2F3C-40C2-8830-19AA7641AE28`；曾在运行态断网通过、完整只读preflight必需result absent后失败关闭。经明确清理授权由plain `utmctl delete`移除注册项与package；host manifest `6b7ac641…a51d`保留，不得凭证据目录恢复或复用 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-80e49ce-rollback-v2.utm` | 已删除；host evidence retained | UUID `BFB3EF09-2F13-4F75-9909-1F3EE2432720`；运行态断网、package/receipt与进程静止通过，私有传输根建立后触发`root-identity`断言，正式preflight未开始。关机磁盘稳定且零句柄；持久host manifest `40dab9b5…680e`复验后按授权删除注册项与package，不恢复或复用 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-80e49ce-rollback-v3.utm` | 已注册；rollback completed stopped | UUID `EFD15599-7177-4D55-BF17-173EE1F0BBDD`；唯一production invocation形成`rollback/target_older/completed`，package精确回到source `38-1`、operation count 3，startup/XDG postflight通过。关机后config/EFI/qcow2 `6295c4ed…29ae`/`3b117def…a8f0`/`9a28509a…995a`，qcow2复算一致且零句柄；不resume、重试、再次调用、清理或直接复用，后续只作remove clone的registered terminal来源 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-80e49ce-remove.utm` | 已注册；remove completed stopped | UUID `5EA2BAA2-B9A1-46CC-B496-B37826DD27A2`；唯一production invocation形成`remove/not_applicable/completed`，operation count 4，package/product tree absent、startup失败关闭、XDG不变。关机后config/EFI/qcow2 `92568d22…626a`/`abea62d2…c8fc`/`ad8a6c6b…55fa`，qcow2复算一致且零句柄；不resume、重试、再次调用、清理或直接复用，后续只作独立reinstall clone的registered terminal来源 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-80e49ce-reinstall.utm` | 已注册；reinstall completed stopped | UUID `E671DB9C-5E2C-447B-9425-8D91D2CFD465`；唯一production invocation形成`install/not_applicable/completed`，operation count 5，target `38-2`完整恢复、startup与XDG postflight通过。关机后config/EFI/qcow2 `db1e59ae…13b90`/`76753750…a1c3`/`feb2ba7e…aaa5`，qcow2复算一致且零句柄；不resume、重试、再次调用、清理、恢复或复用 |
| `RadishLex-L6-Crash-Install-Prepared-80e49ce-Failed-Closed` | 非 VM；首个crash失败关闭证据 | host manifest `5d8c914a…3e42a`覆盖checkpoint、两次inspection失败、noexec relocation、startup/guard父目录诊断、输入与关机身份；summary `dab8cde5…1905`只保存operation ID hash。该目录证明失败关闭与根因，不是crash/retry通过或transaction terminal证据 |
| `RadishLex-L6-Crash-Install-Prepared-d75818f-Input-Harness-Failed-Closed` | 非 VM；input harness失败关闭证据 | host manifest `ae4d1da2…ab8d`覆盖完整临时任务根、关机freeze与最终config；证明input switch、preflight、operation ID和transaction均未开始。对应VM package已删除，不从证据恢复或复用 |
| `RadishLex-L6-Crash-Install-Prepared-d75818f-Preflight-Harness-Failed-Closed` | 非 VM；preflight harness失败关闭证据 | host manifest `d99cbfad…324c`覆盖完整临时任务根、failure manifest、guest state、关机freeze与最终config；证明无mutation preflight、operation ID、state、checkpoint、guard或transaction。对应VM package已删除，不从证据恢复或复用 |
| `RadishLex-L6-First-Batch-Cleanup-20260818` | 非 VM；第一批清理证据 | host manifest `e1930d6c…41f7c`绑定21→17的before/after清单、四套持久证据复验、精确UUID/package identity、零句柄与逐台plain `utmctl delete`结果；四个package absent、未授权注册项无差异、剩余十七台全stopped |
| `RadishLex-L6-Crash-Install-Prepared-d75818f-v3-Clone-Prepared` | 非 VM；第三台retry clone-only证据 | host manifest `13f2e3e9…954a2`覆盖clone控制、17→18注册清单、最终config与两次发布前冻结器失败记录；固定`3EC83EB9…593B9`、`Network=[]`、三项磁盘身份和十八台全stopped，明确未启动、未传input、未生成operation ID且未运行acceptance/maintenance/dpkg |
| `RadishLex-L6-Crash-Install-Prepared-d75818f-v3-Network-Ready` | 非 VM；第三台retry启动/断网证据 | host manifest `4b7081d5…7291`覆盖启动前后与独立postverify清单、控制脚本、observer诊断和两份同hash guest文件回读；network evidence `711f850c…b9d15`固定Debian 13 ARM64、仅`lo`、IPv4/IPv6 main route为空，目标唯一started、其余十七台stopped，未传input或运行preflight/transaction |
| `RadishLex-L6-Crash-Install-Prepared-d75818f-v3-Input-Preflight-Ready` | 非 VM；第三台retry input/preflight证据 | host manifest `0b703d10…5027`覆盖65项控制、guest-copy、result/evidence与空stderr；canonical bundle `7e52f445…dd7a`完成原子input switch，mutation/negative preflight `24724002…f9807`/`0ee3d34c…2fb0`均固定`ReceiptMissing`，operation ID/state/checkpoint/guard absent且目标仍唯一started/断网 |
| `RadishLex-L6-Crash-Install-Prepared-d75818f-v3-Checkpoint-Prepared` | 非 VM；修复后prepared checkpoint证据 | host manifest `a36b0cbc…8396`覆盖73项控制、readiness、controller/inspect回读及串行postverify；operation ID宿主只存hash `55af5046…1828`，checkpoint/crash-state `3fd5df67…4a70`/`e18bde6c…8719`证明process group清零、无dpkg child、receipt prepared、status/log未变及`ActiveGuard` startup。该证据时点未resume或执行dpkg mutation |
| `RadishLex-L6-Crash-Install-Prepared-d75818f-v3-Completed` | 非 VM；修复后exact resume terminal证据 | host manifest `679b7e04…c544`覆盖117项terminal-only控制、回读、guest archive与串行postverify；production resume只调用一次，resume/postflight/postverify `a3da8511…a8bf`/`5319db07…52a0`/`001b5be1…e2cc`证明source `38-1` installed、receipt completed、guard absent、`AllowedProduct`、XDG/进程/断网稳定。原始operation ID未离开guest |
| `RadishLex-L6-Crash-Install-Prepared-d75818f-v3-Stopped` | 非 VM；source terminal关机冻结证据 | host manifest `036bace8…f3b8`逐项绑定terminal manifest复验、十八台all-stopped双清单、config/EFI/qcow2 `62040cc9…eb9`/`f762ee52…e76`/`95e89df3…cb69`、双重qcow2复算与零句柄；正常stop后未执行guest命令、maintenance或下一case |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-Clone-Prepared` | 非 VM；第二个case clone-only证据 | host manifest `ce430efb…aeb8`绑定只读sandbox失败关闭、成功preflight、唯一clone命令、18→19 all-stopped清单、最终config、双重qcow2与零句柄；未启动、传input、生成operation ID或进入guest |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-Start-Failed` | 非 VM；第二个case首次start失败证据 | host manifest `34c0918d…c8d8`绑定唯一start控制、UTM `-1712`/`clone-did-not-start`、started/exec/pull输出absent、十九台all-stopped、三项磁盘未变、双重qcow2与零句柄；committed单次start控制已逐项验证该manifest，仍未retry |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-Start-Retry-Failed-Closed` | 非 VM；第二个case单次控制retry失败证据 | host manifest `337007ff…7ebbb`绑定唯一start的90秒timeout、60次stopped轮询、terminal全停、控制返回10、三项磁盘未变、双重qcow2与零句柄；未进入guest、传input、生成operation ID或执行transaction，不授权第三次start |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v2-Clone-Failed-Closed` | 非 VM；第二个case v2 clone失败证据 | host manifest `65160b12…c1859`绑定唯一clone的exit 0/stderr `-1712`、pre/post/final十九台同清单、新注册与package absent、source/reinstall/旧clone磁盘未变及零句柄；未替换磁盘、启动或进入guest，不授权retry |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v3-Clone-Once` | 非 VM；第二个case v3单次clone证据 | host manifest `7ce53048…e5b7`绑定clean `b86d72f`、前序manifest、十九台canonical全停清单、精确`--name`唯一clone、exit 0/空stderr、唯一新UUID/name stopped及精确package；未自动retry/delete/start或进入guest |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v3-Clone-Prepared` | 非 VM；第二个case v3 clean磁盘冻结证据 | host manifest `b065c7ac…32db`绑定clone manifest、DependencyFrozen与注册壳身份、20台前后同清单、target config/EFI/qcow2 `d04b00e1…1ff4`/`0b797641…1418`/`4967234b…4b18`、`Network=[]`、双重qcow2及三域零句柄；独立postverify通过，未启动或进入guest |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v3-Start-Once` | 非 VM；第二个case v3单次start控制证据 | host manifest `870f56dd…f3a5`覆盖clean head与prepared绑定、唯一start的90秒timeout、60次stopped轮询、terminal全停及控制返回10；terminal为`failed-closed-stopped`，未自动stop/retry或进入guest |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v3-Start-Network-Failed-Closed` | 非 VM；第二个case v3启动门失败证据 | host manifest `59c62c14…bba05`绑定clone/prepared/start、启动前与失败后二十台同一all-stopped清单、目标stopped、外层控制SHA-256及失败阶段；guest exec/file pull均为0次，未传input、生成operation ID或运行transaction |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v3-Start-Failure-Postverify` | 非 VM；第二个case v3启动失败磁盘复核 | host manifest `8e3d5ced…8386`逐项验证四份前序manifest、二十台all-stopped、target config/EFI/qcow2未变、qcow2双重复算、DependencyFrozen对照、`Network=[]`及零句柄；现场冻结且不授权第二次start |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v3-Host-Launch-Diagnostics` | 非 VM；v3首次只读host诊断失败关闭证据 | host manifest `158fe177…77c`绑定clean `a828be2`、三份启动失败证据、二十台全停和目标stopped；`ps ... comm=`返回95,549 bytes并因64KiB上限截断，unified log未执行。六项payload完整、无VM/guest/transaction动作；目录冻结且不得覆盖或作为v2输出根 |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v3-Host-Launch-Diagnostics-v2` | 非 VM；v3第二次只读host诊断失败关闭证据 | host manifest `8ccdbb9f…dc7`绑定clean `5ab391d`、四份前序证据、二十台全停和目标stopped；`ps ... ucomm=`返回31,570 bytes且未截断，解析因非法标识停止，unified log仍未执行。六项payload完整、无VM/guest/transaction动作；目录冻结且不得覆盖或作为v3输出根 |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v3-Host-Launch-Diagnostics-v3` | 非 VM；v3第三次只读host诊断失败关闭证据 | host manifest `f952953a…e5ddf`绑定clean `86304b3`、五份前序证据、二十台全停和目标stopped；`ps ... ucomm=`返回31,080 bytes且未截断，解析仍因非法标识停止，unified log未执行。六项payload完整、无VM/guest/transaction动作；目录冻结且不得覆盖或作为v4输出根 |
| `RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v3-Host-Launch-Diagnostics-v4` | 非 VM；v3第四次只读host诊断失败关闭证据 | host manifest `34ae0563…743e`绑定clean `bf42768`、六份前序证据、二十台全停和目标stopped；31,605-byte进程清单解析完成且相关进程为0，`log show` exit 0/空stderr/未超时，但3,916,860-byte stdout超过旧64KiB上限。八项payload完整、无VM/guest/transaction动作；目录冻结且不得覆盖或作为v5输出根 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-80e49ce-crash-install-prepared.utm` | 已删除；host evidence retained | UUID `C0D96C0A-EC5C-4809-86A0-733C4523A1F8`；UTM注册缓存继承网卡，在input与operation ID前停止。持久manifest `5d8c914a…3e42a`复验后按授权删除注册项与package，不恢复或复用 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-80e49ce-crash-install-prepared-v2.utm` | 已注册；startup contract failed-closed stopped | UUID `FD24ADFF-B160-46F0-B100-10BAAF17C056`；唯一`install_prepared` checkpoint后未resume/dpkg，startup误报`GuardInvalid`并暴露`01777`合同漂移。disposable关机后config/EFI/qcow2恢复`cfb5d343…1f2d`/`0b797641…1418`/`4967234b…4b18`且零句柄；不启动、resume、热替换、补证或复用 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-d75818f-crash-install-prepared.utm` | 已删除；host evidence retained | UUID `45D8205F-64B5-4E20-A129-895A1A4CF2B4`；canonical inventory顺序与build-environment路径错误导致input switch前失败，未preflight、operation ID或transaction。持久manifest `ae4d1da2…ab8d`复验后按授权删除注册项与package，不恢复或复用 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-d75818f-crash-install-prepared-v2.utm` | 已删除；host evidence retained | UUID `0BB24317-5FC7-4DD3-8572-F0AC282B307C`；断网input switch通过，preflight因fresh absent错误沿用remove-terminal预期而失败，实际`ReceiptMissing/no receipt`正确。持久manifest `d99cbfad…324c`复验后按授权删除注册项与package，不恢复或复用 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-d75818f-crash-install-prepared-v3.utm` | 已注册；clean retry source-terminal stopped | UUID `3EC83EB9-094B-492B-9756-D47E61C593B9`；从未改写DependencyFrozen盘创建，启动前config/EFI/qcow2为`62040cc9…eb9`/`0b797641…418`/`4967234b…b18`且`Network=[]`。single controller checkpoint与single exact resume已通过，source `38-1` terminal completed；正常停止后磁盘为`62040cc9…eb9`/`f762ee52…e76`/`95e89df3…cb69`且零句柄。不得重启、复用、运行guest命令或maintenance |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-d75818f-crash-install-artifacts-staged.utm` | 已注册；第二个case double-start-failed stopped | UUID `B0B826F6-D7D3-433C-8987-E2D6993A87B3`；clone-only身份为`038274cb…af32`/`0b797641…418`/`4967234b…b18`与`Network=[]`。首次start返回UTM `-1712`，单次控制retry又timeout并以全停返回10；两次均未进入guest，失败后磁盘未变、零句柄。不得第三次start、修补或复用 |
| `UTM Documents/RadishLex-Debian13-ARM64-L6-d75818f-crash-install-artifacts-staged-v3.utm` | 已注册；第二个case single-start-failed stopped | UUID `5B19AEF1-0F29-40B6-8F24-1B1929117DAB`；唯一clone created后只对新package换入未改写DependencyFrozen EFI/qcow2，config/EFI/qcow2为`d04b00e1…1ff4`/`0b797641…1418`/`4967234b…4b18`且`Network=[]`。唯一start timeout后60次status与terminal均stopped，postverify确认磁盘未变、二十台全停且零句柄；未进入guest，不得第二次start、修补、删除或复用 |

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

### 3.1 Package 冻结后的 maintenance refresh

第六套 target 已形成 terminal receipt 后，repair 修复不能再通过 release-pair builder重建 `38-2` package，也不能改写 `cda70afa…659b` record或热替换失败clone。仓库真相源 [`packaging/linux/l6-maintenance-refresh.json`](../../packaging/linux/l6-maintenance-refresh.json) 固定 `debian13-arm64-maintenance-refresh-v1`：base record、target package/evidence、旧 production ELF分别为 `cda70afa…659b`、`b211d940…d09c`/`2a1132c6…0e1b`、`b060c240…7d81`，refresh root必须是包含 `698fe1f` healthy-repair fix、product metadata与target commit `80e49ce`逐字段相同的clean descendant。旧`b891ed1` handoff是在前一required ancestor下形成的历史证据，不能作为下一次transaction输入。

获单独 builder 授权后，唯一入口为：

```bash
./scripts/build-linux-l6-maintenance-refresh.sh \
  --base-record /absolute/frozen/release-pair.evidence.json \
  --target-package /absolute/frozen/radishlex_26.7.1+38-2_arm64.deb \
  --target-artifact-evidence /absolute/frozen/radishlex_26.7.1+38-2_arm64.deb.evidence.json \
  --refresh-root /absolute/clean/refresh \
  --output /absolute/absent/maintenance-refresh
```

builder 先验证并exclusive-copy三份 `0644` single-link frozen input，再在Debian 13 ARM64、offline Cargo模式下只构建 `--no-default-features` production maintenance与临时actual-package verifier。旧target `.deb`由新production parser再次解析；handoff仅包含旧record、旧target package/evidence、build environment、新maintenance和canonical `radishlex-linux-l6-maintenance-refresh-evidence-v1`。新ELF必须是 `/lib/ld-linux-aarch64.so.1` 的AArch64 ELF、与旧ELF hash不同且不含acceptance markers。完整inventory通过后，Python verifier在同文件系统no-replace rename、同步父目录并发布后重验；不构建Manager/addon/rootfs/package，不产出acceptance executable，也不运行maintenance CLI、guest或dpkg。

commit `b891ed1` 已在独立 Debian 13.6 ARM64 builder 中以 user/network namespace、loopback down、零路由和 offline Cargo完成一次真实构建。record/新 ELF/build environment SHA-256 为 `4b41d1c0d998cd2a9349904ef52c768848403960cc4bacddb9a42f1f5f5fb2cd`/`9a657510737aa4df1e30850b33c8575615c6fb1770282af6fe720f5266554585`/`adcc6e30de383dcd58c8b104c36abee347ae07904615250fbee5b6f703d0c863`；target package/evidence 仍为 `b211d9406825515b2ba1c473b5f98069de00fa709505e2ba3ed3cb9b8af7d09c`/`2a1132c6fb27d4ca2e5e4bbd76864e3e753749c2bb43cae82287635b1c2d0e1b`。canonical USTAR `33a02bfb229a8ceb5117fb39ae4674f54f926ddbe3b3db07557d5e8f1aaaa2e2` 分块回读后由宿主 verifier 复验，并从 absent incoming 原子发布；local evidence `a495e6a234f949226a95bc641d92be03f3e245b74df2361cd0d94308a6001cbb`。两次 runner 口径错误均在 clean output/build 前或只读 postflight 中失败关闭并保留，实际 build仅执行一次；builder package/state保持 absent，未启动 L6 guest、运行 dpkg或重建 package，关机后十一台 VM 全停。

修复与ancestry门禁闭合后，clean `823afca`以完整Git bundle `ab9bd210…d14`再次进入同一独立builder；冻结base record/target package/target evidence仍为`cda70afa…659b`/`b211d940…d09c`/`2a1132c6…0e1b`，required ancestor为`698fe1f`。唯一production build在外层接口down、双路由为空和user/network namespace loopback down的offline Cargo环境完成，cache前后均为8687项；record/ELF/build environment为`b3852845…a146`/`1f37b6f9…cca7`/`adcc6e30…863d`。guest postflight复验package/state absent、dpkg audit为空且无build进程；canonical USTAR `e917f1a9…6ad7`经6块回读、主机重组与当前verifier通过后，以同文件系统no-replace rename原子发布，local evidence为`e581dec4…315f`。控制包装的五次诊断均在network gate、input transfer、host verify或final publication前失败关闭并冻结，没有package/product mutation；实际build仍只执行一次。正常关机后十二台VM全停，未启动transaction guest、生成operation ID、运行maintenance/acceptance CLI、调用dpkg或写用户XDG。

随后从未改写 S3 的精确恢复字节创建 `RadishLex-Debian13-ARM64-L6-b891ed1-repair`，UUID `BE3579E0-B150-438D-ABE0-53A8D46137F8`；启动前 EFI/qcow2 与 S3 相同，config仅 Name/UUID不同且`Network=[]`。第一条实际guest命令即确认只有`lo`，正式证据固定IPv4/IPv6 main route为空；早期误用`utmctl exec --`在宿主参数解析阶段失败，另一次把local-table loopback route误当外部route的诊断也保留，均未进入产品状态。refresh canonical USTAR `33a02bfb…2e2`经6块逐哈希重组后，input-switch验证旧第六套8文件与新refresh inventory/record/ELF，原子保留旧fixed input并切入production-only新根；switch/script evidence为`40c74444…afa`/`cd6d8764…83a`。只读preflight复验target package`38-2`、completed receipt `ccbc4cd0…1e60`、dpkg status/log、20项依赖、字体、manifest/双FFI、startup正负向、XDG `f3df287f…b86b`、WAL/SHM/profile absence、operation/guard/process/mapping与网络静止；没有生成operation ID、运行maintenance/acceptance CLI、调用dpkg、启动产品或写XDG。host/guest evidence `7db3a16d…7502`/`4a92116f…1c62`已原子冻结；两次archive wrapper校验失败都发生于archive创建前并保留。正常关机后config/EFI/qcow2为`b06691d1…0ea9`/`a73a3266…9155`/`6267d82e…89d`，qcow2零句柄、十二台VM全停。

随后在 clean `be6064c` 与十二台 VM 全停基线上，只启动该 clone。正式 boot network evidence `0ab413cd…b22c`证明仅loopback且IPv4/IPv6 main route为空；两次`utmctl exec` input参数诊断均未进入正式脚本或产品状态。mutation preflight result/output `db49cf07…de6e`/`632c6df8…f93d`再次闭合refresh/package/receipt/dependency/font/startup/XDG/process/network。guest内随后生成唯一operation ID，宿主只保留hash `615d768a…b528`，且production maintenance仅调用一次：exit 0、stderr空、stdout `maintenance_outcome=completed`。receipt `75c2152f…af21`为`repair/same_release/completed`、chain 3、target proof installed、failure null；但严格postflight在`dpkg-log-size`失败，独立诊断确认dpkg status `c09365b3…cece`和log `0b963d92…c2da`的size/mtime完全未变，故没有package mutation。源码定位为`drive_target`在installed target匹配且product validation成功时直接返回，repair测试又以`target_valid=false`强制走apply，遗漏健康路径；classifier `c806847a…383`据此固定`completed_without_package_reapply`。host/guest evidence `655f19f4…b428`/`a331fdf1…3481`已冻结；正常关机后config/EFI/qcow2为`b06691d1…0ea9`/`3b117def…8f0`/`3ddd82a8…4ddf`，十二台全停，S3三项identity未变。该clone不得再次调用或复用。

修复后的`823afca` handoff形成后，从S3恢复字节对应的registered terminal创建第三台repair clone `394217A7-BFC9-43C8-94E6-539FF3F2B6FB`。启动前config `818d8725…7425`在去除Name/UUID后与S3相同，EFI/qcow2精确为`0846b1d3…9741`/`6b22499b…b137`且`Network=[]`、零句柄；一次错误的`utmctl start --disposable false`在宿主参数解析阶段exit 64且VM仍stopped，不计入运行证据。正式启动后只有该clone started，文件回读证据`531ec2fd…4c88`证明Debian 13 ARM64、仅`lo`、双main route为空；一次guest-agent exit 0但stdout/stderr空的早期诊断不计为成功。

宿主逐块验证并输入canonical refresh USTAR `e917f1a9…6ad7`，guest原子保留旧`80e49ce` fixed input并切入record/ELF `b3852845…a146`/`1f37b6f9…cca7`，target package/evidence仍为`b211d940…d09c`/`2a1132c6…0e1b`，acceptance/source absent。只读preflight证据`1cd56775…65cd`不调用`dpkg`，只用`dpkg-query`、status/config/log直接哈希、空updates与MD5 inventory `bc36eff3…71f1`校验installed payload；receipt `ccbc4cd0…1e60`、operation count 2、20项依赖、字体、manifest/双FFI、startup正负向、XDG `f3df287f…b86b`、WAL/SHM/profile absence及进程/映射均通过。独立postflight `3610f602…5e1b`确认零漂移。canonical guest archive`79cdf8d6…e406`与host 25项manifest `878056b2…32f4`发布到`RadishLex-L6-Repair-Preflight-823afca`；正常关机后config/EFI/qcow2为`818d8725…7425`/`a73a3266…9155`/`111aaf09…8a6`，qcow2双次hash一致、零句柄，S3四项复算保持不变，十三台全停。未生成operation ID、运行maintenance/acceptance CLI、调用`dpkg`、启动产品、package mutation或写用户XDG；真实repair仍需新授权。

后续授权从clean `df7f4d7`、既有preflight manifest `878056b2…32f4`、clone关机盘与十三台全停开始；只启动`394217A7…B6FB`。首个guest文件回读证据`27d3d814…a4cb`再次确认boot、仅loopback、双main route为空、package/receipt/operation count 2与进程静止；mutation preflight `9d379758…acca`进一步逐哈希闭合refresh input、direct dpkg status/config/log、MD5 inventory、20项依赖、字体、startup正负向和XDG，且明确operation ID/maintenance/dpkg mutation尚未发生。

guest随后生成唯一32 hex operation ID，宿主只保存SHA-256 `9a701cf3…ae0`；one-shot marker后只调用一次`823afca` production maintenance。CLI exit 0、stderr空、stdout为`maintenance_outcome=completed`；receipt `4e23198c…133b`为`repair/same_release/completed`、chain 3、target proof installed、failure null、manual recovery false。dpkg status仍为`c09365b3…cece`，log从`0b963d92…c2da`/882,804增长到`334c7f50…3166`/884,456；1,652-byte delta `cca2898e…178`只含同版`38-2` unpack/configure/installed及发行版trigger lifecycle，installed MD5 inventory `bc36eff3…71f1`复验通过，因此满足repair必须重新应用package的合同。

strict postflight `bce81a6e…f165`再次通过package/receipt、dependency/font、manifest/双FFI、startup正负向、XDG `f3df287f…b86b`、WAL/SHM/profile absence、进程/映射和断网；没有resume、rollback、第二次maintenance invocation、acceptance或产品启动。deterministic guest archive `4e46a101…608b`为71,680 bytes、15项且不含raw operation ID；host目录`RadishLex-L6-Repair-823afca`从absent staging发布，`files.sha256`为`116e3f06…5e90f`并覆盖其余29项。正常关机后clone config/EFI/qcow2为`818d8725…7425`/`3b117def…8f0`/`79adac93…03f6`，qcow2两次复算一致、零句柄；S3三项保持`00bac87d…6456`/`0846b1d3…9741`/`6b22499b…b137`，十三台全停。归档器首次把outcome误读自result的诊断发生在archive root创建前，可靠wrapper固定rc 1后只修正读取位置；repair从未重试。

第三台rollback clone的只读资格冻结后，后续授权从clean `2b6a6ee`、preflight manifest `0b71c24b…3073`、十五台VM全停及S3/clone零句柄开始；只启动`EFD15599…BBDD`。首个inline guest-agent命令返回0却没有形成预定文件，按返回码不可信边界不计为断网成功，也没有进入operation；改用传入脚本、逐字节回读、执行、证据文件回读合同后，network evidence `629680f8…37d6`固定boot `e1d10657…b731`、仅`lo`、IPv4/IPv6 main route为空、`38-2` package、upgrade completed receipt、operation count 2与进程静止。startup、one-shot、postflight和mutation preflight均先回读逐字节比对，再归一化为root-owned `0600` single-link。

正式mutation preflight `3c4894fa…6320`重新闭合pair `cda70afa…659b`、source/target package/evidence、direct dpkg status/config/log、target MD5 inventory、20项依赖、字体、target manifest/双FFI、startup正负向、XDG `f3df287f…b86b`、WAL/SHM/profile absence、进程/映射与断网；此时operation count仍为2，未生成新operation ID或执行maintenance/acceptance/dpkg mutation。guest随后生成唯一operation ID，宿主只保存hash `663623bc…c8b1`；one-shot只调用一次production maintenance `b060c240…7d81`，把当前`38-2`作为source、冻结`38-1`作为target。CLI exit 0、stderr空、stdout `maintenance_outcome=completed`，没有retry或resume。

receipt `b97c7580…664d`为`rollback/target_older/completed`、chain 3、source proof null、target proof installed、failure null、manual recovery false。package回到`38-1`，dpkg status `33c4973d…cff1`；log增长为`b37eff5f…6ec5`/884,456 bytes，1,652-byte delta `6b4e8f39…6721`明确记录`38-2 → 38-1`、configure/installed和发行版triggers，source MD5 inventory `f6afcfb7…a50`完整校验通过。strict postflight `62e17a05…0427`再次闭合source manifest/双FFI、依赖/字体、startup、XDG、进程与断网；没有运行acceptance、启动Manager/Fcitx或写用户XDG。

deterministic guest archive `879ab337…2b7e`为71,680 bytes、16项且不含raw operation ID/receipt；host目录`RadishLex-L6-Rollback-80e49ce`从absent staging原子发布，`files.sha256`为`fef4ea32…851d`并覆盖其余37个`0600`文件。正常request关机后clone config/EFI/qcow2为`6295c4ed…29ae`/`3b117def…a8f0`/`9a28509a…995a`，qcow2两次复算一致且零句柄；S3三项未漂移、零句柄，十五台VM全停。该clone现为source terminal evidence，不直接复用；下一次只另行授权从其stopped registered terminal建立独立remove clone并先做只读preflight。

独立remove clone后续从clean `f8db10b`、rollback host manifest `fef4ea32…851d`、十五台VM全停与rollback/S3零句柄开始；plain `utmctl clone`只从`EFD15599…BBDD`建立`5EA2BAA2-B9A1-46CC-B496-B37826DD27A2`。启动前config `92568d22…626a`去除Name/UUID后与source语义相同，EFI/qcow2精确等于source且`Network=[]`；只启动该clone。运行态文件回读证据`a21afee8…6c3d`固定Debian 13 ARM64、仅`lo`、IPv4/IPv6 main route为空、source `38-1` package、rollback completed receipt与产品/dpkg进程静止。

传输setup、startup/preflight逐字节回读和root-owned `0600` single-link归一化均通过。正式只读脚本`bae27b23…7366`只执行一次；宿主不信任空stdout/stderr，只依据attempt `5cd96f54…d079`、phase `f783172b…c532`、terminal `bc65fdb0…fcbc`与完整evidence `35f58ca3…ed12`判定`passed/postflight`。其闭合canonical pair、source/target artifact、package/receipt、direct dpkg database与source MD5 inventory、20项依赖、字体、manifest/双FFI、startup正负向、XDG `f3df287f…b86b`、WAL/SHM/profile absence、operation count 3、进程/映射与断网；没有创建新operation ID、运行maintenance/acceptance、调用dpkg、执行remove、启动Manager/Fcitx或写用户XDG。guest archive `14506554…d676`与host manifest `ade5743b…9dd8`冻结后正常关机，clone config/EFI/qcow2为`92568d22…626a`/`4b6292f0…d726`/`91db6f02…6bab`，qcow2复算一致、三盘零句柄；rollback source/S3三项未漂移，十六台VM全停。

真实remove后续从clean `3eda15c`、上述preflight manifest逐项通过、remove/rollback/S3三盘零句柄与十六台VM全停开始；只启动`5EA2BAA2…27A2`。文件回读网络证据`633a9a99…f1e0`固定仅`lo`、IPv4/IPv6 main route为空、source `38-1` package/rollback receipt与进程静止；严格mutation preflight `0be5e2a8…7f8d`重新闭合canonical pair、source/target artifact、direct dpkg database与source MD5 inventory、20项依赖、字体、manifest/双FFI、startup、XDG和断网。此时没有新operation ID、maintenance/acceptance调用、dpkg mutation或XDG写入。

guest生成唯一operation ID，宿主只保存hash `93fc5b83…ef3d`；one-shot只调用一次production remove，exit 0、stderr空、stdout精确为`maintenance_outcome=completed`，没有retry、resume、acceptance或手工dpkg。receipt `770a27b7…b40e`、3,890 bytes，为`remove/not_applicable/completed`、chain 4、staged source only、target proof not-installed、failure null、manual recovery false。dpkg status `2c31c35c…e572`；log从884,456增至885,964 bytes，1,508-byte delta `c5dd0066…3ad4`记录installed→remove→half-configured→half-installed→config-files→not-installed及发行版trigger lifecycle。28项payload、product tree与dpkg info absent，20项依赖和字体保持。

Manager/Fcitx startup均为`FailedClosed + RemovedProgram + completed`，XDG fingerprint仍为`f3df287f…b86b`，WAL/SHM/profile absent，产品映射、dpkg进程和双main route为空。guest-agent第一次postflight调用只返回exit 0与空输出且没有terminal artifacts，按合同不计成功；捕获的v1再证明同hash FFI位于`noexec`的`/run`而被`ctypes`拒绝映射。保持transaction不重跑，只把同一探针复制到可执行的`/var/tmp`，terminal evidence `eef62072…e80d`通过。失败result/stderr与成功控制均进入deterministic guest archive `529ee42c…9b8e`，其为92,160 bytes、22项，不含raw operation ID、raw receipt、完整dpkg status/log。

host evidence目录`RadishLex-L6-Remove-80e49ce`从absent staging原子发布，目录`0700`，`files.sha256`为`be498439…97a1`并覆盖其余36项，逐项hash全过。宿主冻结脚本的内嵌`utmctl list`曾退出134并留下零长度registered文件；plain direct枚举仍得到相同十六台stopped清单，替换后由host summary明确记录并完成发布，未改变guest、transaction或磁盘。正常request关机后clone config/EFI/qcow2为`92568d22…626a`/`abea62d2…c8fc`/`ad8a6c6b…55fa`，qcow2复算一致；rollback v3与S3三项未漂移、三盘零句柄且十六台VM全停。该clone已是冻结remove terminal，不resume、重试、再次调用、清理或直接复用；后续只另行授权从它建立独立reinstall clone并先做只读preflight。

独立reinstall clone的先前只读资格由`8ed9d43a…86b25`/`a98dbec6…5006a`冻结。真实mutation从clean `c5b6631`、该manifest、十七台VM全停和四盘零句柄开始；只启动`E671DB9C…D465`。文件回读network evidence `11c00403…be2`固定boot `3e278249…623d`、仅`lo`、IPv4/IPv6 main route为空、package absent与remove receipt chain 4；transfer gate `db9b3c59…3e50`再固定控制文件为root-owned `0600` single-link及精确size/hash。

正式mutation preflight `fca225a7…3a01`为`passed/postflight/reason=none`，闭合canonical pair/target artifact、28项payload/product tree/dpkg info absent、direct dpkg status/log、依赖/字体、`RemovedProgram` startup、XDG `f3df287f…b86b`、WAL/SHM/profile absence、进程/映射与断网；此时仍未创建新operation ID或执行maintenance/acceptance/dpkg mutation。guest随后生成唯一operation ID，宿主只保存hash `fafb05ad…79c8`；不可重复wrapper只调用一次production `reinstall_target`，exit 0、stderr空、stdout精确为`maintenance_outcome=completed`，没有retry、resume、acceptance或手工dpkg。

receipt `3eb44171…e274c`、3,926 bytes，为`install/not_applicable/completed`、chain 5、staged target only、target proof installed、failure null、manual recovery false。package为target `38-2`，dpkg status `c09365b3…cece`；log增长至`906fba41…560c`/887,476 bytes，1,512-byte delta `b0338d25…116a`只记录absent→target install/configure/installed与发行版triggers。target MD5 inventory `bc36eff3…71f1`的28项完整通过。

strict postflight `b4bf57ba…09f93`闭合target manifest/双FFI、依赖/字体、startup正负向、XDG零漂移、进程/映射与loopback-only。脱敏guest archive `4ea296fa…fd3ba`为81,920 bytes、19项；host目录`RadishLex-L6-Reinstall-80e49ce`从absent staging原子发布，manifest `54f1e952…6685`覆盖其余34项并排除raw operation ID、raw receipt/status/full log。正常request关机后clone config/EFI/qcow2为`db1e59ae…13b90`/`76753750…a1c3`/`feb2ba7e…aaa5`，qcow2两次复算一致；remove、rollback v3与S3未漂移，四盘零句柄且十七台VM全停。该clone为冻结reinstall terminal，不resume、重试、再次调用、清理、恢复或复用。

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

首个实机case已在修复pair第三台retry形成terminal并关机冻结。旧pair的`prepared` checkpoint有效，但startup读取合法stale guard时暴露共享父目录策略漂移并在resume前停止；修复pair两台retry又分别暴露input harness与fresh-absent预期缺陷，三台旧现场均不得改写或继续。第三台clean retry `3EC83EB9…593B9`在同一断网boot中完成canonical input、双preflight、单次`install_prepared` checkpoint与单次exact resume。第二个case `install_artifacts_staged`已固定repository-only状态；旧clone `B0B826F6…87B3`两次start均失败关闭，v2 clone又以exit 0/stderr `-1712`且无注册/package失败关闭。新v3 clone `5B19AEF1…7DAB`建立并冻结后，唯一start也timeout并以二十台全stopped失败关闭，仍未进入guest；前三次只读host诊断停在进程阶段，v4越过进程解析后因3,916,860-byte日志输出超过旧64KiB上限失败关闭。诊断控制v5已收敛，真实采集须新授权，不授权原地start。

repository-only `l6_guest_case_contract.py`现把实机case共同输入固定为UTF-8 bytewise canonical inventory，环境文件只接受`build-environment.json`；guest-agent exit/stdout/stderr只保留为观察量，缺少非空canonical result/evidence文件回读一律失败关闭。startup预期按状态拆分：fresh absent固定`FailedClosed/ReceiptMissing/no receipt`，remove terminal固定`FailedClosed/RemovedProgram/completed`，合法stale guard固定`MaintenanceRequired/ActiveGuard/no receipt state`。第二个case的typed crash expectation与matrix单向交叉校验，首次安装Rust回归另证明checkpoint先于staged validation/quiescence/apply且exact resume只apply一次。该合同进入默认L6/repository门禁，不替代每个clone的identity、hash、权限、断网与现场文件回读，也不构成系统动作授权。

repository-only `l6_utm_clone_once.py`把host clone固定为双显式授权下的一次plain `utmctl clone`。控制在调用前绑定clean head、前序manifest逐项hash/identity、canonical全停注册清单、精确source UUID/name、目标name与`.utm` package absent；调用后联合exit/timeout、stderr、terminal注册清单和精确package存在性判定。只有命令确定成功且注册精确增加一个唯一stopped目标、既有VM不漂移、package精确存在才为created；exit 0伴随`-1712`且零落地为failed-closed-absent，timeout、registry/package单侧、重复name、数量或并发状态漂移均为state-indeterminate。输出以create-new `0700`根、exclusive `0600` JSON、64KiB诊断前缀与完整size/hash、逐文件`fsync`和manifest持久化；任何终态都不自动retry/delete/start，不替换磁盘或进入guest。默认门禁只用fake runner和临时package目录，不调用真实UTM，也不构成clone授权。

repository-only `l6_utm_launch_diagnostics.py`只用于冻结start失败后的host观察，不包裹、不重放也不触发start。v5控制要求两项显式授权，并在任何host命令前绑定clean committed head、主控制与binding模块identity、start/failure/postverify及v1-v4四次不完整诊断七份manifest逐项hash与关键终态语义；四次诊断还必须固定同一授权、20台数量、UTC时间窗、各自`comm=`/`ucomm=`命令、进程/日志阶段失败与零mutation终态。随后先以`utmctl list`与目标`utmctl status`重验预期数量、target及所有peer仍stopped，再且仅再执行固定`/bin/ps -axo pid=,ppid=,uid=,ucomm=`与最长15分钟的`/usr/bin/log show --style ndjson --no-pager --timezone UTC --info --no-debug --no-signpost --no-loss`，predicate只覆盖UTM、utmctl和qemu，时间参数使用`log(1)`接受的带UTC offset格式。进程解析接受精确`0/0/0 kernel_task`与macOS legacy UID `-2`但不输出无关记录，仍拒绝负PID/PPID、其他负UID、非精确PID 0与重复PID。输出根必须absent且位于仓库及七份冻结证据之外，目录/文件为`0700`/`0600`；list/status/process仍使用64KiB捕获，只有system log显式使用8MiB上限。process只保存role、PID/PPID/UID与无路径accounting name，log只保存受限metadata、完整message hash及home路径脱敏的有界prefix，并受16,384事件、16KiB单行、32KiB消息和16MiB脱敏总量限制；命令原始stdout不进入证据。超时、非零、stderr、截断、缺少合法NDJSON `finished`终止记录、格式/身份/状态漂移均失败关闭；terminal始终写`root_cause=unattributed`及clone/start/stop/guest/input/operation/transaction/retry/delete全未执行。fake-runner回归不读取真实system log；每次真实采集仍是独立系统授权，单条日志文本不能自行证明根因。

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

UTM/guest-agent 的返回码或空输出不能单独证明系统动作或transaction completed。第五套修复后 preflight 已观察到部分命令 exit status、stdout 与 stdin payload 转发不可靠；v2 clone又观察到plain `utmctl clone` exit 0、stderr `-1712`但注册与package均absent。不得使用单一返回码直接判定；host clone/start须复验注册、package和状态，guest关键package/receipt/status字节须回读，结构化检查须有独立可靠退出合同。长命令结束后还要确认maintenance进程退出，并以canonical receipt、dpkg status/audit与完整package inventory判定结果；缺少后置事实即使返回0也按失败关闭，不推断或补写成功状态。

guest-agent push 到达guest后的owner/mode也不是可信输入；既有第六套证据曾记录初始mode `0666`。每个新clone必须先创建root-owned `0700`私有传输根，把脚本与probe受控复制/安装为`0600` single-link regular file，并在执行前独立复验owner、mode、link count、size和SHA-256；不能直接对push落点施加最终mode断言后将失败静默吞掉。

正式只读preflight使用以下持久化控制合同：

1. 检查开始前以`O_EXCL`创建root-owned `0600` attempt marker，已存在即停止，不覆盖或重跑。
2. 在`pair`、`artifacts`、`package`、`dependency-font`、`receipt`、`startup`、`xdg`、`process`、`postflight`前写入固定phase；phase文件不得包含operation ID、用户路径正文或敏感数据。
3. 正常通过或任一失败退出都必须以临时文件、file `fsync`、原子rename和父目录`fsync`写terminal result，固定分类为`passed`、`predicate_failed`或`transport_inconclusive`，并保存最后phase；写证据失败仍按`transport_inconclusive`停止。
4. 宿主必须回拉并验证attempt、terminal result与实际执行脚本的size/hash；`utmctl exec`返回码、stdout/stderr只能作为诊断。terminal result缺失或不一致时不得推断具体predicate、自动修复或复用该clone。

该合同只修复取证可判定性，不放宽任何package、receipt、startup、XDG、进程或断网predicate，也不授权启动guest或执行transaction。

UTM 磁盘配置使用空 `Network` 数组也不能单独证明 guest 运行态断网；删除整个必填键会使 UTM 4.7.5 冷加载失败，注册缓存仍可能在首次启动挂回虚拟网卡并取得 DHCP。每次启动后、写入 artifact input 或生成 operation ID 前，都必须在 guest 内复验目标接口 down 且 IPv4/IPv6 路由为空；任一网络状态不明立即停止，不把后续断网状态倒推成“从启动起全程离线”。

前三个 L6 分别处于 dpkg config、guard parent 与 target manifest profile 停止线；第四个为 artifact-chain mismatch；第五个为`rolled_back`，第六套形成target `completed`与S3。独立clone又闭合真实repair、rollback、remove和reinstall，六类operation已有分散证据但不是同一session。第三台clean clone `3EC83EB9…593B9`已闭合首个有效crash case。`install_artifacts_staged`旧clone两次start均失败关闭；v2 clone唯一调用exit 0/stderr `-1712`且无注册/package，manifest `65160b12…c1859`证明后置拒绝。新v3 clone `5B19AEF1…7DAB`的创建/物化manifest为`7ce53048…e5b7`/`b065c7ac…32db`，唯一start又以90秒timeout、60次stopped与terminal全停返回10；start/failure/postverify manifest `870f56dd…f3a5`/`59c62c14…bba05`/`8e3d5ced…8386`证明始终未进入guest且磁盘未变。前三次只读诊断manifest `158fe177…77c`/`8ccdbb9f…dc7`/`f952953a…e5ddf`固定进程阶段失败与日志未执行；v4 manifest `34ae0563…743e`固定进程解析完成、相关进程0及3,916,860-byte日志输出截断。下一步须单独授权诊断控制v5真实采集并继续保持目标不启动，其他case与进一步清理仍关闭。

## 10. L6 完成与后续

L6 只有在主序列、八个 crash case、字体/dependency、startup 正负向、XDG 零写入/保留和 guest reboot 对照均由同一 release pair 闭合后完成。完成后仍然：

- 不清理 operation staging、receipt、artifact input 或 evidence；
- 不复用 L6 guest 作为 P04/P05C 日常环境；
- 不推送、创建 tag/Release 或发布 apt repository；
- 不自动修改 Fcitx profile或代表用户完成输入交互；
- P05C 仍需独立 guest 与新的逐步授权。
