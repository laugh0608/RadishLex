# macOS 产品平台宿主与安装适配说明

本文说明 `platforms/macos-product/` 内的平台宿主、固定输入、输出与验证边界，面向维护 M4 数据升级协调器、程序安装事务、Installer、Manager/InputMethod 产品构建和仓库门禁的开发者。本文不包含真实用户目录演练、进程停止授权、Developer ID 凭据或公证步骤；数据状态机见 [macOS 数据升级协调器边界](../../docs/macos-data-upgrade-coordinator.md)，程序事务见 [macOS 程序安装事务](../../docs/macos-installation-transaction.md)，Installer UI/驱动见 [macOS Installer App 边界](../../docs/macos-installer-app-boundary.md)。

## 目录职责

```text
platforms/macos-product/
  UpgradePreflightHost/
    Sources/                 fixed-path read-only preflight
    Tests/                   synthetic contract host
    build.sh
    check.sh
  UpgradeValidationHosts/
    Sources/
      RLXUpgradeValidationSupport.*
      manager_main.m
      input_method_main.m
  UpgradeCoordinatorAdapter/
    src/                     manifest-bound Rust platform port
    Cargo.toml
  InstallAdapter/
    src/                     layout/manifest/signature/staging platform port
    Cargo.toml
  InstallCoordinatorAdapter/
    src/                     install/data receipt and rollback composition
    Cargo.toml
  InstallerDriver/
    src/                     read-only snapshot and action authorization
    Cargo.toml
  InstallerExecutor/
    src/                     authorized intent to restartable transaction execution
    Cargo.toml
  InstallerBridge/
    include/                 versioned native ABI
    src/                     fresh authorization and executor dispatch
    Cargo.toml
  InstallerApp/
    Sources/                 independent AppKit presentation shell
    Tests/                   snapshot and fail-closed contract
    build.sh
    check.sh
```

平台宿主只吸收 macOS 路径解析、Foundation/AppKit 进程与容量 API、bundle 资源定位和 native executable 生命周期。receipt、文件身份、状态转换和候选证据属于 `ime-product-upgrade`；SQLite schema/migration 属于 `ime-userdb`；宿主不得成为新的业务真相源。

发布载体入口位于 `scripts/`：`build-macos-release-installer.sh` 冻结 Developer ID Installer，`build-macos-release-dmg.sh` 生成并复验签名 APFS/UDZO DMG，`notarize-macos-release-dmg.sh` 完成 notary log、staple 与双层 Gatekeeper 资格。纯解析、证据编码和漂移拒绝集中在 `scripts/macos-product/release_carrier.py`，不进入 Installer runtime，也不读取用户数据。真实凭据、上传和挂载只在专用发布 runbook 获授权后执行。

## UpgradePreflightHost

生产 executable 不接受参数，只从用户域 `Application Support/RadishLex` 解析固定 data root，并使用从 `packaging/macos/product.json` 编译注入的 Manager/InputMethod bundle ID。它执行：

- 同时读取 important-usage 与普通 volume available capacity，取可用正值中的保守值；
- 检查 data root 为当前用户所有的 canonical `0700` 目录；
- 检查现存 userdb family/settings 为当前用户所有、`0600`、单 link 普通文件；
- 通过 `NSRunningApplication` 检查固定双 bundle ID；
- 通过固定 `/usr/sbin/lsof` 只读检查受控 SQLite/settings 文件是否仍被打开。

输出为 `radishlex-upgrade-preflight-v1` JSON，只包含 result、available bytes、quiescent、固定 blocker/error 和必要数值错误码，不输出路径、PID、命令行或 `lsof` 正文。host 不创建目录、不 chmod、不清理文件、不停止进程，也不能阻止旧产品在点时检查后重新启动；持续静止必须由 startup gate 与 upgrade guard 共同保证。

`./scripts/check-macos-upgrade-preflight.sh` 构建生产 host，但只执行拒绝参数路径；真实检查逻辑由合成 contract executable 在私有临时目录验证。普通门禁不得无参数运行生产 host，因为那会读取真实 Application Support 的只读状态。

## UpgradeValidationHosts

Manager 与 InputMethod bundle 各自把名为 `Contents/Helpers/RadishLexUpgradeValidationHost` 的 executable 嵌入产品。两端名称相同但实现和 native dependency 不同，不能互换或由一个通用 host 代替。无参数模式固定验证 migration candidate；唯一允许的参数 `--post-switch` 固定验证最终 `userdb.sqlite3`。任何路径参数或其他模式都拒绝。

Manager host 的 candidate 模式固定读取：

- `.radishlex-upgrade-v1/migration-candidate.sqlite3`；
- `.radishlex-upgrade-v1/source-settings.json`（允许不存在）；
- 本 bundle 的 `libradishlex_ime_ffi.dylib`。

post-switch 模式改为读取固定 `userdb.sqlite3` 与 `manager-settings.json`。两种模式都通过 ABI v9 执行 current-schema 只读连接、active/deleted/import/learning 管理查询和 settings format v1 类型兼容检查。

