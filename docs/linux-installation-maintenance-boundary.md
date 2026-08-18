# Linux 安装维护边界

本文定义 M5-P05 的 Linux 产品安装域、Debian 载体、固定布局、版本身份、安装维护事务、数据保留、失败关闭、自动门禁与实机授权边界，读者是 `packaging/linux/`、Linux 平台适配层、安装协调器和发布验证入口的维护者。本文不提供可直接执行的系统安装命令，不定义公开发布流程，也不把 P05A rootfs 装配称为 package 安装成功；Fcitx 运行职责见 [Linux Fcitx5 平台边界](linux-fcitx5-boundary.md)，M5-P04 冻结证据见 [Linux Manager 本地验收边界](linux-manager-local-acceptance.md)。

## 当前结论

截至 2026-08-18，M5-P05A 已完成 metadata/rootfs、双 addon 构建身份与真实 Debian 13.6 ARM64 载荷门禁。P05B 已完成确定性 `.deb`、actual package streaming relationship、恢复型 receipt/advisory guard、fixed-path observer/executor、concrete mutable `DpkgTransactionPort`、`/proc` quiescence、opaque authorized CLI、startup dependency 连接与 fake command/crash matrix。第六套与独立clone已让六类真实operation分别取证。首个`install_prepared` acceptance checkpoint暴露的合法`root:root 01777 /run/lock`误报已修复，target `d75818f`的新pair已冻结；其两台retry又分别因input harness与fresh-absent startup预期错误在transaction前失败关闭。连续完整L6、八个crash/retry与P05C仍未闭合。

- 首个完整产品安装载体固定为 Debian 13 ARM64 的单一系统级本地 `.deb`，package 名固定为 `radishlex`；它是未发布的本地验收载体，不是 apt repository、正式 Release 或通用 Linux 安装包。
- Fcitx addon、两份产品 FFI、Manager bundle、锁定 RimeData、desktop entry、图标和产品 manifest 由同一个 package 绑定；不拆成可独立漂移的 Manager/addon 包。
- 不提供完整用户级程序安装。M5-P02/P04 使用 `FCITX_ADDON_DIRS`、用户级 staging、autostart 副本或 transient service 的装配继续只属于开发/验收环境，不能升级为产品成功路径。
- 用户数据、配置和状态始终留在现有 XDG 用户域；系统 package 和 root 维护脚本不得遍历 home、打开 userdb、改写 privacy/settings、编辑 Fcitx 用户 profile 或清理 P04 guest 资产。
- CJK/Latin 文本字体采用发行版硬依赖，不随 RadishLex package 携带。Debian 13 profile 必须依赖 `fonts-noto-cjk`，并固定兼顾 Latin/数字的 `fonts-dejavu-core`；不能只写 `Recommends` 或依赖桌面环境偶然 fallback。Flutter 生成的 `MaterialIcons-Regular.otf` 只承担图标字形，按下文固定例外处理。
- 首批 distribution identity 固定为 `debian-local-deb-v1`：只证明 Debian package 结构、内容 hash 与本地事务身份，不宣称 repository 签名、发布者认证或公开分发资格。
- 首批 upgrade 与 release rollback 只允许数据 contract 完全相同的 source/target；userdb schema、XDG layout、settings/privacy format、Rime schema 与 RimeData identity 任一变化都在系统 mutation 前失败关闭，等待独立的 Linux 用户态数据协调设计。
- v1 package 不携带 RadishLex 自有 `preinst`/`postinst`/`prerm`/`postrm`、`config`、`templates`、`conffiles` 或 `triggers`；外部 dependency 的 scripts/triggers 只作为 dpkg lifecycle observation，不能写产品 receipt 或冒充事务完成。

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

package version 固定由 `<product-version>+<build>-<debian-revision>` 形成；L6 source/target 当前分别为 `26.7.1+38-1` 与 `26.7.1+38-2`，二者都是未发布的本地验收身份。product version/build、control Version、Fcitx addon Version、Manager version 和 manifest 任一漂移均失败关闭。

## 职责所有者

| owner | 负责 | 不负责 |
| --- | --- | --- |
| `packaging/linux/` | product mirror、install layout、Debian metadata、依赖与字体 profile | 执行 dpkg、读取用户数据、保存构建物或签名凭据 |
| Debian `dpkg`/trigger | package database、dependency、文件 unpack/remove、desktop/icon/system cache 标准生命周期 | RadishLex 业务 receipt、用户 XDG、Fcitx profile 或产品兼容判断 |
| `platforms/linux-product/` | actual `.deb`/manifest/dependency/version/status 关系校验、artifact staging、receipt/advisory guard、恢复、production observer/executor/mutable port、`/proc` quiescence、authorized CLI 与共用只读 startup decision | input key、Rime 候选、userdb/ranker、Flutter 页面、远端同步或用户 XDG mutation |
| `platforms/linux-l6-acceptance/` | compile-isolated 八点 checkpoint、完整测试 process group 终止证明与 canonical 脱敏 evidence | production 维护入口、dpkg/path override、VM 生命周期、probe 原文、用户数据或完成事实 |
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
| `/run/lock/radishlex-install-v1.lock` | `root:root` | `0600` | 零长度、单 link regular file 上的 advisory exclusive lock；以精确 inode 释放，不保存完成事实；父目录只接受非 world-writable 或 root-owned 精确 `01777` sticky mode |

所有 RadishLex-owned payload 节点禁止 symlink、hardlink、device、FIFO、socket、setuid/setgid bit、file capability 和 group/other write。外部发行版 shared library 的内部 symlink 由对应 package 管理，不进入 RadishLex tree hash，但 ELF closure 必须解析到已安装 package 的系统 library root，不能落入 `/tmp`、home、仓库或自定义 `LD_LIBRARY_PATH`。

