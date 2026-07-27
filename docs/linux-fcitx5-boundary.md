# Linux Fcitx5 平台边界

本文定义 RadishLex 第二真实平台的运行职责、FFI 接线、输入语义、XDG 数据、隐私、构建和验收边界，读者是 `platforms/linux-fcitx5/`、`ime-ffi`、Linux Flutter host 与验证入口的实现者。本文不包含发行版安装命令、包管理仓库发布、具体 UI 样式、远端同步启用或逐日验证流水；平台选择见 [ADR 0009](adr/0009-second-platform-linux-fcitx5.md)，当前批次见 [当前状态](status/current.md)。

## 状态与产品范围

状态：M5-P02 开发构建批次。平台无关 C++ contract 与 Debian 13 ARM64 的 Fcitx5 编译/链接已通过；真实桌面运行尚未验证。

M5 要证明 Linux 平台能够复用现有产品核心完成：

```text
system key event
  -> Fcitx5 addon normalization
  -> Rust KeyResult
  -> composition and ranked candidates
  -> Fcitx5 input panel
  -> display-index selection or engine commit
  -> application commit
  -> local learning and Manager management
```

输入热路径必须离线。远端同步、设备授权、恢复码、Android Keystore 和 Go server 均不进入 Fcitx5 按键处理。

## 组件职责

### Fcitx5 addon

原生 addon 只负责：

- Fcitx5 addon 与 input method 生命周期；
- input context 与 Rust session 的绑定、重置和释放；
- Fcitx key event 到 `RadishLexKeyEvent` 的规范化；
- 调用 ABI contract、session、learning context、key result 和 candidate selection；
- 将 composition、candidate、highlight 和 commit 投影到 Fcitx5；
- 使用框架 input panel，不自行绘制跨桌面浮窗；
- 把结构化错误写入脱敏诊断并选择明确的按键回传行为。

addon 不负责：

- 拼音切分、候选生成或候选重排；
- SQLite schema、userdb migration、学习事务或删除语义；
- Rime session 私有生命周期和候选对象；
- Flutter 页面、同步、设备密钥或远端 transport；
- 根据窗口标题、应用正文或原始输入流自行推导个人化特征。

### Rust input runtime

Linux 继续使用 `ime-ffi` ABI v9 已有的：

- `radishlex_ffi_contract`；
- personalized Rime session 与 owner-thread 约束；
- `radishlex_session_handle_key_event`；
- `radishlex_session_select_candidate`；
- `RadishLexKeyResult`、snapshot、display index 和 learning disposition；
- `RadishLexLearningContext`；
- `radishlex_session_reset`、session free 与 Rime runtime shutdown。

如果 M5 实现发现现有 ABI 无法表达 Fcitx5 必需行为，应先补平台无关语义和 ABI contract，再由 macOS 与 Linux 共同验证。不得增加只传 Fcitx 私有对象、原始 key symbol、窗口指针或数据库路径的业务入口。

首批 ABI 审计结论是 v9 足以表达当前 addon 输入链，不需要新增 symbol 或版本：

- Fcitx normalized key 在 C++ 转成稳定 char/named/modifier/phase；
- owned `KeyResult` 同时提供 consumed、commit、learning disposition 与 snapshot；
- snapshot 的 candidate `index` 已是 Rust display index，`engine_index` 只复制到只读投影，不回传；
- Fcitx candidate list 维护可见 cursor，数字、Space 和鼠标选择最终调用同一个 display-index selection；
- PageUp/PageDown 作为稳定 named key 进入 Rust 后用新 snapshot 重建列表；
- reset/free/shutdown 已足以表达 per-context session 与进程 teardown。

当前真实缺口不在 ABI 或 addon 编译，而在 Fcitx5 桌面运行证据、Linux Manager privacy 配置来源、产品 startup gate 和发行安装事务。后三项分别留在 P04/P05，不为 P02 增加平台私有 ABI。

### Flutter Manager

Linux Manager 复用现有 Dart models、ManagerBridge 和 Rust userdb 管理能力。Linux host 只负责：