InputMethod host 按同一模式选择 candidate 或最终固定数据库，并从自身 bundle 解析 `Resources/RimeData`、schema 和 native library。它创建短生命周期 `0700` 临时 Rime user data，只把 bundle 内锁定 YAML 的部署产物写入该目录，不修改只读产品 `RimeData`、candidate 或 Application Support；随后以 privacy mode 创建 personalized runtime、输入固定合成码并读取候选信号，不选择、不提交、不学习。临时 Rime data 必须在退出前删除。

两端都在调用前后比较目标数据库全字节，并在调用前后拒绝 `-wal`、`-shm`、`-journal`。非法参数、目标缺失/损坏、summary version/check bit 不匹配、数据库字节变化、sidecar 或临时目录清理失败都返回非零。

validation host 不是普通用户工具，也不是协调器本身。它只产生当前进程的受控 validation summary；只有持有 upgrade guard 的协调核心可以把两端结果转换为 validation evidence，并按当前 receipt 阶段持久化 `candidate_verified`、`aborted_preserved`、`post_switch_verified` 或 `rollback_required`。

## UpgradeCoordinatorAdapter

adapter 实现 `ime-product-upgrade::UpgradeCoordinatorPort`，但不接收任意 executable 或数据路径。构造时只接收 source/target 产品装配根；内部严格解析两份 `ProductManifest.json`，固定定位两个 component 和 `Contents/Helpers`，并在每次调用前复验 helper 的普通文件身份、长度与 manifest SHA-256。

checkpoint 一律调用 target Manager 内的 `RadishLexUpgradePreflightHost`。candidate 与最终路径验证调用 target 双端 validation host；回滚恢复验证调用 source 双端 validation host。release、build、userdb schema 或 data layout 与 receipt 不一致时，不启动任何 validation host。进程输出不进入 receipt 或日志，执行有固定时限，超时会终止对应 helper 并按未取得 evidence 处理。

M4-P02 的 manifest 绑定只解决“执行哪一代、哪一端产品代码”的内容确定性。安装载体仍须在 M4-P03 证明产品根来源、Developer ID 签名、公证和固定安装位置，不能把调用方传入的任意目录直接当作可信产品。

`qualification-harness` Cargo feature 仅供隔离产品协调与安装恢复门禁。它要求 canonical temp 根下的固定 marker 与 `0700` 合成 user home；升级 host 只对子进程设置 `CFFIXED_USER_HOME`，普通 `load` 始终清除该变量。安装 adapter 还要求 payload/home 都是该根内无 symlink、同 owner、精确 mode 的后代，并只在此边界接受严格 ad-hoc code identity；production Developer ID requirement 不提供 fallback。资格场景使用真实 manifest-bound helper/bundle，故障只在 port 结果边界注入。source qualification 与 target 使用同一份当前 native code/schema，但具有独立 bundle 版本、重新签名、ProductManifest 和 InstallPayload，因此只证明产品路由与恢复编排，不替代历史 source binary 或旧 schema migration 测试。

## InstallAdapter

adapter 组合 `ime-product-install`，但不接受自定义最终路径、bundle 名或数据路径。构造时要求 authoritative current-user home、uid、InstallPayload 根，以及 Manager/InputMethod 各自的 Developer ID designated requirement 和同一 Team ID。它逐字节绑定 committed install layout，严格复验 payload format v2、target 与全部 `UpgradeSources` 的 product manifest、完整 bundle tree、许可证、release 顺序、code identity 和 component-to-target 映射。

production code identity 使用 `/usr/bin/codesign --verify --deep --strict -R=<requirement>`，再从固定 Identifier、TeamIdentifier、CDHash、Signature、CodeDirectory 和 designated requirement 形成脱敏 SHA-256；原始输出不进入 receipt、日志或错误。没有冻结发布要求时只能运行注入合成 verifier 的单元测试，当前 ad-hoc 产品不自动获得发布资格。

staging 使用 `/usr/bin/ditto` 保留 resource fork、extended attributes、ACL、quarantine 和 HFS compression。复制前后都复验 payload target，复制后对 staged tree/code identity 重新形成与 receipt target 相同的逻辑身份，递归 `fsync` 后才调用核心记录 filesystem evidence。完整但未记录的 staged bundle 可以在重启后补记；部分或漂移对象保持现场，不覆盖、不自动清理。

## InstallCoordinatorAdapter

协调组合层依赖 `ime-product-install` 与 `ime-product-upgrade`，但两个核心不互相依赖。它只接受当前已持久化的两个 receipt、两个 guard、固定 Manager/InputMethod `ProgramSwitchStore`、M4-P02 port 和 `InstallAdapter` 实现的程序身份 port；operation ID、data-root identity、source/target release 或 component binding 任一不一致时，在数据写入前失败。

进入 `data_coordinating` 后，M4-P02 的每个 quiescence checkpoint 都附带 installed target 双 bundle 复验。数据 `completed` 才推进外层 `data_settled`；数据 `aborted_preserved` / `rolled_back` 则先持久化外层 `rollback_required`，恢复 source 双程序并重复验证 tree/code identity，最后进入外层 `rolled_back`。中断或暂不可验证时保留两个 receipt 与全部 staging/backup，不清理现场。

