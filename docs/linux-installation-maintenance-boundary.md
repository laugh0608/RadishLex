# Linux 安装维护边界

本文定义 M5-P05 的 Linux 产品安装域、Debian 载体、固定布局、版本身份、安装维护事务、数据保留、失败关闭、自动门禁与实机授权边界，读者是 `packaging/linux/`、Linux 平台适配层、安装协调器和发布验证入口的维护者。本文不提供可直接执行的系统安装命令，不定义公开发布流程，也不把 P05A rootfs 装配称为 package 安装成功；Fcitx 运行职责见 [Linux Fcitx5 平台边界](linux-fcitx5-boundary.md)，M5-P04 冻结证据见 [Linux Manager 本地验收边界](linux-manager-local-acceptance.md)。

## 当前结论

截至 2026-08-08，M5-P05A 已完成 committed metadata、system rootfs 装配器、staged/system 两类 addon 构建身份与真实 Debian 13.6 ARM64 Manager/addon 载荷门禁；P05B 已完成确定性 `.deb` artifact、dependency-analysis identity，以及仓库内 receipt/guard、五类 operation 与 fake dpkg port。当前下一批是 Manager/Fcitx 共用的只读 startup decision；真实 dpkg adapter、package mutation、RadishLex 系统安装和 P05C 实机仍未开始。以下决策继续约束实现与后续批次：

- 首个完整产品安装载体固定为 Debian 13 ARM64 的单一系统级本地 `.deb`，package 名固定为 `radishlex`；它是未发布的本地验收载体，不是 apt repository、正式 Release 或通用 Linux 安装包。
- Fcitx addon、两份产品 FFI、Manager bundle、锁定 RimeData、desktop entry、图标和产品 manifest 由同一个 package 绑定；不拆成可独立漂移的 Manager/addon 包。
- 不提供完整用户级程序安装。M5-P02/P04 使用 `FCITX_ADDON_DIRS`、用户级 staging、autostart 副本或 transient service 的装配继续只属于开发/验收环境，不能升级为产品成功路径。
- 用户数据、配置和状态始终留在现有 XDG 用户域；系统 package 和 root 维护脚本不得遍历 home、打开 userdb、改写 privacy/settings、编辑 Fcitx 用户 profile 或清理 P04 guest 资产。
- CJK/Latin 文本字体采用发行版硬依赖，不随 RadishLex package 携带。Debian 13 profile 必须依赖 `fonts-noto-cjk`，并固定兼顾 Latin/数字的 `fonts-dejavu-core`；不能只写 `Recommends` 或依赖桌面环境偶然 fallback。Flutter 生成的 `MaterialIcons-Regular.otf` 只承担图标字形，按下文固定例外处理。
- 首批 distribution identity 固定为 `debian-local-deb-v1`：只证明 Debian package 结构、内容 hash 与本地事务身份，不宣称 repository 签名、发布者认证或公开分发资格。
- 首批 upgrade 与 release rollback 只允许数据 contract 完全相同的 source/target；userdb schema、XDG layout、settings/privacy format、Rime schema 与 RimeData identity 任一变化都在系统 mutation 前失败关闭，等待独立的 Linux 用户态数据协调设计。

M5-P04 最终同库状态继续冻结为 user terms 2、selection events 5、suppressed 0、deleted 1、import batches 2。既有 guest staging、backup、userdb、导入导出文件与临时服务不属于 P05 安装源、回滚源或清理目标。

## 选择系统级程序安装的原因

Fcitx5 的 addon metadata 会按 XDG data 目录查找，但 shared-library addon 使用 Fcitx 构建时的 addon 目录或显式 `FCITX_ADDON_DIRS`。后者是进程环境覆盖，不是可跨 display manager、desktop session 与发行版稳定继承的用户级产品注册机制。首批若继续依赖环境覆盖，就必须接管 Fcitx daemon/autostart 或改写会话环境，既会污染系统输入法生命周期，也无法把 P04 transient service 证据提升为普通安装能力。

因此首批完整产品只进入发行版认可的系统目录：Fcitx shared addon 位于发行版 multiarch addondir，metadata 位于 `/usr/share/fcitx5`，Manager desktop entry 位于 `/usr/share/applications`。程序全局可读不等于用户状态全局共享；每个用户仍使用自己的 XDG 数据与 Fcitx profile。

用户级范围固定为：

