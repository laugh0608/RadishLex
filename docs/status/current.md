# RadishLex 当前状态

本文是日常推进短入口，供维护者判断里程碑、验证基线、停止线和下一步。详细设计与历史进入边界文档、runbook 和周志。

## 当前判断

- 复核日期：2026-08-05（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品
- 当前主批次：M5-P04 Linux Manager、同库个人化与本地管理验收
- 已退出：M0-M3；M4 macOS build 38 单版本产品验收已冻结；M5-P01 第二平台决策与运行边界；M5-P02 Fcitx5 addon、共享 FFI 与开发构建；M5-P03 真实 Linux 桌面输入与隐私验收
- 真实用户同步：保持关闭；只允许合成数据与受控集成测试

## M5-P03 完成证据与 M5-P04 当前进度

M5-P02/P03 已建立 C++17/CMake addon、ABI v9 owned projection、Fcitx input panel/session、共享 XDG resolver，并在 UTM Debian 13 ARM64 完成 Wayland/X11、GTK/Qt/Electron/Firefox/Terminal、候选交互、生命周期、断网和隐私实机验收。password、terminal、unknown 与 Qt `Sensitive` 均不学习；GTK4 `PRIVATE` 继续按 unknown 失败关闭。用户级开发装配不写 `/usr`，也不冒充 M5-P05 产品安装。

M5-P04 已落地 Linux Flutter runner、engine 前 `umask(0077)`、固定 bundle `.so`、共享 XDG/Manager runtime、严格 `privacy-mode.json`、变更感知和失败关闭的应用粗分类。Wayland/X11 评审后，生产 allowlist 只加入精确 `firefox-esr -> browser`；默认关闭的 evidence 模式只输出固定 opaque token 与 capability 变化，产品二进制不含取证字符串。Debian ARM64 Flutter 3.44.0 Release bundle、ELF、正式 ABI、native-rime 与 Dart smoke 均通过。

2026-08-04 的生产纵向实机已证明 Firefox 选择公开合成候选后，Linux Release Manager 可刷新同一 XDG userdb，后续候选提升与 `browser` rank explain 的 user/frequency/recency/context 信号一致。Manager bridge 已按六个允许粗类别查询 explain；Linux 设置文案和字体 fallback 同时覆盖中文、Latin 与数字。

2026-08-05 继续闭合 privacy 与删除恢复链：Manager 写入 `privacy=true` 后一次 Firefox 候选选择保持 selection/frequency 零增量，写回 `false` 后一次选择只各增加 1。首次 Space 提交暴露 addon 以原始 `states == 0` 判断候选键、导致 GTK/Fcitx 内部状态绕过 display-index selection；实现已改用 Fcitx key matching 语义，并增加第七项真实 Fcitx CTest，ARM64 staged build、loader probe 与 7/7 CTest 通过。

同一修复 addon 随后完成 Manager 删除、既有 session 防复活、Fcitx 重启后新 session 防复活、explicit restore 与恢复后新 session 重新学习。删除期间普通选择只增加 P1 selection，不恢复 term/ranker 或改写 tombstone；恢复版本严格晚于删除版本，旧 frequency 不复活，首次新选择从 frequency 1 重新开始。Manager 终态刷新为 user terms 1、selection events 5、suppressed 0、deleted 0，多次 Fcitx 重启与 Manager 并发期间无投影、busy 或 migration 错误。

M5-P04 尚未完成：真实 Manager 导入检查/导入/导出、Manager 进程重启和桌面会话重启仍待闭合；M5-P05 安装维护继续关闭。

## macOS 冻结参考

macOS `26.7.1 (38)` 保持冻结参考产品；安装、Manager、输入、repair、移除与数据保留证据不变。DMG SHA-256 为 `f171e74bdc0a429655a84b30429481bce3926b17076d09298feed77d9ce4ce4e`，未使用 Developer ID 或 Apple 公证；远端 draft 未发布、没有正式 Git tag。