用户 XDG 目录继续为当前用户 `0700`，userdb/settings/privacy 等敏感普通文件继续为 `0600`、单 link、非 symlink；已有对象 owner/mode/link 不合格时失败关闭，不由 root transaction 或产品启动顺便 `chown`/`chmod` 修正。系统 receipt 只记录版本、状态、稳定错误和 artifact hash，不记录绝对 home、uid 列表、用户词、P1/P2 内容或进程命令行。

## 安装维护事务

### 外层职责

Debian `dpkg` 负责 package database、依赖关系、文件 unpack/remove 和标准 trigger；RadishLex 维护协调层负责：

- 在 package mutation 前验证 operation、source/target artifact、兼容矩阵、静止条件和固定系统根；
- 以 root-owned receipt/guard 记录可重试状态；
- 把 Debian 的 Installed、Unpacked、Half-Configured、Config-Files 等结果投影为稳定产品状态；
- 在 dpkg target apply 后复验完整 manifest、ELF closure、metadata 和权限，只有全部通过才写 `completed`；
- 失败时只使用操作开始前已提供并验证的本地 source `.deb` 做 rollback，不在恢复中联网下载或从已安装文件猜造 source package；
- 把 dpkg 与外部 dependency scripts/triggers 的结果投影为 lifecycle observation；它们不能接收 RadishLex receipt 路径，也不能替协调器推进状态。

直接运行未经过产品前置验证的任意 copy script、把 staged rootfs 当作 installed、或只看 `dpkg` 退出码，都不能形成产品完成证据。协调层不替代 dpkg，也不能改写 dpkg database 冒充回滚。

首批受支持的 mutation 入口只能是 P05B 定义的维护协调器，它接收显式本地 `.deb`、先固定 artifact 与 receipt，再调用 dpkg。直接执行 `dpkg -i`、`dpkg -r` 或等价 apt 旁路即使让 package database 进入 Installed/Not-Installed，也没有自有 maintainer script 可以猜测 artifact 或补写 `completed`；product startup gate 必须返回 `maintenance_required`，只能由重新提供精确 artifact 的受控 `repair`/恢复流程收口。

首次 bootstrap 已固定为：取得 advisory guard 并复验 state root 后，先验证 operation、source/target identity、版本关系与 package snapshot；只有 snapshot 与 operation source 一致时才原子写入 `prepared` receipt。随后把 `.deb` 和同名 evidence 复制为私有 `source`/`target`，记录 size、SHA-256、owner/group、mode、device/inode 与 link count；全部必需 artifact 复验后才进入 `artifacts_staged`。首次安装失败的 source 为空，但恢复请求必须携带已取证 target 以判断 target apply 的不确定结果。

production 载体入口唯一为 `VerifiedArtifactRelationship::verify_package`。它在同一个有界 `.deb` 流上计算 size/SHA-256，严格要求三成员 ar、canonical uncompressed USTAR、control 仅含 `control`/`md5sums`、data 含唯一 product manifest，并把 actual payload inventory 与 manifest/evidence/control、canonical md5 inventory 和 actual `Installed-Size` 逐项交叉；禁止从调用方提交 detached digest 或 detached control/manifest 字节代替 actual package。同域 pure relationship 再校验依赖、Debian version relation 与 dpkg status。每次 target/source mutation、retry 或已匹配 package 的恢复短路之前都必须重跑 staged relationship，并消费仅对当次 quiescence 有效的 move-only permit。

state 持久化允许识别 `receipt.json.tmp`、artifact stage tmp、单侧 artifact 已 rename 与 canonical mode 提交前 owner-only mode 的明确崩溃窗口；维护入口只能在取得 advisory guard、复验 owner/group/type/link/inode/size 后恢复 mode 或补齐另一侧，startup observer 只读阻断且不清理。current operation 必需 slot 必须精确，不得缺失或多余。receipt 内容 `fsync` 后提交 mode、再次 `fsync`，再 rename 并同步直接父目录；新 operation 目录与 staged 文件同样同步直接父目录。v1 旧 operation 只保存 structure/pair metadata，没有历史 `ArtifactFileIdentity` hash proof，也不参与 current recovery。

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
| 字体或系统 dependency 不满足 | startup 已从 terminal actual package/evidence 重建 relationship并拒绝；真实 family/glyph/owner 留 L6 |

production build 不得把“receipt 缺失”解释为首次启动成功，也不得回退 fixture、仓库 `.so`、`LD_LIBRARY_PATH`、用户级 staging 或在线下载。开发 build 与 product build 的 gate 必须在编译身份上分离。

当前只读实现固定为：

- `LinuxSystemStartupPort` 直接读取并严格解析 `/var/lib/dpkg/status`，不执行 `dpkg-query`、`dpkg`、shell 或其他 package mutation 命令；状态文件不可读、重复 package paragraph、未知 tuple 或不完整 identity 都失败关闭。
- 检查顺序先于业务初始化：root/state、active/异常 guard、`receipt.json.tmp`、receipt 与 operation state，再检查 package state、terminal installed artifact、canonical product manifest 与当前 component scope。terminal receipt 引用的 source/target staging 必须按 receipt proof 重新验证固定 slot、file identity、owner/mode/link/size/hash，不能只信 receipt 文本或遗留路径。observer 不创建 state root、不清理 stale guard/tmp/receipt、不打开 XDG/userdb/settings/privacy/Rime。
- `development-staged` 只在 system receipt/state 均不存在且 package 严格为 `NotInstalled` 时得到 `AllowedDevelopment`；`debian-system-product` 缺 receipt 失败关闭。开发 build 遇到任何 system receipt 也按身份隔离拒绝，不能借 development profile 绕过维护现场。
- terminal install/upgrade/repair/rollback 的目标，或失败恢复后的 source，只有 actual staged `.deb`/evidence、package/version/architecture、完整 dependency relationship 与 component 身份完全一致才得到 `AllowedProduct`；nonterminal、active guard、Config-Files/半配置、unmanaged package、completed remove、首次安装失败无 source 和任何身份漂移均要求维护或失败关闭。Manager scope 复验完整 bundle tree；Fcitx scope 复验 addon、同目录 FFI、固定 RimeData、两份 metadata，并交叉验证 Manager/Fcitx 两份 FFI equivalence。该运行时 scope 不执行 fontconfig family/glyph probe，也不冒充 L6 的全系统验证。
- Manager runner 在设置 `umask(0077)` 后、创建 Flutter application/engine 前调用；Fcitx factory 在构造 `Engine` 前取得 move-only startup permit，因此 Engine constructor 内的 input FFI、XDG、privacy 与 Rime 初始化均晚于 gate。共用 C++ binding 在调用 startup ABI 前用 `dladdr` 与 canonical path 验证 startup/error symbols 确实来自当前 component 的精确 sibling FFI；Fcitx 还验证全部输入热路径 FFI symbol origin，拒绝 `LD_LIBRARY_PATH`、preload 或其他 loader interposition。
- `radishlex_linux_product_startup_gate` 使用独立 request/result v1，携带编译 build identity、component 与由 host 从已加载 executable/shared object 取得的 canonical component path；它是 additive Linux startup ABI，输入 session/key ABI contract 仍为 v9。