- `${XDG_DATA_HOME:-$HOME/.local/share}/radishlex`：userdb、Rime user data 和不可重建的用户数据；
- `${XDG_CONFIG_HOME:-$HOME/.config}/radishlex`：Manager settings 与 `privacy-mode.json`；
- `${XDG_STATE_HOME:-$HOME/.local/state}/radishlex`：用户态持久状态与未来数据事务 receipt；
- `${XDG_CACHE_HOME:-$HOME/.cache}/radishlex`：可重建缓存；
- `${XDG_CONFIG_HOME:-$HOME/.config}/fcitx5/profile`：由 Fcitx 和用户管理，RadishLex package 永不直接改写。

用户级 portable Manager、tarball、AppImage、Flatpak addon 或自行注入 `FCITX_ADDON_DIRS` 均不属于首批支持范围。未来若增加任一载体，必须使用新的 distribution identity、独立布局与真实桌面证据，不能复用 `debian-local-deb-v1` 冒充兼容。

## 字体契约

### 发行版依赖而非随包字体

首个 Debian profile 固定：

- `fonts-dejavu-core` 提供稳定 Latin 与数字基础；
- `fonts-noto-cjk` 提供简体中文覆盖；
- Manager 继续显式声明 Latin 优先、CJK 后备的 family 顺序；
- Fcitx 原生候选 panel 继续使用桌面/Fcitx 字体栈，不由 addon 自绘或私自注册字体。

`radishlex` package 不得携带 CJK、Latin 或数字文本字体，也不得自带 fontconfig 配置或在 maintainer script 中直接调用 `fc-cache`。唯一例外是 Flutter `uses-material-design` 为现有 `Icons.*` 生成的 `data/flutter_assets/fonts/MaterialIcons-Regular.otf`：它只提供图标字形，路径必须精确固定，`FontManifest.json` 只能声明 `MaterialIcons`，并由非空 `NOTICES.Z` 绑定生成资产通知。任何其他 `.ttf`、`.otf`、`.ttc` 仍失败关闭。文本字体安装、升级与缓存刷新由发行版 package 与 dpkg trigger 管理；Manager 和 Fcitx panel 因而继续使用同一系统文本字体能力，不引入 CJK 字体复制、重复许可证或私有注册生命周期。

自动门禁必须同时检查：

- Debian control 的两个字体项均为 hard dependency；
- package payload 除固定 Material Icons 图标字形外不含字体文件，也不含 fontconfig mutation；
- 干净 Debian 13 ARM64 容器/guest 在只安装声明依赖后，fontconfig 能解析预期 family；
- Manager 对“萝卜词核 / RadishLex / ABI v9 / 12345”以及学习计数、状态码和候选解释的实际渲染无缺字。

其他发行版不能只替换 package 名后宣称通过。其 profile 必须重新绑定等价字体 package、family name、实际 glyph coverage 与 Fcitx panel/Manager 渲染证据；若以后改为随包字体，也必须使用新的布局版本并完成来源、hash、许可证、Flutter asset 和系统 panel 边界评审。

## 固定系统布局

`<multiarch>` 来自 package 构建目标和 Debian canonical multiarch tuple；首个 ARM64 值为 `aarch64-linux-gnu`。production manifest 必须保存并复验该值，不能从运行时任意环境变量或调用方参数覆盖。

```text
/usr/lib/<multiarch>/radishlex/manager/
  radishlex_manager
  lib/
    libapp.so
    libflutter_linux_gtk.so
    libradishlex_ime_ffi.so
    <locked Flutter/plugin runtime libraries>
  data/
    flutter_assets/
      FontManifest.json
      NOTICES.Z
      fonts/MaterialIcons-Regular.otf
    icudtl.dat

/usr/lib/<multiarch>/fcitx5/
  radishlex.so
  libradishlex_ime_ffi.so

/usr/share/radishlex/
  product-manifest.json
  rime/
    default.yaml
    radishlex_pinyin.schema.yaml
    pinyin_simp.dict.yaml
    SourceManifest.json
    Licenses/
      rime-pinyin-simp/
        LICENSE
        AUTHORS

/usr/share/fcitx5/
  addon/radishlex.conf
  inputmethod/radishlex.conf

/usr/share/applications/
  dev.radishlex.radishlexManager.desktop

/usr/share/icons/hicolor/scalable/apps/
  dev.radishlex.radishlexManager.svg
  fcitx-radishlex.svg

/usr/share/doc/radishlex/
  copyright

/var/lib/radishlex/install-v1/
  receipt.json
  receipt.json.tmp
  operations/
    <operation-id>/
      target.deb
      target.evidence.json
      source.deb
      source.evidence.json

/run/lock/
  radishlex-install-v1.lock
```

