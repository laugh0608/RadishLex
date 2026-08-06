# RadishLex Fcitx5 addon

本文说明 Linux Fcitx5 addon 的职责、开发构建、XDG 数据路径和验证入口，读者是平台壳、Rust FFI 与 Linux Manager host 的维护者。本文不包含发行版安装步骤、系统输入法启用操作、逐日桌面验收流水或 Linux 产品升级；安装职责见 [`docs/linux-installation-maintenance-boundary.md`](../../docs/linux-installation-maintenance-boundary.md)，实时阶段见当前状态，详细证据见本周周志。

## 当前证据

M5-P02 已经建立真实 C++ 源码、CMake target、addon/input method metadata、自动 contract 和可复验的 Debian 13 ARM64 开发镜像。当前证据包括：

- Apple clang 的 C++17 严格编译通过；
- ABI v9 contract、key projection、owned `KeyResult`/snapshot、display-index selection、owner-thread 和 reset/free/shutdown 顺序通过 fake-FFI contract；
- XDG 默认路径、显式 XDG 根、`0700`/`0600`、relative path、symlink、宽权限和 production/test override 隔离通过；
- M5-P04 已增加 Flutter Linux runner、固定 bundle `.so`、共享 XDG/Manager runtime、独立 privacy format、inotify/event-loop 变更感知与受控粗分类 contract；真实 ARM64 Manager Release/FFI smoke、桌面 privacy、删除/恢复、导入导出、Manager/Fcitx 与完整桌面会话重启均已通过；
- Debian 13 ARM64 使用 Rust 1.85.0、CMake 3.31.6、Fcitx5 Core 5.1.12 和 librime 1.13.1，真实编译并动态链接启用 `native-rime` 的 `libradishlex_ime_ffi.so` 与 `radishlex.so`；
- CMake staged install 把 addon、共享 FFI、锁定 RimeData 与两份 Fcitx metadata 形成同一开发装配，addon 只使用 `$ORIGIN` 定位 sibling FFI，不保留仓库或临时构建路径；
- `radishlex_runtime_probe` 在相同 Linux 环境先校验装配文件、symlink 和权限，再对 staged `radishlex.so` 执行 `dlopen(RTLD_NOW)`；
- CTest 在相同 Linux 环境复验 application context、FFI projection、XDG resolver、Manager runtime、privacy monitor、Fcitx candidate key 与 runtime layout 七项 contract。

以上自动结果只证明 Linux ARM64 编译、装配和 headless native loader。M5-P03 另已在 Debian 13 ARM64 GNOME 中完成不写 `/usr` 的用户级开发装配，并取得 Wayland/X11、GTK/Qt/Electron/浏览器/终端、完整候选交互、切换/重启、进程级与整机离线、password/unknown/terminal/`Sensitive` 的人工证据。原生 Qt6 Wayland 只由明确的 Wayland QPA、Fcitx Qt6 input-context 与会话类型共同证明；FeatherPad 映射 `libQt6WaylandClient` 本身不作为 backend 证据。快速跨 X11→Wayland 会话的 daemon 自启动使用 Debian 官方 Fcitx5 desktop entry 的用户级副本闭合；这仍是开发装配，不证明发行安装、升级或移除。实时状态见 [`docs/status/current.md`](../../docs/status/current.md)，详细流水见本周周志。

## 组件结构

```text
include/radishlex/linux/
  application_context.h reviewed program-to-coarse-context contract
  application_evidence.h fixed-candidate desktop evidence contract
  ffi_projection.h    ABI v9 owned C++ projection
  key_projection.h    platform key to RadishLex key contract
  manager_runtime.h   Manager bundle/XDG product bootstrap contract
  privacy_mode.h      strict store and fail-closed runtime snapshot
  privacy_monitor.h   Linux directory change monitor contract
  runtime_layout.h    addon-relative native/RimeData layout contract
  xdg_paths.h         addon/Manager shared XDG resolver
src/
  application_context.cpp exact production allowlist projection
  application_evidence.cpp opt-in opaque identity evidence matcher
  fcitx_addon.*       Fcitx lifecycle, privacy event loop and commit adapter
  fcitx_candidate_key.* Fcitx candidate-key matching boundary
  ffi_projection.cpp  KeyResult/snapshot copy and owner-thread guard
  key_projection.cpp  Unicode/named key/modifier/phase validation
  linked_ffi_api.cpp  only direct C ABI symbol table
  manager_runtime.cpp fixed Manager bundle and private local paths
  privacy_mode.cpp    atomic privacy store and runtime failure categories
  privacy_monitor.cpp inotify serialization for atomic replacements
  runtime_layout.cpp  loaded addon identity and resource validation
  xdg_paths.cpp       effective-user XDG and private path enforcement
config/
  radishlex-addon.conf.in
  radishlex.conf.in
dev/
  Dockerfile            pinned Debian 13 ARM64 development environment
evidence/
  firefox-context.html  offline ordinary/password field fixture
tests/
  application_context_test.cpp
  fcitx_candidate_key_test.cpp
  ffi_projection_test.cpp
  manager_runtime_test.cpp
  privacy_monitor_test.cpp
  runtime_layout_test.cpp
  xdg_paths_test.cpp
tools/
  runtime_probe.cpp    staged addon resource and dlopen diagnostic
```