仓库合同覆盖 receipt absent/terminal/nonterminal、advisory guard、receipt/stage tmp、completed remove、半配置状态、exact current slots、actual `.deb`/manifest/dependency/version relationship、startup/relationship 共用 canonical product/build/positive-revision 规则、fixed dpkg argv/env/timeout/diagnostics、fake executor、`/proc` maps parser、opaque CLI authorization、Manager/Fcitx component scope、双 FFI equivalence、symbol-origin、开发/产品身份隔离、只读目录指纹与调用顺序；acceptance 专项再覆盖八个确定 checkpoint、compile identity、授权参数、完整 process group 终止顺序、无残留 dpkg child 和 canonical evidence redaction。真实 L6 已覆盖六类分散operation、terminal startup与XDG零漂移；首个crash只证明checkpoint和缺陷下失败关闭，不是恢复通过证据。

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

P05B transaction 子批已建立独立 Rust crate `platforms/linux-product/`。它没有复用 macOS 双 `.app` rename，只复用 operation identity、append-only receipt、独占 guard、终态与失败关闭原则；Linux 侧显式投影 Not-Installed、Config-Files、Installed、Unpacked、Half-Configured、Half-Installed 与 trigger 状态。收据使用 canonical JSON、固定 root identity、最多 128 项 operation chain、稳定 failure、source/target proof 和不自动清理的私有 staging；只有终态才能追加下一 operation。

当前 guard 是 regular-file advisory lock，消除了 Unix socket backlog 不能证明唯一 owner 的竞争窗口；active lock 失败关闭，stale unlocked inode 只能由维护 acquisition 重新锁定与收口。共享锁父目录继续要求 root owner/group；非 world-writable mode 按原规则接受，world-writable 只接受 Debian 标准精确 `01777`，依赖 sticky bit 防其他用户替换 root-owned guard，预创建的非 root entry 仍由文件 owner/mode/link/size 检查拒绝。store 已覆盖 restrictive umask、并发 contender、tmp 恢复、exact current required slots、原子 mode、父目录权限矩阵与直接父目录 `fsync` 顺序。每次恢复都重新验证 staged actual package relationship并重新取得 quiescence permit，不把旧 permit、已安装目标或已恢复 source 当作绕过条件。

`LinuxDpkgTransactionPort` 已组合 production fixed-path observer 与 executor。observer 稳定读取 root-owned dpkg status/config 和 staged package，重建 actual relationship、依赖与版本；executor 只运行 fixed `/usr/bin/dpkg` typed invocation，清空继承环境、使用 null stdin、并发 drain 两路有界诊断并设 15 分钟上限。process permit 扫描 `/proc/*/maps` 的固定路径与 device/inode；匿名映射不误报，缺权限、畸形与不确定竞态失败关闭，且不 kill/restart。自动合同覆盖五类 operation、连续 receipt、actual `.deb` 篡改/混配、command exit/signal/overflow、package/source restore 双中断、首次安装 recovery target、target proof crash retry 与 XDG 零接口。该证据证明 production 代码和 L4 合成矩阵，不证明真实 dpkg、真实 `/proc`、外部 script/trigger 或 L6。

首次 L6 install preflight 证明 Debian 13.6 默认 `/etc/dpkg/dpkg.cfg` 使用 `no-debsig` 与 `log /var/log/dpkg.log`；旧 validator 因只接受 `no-pager` 和等号分隔日志路径而在写 `prepared` receipt 前失败关闭。`bb84d4a` 将 `no-debsig`、`log /var/log/dpkg.log` 与 `log=/var/log/dpkg.log` 规范化为三个固定受控值中的两个，并继续拒绝其他日志路径、额外参数、未知项及 `root`、`admindir`、`instdir`、`force-*`、hook、path include/exclude。RadishLex actual `.deb` 的 size/SHA-256、manifest 与 dependency relationship 仍先于 store bootstrap 验证；接受发行版默认 `no-debsig` 不替代产品 artifact identity。

privileged host 只暴露不可直接构造的 `LinuxMaintenanceCommand`。CLI 的 `start`/`resume` 同时要求 `--authorized-system-mutation`、`--preserve-user-data`、小写 32 hex operation ID；start 还要求 operation kind 对应的 root-owned canonical package/evidence pair，evidence 必须与 `.deb` 同目录同名。host 先从 actual package 推导身份与版本关系，再 bootstrap fixed state、取得 advisory guard、写 prepared receipt、复制私有 staging并进入 coordinator；resume 必须与现有 receipt operation ID 精确相同。输出只含稳定 outcome/error/failure code，不含路径、hash、dpkg 原文或用户数据。该 CLI 不是公开 installer，也不会被仓库门禁实际运行。

