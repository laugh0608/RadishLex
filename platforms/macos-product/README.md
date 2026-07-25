# macOS 产品平台宿主与安装适配说明

本文说明 `platforms/macos-product/` 内的平台宿主、固定输入、输出与验证边界，面向维护 M4 数据升级协调器、程序安装事务、Manager/InputMethod 产品构建和仓库门禁的开发者。本文不包含真实用户目录演练、Installer UI、进程停止授权、Developer ID 凭据或公证步骤；数据状态机见 [macOS 数据升级协调器边界](../../docs/macos-data-upgrade-coordinator.md)，程序事务见 [macOS 程序安装事务](../../docs/macos-installation-transaction.md)。

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
```

平台宿主只吸收 macOS 路径解析、Foundation/AppKit 进程与容量 API、bundle 资源定位和 native executable 生命周期。receipt、文件身份、状态转换和候选证据属于 `ime-product-upgrade`；SQLite schema/migration 属于 `ime-userdb`；宿主不得成为新的业务真相源。

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

post-switch 模式改为读取固定 `userdb.sqlite3` 与 `manager-settings.json`。两种模式都通过 ABI v8 执行 current-schema 只读连接、active/deleted/import/learning 管理查询和 settings format v1 类型兼容检查。

InputMethod host 按同一模式选择 candidate 或最终固定数据库，并从自身 bundle 解析 `Resources/RimeData`、schema 和 native library。它创建短生命周期 `0700` 临时 Rime user data，只把 bundle 内锁定 YAML 的部署产物写入该目录，不修改只读产品 `RimeData`、candidate 或 Application Support；随后以 privacy mode 创建 personalized runtime、输入固定合成码并读取候选信号，不选择、不提交、不学习。临时 Rime data 必须在退出前删除。

两端都在调用前后比较目标数据库全字节，并在调用前后拒绝 `-wal`、`-shm`、`-journal`。非法参数、目标缺失/损坏、summary version/check bit 不匹配、数据库字节变化、sidecar 或临时目录清理失败都返回非零。

validation host 不是普通用户工具，也不是协调器本身。它只产生当前进程的受控 validation summary；只有持有 upgrade guard 的协调核心可以把两端结果转换为 validation evidence，并按当前 receipt 阶段持久化 `candidate_verified`、`aborted_preserved`、`post_switch_verified` 或 `rollback_required`。

## UpgradeCoordinatorAdapter

adapter 实现 `ime-product-upgrade::UpgradeCoordinatorPort`，但不接收任意 executable 或数据路径。构造时只接收 source/target 产品装配根；内部严格解析两份 `ProductManifest.json`，固定定位两个 component 和 `Contents/Helpers`，并在每次调用前复验 helper 的普通文件身份、长度与 manifest SHA-256。

checkpoint 一律调用 target Manager 内的 `RadishLexUpgradePreflightHost`。candidate 与最终路径验证调用 target 双端 validation host；回滚恢复验证调用 source 双端 validation host。release、build、userdb schema 或 data layout 与 receipt 不一致时，不启动任何 validation host。进程输出不进入 receipt 或日志，执行有固定时限，超时会终止对应 helper 并按未取得 evidence 处理。

M4-P02 的 manifest 绑定只解决“执行哪一代、哪一端产品代码”的内容确定性。安装载体仍须在 M4-P03 证明产品根来源、Developer ID 签名、公证和固定安装位置，不能把调用方传入的任意目录直接当作可信产品。

`qualification-harness` Cargo feature 仅供隔离产品协调门禁。它要求 canonical temp 根下的固定 marker 与 `0700` 合成 user home，并只对子进程设置 `CFFIXED_USER_HOME`；普通 `load` 始终清除该变量。资格场景使用真实 manifest-bound helper，故障只在 helper 返回后的 port 结果边界注入。source qualification 与 target 使用同一份当前 native code/schema，但具有独立 bundle 版本、重新签名和 manifest，因此只证明产品路由与恢复编排，不替代历史 source binary 或旧 schema migration 测试。

## InstallAdapter

adapter 组合 `ime-product-install`，但不接受自定义最终路径、bundle 名或数据路径。构造时要求 authoritative current-user home、uid、InstallPayload 根，以及 Manager/InputMethod 各自的 Developer ID designated requirement 和同一 Team ID。它逐字节绑定 committed install layout，严格复验 payload/product manifest、完整 bundle tree、许可证和 component-to-target 映射。

production code identity 使用 `/usr/bin/codesign --verify --deep --strict -R=<requirement>`，再从固定 Identifier、TeamIdentifier、CDHash、Signature、CodeDirectory 和 designated requirement 形成脱敏 SHA-256；原始输出不进入 receipt、日志或错误。没有冻结发布要求时只能运行注入合成 verifier 的单元测试，当前 ad-hoc 产品不自动获得发布资格。

staging 使用 `/usr/bin/ditto` 保留 resource fork、extended attributes、ACL、quarantine 和 HFS compression。复制前后都复验 payload target，复制后对 staged tree/code identity 重新形成与 receipt target 相同的逻辑身份，递归 `fsync` 后才调用核心记录 filesystem evidence。完整但未记录的 staged bundle 可以在重启后补记；部分或漂移对象保持现场，不覆盖、不自动清理。

## InstallCoordinatorAdapter

协调组合层依赖 `ime-product-install` 与 `ime-product-upgrade`，但两个核心不互相依赖。它只接受当前已持久化的两个 receipt、两个 guard、固定 Manager/InputMethod `ProgramSwitchStore`、M4-P02 port 和 `InstallAdapter` 实现的程序身份 port；operation ID、data-root identity、source/target release 或 component binding 任一不一致时，在数据写入前失败。

进入 `data_coordinating` 后，M4-P02 的每个 quiescence checkpoint 都附带 installed target 双 bundle 复验。数据 `completed` 才推进外层 `data_settled`；数据 `aborted_preserved` / `rolled_back` 则先持久化外层 `rollback_required`，恢复 source 双程序并重复验证 tree/code identity，最后进入外层 `rolled_back`。中断或暂不可验证时保留两个 receipt 与全部 staging/backup，不清理现场。

## 构建与验证

Manager helper 由 Xcode native library 嵌入阶段构建并签名：

```bash
./scripts/check-manager-product.sh
```

InputMethod helper 由 bundle 构建入口装配并签名：

```bash
./scripts/check-macos-imk.sh
```

完整协调资格会重新装配真实 Manager/InputMethod 产品和独立 source/target manifest，在合成 Application Support 上运行成功、双端失败、静止丢失、回滚及重启恢复：

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