`fcitx_addon.cpp` 不包含 Rime、SQLite、ranker、userdb、privacy 文件解析或同步实现。它只映射 Fcitx capability、按键和生命周期，把共享 privacy monitor 接入 Fcitx event loop，消费 Rust-owned snapshot，使用 Fcitx input panel，并把 Rust commit 交给当前 input context。

## ABI v9 审计结论

可直接复用的接口：

- `radishlex_ffi_contract`：校验 ABI v9、`owner_thread` 和 `catch_unwind`；
- `radishlex_session_new_personalized_rime`：创建共享 runtime 上的每-context 产品 session；
- `radishlex_session_set_learning_context`：传入 secure/sensitive/privacy/known 摘要；
- `radishlex_session_handle_key_event`：取得 consumed、可选 commit 和同事件 snapshot；
- `radishlex_session_select_candidate`：只接受 Rust display index；
- `radishlex_session_reset`、`radishlex_session_free` 与 `radishlex_rime_runtime_shutdown`：形成确定生命周期。

本批没有扩展 ABI。snapshot 不输出 Fcitx 私有对象或候选 UI cursor。Fcitx candidate list 维护当前可见 cursor；数字键、Space 和鼠标选择最终都调用同一个 display-index selection，PageUp/PageDown 仍作为稳定 named key 交给 Rust engine 后重建 candidate list。候选键使用 Fcitx 自身的 `Key::check` / `digitSelection` 语义，允许锁定键与内部投递状态、拒绝命令修饰键，不能以原始 states 严格等于零作为前置条件。这避免 GTK 正常 Space commit 绕过学习，也避免在 C++ 中推断 engine index 或复制 Rime highlight 逻辑。client preedit 只投影同一 snapshot 的 composition 与 UTF-8 字节 cursor，并携带 Fcitx `DontCommit`，避免 input context 失焦时提交未完成的原始拼音。

不需要进入共享 ABI 的 Linux 产品能力：

- Linux install/data startup gate 和版本化产品 identity：M5-P05 前置边界已固定，源码尚未实现；
- Linux Manager privacy 已固定为独立 XDG 文件，addon 变更感知与分类框架已落地；生产 allowlist 只包含经 Wayland/X11 评审的精确 `firefox-esr -> browser`；
- 发行版 package、系统域路径与升级 receipt：M5-P05 文档已固定，metadata/transaction 实现尚未开始。

在普通 context 无已评审生产身份时，addon 传 `context_known = 0`；Rust 因而使用 engine 顺序且不读写 userdb。当前只有精确 `firefox-esr` 映射为 `browser + context_known = 1`；路径、大小写、wrapper、其他 Firefox 候选和 unknown 仍失败关闭。Fcitx 明确提供 `Password`、`Sensitive` 或 `Terminal` capability 时先返回受控摘要且不读取 `program()`；Terminal 固定投影为 `terminal + context_known = 0`，不传 program name、窗口标题或正文。

经单独授权做真实桌面身份评审时，可在开发 build 显式设置 `-DRADISHLEX_APPLICATION_EVIDENCE=ON`。该模式只把源码中固定候选的精确匹配投影为不含原值的稳定 token，未命中统一输出 `unmatched`；Password、Sensitive 与 Terminal 只输出 `*_program_unread`，保持不读取 `InputContext::program()`。同一模式还订阅 Fcitx 公共 capability change 事件，只输出 `password_on/off`、`sensitive_on/off` 与 `terminal_on/off`，不关联或记录任意原始程序身份。该选项默认关闭，不改变生产 allowlist，也不能作为应用已通过 Wayland/X11、frontend 和敏感字段传播评审的替代证据。

`evidence/firefox-context.html` 是无脚本、无表单提交和无外部资源的离线普通/密码字段夹具。它只用于用户实体操作，不记录输入内容；验收必须使用公开合成文本，并结合固定 token、frontend、会话类型、capability 和学习聚合证据，不能凭页面行为单独加入生产 allowlist。

## XDG 路径

production resolver 使用 `geteuid` + `getpwuid_r` 取得 authoritative home，只读取 freedesktop 定义的四个 XDG 环境变量，不读取 `HOME` 或 `RADISHLEX_*` test override。

| 内容 | 默认路径 |
| --- | --- |
| 产品数据 | `$HOME/.local/share/radishlex` |
| userdb | `$HOME/.local/share/radishlex/userdb.sqlite3` |
| Rime user data | `$HOME/.local/share/radishlex/rime` |
| 产品配置 | `$HOME/.config/radishlex` |
| settings | `$HOME/.config/radishlex/settings.json` |
| privacy truth source | `$HOME/.config/radishlex/privacy-mode.json` |
| 持久状态 | `$HOME/.local/state/radishlex` |
| 缓存 | `$HOME/.cache/radishlex` |