L5 当前由 `./scripts/check-linux-startup-gate.sh` 闭合仓库内只读 decision 与初始化顺序合同：Rust fake/system-port 测试覆盖状态矩阵、staging proof、真实格式的 synthetic Manager bundle/Fcitx component scope、双 FFI equivalence 与目录零写入；C++ contract 分别编译 `development-staged`、`debian-system-product` 两种身份并拒绝交叉 allow，另以编译和源码合同固定 sibling symbol-origin 校验先于 gate 调用；源码顺序门禁固定 Manager 的 `umask -> gate -> Flutter` 与 Fcitx 的 `gate -> Engine/input FFI/XDG/Rime`。动态 preload/错误 sibling 拒绝仍须在新的 Linux payload 中复验；该 L5 不等于全 package runtime inventory、ARM64 product payload、Debian package lifecycle 或真实桌面启动证据。

L6 输入合同由 `packaging/linux/l6-matrix.json`、`scripts/linux-product/l6_contract.py` 与 `./scripts/check-linux-l6-contract.sh` 固定：profile 为 `debian13-arm64-ephemeral-v1`，专用用户为 `radishlex-l6`；source/target 必须来自不同 commit 的相邻 Debian revision并保持 product/build/data contract相同；主序列固定 install→upgrade→repair→rollback→remove→reinstall，另有八个完成/回退 crash checkpoint。probe 同时覆盖 actual dependency、字体 family/glyph/owner、Manager/Fcitx startup 正负向、XDG fingerprint、dpkg/procfs 与完整 inventory。该门禁只验证 matrix、不运行 controller或连接 guest，也不证明 L6 已执行；详细步骤见 [Linux L6 Debian package matrix runbook](runbooks/linux-l6-package-matrix.md)。

release-pair 输入合同由 `packaging/linux/l6-release-pair.json` 固定 source commit `55351f2`/revision 1、target clean descendant/revision 2、相同 ABI/schema/XDG/settings/privacy/Rime contract 和两个 executable build profile。source 现精确绑定 S2 terminal package/evidence `09ed1228…bec`/`fe3d6297…cf94` 的文件名、size、SHA-256 与 `prior-terminal-installed-artifact-v1` policy；`scripts/build-linux-l6-release-pair.sh` 不再构建 source，只接受这两个 frozen input、一个 canonical clean target root 与 absent output。`l6_source_anchor.py` 在 target build 前验证 canonical evidence、single-link/mode/identity，做 exclusive copy 与 file/directory `fsync`；target 再构建真实 payload/`.deb`、production maintenance、actual-package verifier 与 compile-isolated acceptance ELF。record 前重新验证 Git commit、谱系和 clean 工作树，target production Rust verifier 逐侧解析 actual package；Python 再校验相邻版本、AArch64 loader、mode/link/markers、canonical envelope 与发布后完整重哈希。21 项行为测试和 6 项源码合同测试拒绝同版本 source 重建、evidence 漂移与 source build 回退。前四套 record/handoff 只作失败或 mismatch 取证，第五套 `d2661cc0…ed15` 绑定 rolled-back 现场；修复 target `80e49ce` 的第六套 record `cda70afa…659b` 已完成同一真实链验证。startup共享目录修复后的target `d75818f`又形成record `c74fac12…9849`：source仍为`09ed1228…bec`，target package/evidence为`4dd00540…dcec`/`9e646c86…3f17`，双production verifier、宿主verifier与no-replace原子发布均通过。

package 冻结后的维护 ELF 不得通过改写上述 pair、重建同版本 `.deb` 或向 guest 热替换来承载。`packaging/linux/l6-maintenance-refresh.json` 因此另立 `debian13-arm64-maintenance-refresh-v1`：根锚点是第六套 canonical record `cda70afa…659b`，并再次固定 target package/evidence `b211d940…d09c`/`2a1132c6…0e1b`、target commit `80e49ce`、旧 production ELF `b060c240…7d81`；required repair fix ancestor现为`698fe1f`。builder 只接受这三份 `0644` single-link frozen input、包含该修复且 product metadata 与 `80e49ce` 逐字段相同的 clean descendant root，以及 absent output；它断网构建 `--no-default-features` production maintenance 与临时 actual-package verifier，不构建 Manager/addon/rootfs/`.deb`，也不产出 acceptance executable。record 绑定旧 pair 内 target 的 size/hash/dependency/manifest、refresh commit、Debian 13 ARM64 build environment 与新 ELF identity/size/hash，并要求新旧 ELF 不同。staging 以 `O_EXCL`/file+directory `fsync` 写入，完整 inventory 和 production Rust verifier通过后以同文件系统 no-replace rename、父目录 `fsync`、发布后重验；26 项行为/源码合同测试拒绝 anchor、metadata、ancestry、architecture、loader、mode/link/hash、inventory、redaction和发布顺序漂移。仓库门禁本身不读取真实 handoff或启动guest；既有`b891ed1` record/ELF `4b41d1c0…fb2cd`/`9a657510…54585`只作旧合同下的历史取证。新`823afca` handoff record/ELF/archive/local evidence为`b3852845…a146`/`1f37b6f9…cca7`/`e917f1a9…6ad7`/`e581dec4…315f`，guest/host verifier与固定mode/link/hash均通过；target package/evidence未改写，builder package/state absent且未运行dpkg、maintenance或acceptance。

独立clone `394217A7…B6FB`已从未改写S3建立并完成该handoff的断网只读preflight：启动前EFI/qcow2等于S3且config为`Network=[]`；运行态仅loopback且IPv4/IPv6 main route为空。input switch先逐哈希保留第六套旧pair，再只投影production maintenance、旧base record和既有target package/evidence；acceptance/source均absent。预检不调用`dpkg`，而以`dpkg-query`、直接status/config/log哈希、空updates目录和`/var/lib/dpkg/info` MD5 inventory复验installed payload，并闭合receipt/operation count 2、依赖/字体、manifest/双FFI、startup正负向、XDG与进程映射。host manifest/guest archive为`878056b2…32f4`/`79cdf8d6…e406`；关机盘为`818d8725…7425`/`a73a3266…9155`/`111aaf09…8a6`，十三台VM全停。该证据只授权输入准备与只读观察，不是repair、package mutation或operation创建证据。

