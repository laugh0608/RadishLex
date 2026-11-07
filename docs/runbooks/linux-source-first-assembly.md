# Linux source-first 装配、seed 归集与 target 编排

本文供构建和验收维护者使用，说明仓库入口、输入、失败保留与验证。合同见[设计](../remediation/linux-m5-source-first-contract.md)，当前资格与授权以[current](../status/current.md)为准。本文件不授权执行真实构建、安装、VM/网络/工具链或完整门禁；下面的ABS路径是待执行申请固定的参数，不能直接运行。

## 单 artifact 与 target

入口为`scripts/linux-product/source_first_build.py`：

- `inspect-environment --output /ABS/environment.json`：只在预先配置好的Debian13 ARM64、loopback-only网络中探测现有工具/Flutter ARM64 release engine/pub配置，输出0600文件供审阅。每个工具版本探测最多15秒，总预算90秒，不配置网络、不下载依赖、不修改全局工具链。
- `source --work /ABS/absent-work --output /ABS/absent-handoff --environment-file /ABS/environment.json --budget-seconds N --allow-build-process-timeout-cleanup`：S必须干净、40-2，work/output必须absent、checkout之外、同卷、不互相嵌套；环境与冻结文件严格一致。预算为显式1–7200秒，旧20/45分钟授权不适用。该flag必须由本轮执行申请明确覆盖，仅可收尾该入口新建的构建进程组；不得据此结束已有会话/程序。
- `target`使用相同参数，另需`--review-commit R --seed-root /ABS/seed --review /ABS/review.json --source-package /ABS/source.deb --source-artifact-evidence /ABS/source.deb.evidence.json`。这些basename须实际为合同的40-2文件名，示意路径不构成合法身份。先完成committed R/S/T及私有seed准入，再拷贝source、仅构建T的40-3。缺baseline时零构建命令、零输出。

依次调用既有metadata、Manager system-product、addon stage、rootfs assemble/verify、真实payload layout、Debian artifact步骤，再构建production maintenance/verifier；target另构建编译隔离的acceptance。Rust locked/offline、Flutter no-pub、并发1、清除注入编译变量，预先准备的pub配置作为有界输入固定前后身份。`target`/Manager build/Flutter ephemeral仍须初始absent；`.dart_tool`不能当作旧产品构建输出整目录拒绝，也不自动生成或联网填充。

完整实际`.deb`关系由本次生产artifact verifier逐侧核验。ELF必须AArch64、固定loader，production/verifier不得含acceptance标记，三个角色摘要互异；source执行器身份继续属于S，T执行器独立记录。source原件、seed/review、pub配置、环境文件、HEAD/锁/配方在构建后重新读取，等字节inode替换也拒绝。

source handoff含artifacts、S production/verifier、build-environment.json、source.json和inventory.json；target handoff另含精确source拷贝、T production/verifier/acceptance和pair.evidence.json。闭合inventory覆盖包、evidence、执行器及record，inventory不把自己反向纳入清单。根0700、发行文件0644/ELF0755、结构化记录0600；Debian builder所需artifacts子目录暂用0755，受外层0700保护，发布前恢复0700。逐文件/目录fsync、重哈希、no-replace及父目录fsync成功才返回。

work包含request、命令/日志、temporary及失败时failure.json；失败不清理、重跑或覆盖。三个既有shell入口新增仅供本调用使用的`RADISHLEX_PRESERVE_BUILD_FAILURES=1`，保留其失败临时根，默认行为不变。每条命令共享总deadline，单日志最多8MiB，超时/超限失败而非截断后冒充成功。父目录同步失败或发布期间超预算时，已发布目录也保留并返回失败；未知状态不得自动继续。

这些是未审阅artifact，不生成baseline，不表示SQLite 3.51.3实际source ID、L1–L5完整平台资格、真实输入或新pair已验收。真实ARM64编译、双FFI/SQLite身份和离线工具/缓存可行性均须V3/V4实测；缺项仍停止，不放宽合同。

## Seed 私有 capture 输入与归集

入口为`scripts/linux-product/source_first_capture.py`，固定参数为`--capture-root /ABS/private --source-package /ABS/package --source-artifact-evidence /ABS/evidence --verifier /ABS/S-verifier --work /ABS/absent-work --output /ABS/absent-seed --budget-seconds N`，预算1–300秒。它读取明确的guest/host授权采集结果，不启动、查询或安装guest；正常production首次安装及guest/host现场采集仍单独申请。collector不是系统observer，也不会把退出码或自填成功值当作实际观察。