- 通过统一 XDG resolver 取得固定产品路径；
- 加载与产品 metadata 匹配的 Rust native library；
- 提供平台文件选择与本地权限收紧；
- 把隐私模式等平台配置映射到既有受控 bridge；
- 显示 Linux 产品诊断，不重新解释 ranker、删除或同步语义。

Manager 与 addon 必须打开同一 userdb，使用现有 WAL、busy timeout 和 migration 所有权；不得各建一份数据库或通过网络协调本地状态。

## 生命周期与线程

`RadishLexSession*` 保持 `owner_thread` 策略。M5-P02 必须固定一个可验证的 addon 调用线程模型：

- 所有 Rime session 创建、按键、选择、reset 和释放在同一串行 owner thread 执行；
- 每个活动 Fcitx input context 拥有独立 Rust session，不共享可变 composition；
- addon deactivate、input context 销毁、程序切换和异常取消必须 reset 对应 session；
- session drop 只释放本 session；进程退出先释放全部 session，再在 owner thread 调用 Rime runtime shutdown；
- UI 更新只能消费同一次 FFI 调用返回的 owned `KeyResult`/snapshot，不能异步拼接旧 composition 与新 candidate；
- Fcitx 回调线程若不能直接满足 owner-thread contract，使用显式串行执行器，不通过无锁共享 session 指针规避约束。

进程级 runtime 初始化失败必须阻止 RadishLex input method 激活，并返回稳定错误；不得退回 demo engine 或空候选成功态。

## 按键规范化与回传

addon 必须按 `docs/ffi-boundary.md` 的稳定模型形成字符键、named key、modifier 与 phase：

- 只有可验证的 Unicode scalar value 才形成 char event；
- Space、Enter、Backspace、Escape、Tab、方向键和 PageUp/PageDown 映射到稳定 named key；
- Shift、Control、Alt、Meta 只使用 ABI 已知 bit；
- 未知 key、未知 modifier、无有效文本的快捷键和平台保留组合不得猜测为普通字符；
- release 是否送入 runtime 由明确 contract 决定，不能和 press 重复提交；
- `consumed = 0` 时把原始事件交还 Fcitx/宿主应用；
- `consumed = 1` 时不得让同一事件再次进入应用；
- `commit_present = 1` 时只提交同一次结果携带的 UTF-8 commit。

FFI status、结构版本、UTF-8 或 snapshot 校验失败时，addon 清除不可证明一致的 UI 状态并记录稳定错误。已有 composition 期间不能静默把失败按键当作成功提交。

## Composition、候选与选择

Fcitx5 input panel 是候选展示真相，Rust snapshot 是候选内容与排序真相：

- preedit 来自 snapshot composition；
- candidate 的 text、reading、comment 和排序来自 Rust snapshot，不从 Rime 私有对象补字段；
- 当前页和 highlight 使用 Fcitx5 candidate list 能力；
- 选择只把 Rust display index 传给 `radishlex_session_select_candidate`；
- display index 到 engine index 的映射只由 Rust runtime 保存和消费；
- 键盘数字、Space、方向导航、PageUp/PageDown 与鼠标选择最终进入同一 Rust selection 语义；
- snapshot 更新后旧 candidate 对象和旧 index 全部失效；
- commit 后清空 preedit/candidate；分段 composition 只按新 snapshot 重绘；
- addon 不缓存跨 input context 的候选列表。

Wayland 下不得自行定位或绘制候选浮窗。X11 兼容也继续使用 Fcitx5 UI addon；桌面环境视觉差异不进入 Rust core。

## 学习与隐私上下文

每次可能影响选择或学习的输入前，addon 都要提供当前 `RadishLexLearningContext`：

- 平台明确报告 password、secure 或敏感输入时设置 `secure_input` 或 `sensitive_application`；
- 无法可靠判断当前上下文时设置 `context_known = 0`；
- 只允许 `general`、`browser`、`chat`、`code`、`editor`、`office`、`terminal`、`other`；
- 不向 Rust 传递窗口标题、文档内容、URL、联系人、原始应用 ID 或输入框正文；
- context 改变时遵循 FFI 规则清除未确认选择意图并刷新 snapshot；
- secure、sensitive 或 unknown 上下文只使用 engine 顺序且不读写 userdb；
- privacy mode 可以使用既有本地摘要，但不产生新学习写入。