后续单步授权先复验该preflight、十三台只运行目标clone与运行态断网，再形成mutation preflight `9d379758…acca`；此时receipt仍为upgrade completed、operation count 2，maintenance/operation/dpkg mutation均未开始。guest随后生成唯一operation ID，宿主只保存hash `9a701cf3…ae0`，不可重复wrapper只调用一次production maintenance：exit 0、stderr空、stdout为`maintenance_outcome=completed`。新receipt `4e23198c…133b`为`repair/same_release/completed`、chain 3、target proof installed、failure null、manual recovery false；dpkg log从882,804增长到884,456 bytes，受限delta `cca2898e…178`明确记录同版`38-2` upgrade/configure/installed，而dpkg status与installed payload MD5 inventory保持不变。strict postflight `bce81a6e…f165`再次通过20项依赖、字体、manifest/双FFI、startup正负向、XDG fingerprint、WAL/SHM/profile absence、进程映射与断网。guest archive/host manifest `4e46a101…608b`/`116e3f06…5e90f`已冻结；正常关机后clone config/EFI/qcow2为`818d8725…7425`/`3b117def…8f0`/`79adac93…03f6`，qcow2复算一致且零句柄，S3三项未漂移，十三台VM全停。该clone已是terminal repair现场，不得resume、重试、再次调用或复用。

随后从S3对应registered terminal建立首台rollback clone `9FE6265D…AE28`。启动前EFI/qcow2精确等于S3，config去除Name/UUID后语义相同且`Network=[]`；文件回读证据`a18e1ab9…217f`证明仅loopback、IPv4/IPv6 main route为空、target package/receipt baseline和产品/dpkg进程静止。宿主当前release-pair verifier再次通过record `cda70afa…659b`，source/target package/evidence仍为`09ed1228…bec`/`fe3d6297…cf94`与`b211d940…d09c`/`2a1132c6…0e1b`。

完整只读preflight脚本与startup probe在guest/host逐字节相同，但guest-agent只返回exit 0和空stdout/stderr，预定结果文件不存在；按既有返回码不可信边界不计为通过，也未原地重试或诊断。failed-closed host manifest为`6b7ac641…a51d`；正常关机后config/EFI/qcow2为`fdfc0502…23a5`/`a73a3266…9155`/`2b323e27…bb4a`，qcow2复算一致、clone/S3零句柄、S3身份未漂移且十四台VM全停。已冻结证据无法证明guest内精确失败谓词；旧第六套证据明确记录guest-agent push初始mode为`0666`，而本次startup probe要求`0600`但没有先归一化，因此它只构成高概率触发点。确定的控制缺口是失败分支没有持久化phase与terminal result。

该首台clone的UTM注册项与package随后在明确清理授权下删除，host证据目录及manifest保持`6b7ac641…a51d`，不得用证据目录替代registered VM身份。第二台独立clone `BFB3EF09…2720`从同一未改写S3建立，启动前EFI/qcow2与S3相同、config仅Name/UUID不同且`Network=[]`；文件回读`d7bac2bc…6bd`再次证明Debian 13 ARM64、仅loopback、IPv4/IPv6 main route为空、target package/receipt与产品/dpkg进程静止，宿主canonical pair复验仍通过。

第二台先创建私有传输根，随即因脚本要求目录`stat -c '%a|%U|%G|%h'`精确等于`700|root|root|1`而返回`transfer_setup_error=root-identity`。正式只读preflight脚本未传入、未执行，attempt marker、operation ID、maintenance/acceptance、`dpkg`和rollback均未发生；没有自动修复、重试或guest诊断。目录link count为1是高概率控制缺陷，但实际guest `stat`未冻结，故不能改写成确定根因、guest身份漂移或产品/package失败。host manifest `40dab9b…680e`与关机盘`5258c84a…005`/`a73a3266…9155`/`1d20869b…57b`已冻结；S3三项未漂移、clone/S3零句柄，十四台VM全停。

第三台独立clone `EFD15599-7177-4D55-BF17-173EE1F0BBDD`从同一未改写S3对应registered terminal建立。启动前config `6295c4ed…29ae`去除Name/UUID后与S3语义相同，EFI/qcow2精确等于S3且`Network=[]`；只启动该clone。文件回读网络证据`42e7da47…f454`固定Debian 13 ARM64、仅`lo`、IPv4/IPv6 main route为空、target package/receipt与产品/dpkg进程静止。

传输setup只验证root-owned `0700`真实directory且非symlink；startup probe和正式脚本经不可信push后受控归一化为root-owned `0600` single-link regular file，host/incoming/final guest副本逐字节相同。正式脚本只执行一次，以`O_EXCL`落attempt并到达`postflight`；回读terminal `64f1b1ab…8ee`为`passed/postflight/reason=none`，完整证据`4edcbb6b…9f2`闭合canonical pair、source/target artifact、package `38-2`、upgrade completed receipt、20项依赖、字体、startup正负向、XDG `f3df287f…b86b`、WAL/SHM/profile absence、operation count 2、进程/映射与loopback-only网络。existing operation ID只读取hash，新operation ID未创建；maintenance/acceptance、`/usr/bin/dpkg`、package mutation、rollback、Manager/Fcitx与用户XDG写入均未发生。

正常request关机后clone config/EFI/qcow2为`6295c4ed…29ae`/`a73a3266…9155`/`34733d78…3b10`，qcow2复算一致且clone/S3零句柄；S3三项保持原值，十五台VM全停。host证据目录`RadishLex-L6-Rollback-Preflight-0611b86-Passed`为`0700`、文件为`0600`，manifest `0b71c24b…3073`覆盖其余26项。该结果只闭合rollback前只读资格，不是rollback operation或package mutation证据。