build 38 的详细身份、事务、载体与实机流水由 [macOS 产品包边界](../macos-product-package-boundary.md)、[macOS 程序安装事务](../macos-installation-transaction.md)、[社区载体 runbook](../runbooks/macos-release-carrier.md) 和历史周志维护，不在当前状态入口重复展开。真实跨发布升级、source rollback、Developer ID/公证和正式 Release 仍属于后续兼容与统一发布评审。

## 当前停止线

- 不发布 build 38，不创建正式 Git tag，不修改远端 draft；本地提交不推送，任何远端动作仍需另行授权。
- 保留 build 38 assembly、hash/evidence、验收记录与默认 remove 后的数据语义，不清理或改写为其他基线。
- 不宣称社区包具备 Developer ID、Apple 公证、Gatekeeper 自动通过或 Apple 已验证。
- 首个正式版本继续关闭真实用户同步，不开放恢复码、设备授权、撤销或轮换的产品成功入口。
- P0 永不学习/同步；P1 原始事件只留本地；P2 只允许端到端加密对象。
- 输入热路径保持完全本地；Go server 不解密、不排序、不保存明文用户词或候选偏好。
- M5 只推进 Linux Fcitx5，不并行实现 Android、Windows 或 iOS，不为形式统一把 Application Support v1 迁入 App Group。
- Fcitx5 addon 只承担平台生命周期、按键、框架候选面板、commit、共享 privacy snapshot 与 FFI 接线；不得复制 engine、ranker、userdb、Rust 学习策略或同步逻辑。
- 不自动清理 staging、backup、历史 operation 或身份绑定终态材料。

## 下一步顺位

1. 在同一 XDG userdb 上闭合 Manager 导入检查、导入与导出，确认普通导入不能复活 tombstone，导出只含用户明确请求的 P2 词条视图。
2. 完成 Manager 进程重启和桌面会话重启；复核现有 Fcitx 重启、双进程 WAL/busy 与终态刷新证据没有重复学习、迁移漂移或状态丢失。
3. M5-P05 需明确 Linux CJK 字体是随包交付还是由发行依赖保证；P04 当前系统字体 fallback 的实机通过不能替代安装产品依赖治理。
4. M5-P05 安装维护、旧临时资产清理、远端推送和其他平台均不进入当前批次；对应动作继续单独授权。

## 验证入口

```bash
./scripts/check-macos-product-metadata.sh
./scripts/check-macos-install-layout.sh
./scripts/check-product-install-core.sh
./scripts/check-macos-install-adapter.sh
./scripts/check-macos-install-coordinator.sh
./scripts/check-macos-installer.sh
./scripts/check-macos-release-carrier.sh
./scripts/check-manager-product.sh
./scripts/check-macos-imk.sh
./scripts/check-macos-upgrade-product-coordination.sh
./scripts/check-linux-fcitx5.sh
./scripts/check-manager-linux-product.sh
./scripts/build-linux-fcitx5-container.sh
./scripts/check-repo.sh
./scripts/check-docs.sh
git diff --check
```

真实系统安装、输入源变更、用户数据、公开上传和 Release 仍需对应授权。

## 阅读索引

- [路线图](../roadmap.md)
- [第二平台 Linux Fcitx5 ADR](../adr/0009-second-platform-linux-fcitx5.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [Manager 本地验收](../manager-local-acceptance.md)
- [macOS 产品包边界](../macos-product-package-boundary.md)
- [macOS 程序安装事务](../macos-installation-transaction.md)
- [macOS Installer App 边界](../macos-installer-app-boundary.md)
- [macOS 数据升级协调器](../macos-data-upgrade-coordinator.md)
- [macOS 社区 ad-hoc DMG Runbook](../runbooks/macos-release-carrier.md)
- [本周周志](../devlogs/2026-W32.md)