所有 production 输入必须为绝对、无 `.`/`..` 的路径。既有 symlink、错误 owner、非目录节点、product 目录的 group/other 权限，以及 userdb 的 group/other 权限均失败关闭。product/Rime 目录以 `0700` 创建，userdb 以 `0600`、`O_EXCL`、`O_NOFOLLOW` 创建。测试注入接口只有定义 `RADISHLEX_XDG_TESTING` 的测试编译单元可见，production library 不读取 fixture 路径。

Linux Manager host 直接编译同一个 `xdg_paths.h`/`xdg_paths.cpp` 组件，不在 Dart 或 Flutter runner 中重新拼接 XDG 字符串。`privacy-mode.json` 使用严格 format v1、`0600`、无 symlink/hardlink、原子替换与读回；非法状态失败关闭，不让 addon 解析完整 Manager settings。

addon 的 Linux-only monitor 先监听 config 目录再读取初始 snapshot，避免启动窗口漏掉原子替换；同一 Fcitx event loop 和 FFI 操作前的非阻塞 drain 只在目标事件出现时严格重读。格式、owner/权限、IO、watch 丢失、目录变化或队列异常都转为 privacy enabled，并且日志只保留稳定错误类别。privacy 位变化会刷新已有 session 的 `LearningContext`，防止旧待选择状态继续学习。

## 开发装配

addon 不再从编译期仓库绝对路径读取 RimeData。CMake build 和 staged install 使用同一职责结构：

```text
<fcitx-libdir>/fcitx5/
  radishlex.so
  libradishlex_ime_ffi.so
  radishlex-rime/
    default.yaml
    radishlex_pinyin.schema.yaml
    pinyin_simp.dict.yaml
<prefix>/share/fcitx5/
  addon/radishlex.conf
  inputmethod/radishlex.conf
```

`radishlex.so` 的 ELF runpath 只允许 `$ORIGIN`。运行时从实际已加载 addon 路径派生 sibling FFI 和 `radishlex-rime`，要求固定文件名、绝对规范路径、regular file/目录、无 leaf symlink、addon 目录与资源不允许 group/other 写入；缺失或不安全时 addon 失败关闭，不回退到仓库、当前工作目录或在线下载。

`radishlex_runtime_probe` 是开发诊断而不是 Fcitx daemon 替代品。它对 staged layout 执行相同校验并使用 `dlopen(RTLD_NOW)` 验证 native dependency closure，只输出稳定原因类别，不输出用户数据路径。probe 成功不代表 Engine 已实例化，也不代表 Fcitx input context、Wayland/X11 或真实应用提交已经运行。

## 开发验证

不需要 Fcitx5 或 native library的平台无关 contract：

```bash
./scripts/check-linux-fcitx5.sh
```

真实 Linux Flutter staged bundle 与不启动 GUI 的产品 smoke：

```bash
./scripts/build-manager-linux-product.sh
./scripts/check-manager-linux-product.sh
```

Apple Silicon macOS 的固定 Linux ARM64 编译门禁：

```bash
./scripts/build-linux-fcitx5-container.sh
```

该入口构建固定 digest 的 Debian 13 镜像，以只读方式挂载仓库，并使用 `radishlex-linux-fcitx5-cargo`、`radishlex-linux-fcitx5-target` 两个 Docker named volume 缓存依赖和产物。它会构建 `native-rime` FFI、形成临时 staged install、检查 ARM64 ELF 依赖与 `$ORIGIN`、运行 native loader probe 和七项 CTest；不会写系统目录、安装/启用输入法、写宿主仓库或提供桌面 session。首次执行需要下载 Debian 镜像与软件包，之后复用 Docker/Cargo 缓存。

真实 Linux 开发构建需要既有 C++17、CMake 3.21+、Fcitx5 Core 5.1.9+、librime development environment，以及启用 `native-rime` 的 Rust cdylib。命令只生成开发 build，不安装 addon：

```bash
cargo build -p radishlex-ime-ffi --release --features native-rime
RADISHLEX_IME_FFI_LIBRARY="$PWD/target/release/libradishlex_ime_ffi.so" \
  ./scripts/check-linux-fcitx5.sh --require-fcitx
```

`--require-fcitx` 在非 Linux、缺失 CMake、缺失 cdylib 或 Fcitx5 CMake package 时失败，不自动下载依赖、不启动容器、不写系统目录。脚本固定 `umask 022`，避免开发账号的宽松默认 umask 把 group/other writable 权限泄漏进临时 stage；runtime probe 对这类宽权限的拒绝规则不放宽。Docker wrapper 才负责显式建立依赖环境。开发装配、Fcitx 重启、输入法启用和真实应用交互仍需单独授权。M5-P05 当前只允许按独立安装边界先实现 metadata/rootfs 自动门禁；任何 package/system mutation 仍未授权。