后续单步授权从clean `2b6a6ee`、origin/dev ahead 13、上述preflight manifest与十五台VM全停开始；只启动`EFD15599…BBDD`。本次boot先以文件回读证据`629680f8…37d6`固定Debian 13 ARM64、仅`lo`、IPv4/IPv6 main route为空、`38-2` package、upgrade completed receipt、operation count 2及产品/dpkg进程静止。传输根继续只对目录验证mode/owner/group/type/non-symlink，startup、one-shot、postflight与mutation preflight经逐字节回读后归一化为root-owned `0600` single-link；正式mutation preflight `3c4894fa…6320`重新闭合canonical pair、source/target artifact、package/receipt、direct dpkg database与MD5 inventory、20项依赖、字体、startup正负向、XDG `f3df287f…b86b`、WAL/SHM/profile absence、进程/映射及断网。此时没有新operation ID、maintenance/acceptance调用、dpkg mutation或用户XDG写入。

guest随后生成唯一32 hex operation ID，宿主只保存hash `663623bc…c8b1`；不可重复wrapper只调用一次pair内production maintenance `b060c240…7d81`。rollback参数把当前target `38-2` package/evidence作为operation source，把冻结source `38-1` package/evidence作为operation target；exit 0、stderr空、stdout精确为`maintenance_outcome=completed`。新receipt `b97c7580…664d`、6,947 bytes，语义为`rollback/target_older/completed`、chain 3、source proof null、target proof installed、failure null、manual recovery false。package回到`26.7.1+38-1`，dpkg status为`33c4973d…cff1`；log从882,804增长到884,456 bytes，1,652-byte delta `6b4e8f39…6721`明确记录`38-2 → 38-1`的unpack/configure/installed与发行版trigger lifecycle，source MD5 inventory `f6afcfb7…a50`完整校验通过。

strict postflight `62e17a05…0427`再次通过source manifest `b1322933…467b`、双FFI `d845abfe…6a74`、依赖/字体、startup正负向、XDG固定指纹、WAL/SHM/profile absence、进程/映射和loopback-only；没有运行acceptance、启动Manager/Fcitx或写用户XDG。guest archive `879ab337…2b7e`为71,680 bytes、16项且不含raw operation ID；host证据目录`RadishLex-L6-Rollback-80e49ce`从absent staging发布，manifest `fef4ea32…851d`覆盖其余37项。正常request关机后clone config/EFI/qcow2为`6295c4ed…29ae`/`3b117def…a8f0`/`9a28509a…995a`，qcow2两次复算一致、clone/S3零句柄；S3三项保持`00bac87d…6456`/`0846b1d3…9741`/`6b22499b…b137`，十五台VM全停。该clone现为冻结terminal rollback现场，不resume、重试、再次调用、清理或直接复用；后续连续链只在新授权下从其stopped registered terminal建立独立remove clone。

后续单步授权从clean `f8db10b`、origin/dev ahead 14、rollback manifest `fef4ea32…851d`与十五台VM全停开始。plain `utmctl clone`从`EFD15599…BBDD`的stopped registered terminal建立独立remove clone `5EA2BAA2-B9A1-46CC-B496-B37826DD27A2`；启动前config `92568d22…626a`去除Name/UUID后与source语义相同，EFI/qcow2精确等于source且`Network=[]`。只启动该clone后，文件回读网络证据`a21afee8…6c3d`固定Debian 13 ARM64、仅`lo`、IPv4/IPv6 main route为空、source `38-1` package、rollback completed receipt与产品/dpkg进程静止。

私有`/run`控制根只验证root-owned `0700` directory/non-symlink；startup/preflight的guest push初始mode为`0666`，经回读逐字节一致后归一化为root-owned `0600` single-link。唯一正式attempt terminal `bc65fdb0…fcbc`为`passed/postflight/reason=none`；证据`35f58ca3…ed12`闭合canonical pair/source artifact、installed `38-1`与MD5 inventory、rollback receipt chain 3、20项依赖、字体、source manifest/双FFI、startup正负向、XDG `f3df287f…b86b`、WAL/SHM/profile absence、进程/映射与断网。没有创建新operation ID或运行maintenance/acceptance、`/usr/bin/dpkg`、remove、Manager/Fcitx及用户XDG写入。guest archive/host manifest为`14506554…d676`/`ade5743b…9dd8`；正常request关机后clone config/EFI/qcow2为`92568d22…626a`/`4b6292f0…d726`/`91db6f02…6bab`，qcow2复算一致。v3/S3未漂移、三盘零句柄且十六台VM全停；该clone只在新的单步授权下继续真实remove operation。

后续单步授权从clean `3eda15c`、remove preflight manifest `ade5743b…9dd8`逐项通过、十六台VM全停及三盘零句柄开始。只启动`5EA2BAA2…27A2`，直接文件回读再次固定`Network=[]`、仅`lo`与IPv4/IPv6 main route为空；mutation preflight `0be5e2a8…7f8d`闭合canonical pair、source/target artifact、installed `38-1`、rollback receipt chain 3、direct dpkg database/MD5 inventory、20项依赖、字体、manifest/双FFI、startup、XDG、进程与断网。此时没有新operation ID、maintenance/acceptance调用、dpkg mutation或XDG写入。

guest随后生成唯一operation ID，宿主只保存hash `93fc5b83…ef3d`；one-shot只调用一次production remove，exit 0、stderr空、stdout精确为`maintenance_outcome=completed`，没有retry、resume、acceptance或手工dpkg。receipt `770a27b7…b40e`为`remove/not_applicable/completed`、chain 4、staged source only、target proof not-installed、failure null、manual recovery false。dpkg status为`2c31c35c…e572`，1,508-byte delta `c5dd0066…3ad4`记录installed→remove→half-configured→half-installed→config-files→not-installed及发行版trigger lifecycle；28项payload、完整product tree与dpkg info均absent，依赖和字体保持。