布局规则：

- Manager desktop entry 使用固定 `TryExec`/`Exec=/usr/lib/<multiarch>/radishlex/manager/radishlex_manager`，不经过 shell、wrapper、`PATH`、环境变量或 `current` symlink；application ID 与现有 Linux runner 的 `dev.radishlex.radishlexManager` 精确一致。
- Manager 继续只从 executable 的 canonical parent 加载 sibling `lib/libradishlex_ime_ffi.so`，RPATH 只允许 bundle-relative `$ORIGIN/lib`。
- Fcitx metadata 的 `Library=radishlex` 与 input-method `Addon=radishlex` 保持精确配对；`Version` 必须来自产品版本真相源，不能继续使用 CMake 项目占位版本。
- `radishlex.so` 只以 `$ORIGIN` 加载同目录 FFI；Rime shared data 改由编译进 product layout contract 的 `/usr/share/radishlex/rime` 取得，并通过同一 product manifest 复验，不能读取仓库、工作目录、用户 Rime 或在线资源作为 fallback。
- addon 与 Manager 各自携带一份普通文件形式的 `libradishlex_ime_ffi.so`。两份必须字节相同、ABI 相同且由 manifest 绑定相同 SHA-256，但不得用 hardlink 或 symlink 合并生命周期。
- RimeData 必须从现有 `packaging/rime/product-rime-data.json` 离线装配，包含来源 manifest 与逐资产许可证；P02 仅复制三个 YAML 的 staged layout 不能直接成为产品 payload。
- `librime`、Fcitx5 Core、GTK/Flutter 系统 closure 与字体走 Debian dependency；package 不复制发行版 shared library。首个 profile 至少绑定 Fcitx5 5.1.9+ 与 Debian 13 的 `librime1t64` 1.13.1+，最终 Depends 还必须由 ELF/shlibs 扫描补全。
- package 不创建 `/etc/radishlex`、systemd service、XDG autostart 副本或会话环境文件；首版没有系统级可修改配置。

安装后仍由用户在 Fcitx 配置工具中手动加入 RadishLex，并在授权步骤中手动重启 Fcitx 或重新登录。package 不能自动编辑 profile、选择输入法、启动/停止 daemon 或合成按键。

## 产品与载体身份

仓库根 `version.json` 继续是产品版本/build 的唯一人工真相源。P05A 已建立：

```text
packaging/linux/
  README.md
  product.json
  install-layout.json
  debian/
    control.in
  assets/
    dev.radishlex.radishlexManager.desktop
    radishlex.svg

scripts/linux-product/
  product_metadata.py
  source_contract.py
  rootfs.py
  test_product_metadata.py
  test_rootfs.py
```

`packaging/linux/product.json` 是 Linux 镜像，不可独立改版本；至少固定：

- `product_id=radishlex-linux`、product version、build number；
- manifest/layout format、`distribution_identity=debian-local-deb-v1`；
- Debian package name、package revision、architecture 与 multiarch tuple；
- Fcitx minimum version、librime dependency profile 与 Manager application ID；
- FFI ABI、userdb schema、XDG data layout、settings/privacy format；
- Rime schema ID、RimeData lock SHA-256 与 manifest identity；
- 字体 dependency profile；
- Manager、addon、FFI、RimeData、metadata、desktop entry、icons 与 licenses 的固定 component-to-path 映射；
- 默认 remove 保留全部用户 XDG 数据的语义。

装配后的 `product-manifest.json` 必须以 canonical UTF-8 JSON 绑定上述字段、完整 rootfs 文件 inventory、规范化系统目标路径、size、mode、预期 uid/gid 与 SHA-256。它不保存构建机路径、用户名、时间戳、用户数据、原始 `ldd` 输出或 secret。

本地 `.deb` artifact evidence 另绑定 package 文件名、size、SHA-256、control metadata 与 product manifest SHA-256。`debian-local-deb-v1` 不包含 repository Release/InRelease 签名或开发者证书；任何公开 apt repository、正式下载页或发行签名必须切换新的 distribution identity 并重新评审，不能给现有 identity 增补宣传含义。