private根和各子目录0700、文件0600。capture.json严格包含format/profile/source/binding/roles；format为`radishlex-linux-source-private-capture-v1`，source/binding沿用portable合同。roles精确12项，每项含report引用及materials映射；引用都是`{path,file:{filename,size,sha256}}`，安全相对路径不超过180字节、不允许链接/别名/未知文件，最多512文件、16MiB，结构化文件各最多1MiB。

每个report格式为`{format,role,binding,facts}`，format为`radishlex-linux-source-capture-report-v1`；facts从授权观察生成，必须满足portable同角色的非摘要事实，不允许预填下表派生摘要。归集器读取并散列实际文件，不自动补齐成功事实。

| 角色 | 必需materials与派生摘要 |
| --- | --- |
| receipt | `receipt_sha256`引用原始compact canonical receipt（最多64KiB），`root_staging_sha256`引用root/staging报告 |
| xdg | before_sha256、after_sha256分别引用实际fingerprint JSON，字节必须相同 |
| guest-result | result_sha256引用固定guest结果文件 |
| host-readback | result_sha256、guest_inventory_sha256、host_inventory_sha256分别引用回读文件；实际摘要须与guest及彼此匹配 |
| 其余8个角色 | materials为空；observation_sha256从该角色实际report字节派生 |

S verifier先按现有Rust actual relationship核验包，再用新增`--seed-receipt /ABS/raw --package /ABS/package --evidence /ABS/evidence`严格decode原receipt。只接受install/not_applicable/completed、chain1、严格not_installed（config_files不算fresh）、source/proof为空、完整target及installed proof匹配、无failure/manual recovery。输出只含facts和receipt/operation/root identity摘要，不导出原operation ID、路径或receipt。原4参数artifact核验入口保持原行为。

receipt report的非摘要facts必须与Rust结果完全相同；root/staging材料含`{binding,root_identity_sha256,staging_inventory_sha256}`，根identity必须绑定Rust receipt，staging inventory摘要非空。root identity摘要算法是对serde JSON Value的compact编码散列；该摘要本身不证明root/staging实物，仍由独立审阅核对原材料。其余report也只能来自获准的固定observer和host收证，任意自填报告即使格式合法也不构成真实资格。

collector核验S verifier实际文件/ELF和source在Git中的S metadata/锁/配方、读取窗口内全部私有材料、source包/evidence及执行器前后identity/hash，再生成12个脱敏附件和seed.evidence.json，复用closed-seed/guest-host一致性核验并独占发布。raw receipt及私有材料留原根，work另保留private inventory摘要与raw-material-review=required。错误、panic或发布后同步失败均保留已产生材料，没有R生产锚点自动生成。

## V3 源码预审与环境核验申请

2026-11-07 仓库预审通过。拟将 `a31152b6688946aaface232de9ddd1184b128456` 列为 S 的源码候选。该提交是 `8800142` 的后继，已提交的 Linux product metadata 与 candidate 的 `source_metadata` 逐字段一致。产物配置指向 `radishlex_26.7.1+40-2_arm64.deb` 及同名 `.evidence.json`。以下 SHA-256 来自已提交字节，元数据与配方摘要使用 source-first 的 canonical 编码。三份锁及 18 个配方文件与 `8800142` 的字节一致。

| 输入 | SHA-256 |
| --- | --- |
| Linux product metadata | `806412de2cb2e120d0098497718156d0b81417c9ddad0a5c0ace22aa82016c73` |
| `Cargo.lock` | `c6389589be716706fb0d3c0f61d2d40f75c83d2b29d2a1d017b01b9a2c83a67a` |
| Manager `pubspec.lock` | `a0c0468f85ad4cbd3f19bc85a8f92e258bb76588eb51e8f17f2233343be504cd` |
| RimeData 来源锁 | `5722b2f31769758d392db395e0c367f2a30e189f25e7cc23bf05a3e3385698a1` |
| 18 项配方摘要映射 | `9c25926084c69045d1df9802583f16027524bc7be6dac9c1949bba6271e5d3eb` |

