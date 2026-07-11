# macOS InputMethodKit 平台边界

本文档定义 RadishLex 第一真实平台的稳定实现边界，读者是实现 `platforms/macos-imk/`、Rust input runtime、C ABI 和 macOS 平台验证的开发者。本文不包含系统输入法安装命令、签名与公证步骤、视觉稿、Apple 私钥 backend、远端同步或逐日开发状态；安装与移除另写 runbook，实时批次见 `docs/status/current.md`。

## 产品范围

macOS InputMethodKit 是 M1 离线输入 Alpha 的第一真实平台。M1 必须在真实应用输入框中闭合：

```text
system key event
  -> platform normalization
  -> Rust key result
  -> composition and candidates
  -> candidate selection or engine commit
  -> text commit
```

M1 不要求远端同步、设备授权、恢复码、完整 manager 产品包、最终签名安装包或第二平台。M2 再接真实学习与本地 manager，M4 再闭合普通用户分发。

## 职责分工

InputMethodKit 薄壳负责：

- 系统输入法生命周期和输入 client 切换；
- macOS key event 到 `RadishLexKeyEvent` 的规范化；
- 调用 Rust FFI 并处理 status、`consumed`、commit 和 snapshot；
- marked text、候选展示、候选选择和文本提交；
- 把平台可判断的 secure input、隐私模式和应用类别传给 Rust runtime；
- 将结构化错误送入脱敏诊断，不吞错、不伪装成功。

薄壳不负责：

- 拼音切分、候选生成或候选重排；
- userdb schema、学习事务、删除语义或同步合并；
- 加密、设备授权、恢复码或远端 transport；
- 把平台对象、窗口句柄或 InputMethodKit 生命周期注入 `ime-core`；
- 在按键热路径读取 manager 状态或发起网络请求。

## Runtime 与 session 生命周期

`librime` setup、initialize、notification 和 finalize 必须由 Rust 进程级 runtime 统一管理：

- 同一输入法进程只初始化一次；多个输入 session 复用同一 runtime。
- 单个 Rust input session 只拥有对应 engine session、composition 与候选状态。
- 每个活动输入 context 使用独立 session；不得让两个输入 client 共享可变 composition。
- session 创建、按键、候选提交、reset 和释放遵守 `ime-ffi` owner-thread 契约。
- client 切换、输入法停用、异常取消和进程退出必须有明确 reset/drop 路径。
- finalize 只在进程级 runtime 确认没有活动 session 后执行，不由任意 session drop 触发。

runtime 初始化失败必须返回结构化错误。平台不得静默切换 demo engine，也不得把未初始化状态显示为可用输入法。

## 按键处理契约

平台主入口使用 `docs/ffi-boundary.md` 定义的版本化 key result。处理顺序固定为：

1. 只把当前支持的 key、modifier 和 phase 规范化为稳定 ABI；未知组合明确交还宿主或返回可诊断错误。
2. 在 session owner thread 调用 Rust 按键入口。
3. status 失败时不读取部分结果，并按错误类别执行安全 reset、保留原按键或提示诊断。
4. `consumed = 0` 时返回未处理，让宿主应用继续接收按键。
5. 有即时 commit 时提交对应文本，不从 composition 或下一次 snapshot 猜测。
6. 使用同一 key result 的 snapshot 更新 marked text 与候选。
7. 复制所有 borrowed view 后及时释放 key result；平台状态不得持有 Rust 裸指针。

`Backspace`、`Escape`、`Enter`、`Space`、方向键、翻页键、数字选择、modifier-only、key release 和未知键必须有明确行为表与黑盒测试。未消费按键不能被候选窗或错误处理意外吞掉。

## Composition、候选与提交

- composition 为空时清除 marked text；非空时更新 marked text 与 cursor。
- 候选展示使用 macOS 原生机制或 InputMethodKit 兼容机制，不自造跨平台统一浮窗协议。
- 平台展示索引必须稳定映射到 RadishLex ranked candidate 与 engine commit index。
- 用户选择候选后通过 Rust commit API 提交，平台不得直接把展示文本当作 engine 选择结果。
- engine 即时 commit 与显式候选 commit 使用同一文本提交边界，并清理相应 marked text。
- 取消、失焦、client 切换和 schema 切换不能把旧 composition 提交到新 client。