package version 固定由 `<product-version>+<build>-<debian-revision>` 形成，例如当前元数据的示意值为 `26.7.1+38-1`；示意值不是 Linux 已发布版本。product version/build、control Version、Fcitx addon Version、Manager version 和 manifest 任一漂移均失败关闭。

## 职责所有者

| owner | 负责 | 不负责 |
| --- | --- | --- |
| `packaging/linux/` | product mirror、install layout、Debian metadata、依赖与字体 profile | 执行 dpkg、读取用户数据、保存构建物或签名凭据 |
| Debian `dpkg`/trigger | package database、dependency、文件 unpack/remove、desktop/icon/system cache 标准生命周期 | RadishLex 业务 receipt、用户 XDG、Fcitx profile 或产品兼容判断 |
| `platforms/linux-product/` | 固定系统路径、artifact 暂存与取证、quiescence port、package state 投影、receipt/guard 与恢复；后续承载真实 dpkg adapter 和 startup decision | input key、Rime 候选、userdb/ranker、Flutter 页面或远端同步 |
| Fcitx addon / Linux Manager host | 在业务初始化前读取同一 startup decision，并验证自身 component identity | 安装、repair、remove、rollback 或改写 root receipt |
| XDG resolver / Rust userdb | 每个有效用户的固定数据路径、权限、schema 与本地业务语义 | 枚举系统用户、解释 dpkg 状态或修改系统程序 |
| 用户/管理员 | 授权 package/system mutation，关闭程序，手动配置/切换输入法 | 通过手工复制或删除 receipt 绕过产品门禁 |

现有 `ime-product-install` 固定 macOS 双 `.app` 名、目录 rename/inode switch 与用户域事务槽位，不能被 Linux adapter 直接调用后声称复用。P05B 可以复用其 operation identity、只追加 receipt、guard、终态和失败关闭原则；若要提取通用类型，必须先去除 macOS 路径假设并保持 M4 全量回归。Linux 的 dpkg 状态与多文件 package mutation 仍由 `platforms/linux-product/` 适配，不为形式统一复制 macOS switch 实现。

## Owner、权限与链接规则

系统程序域固定：

| 对象 | owner/group | mode | 链接规则 |
| --- | --- | --- | --- |
| `/usr` 下 product 目录 | `root:root` | `0755` | 真实目录，祖先不得 group/other writable |
| Manager executable | `root:root` | `0755` | 单 link 普通文件 |
| addon、FFI、Flutter/system-private `.so` | `root:root` | `0644` | 单 link 普通文件，不要求 executable bit |
| RimeData、metadata、desktop、icon、license | `root:root` | `0644` | 单 link 普通文件 |
| `/var/lib/radishlex/install-v1` | `root:root` | `0755` | 只允许 receipt 与固定 `operations` 目录 |
| terminal/nonterminal `receipt.json` | `root:root` | `0644` | 单 link、非敏感、供用户进程只读 gate |
| 写入中的 `receipt.json.tmp` | `root:root` | 创建时 `0600` | fsync 后改为 `0644` 再同目录原子替换；残留即阻断新 operation |
| `operations/<operation-id>` 与本地 package 副本 | `root:root` | 目录 `0700`、文件 `0600` | 只接受固定 source/target 名称与单 link 普通文件 |
| `/run/lock/radishlex-install-v1.lock` | `root:root` | `0600` | root-only Unix socket guard，以精确 inode 释放，不保存持久完成事实 |

所有 RadishLex-owned payload 节点禁止 symlink、hardlink、device、FIFO、socket、setuid/setgid bit、file capability 和 group/other write。外部发行版 shared library 的内部 symlink 由对应 package 管理，不进入 RadishLex tree hash，但 ELF closure 必须解析到已安装 package 的系统 library root，不能落入 `/tmp`、home、仓库或自定义 `LD_LIBRARY_PATH`。

用户 XDG 目录继续为当前用户 `0700`，userdb/settings/privacy 等敏感普通文件继续为 `0600`、单 link、非 symlink；已有对象 owner/mode/link 不合格时失败关闭，不由 root maintainer script 或产品启动顺便 `chown`/`chmod` 修正。系统 receipt 只记录版本、状态、稳定错误和 artifact hash，不记录绝对 home、uid 列表、用户词、P1/P2 内容或进程命令行。

## 安装维护事务

### 外层职责