Fcitx5 或桌面协议无法提供可靠 secure signal 的环境不得标记为普通上下文。该环境只能以 unknown 的失败关闭策略继续输入，并在平台兼容矩阵中记录个人化受限。

## XDG 产品路径

Linux 不复用 macOS `Application Support`。M5-P02 先实现单一、可测试的 XDG resolver，由 addon、Manager、诊断和未来安装协调层共同调用。

语义分区固定为：

| 类别 | XDG 根 | 内容 |
| --- | --- | --- |
| 用户数据 | `XDG_DATA_HOME` | userdb、Rime user data、不可重建的用户词库与学习状态 |
| 用户配置 | `XDG_CONFIG_HOME` | 用户可管理且与 schema 有兼容约束的本机设置 |
| 持久运行状态 | `XDG_STATE_HOME` | 产品事务 receipt、脱敏运行状态与必要审计摘要 |
| 可重建缓存 | `XDG_CACHE_HOME` | 可删除并重新生成的 cache，不保存唯一用户数据 |

当变量未设置时使用 freedesktop XDG 规范默认位置。resolver 必须：

- 从有效用户上下文形成绝对规范路径，不接受 UI、环境外参数或任意调用方覆盖；
- 拒绝相对路径、`..`、symlink 逃逸、错误所有者和宽权限敏感文件；
- 明确新建目录/文件的权限与所有权；
- 返回结构化路径对象，不让 C++、Dart 与 Rust 分别拼接字符串；
- 保持测试 override 与生产 resolver 隔离，生产构建不读取 fixture 路径；
- 在 M5-P05 前固定具体子目录、receipt、安装目标和 migration 清单。

M5-P02 已固定当前数据子路径：

```text
${XDG_DATA_HOME:-$HOME/.local/share}/radishlex/userdb.sqlite3
${XDG_DATA_HOME:-$HOME/.local/share}/radishlex/rime
${XDG_CONFIG_HOME:-$HOME/.config}/radishlex/settings.json
${XDG_STATE_HOME:-$HOME/.local/state}/radishlex
${XDG_CACHE_HOME:-$HOME/.cache}/radishlex
```

production resolver 使用 `geteuid` + `getpwuid_r` 取得 authoritative home，不读取 `HOME` 或 `RADISHLEX_*` fixture override。四个 XDG 环境变量只接受绝对且无 dot component 的路径；既有 symlink、错误 owner、非目录节点、product 目录 group/other 权限和 userdb group/other 权限失败关闭。product/Rime 目录以 `0700` 创建，userdb 以 `0600`、`O_EXCL`、`O_NOFOLLOW` 创建。测试注入 API 只在 `RADISHLEX_XDG_TESTING` 编译态存在；未来 Linux Manager host 必须链接同一 resolver 源码，不得在 Dart 中重复拼接。

配置与数据分居多个 XDG 根时，升级协调必须显式覆盖两者的一致性，不能只迁移 userdb 后假定 settings 兼容。P02/P03 不以未设计的自动升级写入真实用户数据。

## Native library、RimeData 与构建

M5-P02 的开发构建必须形成可复验依赖图：

- C++ addon 通过稳定 C header 链接 Rust `cdylib`；
- `librime` 只由 Rust engine adapter 使用，C++ addon 不直接链接其业务 API；
- RimeData 使用仓库已锁定的产品来源、hash 和逐资产许可证；
- native library、ABI version、userdb schema、RimeData version 和 addon metadata 在构建时一致；
- 构建不从运行时下载 schema、词库、模型或二进制；
- 开发安装与正式发行载体分开，P02 不把本地复制命令称为产品安装；
- 发行版包、签名、系统域目标、升级与移除在 M5-P05 单独固定。

当前 `platforms/linux-fcitx5/CMakeLists.txt` 已固定 C++17、CMake 3.21+、Fcitx5 Core 5.1.9+、native-rime `libradishlex_ime_ffi` 显式路径和仓库锁定 RimeData。`./scripts/check-linux-fcitx5.sh` 在无 Fcitx 环境编译 projection/XDG contract；`--require-fcitx` 只在 Linux 且调用方提供既有 native-rime cdylib 时配置、构建 addon、检查 ELF 动态依赖并运行 CTest，不下载依赖、不安装或启用输入法。