upgrade 产品终态在 `final_verified` 与 `completed` 前分别复验外层/data receipt、双 guard、Application Support identity、source/target release、data `completed` 和 installed 双 bundle。ABI v9 外层 startup gate 不接收运行 identity；它只从当前 executable 所在固定用户域 bundle 形成 Info.plist release、完整 tree 与 Developer ID code identity，并在 Manager/InputMethod 的既有数据 gate 和全部业务初始化之前执行。

## InstallerExecutor

执行器只接受 `InstallerDriver` 已授权的 intent，但仍在 guard 内重新读取 current receipt 并执行 manifest-bound preflight。begin/retry/remove 先持久化 `prepared` 并返回；用户重新确认静止后才进入 `quiesced` 和程序 mutation。first install、repair、remove 共用外层程序终态；upgrade 从外层 receipt 和固定 data root bootstrap 或重绑同 operation 的 M4-P02 receipt，并从任一已持久化状态继续数据协调、程序恢复或两段终态。

执行器不接受 UI 路径、`HOME`、release、bundle identity 或 available bytes 自报值。随机 operation ID 来自系统熵；active guard、stale intent、缺失 upgrade context、preflight/receipt/identity 失败均关闭。

## InstallerBridge

ABI v1 固定整数 enum、POD snapshot 与 contract/snapshot/perform 三个 symbol。AppKit 已静态链接并实际调用；Objective-C 不解释 receipt，只把已知 enum 映射为 driver snapshot。Rust dispatch 每次重新投影并授权 fresh action，再调用 executor。隔离测试证明 prepared/restart/stale/active guard。

生产只读 bootstrap 通过 `geteuid/getpwuid_r` 取得 authoritative current-user home，从当前 executable 固定推导 Installer resources，并严格读取 sealed `ReleaseIdentity.json` 与内嵌 InstallPayload。resource 同时绑定 Installer/Manager/InputMethod exact Developer ID requirement 与同一 Team ID，Installer 自身先过 strict signature 验证；ad-hoc 构建不携带 release identity，稳定返回 `product_identity_unavailable`。身份与 payload 通过后四类 operation 都进入真实 executor；upgrade 从外层 receipt 选择 exact release 的 `UpgradeSources/<version>-<build>`，并在 dispatch 前构造 `MacOsUpgradeCoordinatorAdapter`。缺失 source 返回 `driver_unavailable`；source manifest/tree/signature、release 顺序或 helper 漂移返回产品身份阻断。

`./scripts/build-macos-release-installer.sh` 只接受本机有效的 `RADISHLEX_DEVELOPER_ID_APPLICATION`，按嵌套 Mach-O、code container、产品 bundle、manifest、Installer 初签、release identity、Installer 终签顺序启用 Hardened Runtime 与 trusted timestamp。可重复传入 `--upgrade-source-product-root <historical-product-root>`；每个 source 必须是严格签名的真实历史 assembly，双 component designated requirement 与当前 target 精确一致，且 payload 工具要求 build 唯一并早于 target。未传入时稳定生成空 `UpgradeSources`，不能升级已有旧版。默认门禁只验证失败关闭和脚本契约；没有本机身份时不生成假 Team ID 或成功证据。

## 构建与验证

Installer 的只读驱动、restartable executor 和独立 AppKit 壳使用专用门禁。它验证四类 operation、restart snapshot、stale action、preflight/active guard 阻断、staging 与 `final_verified` 中断续跑、remove 数据保留授权、未知结果失败关闭、固定 bundle metadata/layout 和禁止 UI 跨越 receipt/TIS/路径边界；不会启动 GUI 或执行真实安装：

```bash
./scripts/check-macos-installer.sh
```

Manager helper 由 Xcode native library 嵌入阶段构建并签名：

```bash
./scripts/check-manager-product.sh
```

InputMethod helper 由 bundle 构建入口装配并签名：

```bash
./scripts/check-macos-imk.sh
```

完整协调资格会重新装配真实 Manager/InputMethod 产品、独立 source/target manifest 与 InstallPayload。它先运行数据协调既有场景，再在合成 Application Support 上串联程序切换、数据协调、两段终态、startup gate、source 回滚和部分程序提交重启：

```bash
./scripts/check-macos-upgrade-product-coordination.sh
```

安装适配器的普通门禁只使用合成 home、payload、bundle 和注入 verifier/copy port，不调用真实签名身份或用户目录：

```bash
./scripts/check-macos-install-adapter.sh
```

外层/data receipt 组合门禁使用同一合成数据根、双 bundle 和 userdb，验证成功、失败与重启恢复：

```bash
./scripts/check-macos-install-coordinator.sh
```

两个产品门禁都会先执行 `UpgradeValidationHosts/check.sh`，验证仅允许的两种参数形式和固定路径映射，并拒绝任意路径及多余参数。带真实 native Rime 的候选信号验证仍需使用隔离的 locked RimeData 和产品门禁；不得把 `RADISHLEX_RIME_SHARED_DATA` 指向用户 Rime 或 RadishLex Application Support。上述普通检查不安装、不启动真实 Manager/InputMethod，也不调度 validation host 访问真实 candidate。