Debian `dpkg` 负责 package database、依赖关系、文件 unpack/remove 和标准 trigger；RadishLex 维护协调层负责：

- 在 package mutation 前验证 operation、source/target artifact、兼容矩阵、静止条件和固定系统根；
- 以 root-owned receipt/guard 记录可重试状态；
- 把 Debian 的 Installed、Unpacked、Half-Configured、Config-Files 等结果投影为稳定产品状态；
- 在 `postinst` 后复验完整 manifest、ELF closure、metadata 和权限，只有全部通过才写 `completed`；
- 失败时只使用操作开始前已提供并验证的本地 source `.deb` 做 rollback，不在恢复中联网下载或从已安装文件猜造 source package；
- 保持 maintainer script 幂等，正确处理 install/upgrade/downgrade、abort-upgrade、failed-upgrade、remove、abort-remove 与 purge 调用。

直接运行未经过产品前置验证的任意 copy script、把 staged rootfs 当作 installed、或只看 `dpkg` 退出码，都不能形成产品完成证据。协调层不替代 dpkg，也不能改写 dpkg database 冒充回滚。

首批受支持的 mutation 入口只能是 P05B 定义的维护协调器，它接收显式本地 `.deb`、先固定 artifact 与 receipt，再调用 dpkg。直接执行 `dpkg -i`、`dpkg -r` 或等价 apt 旁路即使让 package database 进入 Installed/Not-Installed，也不得由 maintainer script 猜测调用方 artifact 路径或补写 `completed`；product startup gate 必须返回 `maintenance_required`，只能由重新提供精确 artifact 的受控 `repair`/恢复流程收口。

首次 bootstrap 已固定为：取得系统 guard 并复验 state root 后，先让 package port 验证 operation、source/target identity、版本关系与环境前提，再读取明确的 package snapshot；只有 snapshot 与 operation source 一致时才原子写入 `prepared` receipt。随后逐项把调用方 `.deb` 和同名 evidence 复制为 operation 私有 `source`/`target` 文件，记录 size、SHA-256、owner/group、mode、device/inode 与 link count；全部必需 artifact 复验后才进入 `artifacts_staged`。任何 package mutation 都晚于该持久状态与 quiescence evidence，不能由 shell fallback、maintainer script 或调用方临时路径绕过。

### Operation kind

| kind | source | target | 用户数据 | 主要完成条件 |
| --- | --- | --- | --- | --- |
| `install` | package 不存在或 terminal remove | 新本地 `.deb` | 保留既有 XDG；首次无数据也允许 | dpkg Installed、target manifest/closure 全过、receipt completed |
| `upgrade` | 已安装较早 release | 明确允许的较新 `.deb` | data contract 必须完全相同 | source/target 与本地 rollback `.deb` 均取证，target 完成或恢复 source |
| `repair` | 已登记同 release，可有文件缺失/损坏 | 同一 release 的已验证 `.deb` | 零写入 | 完整重装并复验同一 target identity |
| `remove` | 已安装 release | 无程序 target | 默认完整保留 XDG | package files 与 desktop/Fcitx metadata absent、receipt completed |
| `rollback` | 已安装较新或失败中的 target | manifest 允许的较早本地 `.deb` | data contract 必须完全相同 | dpkg 回到精确 source、完整复验、receipt rolled_back/completed |

`rollback` 有两种触发：当前 upgrade 失败后的恢复，以及用户显式选择已声明兼容的旧 release。两者都要求本地 source artifact 已在 mutation 前完成 SHA-256/manifest 取证；“从网络找一个同版本包”、只复制旧 `.so`、只恢复 Manager 或手改 dpkg 状态均不成立。

首批兼容矩阵要求 source/target 的 `ffi_abi=9`、`userdb_schema=9`、`xdg-v1`、settings/privacy format、Rime schema 与完整 RimeData manifest identity 全部相同。允许变化的是产品代码、build、package revision 和不改变数据 contract 的资源。该限制使 upgrade/repair/rollback 都不需要 root 或旧程序打开用户数据库，也避免系统 package 在多用户机器上协调未知 home。

### 静止、guard 与 receipt

系统 mutation 前必须：

