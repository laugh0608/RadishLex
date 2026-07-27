# RadishLex Fcitx5 addon

本文说明 Linux Fcitx5 addon 的职责、开发构建、XDG 数据路径和验证入口，读者是平台壳、Rust FFI 与未来 Linux Manager host 的维护者。本文不包含发行版安装、系统输入法启用、真实桌面验收或 Linux 产品升级；这些分别属于 M5-P03 与 M5-P05。

## 当前证据

M5-P02 首个实现批次已经建立真实 C++ 源码、CMake target、addon/input method metadata 和自动 contract，不是占位目录。当前机器是 macOS，未安装 CMake、Fcitx5 development package 或 Linux runtime，因此目前只证明：

- Apple clang 的 C++17 严格编译通过；
- ABI v9 contract、key projection、owned `KeyResult`/snapshot、display-index selection、owner-thread 和 reset/free/shutdown 顺序通过 fake-FFI contract；
- XDG 默认路径、显式 XDG 根、`0700`/`0600`、relative path、symlink、宽权限和 production/test override 隔离通过；
- CMake source 固定 Fcitx5 Core、native-rime `ime-ffi`、锁定 RimeData 和框架 input panel 依赖。

这些结果不是 Fcitx5 addon 的 Linux 编译或运行证据，也不证明 Wayland、X11 或真实应用输入。

## 组件结构

```text
include/radishlex/linux/
  ffi_projection.h    ABI v9 owned C++ projection
  key_projection.h    platform key to RadishLex key contract
  xdg_paths.h         addon/Manager shared XDG resolver
src/
  fcitx_addon.*       Fcitx lifecycle, input panel and commit adapter
  ffi_projection.cpp  KeyResult/snapshot copy and owner-thread guard
  key_projection.cpp  Unicode/named key/modifier/phase validation
  linked_ffi_api.cpp  only direct C ABI symbol table
  xdg_paths.cpp       effective-user XDG and private path enforcement
config/
  radishlex-addon.conf.in
  radishlex.conf.in
tests/
  ffi_projection_test.cpp
  xdg_paths_test.cpp
```

`fcitx_addon.cpp` 不包含 Rime、SQLite、ranker、userdb、privacy policy 或同步实现。它只映射 Fcitx capability、按键和生命周期，消费 Rust-owned snapshot，使用 Fcitx input panel，并把 Rust commit 交给当前 input context。

## ABI v9 审计结论

可直接复用的接口：

- `radishlex_ffi_contract`：校验 ABI v9、`owner_thread` 和 `catch_unwind`；
- `radishlex_session_new_personalized_rime`：创建共享 runtime 上的每-context 产品 session；
- `radishlex_session_set_learning_context`：传入 secure/sensitive/privacy/known 摘要；
- `radishlex_session_handle_key_event`：取得 consumed、可选 commit 和同事件 snapshot；
- `radishlex_session_select_candidate`：只接受 Rust display index；
- `radishlex_session_reset`、`radishlex_session_free` 与 `radishlex_rime_runtime_shutdown`：形成确定生命周期。

本批没有扩展 ABI。snapshot 不输出 Fcitx 私有对象或 UI cursor。Fcitx candidate list 维护当前可见 cursor；数字键、Space 和鼠标选择最终都调用同一个 display-index selection，PageUp/PageDown 仍作为稳定 named key 交给 Rust engine 后重建 candidate list。这避免在 C++ 中推断 engine index 或复制 Rime highlight 逻辑。

尚未进入共享 ABI 的 Linux 产品能力：

- Linux install/data startup gate 和版本化产品 identity：M5-P05；
- Linux Manager 的 privacy mode 配置来源：M5-P04；
- 发行版包、系统域路径与升级 receipt：M5-P05。

在普通、非 terminal context 无可靠分类信号时，addon 当前传 `context_known = 0`；Rust 因而使用 engine 顺序且不读写 userdb。Fcitx 明确提供 `Password`、`Sensitive` 或 `Terminal` capability 时只投影对应受控摘要，不传 program name、窗口标题或正文。

## XDG 路径

production resolver 使用 `geteuid` + `getpwuid_r` 取得 authoritative home，只读取 freedesktop 定义的四个 XDG 环境变量，不读取 `HOME` 或 `RADISHLEX_*` test override。

| 内容 | 默认路径 |
| --- | --- |
| 产品数据 | `$HOME/.local/share/radishlex` |
| userdb | `$HOME/.local/share/radishlex/userdb.sqlite3` |
| Rime user data | `$HOME/.local/share/radishlex/rime` |
| 产品配置 | `$HOME/.config/radishlex` |
| settings | `$HOME/.config/radishlex/settings.json` |
| 持久状态 | `$HOME/.local/state/radishlex` |
| 缓存 | `$HOME/.cache/radishlex` |

所有 production 输入必须为绝对、无 `.`/`..` 的路径。既有 symlink、错误 owner、非目录节点、product 目录的 group/other 权限，以及 userdb 的 group/other 权限均失败关闭。product/Rime 目录以 `0700` 创建，userdb 以 `0600`、`O_EXCL`、`O_NOFOLLOW` 创建。测试注入接口只有定义 `RADISHLEX_XDG_TESTING` 的测试编译单元可见，production library 不读取 fixture 路径。

未来 Linux Manager host 必须链接同一个 `xdg_paths.h`/`xdg_paths.cpp` 组件，不得在 Dart 或 Flutter runner 中重新拼接 XDG 字符串。

## 开发验证

不需要 Fcitx5 或 native library的平台无关 contract：

```bash
./scripts/check-linux-fcitx5.sh
```

真实 Linux 开发构建需要既有 C++17、CMake 3.21+、Fcitx5 Core 5.1.9+、librime development environment，以及启用 `native-rime` 的 Rust cdylib。命令只生成开发 build，不安装 addon：

```bash
cargo build -p radishlex-ime-ffi --release --features native-rime
RADISHLEX_IME_FFI_LIBRARY="$PWD/target/release/libradishlex_ime_ffi.so" \
  ./scripts/check-linux-fcitx5.sh --require-fcitx
```

`--require-fcitx` 在非 Linux、缺失 CMake、缺失 cdylib 或 Fcitx5 CMake package 时失败，不自动下载依赖、不启动容器、不写系统目录。开发安装、Fcitx 重启、输入法启用和真实应用交互需要单独授权与 M5-P03 runbook。
