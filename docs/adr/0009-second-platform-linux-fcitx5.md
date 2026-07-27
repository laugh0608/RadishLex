# ADR 0009: 第二真实平台选择 Linux Fcitx5

本文固定 RadishLex 在 macOS 参考产品冻结后的第二真实平台选择、进入顺序和发布关系，读者是平台壳、Rust FFI、Flutter Manager、产品装配与验证维护者。本文不包含 Fcitx5 API 逐函数说明、发行版打包命令、界面稿、实机流水或 Android/Windows/iOS 的具体实现设计；Linux 运行边界见 [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)。

## 状态

Accepted，2026-07-27

## 背景

macOS `26.7.1 (38)` 已完成独立下载、首次安装、固定路径 Manager、公开合成输入、程序修复、默认程序移除和数据保留验收，可以作为第一平台的冻结参考产品。公开发布、Git tag 与真实跨发布升级证据继续延期，不再占用第二平台开发主线。

现有路线要求：

- 第一平台具备可重复日常输入、本地个人化和真实数据管理证据后，才进入第二平台；
- 每次只推进一条真实平台主线；
- 第二平台必须复用 Rust core、engine adapter、userdb、ranker、privacy 和 FFI，不能复制业务真相源；
- 系统候选界面优先使用平台输入法框架能力；
- 真实用户同步继续关闭。

第二平台候选是 Linux Fcitx5 与 Android。Windows TSF 和 iOS Keyboard Extension 保持后置。

## 决策

### 选择 Linux Fcitx5

第二真实平台固定为 Linux Fcitx5，进入 M5。Fcitx5 addon 使用原生 C++/CMake 薄壳连接现有 Rust FFI：

```text
Linux application
  -> Fcitx5 input context / key event
  -> RadishLex Fcitx5 addon
  -> ime-ffi
  -> ime-runtime + engine adapter + ranker + userdb + privacy
  -> Fcitx5 input panel / commit API
```

选择理由：

- Linux 桌面平台能直接验证 Rust 输入核心是否仍含 macOS 路径、生命周期或 UI 假设；
- Fcitx5 以 addon 方式接入输入法，并提供输入上下文、候选和界面扩展点，适合维持原生薄壳；
- Wayland 与 X11 可以复用同一业务核心，平台差异留在 Fcitx5 和桌面环境；
- 相比 Android，本阶段不需要同时承担软键盘 UI、`InputMethodService`、NDK/OEM 差异和移动端资源约束；
- 相比先实现 IBus，Fcitx5 是路线图既定的 Linux 第一选择；IBus 只有在 Fcitx5 主路径稳定且存在明确用户需求时再评估。

Fcitx5 官方项目与开发入口：

- [Fcitx5 repository](https://github.com/fcitx/fcitx5)
- [Develop a simple input method for Fcitx5](https://fcitx-im.org/wiki/Develop_an_simple_input_method)
- [Fcitx basic concepts](https://fcitx-im.org/wiki/Basic_concept)

### 平台推进顺序

计划内真实平台顺序固定为：

1. macOS InputMethodKit：冻结参考产品与回归基线；
2. Linux Fcitx5：当前主线；
3. Android `InputMethodService`；
4. Windows TSF；
5. iOS Keyboard Extension。

进入后一平台前，前一平台必须达到路线图定义的输入、个人化、隐私、数据并发、安装维护和实机证据。不得以创建目录、生成空包、编译单个桥或展示静态候选替代平台退出标准。

### 对外发布与平台验收分离

当前不发布 macOS build 38，不创建正式 Git tag，也不把远端 draft 转为正式 Release。对外正式发布暂缓，待计划内 macOS、Linux Fcitx5、Android、Windows 与 iOS 均达到各自退出标准后再进行统一发布评审。

该决定不合并各平台的内部验收：

- 每个平台仍有独立里程碑、退出标准和回归基线；
- 已冻结平台继续接受公共 ABI、userdb、ranker、RimeData 和 Manager 变更的回归检查；
- 后续平台不能通过修改共享语义破坏已验收平台；
- 统一发布评审只决定是否公开交付，不替代任何平台证据。

### M5 批次

M5 按以下顺序推进：

1. `M5-P01`：第二平台 ADR、Linux 平台边界、阶段路线与停止线；
2. `M5-P02`：Fcitx5 addon、Rust FFI 接线、确定性开发构建与自动 contract；
3. `M5-P03`：真实 Linux 应用输入、Wayland 主路径、X11 兼容、生命周期与隐私验收；
4. `M5-P04`：Linux Flutter Manager、XDG 产品数据、并发 userdb 与本地个人化验收；
5. `M5-P05`：Linux 安装、升级、修复、移除、数据保留与发行载体。

每一批完成匹配验证后再进入下一批。P01 的边界批准并不证明 Linux 平台能力已经实现。

## 后果

- `platforms/linux-fcitx5/` 在 M5-P02 才创建；P01 不用占位目录表示进度。
- 现有 `ime-ffi` 是唯一跨语言输入边界；新增能力优先扩展稳定 ABI，不为 Fcitx5 建第二套 Rust bridge。
- Fcitx5 addon 只保存平台对象与 session 绑定，不读取 SQLite、调用 Rime 私有 API 或实现候选排序。
- 候选展示使用 Fcitx5 input panel，不引入 Flutter、egui 或独立浮窗。
- Linux 路径必须经单一 XDG resolver 形成，addon 与 Manager 不能分别拼接路径。
- macOS build 38 的产物、校验、验收记录和保留数据语义继续作为参考证据；不因主线切换而删除或改写。
- Android Keystore bridge 继续只表示平台密钥能力研究，不表示 Android IME 已开始或已具备输入能力。

## 停止线

- 不同时实现 Linux、Android、Windows 或 iOS 平台壳。
- 不把 macOS `Application Support`、AppKit、InputMethodKit、TIS 或 bundle 生命周期注入共享 Rust core。
- 不在 C++ addon 中复制 userdb、ranker、privacy、同步或 engine adapter。
- 不自造候选浮窗，不让 Flutter Manager 进入输入热路径。
- 不以合成 host、容器内单元测试或静态包替代真实 Linux 桌面输入。
- 不启用真实用户同步。
- 不发布、打 tag、上传新载体或修改远端 draft，除非另行获得授权。