1. 复验目标 Debian/architecture/multiarch 与依赖可用性；依赖安装是独立 package-manager 动作，可能需要网络和授权，不由 RadishLex 隐式完成。
2. 拒绝任何正在运行的 RadishLex Manager、任何已加载 RadishLex addon/FFI 的 Fcitx 进程，以及无法可靠判断映射身份的相关进程。
3. 不自动 kill、restart 或跨用户 session 操作；返回稳定 `programs_running`，由用户/管理员另行授权处理。
4. 取得系统级独占 guard，确认没有 nonterminal receipt、`receipt.json.tmp` 或未知 operation 对象。
5. 将 target `.deb`，以及 upgrade/rollback/remove 恢复必需的 source `.deb` 和各自 evidence，复制到 root-owned 私有 operation staging 后以固定 inode 与 receipt hash 复验；不从调用方路径反复读取可替换文件。

稳定状态至少覆盖：

```text
prepared -> artifacts_staged -> quiesced -> package_mutating -> package_verified -> completed

pre-mutation failure -> aborted_preserved
post-mutation uncertainty -> rollback_required
rollback_required -> source_restoring -> source_verified -> rolled_back
```

`completed`、`aborted_preserved` 与 `rolled_back` 是终态。package manager 处于半安装/半配置、receipt 非终态、target/source identity 不确定或 rollback artifact 不可用时，现场保持并阻止 Manager/addon 启动；不能删除 receipt、覆盖错误或把失败降级为 staged/dev mode。

首批不自动清理 operation staging、source package、receipt history 或失败材料。终态清理需要后续固定白名单、身份复验和单独授权，不能成为下一次 operation 的隐式前置动作。

## 数据保留与启动门禁

所有五类 operation 默认对用户 XDG 零写入：

- install/reinstall 可以重新连接既有数据，但必须先证明 manifest 的数据 contract 与现有启动 gate 兼容；
- upgrade/rollback 首批只允许完全相同的数据 contract，不运行 migration；
- repair 不打开 userdb，不重建 settings/privacy，不清 cache；
- remove 与 Debian purge 都不遍历 `/home`、authoritative user home 或自定义 XDG 根；userdb、Rime user data、settings、privacy、state、cache、导入导出文件和 tombstone 全部保留；
- “移除程序并删除用户数据”是未来独立用户态 operation，需要逐用户授权、固定路径、数据库静止与独立 receipt，不属于 package purge。

Manager 与 addon 的 product build 必须在 Flutter engine/settings/userdb 或 Fcitx session/Rime runtime 之前执行只读系统安装 gate：

| 现场 | 结果 |
| --- | --- |
| terminal install/upgrade/repair/rollback 且 running component/manifest 匹配 | 允许继续 |
| 无 system receipt 的明确 development build | 仅允许既有显式开发模式 |
| nonterminal、guard active、tmp receipt、dpkg 非 Installed | `maintenance_required` |
| completed remove、未知 receipt/identity、manifest/hash/owner/link/ABI 漂移 | 失败关闭 |
| 字体或系统 dependency 不满足 | 产品兼容性失败，不宣称安装可用 |

production build 不得把“receipt 缺失”解释为首次启动成功，也不得回退 fixture、仓库 `.so`、`LD_LIBRARY_PATH`、用户级 staging 或在线下载。开发 build 与 product build 的 gate 必须在编译身份上分离。

## 自动门禁

M5-P05 最终需要以下分层证据：

| 编号 | 门禁 | 必须覆盖 |
| --- | --- | --- |
| L1 | Linux metadata | version 镜像、distribution identity、package version/arch/multiarch、ABI/schema/layout、dependency/font profile |
| L2 | rootfs/package layout | 全 inventory、mode、owner 语义、无 symlink/hardlink、desktop/Fcitx metadata、icons/licenses、两份 FFI 同 hash |
| L3 | ELF/RimeData | addon/Manager closure、RPATH、无构建路径、system dependency owner、完整 Rime source/hash/license manifest |
| L4 | transaction contract | 五类 operation、dpkg 调用矩阵、幂等 retry、guard、receipt、crash point、source artifact rollback、unknown state 拒绝 |
| L5 | startup gate | terminal/nonterminal/remove/half-configured/identity drift、product/dev 隔离、业务初始化前失败关闭 |
| L6 | ephemeral Debian matrix | 干净 Debian 13 ARM64 内 install→upgrade→repair→rollback→remove→reinstall，data contract 与默认保留对照 |
| L7 | product regression | Linux addon/Manager、P04 同库 contract、macOS build 38 冻结门禁、文档/文本/仓库检查 |