Manager/Fcitx startup均返回`FailedClosed + RemovedProgram + completed`，XDG指纹保持`f3df287f…b86b`，WAL/SHM/profile absent，产品映射、dpkg进程和双main route为空。postflight v1把同hash FFI复制到`noexec`的`/run`，`ctypes`无法映射并留下失败result/stderr；改在可执行的`/var/tmp`承载相同探针后，terminal evidence `eef62072…e80d`通过，期间没有重跑maintenance。guest archive `529ee42c…9b8e`保留失败与成功控制且不含raw operation ID、raw receipt、完整dpkg status/log。host manifest `be498439…97a1`覆盖其余36项；宿主冻结脚本的内嵌`utmctl list`曾退出134并留下零长度文件，后用plain direct枚举的同一十六台stopped清单替换并在host summary中记录，未改变guest或transaction。正常关机后clone config/EFI/qcow2为`92568d22…626a`/`abea62d2…c8fc`/`ad8a6c6b…55fa`，qcow2复算一致；v3/S3未漂移、三盘零句柄且十六台VM全停。该clone现为冻结remove terminal，不resume、重试、再次调用、清理或直接复用；后续只在新授权下从它建立独立reinstall clone并先做只读preflight。

后续单步授权从clean `bbf1e48`、remove manifest `be498439…97a1`逐项通过、十六台VM全停与三盘零句柄开始。plain `utmctl clone`从stopped remove terminal建立独立reinstall clone `E671DB9C-5E2C-447B-9425-8D91D2CFD465`；默认sandbox内三次clone调用均exit 134且未产生资产，获授权后以同一plain命令成功建立。启动前EFI/qcow2与source精确相同，config去除Name/UUID后语义相同且`Network=[]`；运行态仅该clone启动、仅`lo`且IPv4/IPv6 main route为空。

网络探针`42feb8e7…604a`与唯一正式attempt的terminal `2858f0ed…562f`均经guest文件回读；后者为`passed/postflight/reason=none`。完整证据`8ed9d43a…86b25`闭合canonical pair与target artifact、package not-installed、28项payload/product tree/dpkg info全absent、remove receipt `770a27b7…b40e` chain 4、direct dpkg status/log、20项依赖、字体、`RemovedProgram` startup、XDG `f3df287f…b86b`、WAL/SHM/profile absence、进程/映射与断网；operation count保持4，未创建新operation ID或执行maintenance/acceptance、`/usr/bin/dpkg`、reinstall及用户XDG写入。guest archive/host manifest为`31855e48…80782`/`a98dbec6…5006a`；正常关机后clone config/EFI/qcow2为`db1e59ae…13b90`/`d9bd0aa3…97463`/`cdf0ee06…72dd`，qcow2两次复算一致。remove、rollback v3与S3未漂移，四盘零句柄，十七台VM全停。该clone现只冻结reinstall前只读资格，真实`reinstall_target`仍须新的单步授权。

真实reinstall从clean `c5b6631`、上述host manifest、十七台VM全停及四盘零句柄开始，只启动`E671DB9C…D465`。network/transfer/mutation-preflight `11c00403…be2`/`db9b3c59…3e50`/`fca225a7…3a01`固定同一boot仅`lo`、双main route为空、控制文件root-owned `0600` single-link，并重新闭合package absent、remove receipt chain 4、pair/target artifact、依赖/字体、`RemovedProgram` startup、XDG与进程静止；通过前未创建新operation ID或执行maintenance/dpkg。

guest随后生成唯一operation ID，宿主只保存hash `fafb05ad…79c8`；one-shot只调用一次production `reinstall_target`，exit 0、stderr空、stdout精确为`maintenance_outcome=completed`，没有retry、resume、acceptance或手工dpkg。receipt `3eb44171…e274c`、3,926 bytes，为`install/not_applicable/completed`、chain 5、staged target only、target proof installed、failure null、manual recovery false。target `38-2` dpkg status为`c09365b3…cece`；1,512-byte delta `b0338d25…116a`只含absent→target install/configure/installed与发行版triggers，target MD5 inventory `bc36eff3…71f1`的28项完整通过。

strict postflight `b4bf57ba…09f93`闭合target manifest/双FFI、依赖/字体、startup正负向、XDG `f3df287f…b86b`零漂移、WAL/SHM/profile absence、进程/映射与断网。guest archive `4ea296fa…fd3ba`为81,920 bytes、19项且不含raw operation ID/receipt/status/full log；host manifest `54f1e952…6685`覆盖其余34项。正常request关机后clone config/EFI/qcow2为`db1e59ae…13b90`/`76753750…a1c3`/`feb2ba7e…aaa5`，qcow2两次复算一致；remove、rollback v3与S3未漂移，四盘零句柄且十七台VM全停。该clone为冻结reinstall terminal，不resume、重试、再次调用、清理、恢复或复用。

后续所有L6只读probe必须把guest-agent传入文件视为不可信mode：先创建私有`0700`传输根，目录只验证root owner/group、精确mode、真实directory且非symlink，不套用普通文件single-link条件；再以受控复制/安装把输入归一化为root-owned `0600` single-link regular file，并复验size/hash后使用。正式检查开始前须以`O_EXCL`创建attempt marker；在pair、artifact、package、dependency/font、receipt、startup、XDG、process与postflight前更新固定phase，并在成功或任一失败退出时原子写root-owned `0600` terminal result。宿主必须回拉并逐字节验真attempt/result；结果缺失归类为transport-inconclusive并失败关闭，不得自动修复或复用同一clone。

checkpoint/evidence controller 归属独立 `platforms/linux-l6-acceptance/` crate 和 executable identity。production crate 的 `l6-acceptance-checkpoints` feature 默认关闭，production maintenance main 只绑定 disabled sink；它不识别 acceptance 参数，也不读取环境变量或路径覆盖。acceptance worker 只通过该 compile feature 接入：`prepared` 在全部必需 artifact 已复制、取证且 receipt 仍为 prepared 时发出，随后七点分别绑定 `artifacts_staged`、`quiesced`、persisted package mutation、target dpkg 返回、`rollback_required`、source dpkg 前与 source dpkg 返回后。controller 从继承 pipe 得到 typed notification并暂停 worker，不读取 receipt 抢时机。

