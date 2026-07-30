# RadishLex 当前状态

本文是日常推进短入口，供维护者判断里程碑、验证基线、停止线和下一步。详细设计与历史进入边界文档、runbook 和周志。

## 当前判断

- 复核日期：2026-07-30（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品
- 当前主批次：M5-P04 Linux Manager、同库个人化与本地管理验收
- 已退出：M0-M3；M4 macOS build 38 单版本产品验收已冻结；M5-P01 第二平台决策与运行边界；M5-P02 Fcitx5 addon、共享 FFI 与开发构建；M5-P03 真实 Linux 桌面输入与隐私验收
- 真实用户同步：保持关闭；只允许合成数据与受控集成测试

## M5-P03 完成证据与 M5-P04 入口

M5-P02 已建立 `platforms/linux-fcitx5/` 的 C++17/CMake addon、ABI v9 owned projection、Fcitx input panel/session、共享 XDG resolver、addon/input method metadata 与自动门禁。固定 Debian 13 ARM64 环境已通过 native-rime FFI、addon-relative staged 装配、`$ORIGIN`、资源权限/symlink、构建路径泄漏、`dlopen(RTLD_NOW)` 和三项 CTest；容器证据只承担可持续构建与无头加载，不替代桌面验收。

M5-P03 在 UTM Debian 13.6 ARM64 建立了 GNOME Wayland/X11 真实桌面基线和不写 `/usr` 的用户级开发装配。Fcitx5 daemon 实际加载已校验的 `radishlex.so`、sibling FFI 与锁定 RimeData；产品 XDG 目录/文件保持 `0700`/`0600`。候选 press/release 配对与 preedit UTF-8 cursor/`DontCommit` 两项修复均通过宿主 contract、guest 强门禁和用户实体操作复验，ABI 仍为 v9，候选内容、排序与 display-index 选择仍由 Rust 真相源负责。

Wayland 主路径已覆盖 GTK4 Text Editor、GNOME Terminal、Firefox、原生 Qt6 Wayland 临时 fixture 和官方 Electron v43.2.0 ARM64 runtime；X11 对照覆盖 GTK、Qt/XCB、Firefox、终端和输入法切换。人工矩阵包含 composition/cursor、方向与翻页、数字/Space/鼠标选择、commit、Escape/Backspace reset、带候选焦点切换、Fcitx 重启、桌面会话重启和框架切换。FeatherPad 映射 `libQt6WaylandClient` 不能单独证明 Wayland backend；原生 Qt6 Wayland 结论只使用明确的 Wayland QPA 与 Fcitx Qt6 plugin 进程映射。

隐私验收覆盖 unknown、terminal、Firefox password 和 Qt `Sensitive`。Qt 原生 Wayland capability 在敏感字段精确增加 Fcitx `Sensitive` 的 `2^36` bit；GTK4 `PRIVATE` 在当前 Debian frontend 未传播该 bit，因此继续由 `context_known = 0` 失败关闭。上述路径、进程级地址族限制和整台 guest 断网输入后，userdb v9 的 P1/P2、context、tombstone 与 audit 聚合始终为零。桌面快速 X11→Wayland 切换还暴露 Debian `im-launch` 因持久 user-manager 环境跳过 daemon 的问题；用户级复制 Debian 官方 Fcitx5 desktop entry 后，重新登录实现单实例同秒自启动、addon/FFI 加载和输入复验。该设置只是可逆开发基线，不冒充 M5-P05 产品安装。

## macOS 冻结参考

macOS `26.7.1 (38)` 保持冻结参考产品：双 bundle、Installer、社区 ad-hoc DMG、ProductManifest/ReleaseIdentity、独立下载、首次安装、Manager、公开合成输入、repair、默认程序移除与数据保留均已有证据。DMG SHA-256 为 `f171e74bdc0a429655a84b30429481bce3926b17076d09298feed77d9ce4ce4e`，明确未使用 Developer ID 或 Apple 公证；远端 draft 未发布、没有正式 Git tag。

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

1. M5-P01/P02/P03 已依次固定平台边界、完成 addon/FFI/build，并取得 Wayland/X11、常见应用、生命周期、离线与隐私实机证据；详细流水见本周周志。
2. 明日先在单独授权下删除 guest 与宿主的 Electron/敏感字段临时验收资产；保留已安装的 Fcitx/Qt runtime、用户级开发装配、userdb 零基线和可复验 VM，不把临时清理扩成软件包或产品数据删除。
3. M5-P04 代码前先对照 [Manager 本地验收](../manager-local-acceptance.md) 与 Linux 平台边界，补齐 Linux host、共享 XDG/userdb、WAL 并发、privacy 配置来源、学习/删除/恢复、导入导出和 explain 的设计与验收矩阵。
4. M5-P05 再推进 Linux 安装、升级、修复、默认移除和数据保留；不把 P03 的用户 autostart/开发复制冒充产品安装，也不提前并行 Android IME。

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
- [Manager 本地验收](../manager-local-acceptance.md)
- [macOS 产品包边界](../macos-product-package-boundary.md)
- [macOS 程序安装事务](../macos-installation-transaction.md)
- [macOS Installer App 边界](../macos-installer-app-boundary.md)
- [macOS 数据升级协调器](../macos-data-upgrade-coordinator.md)
- [macOS 社区 ad-hoc DMG Runbook](../runbooks/macos-release-carrier.md)
- [本周周志](../devlogs/2026-W31.md)