容器或隔离 rootfs 可以执行 package mutation contract，但不能证明桌面 menu、真实 Fcitx daemon、Wayland/X11、用户 profile、重启或数据保留实机体验。真实系统目录、服务与输入法配置仍只由授权实机批次证明。

P05A 已在 2026-08-06 使用 committed `e1ce740` 的全新源码通过 Debian 13.6 ARM64 L1-L3 rootfs/载荷层：真实 Manager bundle、product-profile addon、双 FFI、ARM64 ELF、RPATH、依赖闭包、ABI symbol、系统字体解析、构建路径与临时 rootfs manifest 均通过。首次真实构建暴露的 Rust/Cargo/C++ 路径泄漏和调用方 `umask=0002` 漂移已在门禁层修复，权限规则未放宽。授权安装的 `fonts-noto-cjk` 只是目标环境 hard dependency，不是 RadishLex package 安装事实。

P05B 载体子批随后以 committed `ce74981` 固定 `debian-local-deb-v1`：ar 成员只允许 `debian-binary`、`control.tar`、`data.tar` 且顺序固定；control/data 使用无压缩 canonical USTAR、epoch 0、root owner/mode，`md5sums` 只作 dpkg 兼容 inventory，产品和 evidence 身份使用 SHA-256。`dpkg-shlibdeps` 在临时 Debian package tree 扫描全部六个 ELF，必须输出单一已解析依赖行；私有未版本化 Flutter/FFI 库与 `libc6` usrmerge 只接受固定诊断 multiset，并独立复验私有 ELF、副本 hash、loader owner 与 diversion，未使用 `--ignore-missing-info` 或伪造公共 shlibs mapping。

同一真实 rootfs 连续两次构建的 `.deb` 与 evidence 均逐字节一致；package SHA-256 为 `b56ba9494e715df847a778a59a09bea2ccef091ce023a596849c13c9f1db27cd`，evidence SHA-256 为 `858fe66515f41b20b29b023c53970a7cb23219575fb077aba2107db3eb045fba`，product manifest SHA-256 仍为 `41b5d98ce0bc77ad2b2138737b5891a7d862c28edec5d7ae603618492a34a6de`。`dpkg-deb` 结构检查通过，guest package database 明确为 `not-installed`。该证据补齐载体输出与 L1-L3 交叉身份，不创建 receipt，不进入 L4-L6，也不证明系统安装。

P05B transaction 子批已建立独立 Rust crate `platforms/linux-product/`。它没有复用 macOS 双 `.app` rename/inode switch，只复用 operation identity、append-only receipt、独占 guard、终态与失败关闭原则；Linux 侧显式投影 Not-Installed、Config-Files、Installed、Unpacked、Half-Configured、Half-Installed 与 trigger 状态。收据使用 canonical JSON、固定 root identity、最多 128 项 operation chain、稳定 failure code、source/target package proof 和不自动清理的私有 staging；相同证据写入可幂等重试，只有终态才能追加下一 operation。

`DpkgTransactionPort` 当前只由合成 fake 实现。自动合同覆盖五类 operation、install→upgrade 连续收据链、repair 重装、remove absence、显式 rollback、运行进程前置中止、active guard、未知 package state、暂存篡改/hardlink、package mutation 中断、source restore 再次中断，以及在 target proof 已持久化后的无重复 mutation 恢复；测试 API 不接收 XDG 或 home 路径。稳定入口为 `./scripts/check-linux-package-transaction.sh`，并已纳入仓库门禁。该证据形成 L4 的平台无关事务核心，但不证明真实 dpkg 命令、Debian maintainer-script 调用、系统 owner 映射、L5 startup gate 或 L6 matrix。

## M5-P05 实现状态

`M5-P05A` 只建立可复验的 metadata 与 rootfs assembly，不安装 `.deb`。当前实现为：