controller 为 worker 创建独立 process group，命中 checkpoint 后以固定 `/usr/bin/kill` 对完整 group 发送 `SIGKILL`，等待 worker signal 终止，再连续扫描 `/proc/*/stat`，只有 group member 为零且无 `dpkg` child 才生成 `radishlex-linux-l6-checkpoint-evidence-v1`。envelope 使用 canonical JSON、deny-unknown fields，只保存 matrix/build/scenario/checkpoint、repository/guest/snapshot identity、operation ID SHA-256、授权/fault/termination 分类与 expected terminal；不保存 operation ID 原值、PID、artifact/staging 路径、proc maps/dpkg stdout/stderr 或用户数据。controller 仍不替换 `/usr/bin/dpkg`、不使用 shell/`PATH`/`--force-*`，实际运行继续需要 crash 与 system mutation 两层明确授权。

首个`install_prepared`实机case在独立v2 clone只执行一次controller，operation ID宿主只存hash `9429b171…9145a`，checkpoint与process-group终止通过且没有dpkg child。crash-state首次探针因`/run`为`noexec`失败；将同hash probe复制到`/var/tmp`后未重跑transaction，Manager/Fcitx仍返回`FailedClosed/GuardInvalid`。独立证据固定guard为`root:root 0600`零长度单链接文件，父目录为canonical `root:root 01777 /run/lock`，故根因是startup只读validator遗漏store已接受的共享父目录合同。case未resume、未调用dpkg，也未生成成功crash-state/postflight。

`a6622d4`把父目录权限判定收敛为store/startup共用纯策略：非world-writable仍按既有规则接受；world-writable只接受root-owned精确`01777`，guard自身的owner/mode/type/link/size检查不放宽。startup回归接受`0755`、`0775`、`01777`并拒绝`0777`、`01703`、`01733`、`01757`，保持只读且不调用package/component observer。默认119项与L6-feature 124项Linux product测试、acceptance 10项及clippy/controller合同通过。

修复target `d75818f`的新pair只在唯一离线ARM64 builder中构建一次。外层网卡down、双main route为空；user/network namespace内loopback down、零路由且`CARGO_NET_OFFLINE=true`，Cargo/pub/Flutter冻结cache前后稳定。record `c74fac12…9849`、canonical USTAR `bbc96507…b4db`和八文件mode/link/hash经guest Python、双production Rust及宿主Python verifier一致通过，再从absent incoming以内核级no-replace rename发布`RadishLex-L6-Handoff-d75818f`。builder package/state始终absent，未运行maintenance/acceptance/dpkg；关机盘已冻结。

新pair首台retry `45D8205F…F2B4`因host inventory未按canonical sort且case误写build-environment文件名，在input switch前失败并已停止。第二台`0BB24317…307C`使用修正版transfer闭合断网与input switch，但preflight把fresh absent错误预期成remove-terminal `RemovedProgram/Completed`；target startup gate实际对Manager/Fcitx均返回`FailedClosed/ReceiptMissing/no receipt`，与源码合同一致。failure manifest `66212f2c…a8a3e`绑定实际tuple、phase failure与guest state；mutation preflight evidence、operation ID、state、checkpoint、guard及transaction均absent。正常关机freeze `65157b95…cb2b`固定磁盘双重哈希与零句柄，二十一台VM全部stopped；两台retry均不得原地修补或复用，删除前还须把临时证据归档到持久host root并另行授权。

`./scripts/check-linux-l6-controller.sh` 编译 production feature 边界与独立 acceptance crate，并运行八点中断/恢复、无重复 mutation、target validation acceptance rejection、参数授权、进程组顺序、无 dpkg child、canonical/redaction 与源码边界正负向测试。它只使用 fake port/backend 和临时目录，不运行 acceptance/maintenance executable，不写 fixed evidence root，也不证明 Linux process group 或 dpkg lifecycle 已实测。

## M5-P05 实现状态

`M5-P05A` 只建立可复验的 metadata 与 rootfs assembly，不安装 `.deb`。当前实现为：

1. 新建 `packaging/linux/`，固定 `product.json`、`install-layout.json`、Debian control template、字体/dependency profile 与文档入口。
2. `build-linux-product-addon-stage.sh` 以 CMake `system` profile 生成不携带 sibling RimeData 的 addon/FFI/metadata stage；`rootfs.py` 再从显式 Manager Release bundle、该 stage 和锁定 RimeData 形成临时 `DESTDIR` rootfs，不写宿主 `/usr`、`/var`、user XDG 或 guest。
3. 生成 canonical Linux product manifest，复验 version/build、完整 inventory、mode/link、双 FFI hash、Fcitx metadata、desktop entry、ELF closure 与 RimeData/license。
4. 稳定入口 `./scripts/check-linux-product-metadata.sh` 与 `./scripts/check-linux-product-layout.sh` 已加入仓库门禁，覆盖缺字体 dependency、错误 multiarch、版本漂移、缺文件、宽权限、symlink/hardlink、FFI 不同、RimeData/license 漂移和构建路径泄漏。
5. 保留 `./scripts/check-linux-fcitx5.sh` 与 `./scripts/check-manager-linux-product.sh` 的开发/staged 职责；新门禁不能把二者改名为安装，也不能执行 `dpkg`、启动 GUI/Fcitx 或修改系统。

P05A 只证明 committed 产品输入能形成 Debian 目标布局。P05B 已完成确定性 `.deb`、actual relationship、恢复事务、production system port/host、共用startup gate、L6 format与controller；六类operation已有独立证据且XDG零漂移。首个crash checkpoint暴露的startup共享锁父目录漂移已修复并冻结`d75818f` pair；新pair两台retry又分别在input switch前和preflight内失败关闭，均未进入transaction且已停止。当前二十一台VM全部stopped；清理须按持久证据与唯一职责另行授权，开发下一步仍只可修正fresh-absent预期并另建clean clone。连续完整L6、process/external lifecycle与P05C继续关闭。

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
