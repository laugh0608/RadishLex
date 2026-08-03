# RadishLex 当前状态

本文是日常推进短入口，供维护者判断里程碑、验证基线、停止线和下一步。详细设计与历史进入边界文档、runbook 和周志。

## 当前判断

- 复核日期：2026-08-03（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品
- 当前主批次：M5-P04 Linux Manager、同库个人化与本地管理验收
- 已退出：M0-M3；M4 macOS build 38 单版本产品验收已冻结；M5-P01 第二平台决策与运行边界；M5-P02 Fcitx5 addon、共享 FFI 与开发构建；M5-P03 真实 Linux 桌面输入与隐私验收
- 真实用户同步：保持关闭；只允许合成数据与受控集成测试

## M5-P03 完成证据与 M5-P04 入口

M5-P02 已建立 `platforms/linux-fcitx5/` 的 C++17/CMake addon、ABI v9 owned projection、Fcitx input panel/session、共享 XDG resolver、addon/input method metadata 与自动门禁。固定 Debian 13 ARM64 环境已通过 native-rime FFI、addon-relative staged 装配、`$ORIGIN`、资源权限/symlink、构建路径泄漏、`dlopen(RTLD_NOW)` 和三项 CTest；容器证据只承担可持续构建与无头加载，不替代桌面验收。

M5-P03 在 UTM Debian 13.6 ARM64 建立了 GNOME Wayland/X11 真实桌面基线和不写 `/usr` 的用户级开发装配。Fcitx5 daemon 实际加载已校验的 `radishlex.so`、sibling FFI 与锁定 RimeData；产品 XDG 目录/文件保持 `0700`/`0600`。候选 press/release 配对与 preedit UTF-8 cursor/`DontCommit` 两项修复均通过宿主 contract、guest 强门禁和用户实体操作复验，ABI 仍为 v9，候选内容、排序与 display-index 选择仍由 Rust 真相源负责。

Wayland 主路径已覆盖 GTK4、Terminal、Firefox、原生 Qt6 和官方 Electron ARM64 runtime；X11 对照与候选、commit、reset、焦点、Fcitx/桌面重启矩阵通过。Qt 结论只采用明确 Wayland QPA 与 Fcitx Qt6 plugin 证据，不以 FeatherPad 的库映射代替 backend 取证。

隐私验收覆盖 unknown、terminal、Firefox password 和 Qt `Sensitive`；GTK4 `PRIVATE` 未传播敏感 bit，因此继续由 `context_known = 0` 失败关闭。受限网络与整机断网后 userdb v9 学习聚合始终为零。跨会话 autostart 已按 Debian 官方 desktop entry 修复并复验，但仅是可逆开发基线，不冒充 M5-P05 产品安装。

M5-P04 已在 [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md) 固定 product bootstrap、bundle `.so`、共享 XDG/userdb、独立 privacy 配置、受控程序粗分类、WAL 并发和验收矩阵。现有 Flutter 页面、ManagerBridge、ABI v9 与 Rust 业务能力直接复用。当前入口是完整 Linux host/真实 bridge/同库自动证据，不先建空 runner；普通应用保持 unknown，直到程序身份和敏感字段传播经实机评审。

首个代码子批已落 Linux runner、engine 前 `umask(0077)`、固定 bundle `.so`、共享 XDG/Manager runtime、严格 `privacy-mode.json` 原子读写/回滚和平台中立 Dart 路径契约。平台无关 C++ contract、Flutter analyze/97 项测试、仓库基线及“runtime 学习—Manager C ABI 管理—新 runtime 观察”的 native-rime 同库链已通过；真实 Linux Flutter bundle 尚未构建，因此本子批未退出，也未宣称 Linux Manager 可用。

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
- Fcitx5 addon 只承担平台生命周期、按键、框架候选面板、commit 与 FFI；不得复制 engine、ranker、userdb、privacy 或同步逻辑。
- 不自动清理 staging、backup、历史 operation 或身份绑定终态材料。

## 下一步顺位

1. 在既有 Debian ARM64 环境补齐 Flutter Linux 工具链前置检查，执行 `check-manager-linux-product.sh`，取得真实 Release bundle、`.so`/ELF/ABI、无路径 override 和 FFI smoke；不启动 GUI或安装系统文件。
2. Linux bundle 自动门禁通过后，再实现 addon 对 `privacy-mode.json` 的安全变更感知和受控应用粗分类；unknown 与 GTK4 `PRIVATE` 传播缺口继续失败关闭。
3. 自动门禁稳定后，在单独授权的 Debian guest 中完成学习影响排序、Manager 刷新、删除不复活、显式恢复、导入导出、explain、privacy 零增量、并发和重启桌面验收。
4. Electron/敏感字段临时资产清理仍需单独授权，只删除 P03 临时材料并保留 Fcitx/Qt runtime、用户级开发装配、userdb 零基线和可复验 VM；该清理不阻塞 P04 代码。
5. M5-P05 再推进 Linux 安装、升级、修复、默认移除和数据保留；不把 P03 的用户 autostart/开发复制或 P04 staged bundle 冒充产品安装，也不提前并行 Android IME。

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