1. 新建 `packaging/linux/`，固定 `product.json`、`install-layout.json`、Debian control template、字体/dependency profile 与文档入口。
2. `build-linux-product-addon-stage.sh` 以 CMake `system` profile 生成不携带 sibling RimeData 的 addon/FFI/metadata stage；`rootfs.py` 再从显式 Manager Release bundle、该 stage 和锁定 RimeData 形成临时 `DESTDIR` rootfs，不写宿主 `/usr`、`/var`、user XDG 或 guest。
3. 生成 canonical Linux product manifest，复验 version/build、完整 inventory、mode/link、双 FFI hash、Fcitx metadata、desktop entry、ELF closure 与 RimeData/license。
4. 稳定入口 `./scripts/check-linux-product-metadata.sh` 与 `./scripts/check-linux-product-layout.sh` 已加入仓库门禁，覆盖缺字体 dependency、错误 multiarch、版本漂移、缺文件、宽权限、symlink/hardlink、FFI 不同、RimeData/license 漂移和构建路径泄漏。
5. 保留 `./scripts/check-linux-fcitx5.sh` 与 `./scripts/check-manager-linux-product.sh` 的开发/staged 职责；新门禁不能把二者改名为安装，也不能执行 `dpkg`、启动 GUI/Fcitx 或修改系统。

P05A 的完成只证明同一 committed 产品输入能形成 Debian 目标布局并受负向门禁约束，不证明 package transaction 或系统安装。P05B 的确定性 `.deb` artifact、manifest/依赖身份与平台无关事务核心已经完成；后续按“Manager/Fcitx 共用 startup decision → 真实 dpkg adapter 与隔离 Debian install/upgrade/repair/rollback/remove/reinstall matrix”顺序推进，并继续覆盖完整 crash/retry、source artifact rollback、默认数据保留与 unknown state 拒绝。P05C 才在独立 guest 完成授权实机，不复用或清理 P04 冻结现场。

## 实机授权边界

无需实机授权即可执行：

- 读取规范、生成 metadata、在仓库临时目录或容器内构建 rootfs/package；
- 对合成 package、合成 XDG 根和合成 receipt 运行自动 contract；
- 只读检查本地 artifact 与 Git 状态。

必须另行明确授权：

- 安装依赖、调用 apt/dpkg 修改任何真实 rootfs、写 `/usr` 或 `/var`；
- 启动、停止、重启 Fcitx/Manager、修改 systemd/user service 或 desktop session；
- 通过 Fcitx 配置工具添加/移除输入法、切换 source、注销或重启；
- 对真实系统执行 repair、remove、rollback 或任何故障注入；
- 清理 P04 guest staging、backup、userdb、导入导出文件、临时服务或旧 operation 材料；
- 上传 package、推送远端、创建 tag/Release、repository metadata 或签名。

首个实机环境必须使用 P04 guest 的独立 clone/snapshot 或另一台 Debian 13 ARM64 guest，并使用新的合成 XDG 数据。AI 只做已授权的构建、系统写入与只读监视；用户负责关闭应用、手动配置/切换输入法和实体输入。每次只推进一组可回退步骤，任何显示、receipt、package state 或数据身份冲突都先停止并保留现场。

## M5-P05 退出标准

- 字体 dependency、系统/用户域、完整布局、产品身份和 dependency closure 均由 committed metadata 与负向门禁固定；
- install、upgrade、repair、remove、rollback 在 Debian 13 ARM64 ephemeral matrix 中具有幂等、故障注入、恢复和 startup gate 证据；
- 独立真实 guest 证明安装、手动启用、离线输入、Manager、同库数据保留、upgrade、repair、rollback、默认 remove 与 reinstall；
- 默认 remove 后用户 XDG 数据、tombstone 与 privacy/settings 保持，程序和 desktop/Fcitx metadata 精确 absent；
- 未运行或改写真实用户同步，未自动改 Fcitx profile/autostart，未清理 P04 资产；
- macOS build 38 冻结参考与仓库门禁继续通过；
- package 仍是 `debian-local-deb-v1` 本地验收载体，未创建公开发布事实。

## 规范依据

- [Fcitx5 addon 结构与安装目录](https://fcitx-im.org/wiki/Develop_an_simple_input_method)
- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir/latest/)
- [Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry/latest-single/)
- [Debian Policy：maintainer scripts 与安装状态](https://www.debian.org/doc/debian-policy/ch-maintainerscripts.html)
- [Debian Policy：文件、私有 shared object 与配置边界](https://www.debian.org/doc/debian-policy/ch-files.html)
- [Debian Policy：desktop entry 与 dpkg trigger](https://www.debian.org/doc/debian-policy/ch-opersys.html)
- [Debian 13 `fonts-noto-cjk` 文件与版本](https://packages.debian.org/trixie/all/fonts-noto-cjk/filelist)
- [Debian 13 librime source/profile](https://packages.debian.org/source/trixie/librime)