`platforms/linux-fcitx5/dev/Dockerfile` 以 digest 固定 Debian 13 ARM64 基础镜像，安装发行版提供的 Rust 1.85.0、CMake 3.31.6、Fcitx5 Core 5.1.12 和 librime 1.13.1 development package。`./scripts/build-linux-fcitx5-container.sh` 只读挂载仓库，使用独立 named volume 缓存 Cargo registry/target，构建启用 `native-rime` 的 ARM64 ELF cdylib，再执行 `--require-fcitx` 强门禁。该环境已真实编译并动态链接 `libradishlex_ime_ffi.so` 与 `radishlex.so`，也修正了 Linux ARM64 `c_char` signedness、GCC 14 enum boundary 和 `fcitx::Key` 非 literal type 差异。

Docker Desktop 提供的 Linux VM 是 M5-P02 可持续编译环境，不是桌面验收环境：容器没有 Fcitx daemon、Wayland/X11 session 或真实应用 input context。因此当前只能记为 Linux ARM64 编译/链接验证，不能记为 Fcitx5 平台运行或 M5 退出。

## 验证分层

### 自动 contract

M5-P02 至少覆盖：

- C header 与 Rust ABI contract/version；
- key、modifier、phase 和 UTF-8 映射；
- consumed/commit/snapshot 的单次结果一致性；
- display index 选择与候选失效；
- input context/session 隔离；
- reset、deactivate、drop 与 runtime shutdown 顺序；
- secure/sensitive/unknown/privacy mode；
- XDG resolver、权限、symlink 和生产/test override 隔离；
- addon metadata、native dependency 和 RimeData presence；
- macOS 与仓库既有 ABI 回归。

### 真实平台

M5-P03/P04 至少覆盖：

- 一套可持续复验的 Linux 桌面和 Fcitx5 版本；
- Wayland 主路径与 X11 兼容路径；
- GTK、Qt、Electron、浏览器和终端文本输入；
- composition、候选导航、翻页、数字/Space/鼠标选择、commit、cancel 和 reset；
- 应用切换、input method 切换、Fcitx 重启和桌面会话重启；
- 断网输入；
- password/secure/unknown 上下文不学习；
- Manager 与 addon 并发读取、写入、删除、恢复及重启保持；
- 合成词学习后排序变化、删除不复活和 explain 一致；
- 不含真实输入历史的脱敏诊断。

容器、headless 测试和合成 input context 只能证明局部 contract，不替代真实桌面证据。需要用户手动切换输入法或实体输入时，AI 只读监视并等待结果，不合成操作冒充验收。

## M5 退出标准

- Fcitx5 addon 在真实 Wayland 与 X11 主路径中稳定输入并使用框架候选面板；
- Rust core、engine adapter、ranker、userdb 和 privacy 仍是唯一业务真相源；
- addon 与 Manager 使用同一 XDG 数据和同一 userdb；
- 本地学习可复验地改变后续候选，用户可查看、导出、删除和恢复；
- secure、sensitive 与 unknown 上下文不产生个人化读写；
- 多 input context、Manager 并发、Fcitx 重启和桌面会话重启不损坏数据；
- 安装、升级、修复、默认移除和数据保留具备自动门禁与实机证据；
- macOS 冻结参考基线和仓库门禁继续通过；
- 真实用户同步保持关闭，未执行公开发布。

## 当前停止线

- P01 只冻结设计，不创建占位平台目录或宣称 Linux 已实现。
- P02 先完成 addon/FFI/build contract，不提前并行做 Android IME。
- 不把 Docker 编译通过写成 Fcitx5 桌面或平台验收。
- 不复制 Fcitx5 或其他输入法实现；只依据公开 API、行为规格和自己的测试实现。
- 不把系统级安装、包管理写入或桌面设置变更纳入无授权自动验证。
- 不把同步、云端候选或远端配置引入输入热路径。