预审时 macOS 工作区含 CI 记录改动。实际 builder 须另有绑定该 SHA 的干净 checkout。正式 S 选择、ARM64 产物、seed 安装及 R 锚点各按后续证据审阅。[环境核验结果](../devlogs/2026-W45.md#2026-11-07-v3-离线环境核验)

### 离线输入清单

| 组别 | 构建前应具备 | 本次可核对的范围 |
| --- | --- | --- |
| Builder | Debian 13 trixie ARM64，`aarch64-linux-gnu`，已隔离且仅有 `lo` 的执行会话 | OS、架构、Debian 包架构与当前网络接口 |
| Rust | 可用的 ARM64 Rust/Cargo 工具链，`Cargo.lock` 对应的 registry index 及 crate 缓存，编译器、链接器与 ARM64 sysroot | 工具版本、registry 目录和工具可见性。锁内全部 crate 的离线解析及构建留待构建批 |
| Flutter | 已初始化的 Linux ARM64 Flutter SDK、Dart SDK、tool snapshot/stamp、release engine，锁定的 pub 缓存与该 checkout 的 `.dart_tool/package_config.json` | release engine 目录、snapshot 与 stamp 存在，记录 Flutter 版本和 pub 配置 SHA-256。包配置引用是否完整可用留待构建批 |
| 原生与打包 | C/C++、Clang、CMake/CTest、Ninja、pkg-config、Fcitx5Core ≥ 5.1.9、librime ≥ 1.13.1、GTK/GLib/GIO 开发库，Debian 打包工具、ELF 工具及 fontconfig | 工具可见性、限定包安装状态与版本、Rime/GTK pkg-config。实际 Fcitx CMake 解析、字体匹配、ELF 和 `dpkg-shlibdeps` 关系留待构建批 |
| 载体与数据 | 提交内锁定 RimeData 及许可证，Debian 的 `fcitx5`、`librime1t64`、DejaVu 和 Noto CJK 字体 | 已提交 Rime 锁摘要与目标 metadata 一致，builder 上只读核对依赖包。双 FFI、SQLite 3.51.3/source ID、Rime payload 与 L1–L5 取证留待实际产物 |

### 一次 90 秒核验

拟使用日常 `Debian13-ARM64`，迁移记录中的载体为 VM 根下的 `utms/Debian13-ARM64.utm`。历史登记 UUID `755199B1-1C18-4441-8A1E-D423C8DE0022` 同时用于未注册的 `DependencyFrozen` 备份，因此须核对实际载体路径。执行前由操作者确认是已准备好的日常机，会话已经运行且无其他运行中的 VM。若需要启动 VM、配置隔离、准备 checkout 或补缓存，先停在对应缺项并单独确定范围。[本机 VM 约定](/Users/luobo/VirtualMachines/README.md)

拟定持久工作根为 guest 内 `/srv/radishlex-v3/a31152b`。其 `checkout`、`inputs/flutter`、`inputs/cargo`、`inputs/rustup` 和 `inputs/pub-cache` 须事先准备，由当前专用非 root 账号拥有，与其他任务和冻结资产隔离。批准范围为在该会话执行下面命令一次，在新建的 `probe-01` 写私有日志和 `environment.json`。Flutter 版本命令可能刷新 SDK stamp 和该专用账号的工具状态，因此写入范围也涵盖该专用 Flutter SDK 缓存和工具状态。会话应无转发代理，工具探测串行，不占用第二台 VM。

```bash
/usr/bin/env -u BASH_ENV -u ENV /usr/bin/timeout --verbose --signal=TERM \
  --kill-after=5s 85s /bin/bash --noprofile --norc -s <<'V3_PROBE'
set -eEuo pipefail
umask 077
root=/srv/radishlex-v3/a31152b
checkout="$root/checkout"
attempt="$root/probe-01"
export FLUTTER_ROOT="$root/inputs/flutter" CARGO_HOME="$root/inputs/cargo"
export RUSTUP_HOME="$root/inputs/rustup" PUB_CACHE="$root/inputs/pub-cache"
export PATH="$FLUTTER_ROOT/bin:$CARGO_HOME/bin:/usr/bin:/bin"
export LC_ALL=C PYTHONDONTWRITEBYTECODE=1 GIT_OPTIONAL_LOCKS=0
export CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0

trap 'printf "V3 probe stopped at line %s\n" "$LINENO" >&2' ERR
builder_uid="$(id -u)"
test "$builder_uid" != 0
test "$(uname -s)/$(uname -m)" = Linux/aarch64
test "$(printf '%s\n' /sys/class/net/*)" = /sys/class/net/lo
for dir in "$root" "$checkout" "$FLUTTER_ROOT" "$CARGO_HOME" "$RUSTUP_HOME" "$PUB_CACHE"; do
  test -d "$dir"
  test "$(realpath -e -- "$dir")" = "$dir"
  test "$(stat -c %u -- "$dir")" = "$builder_uid"
done
test "$(stat -c %a -- "$root")" = 700
mkdir -m 0700 -- "$attempt"
exec >"$attempt/probe.log" 2>&1
cd -- "$checkout"
test "$(git -c core.fsmonitor=false rev-parse --show-toplevel)" = "$checkout"
test "$(git -c core.fsmonitor=false rev-parse HEAD)" = a31152b6688946aaface232de9ddd1184b128456
checkout_status="$(git -c core.fsmonitor=false status --porcelain=v1 --untracked-files=all --ignore-submodules=none)"
test -z "$checkout_status"
for output in target apps/radishlex-manager/build apps/radishlex-manager/linux/flutter/ephemeral; do
  test ! -e "$output" && test ! -L "$output" || exit 1
done

test "$(dpkg --print-architecture)" = arm64
for tool in python3 cargo rustc cmake ctest clang clang++ cc c++ ninja pkg-config \
    dpkg-query dpkg-deb dpkg-shlibdeps rg file ldd readelf nm strings fc-match; do
  command -v "$tool"
done
test -d "$CARGO_HOME/registry/index"
[[ -d "$CARGO_HOME/registry/cache" || -d "$CARGO_HOME/registry/src" ]]
test "$(command -v cargo)" = "$CARGO_HOME/bin/cargo"
test "$(command -v rustc)" = "$CARGO_HOME/bin/rustc"
test "$(command -v flutter)" = "$FLUTTER_ROOT/bin/flutter"
test "$(realpath -e "$FLUTTER_ROOT/bin/flutter")" = "$FLUTTER_ROOT/bin/flutter"
test "$(realpath -e "$FLUTTER_ROOT/bin/cache")" = "$FLUTTER_ROOT/bin/cache"
test -x "$FLUTTER_ROOT/bin/cache/dart-sdk/bin/dart"
test -s "$FLUTTER_ROOT/bin/cache/flutter_tools.snapshot"
flutter_revision="$(git -c core.fsmonitor=false -C "$FLUTTER_ROOT" rev-parse HEAD)"
test "$(cat "$FLUTTER_ROOT/bin/cache/flutter_tools.stamp")" = "$flutter_revision:"
test -s "$FLUTTER_ROOT/packages/flutter_tools/pubspec.lock"
test ! "$FLUTTER_ROOT/packages/flutter_tools/pubspec.yaml" -nt "$FLUTTER_ROOT/packages/flutter_tools/pubspec.lock"
pkg-config --atleast-version=1.13.1 rime
pkg-config --modversion rime gtk+-3.0 glib-2.0 gio-2.0
for package in libfcitx5core-dev librime-dev libgtk-3-dev fcitx5 librime1t64 \
    fonts-dejavu-core fonts-noto-cjk; do
  test "$(dpkg-query -W -f='${Status}' "$package")" = 'install ok installed'
  dpkg-query -W -f='${binary:Package}\t${Version}\t${Architecture}\n' "$package"
done
fcitx_version="$(dpkg-query -W -f='${Version}' libfcitx5core-dev)"
dpkg --compare-versions "$fcitx_version" ge 5.1.9

python3 -B scripts/linux-product/source_first_build.py inspect-environment \
  --output "$attempt/environment.json"
test -s "$attempt/environment.json"
test "$(stat -c '%u:%a:%h' -- "$attempt/environment.json")" = "$builder_uid:600:1"
test "$(git -c core.fsmonitor=false rev-parse HEAD)" = a31152b6688946aaface232de9ddd1184b128456
checkout_status="$(git -c core.fsmonitor=false status --porcelain=v1 --untracked-files=all --ignore-submodules=none)"
test -z "$checkout_status"
sha256sum -- "$attempt/environment.json"
V3_PROBE
```

外层预算包含目录前检、工具核对、探测与输出回读。第 85 秒向本次调用的进程组发送 TERM，五秒后仍运行则发送 KILL。既有进程与会话不在此收尾范围。任一前检失败、探测非零、超时或输出身份不符，保留已有日志和文件，按失败记录一次，后续重新执行需重新批准。成功须同时有退出码 0、同一 checkout、独占写入的 0600 环境 JSON 和摘要，之后只进入环境结果审阅。[Debian timeout 手册](https://manpages.debian.org/trixie/coreutils/timeout.1.en.html)

## 首次仓库定向验证计划与执行记录

以下保留初批计划和失败诊断；后续批准的复验及完整自动结果按时间记录在后续小节。仓库实现已提交`8800142`，不因本节命令仍列出而自动重跑；下一执行顺位只读current。

当前轮只首次执行新增PipelineTests及Rust source_seed_五个用例，串行，各预算最多5分钟；不重放上一批41项，不执行全仓/可靠性或Linux实际构建。Python模块为`scripts/linux-product/test_source_first_pipeline.py`；所有合成目录/日志保留，命令执行器仅用本轮新建的假工具和一次打印文本的Python子进程。Rust用例只用FakeDpkg，保留全新合成根，用户数据触碰计数为0。

```bash
./scripts/run-low-impact.sh env PYTHONDONTWRITEBYTECODE=1 python3 scripts/linux-product/test_source_first_pipeline.py --test-root /ABS/registered-new-root/cases
./scripts/run-low-impact.sh env CARGO_NET_OFFLINE=true cargo test --locked --offline -p radishlex-linux-product-install --lib source_seed_ -- --nocapture
./scripts/run-low-impact.sh env CARGO_NET_OFFLINE=true cargo clippy --locked --offline -p radishlex-linux-product-install --lib --tests -- -D warnings
```

Rustfmt、Python AST、shell语法、文档/文本与diff静态检查另执行。macOS主机上的lib/test覆盖typed receipt，不能冒充Linux分支CLI或ARM64实际二进制编译通过。新pipeline已接入未来完整contracts入口，本轮不运行该完整入口。

精确Python用例：

```text
test_actual_addon_script_retains_failed_temporary_build
test_actual_manager_offline_mode_sets_cargo_offline_before_compilation
test_build_failure_runs_once_and_preserves_source_copy_and_phase
test_capture_path_traversal_rejected
test_capture_replacement_during_verifier_is_rejected
test_capture_source_git_provenance_required_before_verifier
test_capture_unknown_file_rejected_before_work
test_collection_panic_preserves_partial_output_and_raw_capture
test_collects_closed_seed_without_exporting_private_receipt
test_command_deadline_exhausted_before_child_creation
test_environment_drift_stops_before_commands
test_existing_publication_never_overwritten
test_host_raw_result_must_match_guest_raw_result
test_invalid_build_budget_has_no_output_or_commands
test_missing_review_baseline_stops_before_work_or_commands
test_post_publish_sync_failure_preserves_output_without_success
test_real_command_runner_retains_one_synthetic_log
test_receipt_operation_cannot_be_substituted
test_receipt_root_identity_must_match_retained_root_capture
test_seed_inode_replacement_during_build_rejected
test_source_artifact_pipeline_uses_existing_steps_without_pair_or_acceptance
test_source_inode_replacement_during_build_rejected_and_retained
test_target_only_pipeline_preserves_source_and_validates_both_actual_artifacts
test_typed_receipt_exit_only_is_not_success
test_unknown_artifact_output_rejected
test_verifier_replacement_during_collection_is_rejected
test_xdg_raw_fingerprints_cannot_claim_unchanged_when_different
```

精确Rust用例：

```text
source_seed_fresh_receipt_projects_full_target_without_private_ids
source_seed_wrong_artifact_and_incomplete_receipt_rejected
source_seed_previous_chain_cannot_be_presented_as_first_install
source_seed_residual_package_configuration_is_not_fresh
source_seed_raw_receipt_requires_original_canonical_bytes
```

2026-11-01 结果：Python原26项首次执行在第1项失败即停止（0.069秒），其余25项未执行。诊断为测试去TMPDIR寻找addon临时根，但既有脚本在输出目录旁建立`.radishlex-addon.*`；stderr与实物确认失败根已正确保留。断言已修正，未重跑。收尾还补了collector读取S的committed Git来源核对及一项未执行负例，当前共27项等待新一轮有界复验。

Rust source_seed_五项首次通过，编译7.12秒、执行0.84秒（119项filtered out，未运行全crate测试）；定向Clippy首次通过，3.19秒。所有调用都报告nice调整被沙盒拒绝，不能声明nice19已应用。计划、日志、失败材料与diagnosis保留在`/private/tmp/radishlex-source-first-pipeline-20261101-2dxwql81/`；五个Rust合成根见rust-tests.log，未清理。

本轮不重放旧41项或失败的Python命令；最终Python实现未取得全集通过，Linux-only CLI分支也未在Linux编译/执行。完整验收及V3–V5保持待批准；先另批一次27项、5分钟预算的Python定向复验，不直接进入真实构建或VM。

## 2026-11-01：批准后的27项Python定向复验

所有者批准新合成根、300秒预算串行一次。执行前21文件摘要与上轮final-result匹配，HEAD仍为fde0ecb。前述run-low-impact与test_source_first_pipeline.py命令使用test-root /private/tmp/radishlex-source-first-pipeline-review-20261101-78nxopac/cases。

本次1.082秒，failfast在第5项停止：4过、1失败、22未执行。第1项已覆盖前次addon目录断言修正；新失败为capture替换负例没有生成预期work/failure.json。只读诊断确定private根/reports为0700、materials为0755、末级materials/receipt为0700；stdlib mkdir(parents=True, mode=0700)只给末级应用该mode。collector按原合同在注入前正确拒绝，并非失败日志丢弃。

夹具改为逐层创建缺失0700父目录，不修改旧失败目录；补source/verifier回调计数与精确异常原因断言，防止其他前置拒绝冒充目标负例覆盖。总数仍27，修正后未重跑，不能认定全集通过。计划、python.log/result、失败根及diagnosis均保留在本次根；上批失败根和Rust材料也不改动。nice仍被沙盒拒绝，不声称调度降级已应用。

本轮未重放旧41项、Rust5项或Clippy，没有完整门禁、VM/真实构建。下一步须另批同27项一次5分钟定向复验，先闭合夹具与预期负例，再考虑完整验收；不可直接认定V1/V2收口或进入V3–V5。

## 2026-11-01：再次批准的27项复验通过

执行前HEAD仍fde0ecb，21文件与上轮final-result逐项摘要匹配，使用新根 /private/tmp/radishlex-source-first-pipeline-review2-20261101-a1uyuhkz/cases；相同指定命令串行一次，27 passed、0 failures/errors，7.847秒，未超300秒。plan、python.log、result及全部合成材料保留；两批原失败根不改写、不清理。

本轮覆盖夹具逐层0700、预期注入及精确拒绝原因、S source装配、T仅构建target/双侧actual verifier调用顺序、closed seed/Rust receipt桥的合成结果、XDG/host读回、读取替换、panic与发布后同步失败保留。实际构建命令是合成执行器，实际receipt子进程被fixture替代；不能冒充真实包/seed、Linux CLI或生产资格通过。nice setpriority仍受沙盒限制，未重放旧41项、Rust/Clippy或完整门禁，未操作VM/安装/全局服务。

## 下一轮完整验收申请（待批准，尚未执行）

必要性：本批跨Rust receipt只读投影、shell离线/失败保留和source-first编排，当前27项与前批Rust/Clippy只提供定向证据；旧41项及全仓交互未在最终快照完整执行。申请对当前HEAD加工作区完整摘要固定快照，串行各一次，第一条通过才执行第二条：

```bash
/usr/bin/time -l env CARGO_NET_OFFLINE=true GOPROXY=off GOTOOLCHAIN=local ./scripts/check-repo.sh
/usr/bin/time -l env CARGO_NET_OFFLINE=true GOPROXY=off GOTOOLCHAIN=local ./scripts/check-repo.sh --component reliability
```

本机macOS，预计热缓存约30–45分钟；停止预算常规20分钟、可靠性45分钟。只沿用预算提案，不沿用历史轮授权；常规五组件及适用22项可靠性，未改变选择。离线既有依赖、不清缓存、不并行重型任务；门禁单层run-low-impact/nice19，Cargo/libtest/Go并发1，Flutter内部线程不构成硬CPU/RSS配额。编译与测试有资源负载，耗时及峰值不作性能承诺。

申请同时覆盖采样、监护以及仅本轮新建验证进程的超时收尾；沙盒nice/权限使验证失真时只申请必要宿主权限，不自行扩大命令。保存前后HEAD、tracked diff、未跟踪清单/文件摘要，保留所有失败根和日志；失败先诊断、不自动重跑。无VM、真实产品构建/安装、冻结现场、输入法/会话或推送动作。批准当前快照后才执行，完整自动通过也不替代V3–V5实机资格。

## 2026-11-01：完整常规失败，可靠性未启动

本轮由所有者明确批准前述两条完整命令、20/45分钟预算、串行各一次，第一条通过才第二条，允许采样/监护和仅本轮验证进程超时收尾，不自动重跑、不推送。HEAD为fde0ecb加21文件，逐项与前轮final-result核对；新证据根 /private/tmp/rlxgate-source-first-20261101-5k0b7p3u/。必要宿主权限落实监护nice0、门禁nice19及Cargo/libtest/Go并发1。

常规109.77秒后exit101，未超1200秒：hygiene、workspace/显式feature Clippy通过；workspace all-targets test在Linux product库122过/9 startup失败处停止。workspace其余target、doc-test、contracts、Go、Manager及22项可靠性没有本轮完成证据；第二条从未启动。前后HEAD/diff/status/untracked摘要完全相同，无监护错误、无遗留自有验证进程，不重跑。

共同失败为ReceiptInvalid/FailedClosed等提前拒绝。监护启动时为保护日志设置了umask077，门禁也继承了；宿主正常022。原startup TestSite.write_receipt仅以mode0644创建文件，没有随后fchmod，在077下实际为0600，而production reader严格要求0644。新建单文件诊断确认0644请求产生0600；没有重跑Rust用例或修改原现场。该环境偏差足以解释相同前门拒绝，纠正后是否通过仍待另批验证，不能推断产品无缺陷。

监护脚本、approved/baseline、baseline.patch、21文件副本、routine.log、state、22次约5秒采样与analysis均保留。采样树RSS峰值585.64MiB、ps CPU相加峰值100.1%，仅是未完成运行的采样，不是硬配额或完整性能结论。九项原startup根已被既有TestSite Drop在panic展开时删除；日志/源码快照仍在，五个新source_seed根保留，未恢复、重建或伪造已保存原receipt。

未改产品/测试实现。准备了未应用的startup-preservation-proposed.patch（panic时保留TestSite根）、未执行的next-monitor-proposal.py（证据监护保持077，只为新gate子进程恢复022并预检）及next-targeted-plan.json（9个明确失败用例各一次，总预算300秒，首失败停）。须新授权后才应用/执行；先有界核验，再申请完整验收，不沿用本轮未使用的可靠性授权。不操作VM/真实构建/安装/冻结材料，未提交或推送。

## 2026-11-01：监护修正与9项startup定向通过

所有者明确批准前述修正及9条exact命令，串行各一次、总预算300秒、失败先诊断。起点dev/fde0ecb加前轮21文件逐项摘要相同，随后仅新增startup/tests/helper.rs的test-only改动：TestSite析构若正在panic则打印路径并保留根，否则沿用原成功清理逻辑；不修改receipt创建/读取权限或生产事务。

新证据根 /private/tmp/rlx-startup-targeted-20261101-115ixjwp/。监护脚本私有umask077；仅新gate子进程通过preexec恢复022，启动前probe严格核对监护nice0、gate nice19/umask022和Cargo/libtest/Go并发1。必要宿主权限用于该定向的降载、监护及仅本轮自有进程的预算收尾，无全局会话变更。

按原9条cargo test --locked --offline --lib、完整名称、--exact/--nocapture依序各一次，9项全部通过，总5.927秒（含预检与编译，阶段合计5.141秒），首条编译2.01秒。逐日志确认每条恰好1 passed、0 failed、123 filtered out，无空选择；每阶段前后快照均匹配，无超时、监护错误或剩余自有进程。正常日志与plan、baseline/源码副本、policy、state及analysis均保留。

这些是default crate的9项定向，不代替前轮workspace统一feature配置、全仓组件或可靠性。新增panic保留分支被Rust编译，但本轮没有故意注入额外panic，未将它写成独立运行验收。原9项失败夹具此前被自动清理的事实不回写，旧失败日志/源码副本、五个source_seed根及全部Python现场仍保留。

已准备基于同一修正前门的corrected-full-monitor-template.py，增加既有证据根拒绝覆盖；这是未执行模板，下一完整轮须复制至新根、绑定新的HEAD/diff/untracked摘要并重新授权。下一步可按上文两条完整命令申请常规20分钟/可靠性45分钟、串行各一次、第一条通过才第二条，预计30–45分钟，采样/监护及仅本轮自有验证进程超时收尾随申请明确授权；不沿用本次9项许可。

## 2026-11-01：完整常规与可靠性通过，本批自动验收收口

所有者明确批准当前快照两条命令，并追问多轮门禁成本。HEAD为dev/fde0ecb加22文件，启动前与9项定向后的final-result全部SHA匹配，领先本地origin/dev 1，未fetch/提交/推送。新根 /private/tmp/rlxgate-source-first-corrected-20261101-58tjbih5/ 保存approved、baseline/diff/22文件副本、监护、policy、两阶段日志/采样、state与analysis；启动拒绝已有证据根，保护旧现场。

| 阶段 | 实际完成 | 耗时与停止预算 |
| --- | --- | --- |
| 常规 | hygiene/Rust/contracts/Go/Manager五组件全部通过；workspace all-targets 609.99秒；source-first基础41项与新pipeline27项通过 | 723.57秒，20分钟内 |
| 可靠性 | 两包feature Clippy、逐target非空清单核对；Executor 2、preparation_history 16、source_preparation 4项全部通过 | 1044.31秒，45分钟内 |

Go race命中缓存，未强制清缓存，不表述为新一轮实际race用例执行。两条串行各一次，常规通过才启动第二条，总1767.88秒（29分27.88秒）。每阶段exit0、前后HEAD/diff/status/untracked摘要一致，无超时、监护异常或剩余自有进程，未重跑/跳项。monitor nice0/umask077，gate单层nice19/umask022，Cargo/libtest/Go并发1，策略前检通过；前轮9个startup在workspace统一feature图也全部通过。

约5秒采样常规141次、可靠性203次，树RSS峰值1672.00/101.02MiB，ps CPU相加峰值431.7%/95.2%；time最大单进程RSS为554.05/55.92MiB。采样可能遗漏短峰，nice及并发1不是CPU/RSS硬配额，Flutter内部并行仍可使用多核；不据缓存/样本差异宣称性能收益或本机5分钟目标已达。

### 为什么此前看起来反复验证

前面的26项、两轮27项及9项是局部验证；本批此前只执行过一轮完整常规，约110秒因Agent把umask077传播到gate而失败，可靠性从未启动。Python的路径断言和中间目录夹具错误也属于Agent执行质量造成的额外成本；不能将这些失误表述为产品每轮都必须全量的理由。旧90e7717完整通过覆盖另一代码快照，不当作当前source-first的新通过。

读取validation_impact.py并对实际路径作只读核验：本批build/validation脚本走保守全局分类，Linux product还经target_os=linux的反向依赖进入ime-ffi可靠性关注包，因此现有选择器要求可靠性；纯文档路径返回false。该PR选择器跨平台依赖判定偏保守，不意味着每个Linux本地小批都应机械执行全部macOS矩阵。本轮保持已批准覆盖，未改选择器或削减22项。impact-audit.json是完整解释；最初impact-explanation.json的原因字符串过于概括，已以audit纠正，原布尔核验值保持。

执行纪律仍按既有门禁手册：日常按风险定向，修复后先做局部复验；稳定阶段集中完整验收，不因文档收尾、提交或耗时未达目标重跑整套，不反复用全量找热点。本批代码验证已收口，只更新4份结果文档并做文档/文本/diff检查；后续回到Linux M5 source实际环境与交付资格，不另开门禁优化或macOS支线。

### 资格限制

本轮是macOS宿主自动验证；Linux-only CLI/ARM64真实产物、实际SQLite/source ID/双FFI、离线环境可行性、fresh seed安装/host收证、R审阅锚点、真实pair、crash/连续L6/P05C与M5退出均未因此通过。没有启动VM/GUI、进行真实产品构建/安装、改冻结材料、下载依赖或推送。此前失败材料和已清理原startup夹具的事实保留，不补写为完整现场。