候选窗口视觉、分页快捷键和无障碍细节可以迭代，但不能改变索引映射、所有权或提交语义。

## 隐私与本地数据

- 输入热路径完全离线；Go server、manager 和网络不参与按键处理。
- secure text entry、P0 应用、用户隐私模式或无法安全判断的敏感场景不得产生学习事件。
- 日志、崩溃报告、截图和人工 smoke 记录不得包含真实输入历史、联系人、密码、证件或支付信息。
- M1 可以不写学习事件，但必须保留向 M2 runtime 传递 privacy context 的稳定位置。
- M2 输入法与 manager 若共享 userdb，必须固定 App Group 或等价目录、文件权限、WAL/busy 策略、migration 所有权和并发测试。
- 平台壳不得直接打开 SQLite；目录只作为受控配置传给 Rust runtime 或 manager bridge。

## Native 依赖与目录

M1 开发版必须明确并隔离：

- RadishLex native library；
- `librime` 动态库及其加载路径；
- 合法来源的 schema/shared data；
- 输入法专用 user data；
- 可选诊断目录。

开发 smoke 不得读取用户现有 Rime 配置或词库目录，也不得把本机绝对路径写入 committed 文档或 fixture。M1 可以使用 runbook 指定的开发目录；M4 必须形成可安装、可升级、可移除且许可证口径清晰的完整 bundle。

## Header、线程与错误

- Swift / Objective-C 只依赖仓库生成或维护并受编译测试约束的 C header/module map。
- ABI version、结构大小、常量值、bool 表达、UTF-8 view、handle 释放和 panic boundary 必须由 host test 固定。
- 平台对象停留在 Swift / Objective-C 层；Rust 不保存 `IMKInputController`、client、event 或 UI object 指针。
- session owner thread 外的调用必须通过明确调度返回 owner thread，不能靠锁绕过未声明的线程约束。
- engine、FFI 或目录错误必须转成结构化、脱敏、可测试的错误；不能 fallback 到 demo engine、空候选或默认成功。

## 开发安装边界

新增、启用或移除系统输入法会修改本机状态，必须在独立 runbook 中说明影响、路径、回滚和 smoke 数据要求，并在执行前获得用户明确授权。自动测试默认只构建 bundle、检查结构和运行 host contract，不自动安装、启用或重启系统输入法服务。

## M1 验收证据

自动证据至少覆盖：

- key result 的 `consumed`、即时 commit、snapshot、错误和释放契约；
- C header/module map 与 Swift/Objective-C 最小调用方编译；
- 进程级 runtime 的单次初始化、多 session、异常释放和最终 finalize；
- key normalization、候选索引映射、取消、reset、schema 切换和未消费按键；
- bundle 结构、native library 加载和缺失依赖的明确失败。

经授权的人工 smoke 至少覆盖：

- 在两个真实 macOS 应用输入框中连续输入合成中文；
- composition、首候选、非首候选、翻页、取消、Backspace、Enter 和中英文混输；
- 未消费快捷键或普通按键仍由宿主接收；
- 断网后输入能力不变；
- 切换输入 client、停用再启用和进程重启后状态正确；
- smoke 只使用合成词，不记录真实敏感输入。

M1 只有在开发者可按 runbook 重复构建、安装、输入和移除后退出。CLI、fixture、host smoke 或 manager 页面不能替代真实 InputMethodKit 证据。

## 停止线

- key result、进程级 runtime 和 header 未闭合前，不开始堆叠复杂候选 UI。
- M1 基础输入可在 M2 学习语义完成前推进，但不得用假学习或平台侧词库绕过 Rust runtime。
- 不为 macOS 壳引入远端同步、平台私钥、设备授权或 manager 依赖。
- 第一平台达到可重复日常输入前，不启动第二平台实现。
- 未经授权不执行安装、启用、系统目录写入、服务重启、签名或发布操作。
