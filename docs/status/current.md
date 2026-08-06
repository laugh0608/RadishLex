# RadishLex 当前状态

本文是日常推进短入口，供维护者判断里程碑、验证基线、停止线和下一步。详细设计与历史进入边界文档、runbook 和周志。

## 当前判断

- 复核日期：2026-08-06（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品
- 当前主批次：M5-P05A Linux metadata/rootfs assembly；committed 实现与平台无关门禁已完成，停在真实 Debian 13 ARM64 Manager/addon 载荷门禁前
- 已退出：M0-M3；M4 macOS build 38 单版本产品验收已冻结；M5-P01 第二平台决策与运行边界；M5-P02 Fcitx5 addon、共享 FFI 与开发构建；M5-P03 真实 Linux 桌面输入与隐私验收；M5-P04 Linux Manager 与同库个人化验收
- 真实用户同步：保持关闭；只允许合成数据与受控集成测试

## M5-P04 冻结基线

M5-P03/P04 已在 UTM Debian 13 ARM64 完成 Wayland/X11、GTK/Qt/Electron/Firefox/Terminal、候选交互、生命周期、断网、隐私、Linux Release Manager、同库学习、删除/恢复、导入导出与重启验收。生产 allowlist 只包含精确 `firefox-esr -> browser`；password、terminal、unknown 与 Qt `Sensitive` 不学习，GTK4 `PRIVATE` 继续按 unknown 失败关闭。

同一 guest/XDG userdb 的最终状态为 user terms 2、selection events 5、suppressed 0、deleted 1、import batches 2。Manager 导入检查零写入、普通导入防 tombstone 复活、P2 导出、Manager/Fcitx 与完整桌面会话重启均已通过；schema v9/WAL 与 `integrity_check=ok` 保持，无 busy、migration、projection、重复学习或状态丢失。

上述证据不再复跑。guest staging、backup、userdb、导入导出文件、公开合成输入和临时服务继续原样保留，不作为 P05 安装源、回滚源或清理目标。

## M5-P05 设计结论

完整边界见 [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)，当前固定：

- 首个载体为 Debian 13 ARM64 系统级单 package 本地 `.deb`，identity 为 `debian-local-deb-v1`；不构成公开 repository、Release 或通用 Linux 包。P02/P04 用户 staging 与 transient service 仍只属于开发/验收。
- 程序 layout 绑定 Manager、两份同 hash/不同 inode 的 FFI、addon、完整 RimeData/source/license、desktop/icon 和 manifest；文本字体硬依赖 `fonts-dejavu-core`/`fonts-noto-cjk`，payload 只例外保留 Flutter Material Icons 图标字形。
- 五类维护 operation 必须有显式身份、guard/receipt、静止复验与失败关闭；默认保留全部 XDG 数据。首批升降级只接受 ABI/schema/XDG/settings/privacy/Rime contract 完全相同的 release pair。

## M5-P05A 实现状态

- `packaging/linux/` 已绑定 `26.7.1+38-1`、ARM64 multiarch、依赖、组件路径、ABI/schema/settings/privacy 和 RimeData lock；Fcitx CMake 保留默认 `staged`，新增固定 `/usr/share/radishlex/rime` 的 `system` profile。
- rootfs 装配器严格验证 Manager/addon 输入，离线装配 RimeData，生成并复验 canonical manifest、inventory、mode/owner、链接、双 FFI、metadata、desktop、字体例外和禁止路径。
- 两个 Linux product 无参数门禁已通过；真实模式的 ARM64 ELF、RPATH/closure、ABI symbol、构建路径与系统字体检查尚未获授权执行。P05A 未退出，`.deb`、transaction/startup gate 和安装实机仍关闭。

## macOS 冻结参考

macOS `26.7.1 (38)` 保持冻结参考产品；DMG SHA-256 为 `f171e74bdc0a429655a84b30429481bce3926b17076d09298feed77d9ce4ce4e`，未使用 Developer ID/公证，远端 draft 未发布且无正式 tag。详细身份、事务与实机流水见 [macOS 产品包边界](../macos-product-package-boundary.md) 和历史周志。

## 当前停止线

- P05A 只允许仓库 metadata、构建 profile、临时 rootfs 与自动门禁；不生成 `.deb` 或安装成功事实，不实现 transaction/startup gate，不运行 `dpkg`，不写 `/usr`、`/var`、用户 XDG、Fcitx profile/autostart 或 systemd 配置。
- 不启动、停止或重启 Fcitx/Manager/桌面会话，不安装依赖，不执行真实 repair/remove/rollback；所有实机与系统变更继续逐项授权。
- 不复跑 M5-P04 实机验收，不清理、reset、覆盖或改写其 guest staging、backup、userdb、导入导出文件和临时服务。
- 不发布 build 38 或 Linux package，不推送本地提交，不创建 tag/Release，不修改远端 draft 或仓库设置。
- 首个正式版本继续关闭真实用户同步；P0 永不学习/同步，P1 原始事件只留本地，P2 只允许端到端加密对象。
- 输入热路径保持完全本地；Fcitx addon 不复制 engine、ranker、userdb、privacy 或同步逻辑，安装协调层不打开用户数据库。
- M5 只推进 Linux Fcitx5，不并行实现 Android、Windows 或 iOS。
- 不自动清理 staging、backup、历史 operation、receipt 或身份绑定终态材料。

## 下一步顺位

1. 保持 P04 guest 与 macOS build 38 冻结现场不变。
2. 另行授权隔离 Debian 13 ARM64 构建后，使用全新输出目录运行 Manager product build、product-profile addon stage 与 `check-linux-product-layout.sh` 真实载荷模式；不安装 package、不启动 GUI/Fcitx，也不复用 P04 staging 作为产品输入。
3. 真实载荷门禁通过并记录身份后退出 P05A，再设计评审 P05B 的 `.deb`、receipt/guard、五类 operation、startup gate 与 ephemeral Debian matrix；不能顺带写真实系统。
4. P05C 必须使用 P04 guest 的独立 clone/snapshot 或另一台 guest，并逐项取得依赖安装、dpkg、服务/输入法配置、注销和故障注入授权。
5. 旧临时资产清理、远端推送、tag/Release、真实同步和其他平台均保持独立授权与后续顺位。

## 当前验证入口

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
./scripts/check-linux-product-metadata.sh
./scripts/check-linux-product-layout.sh
./scripts/check-manager-linux-product.sh
./scripts/build-linux-fcitx5-container.sh
./scripts/check-repo.sh
./scripts/check-docs.sh
git diff --check
```

前两个 Linux product 入口的无参数模式只验证 committed metadata 与平台无关 rootfs contract；带真实 Manager/addon 输入的 ARM64 强门禁尚未执行。真实系统安装、输入源变更、用户数据、公开上传和 Release 仍需对应授权。

## 阅读索引

- [路线图](../roadmap.md)
- [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)
- [第二平台 Linux Fcitx5 ADR](../adr/0009-second-platform-linux-fcitx5.md)
- [Linux Fcitx5 平台边界](../linux-fcitx5-boundary.md)
- [Linux Manager 本地验收边界](../linux-manager-local-acceptance.md)
- [macOS 产品包边界](../macos-product-package-boundary.md)
- [macOS 程序安装事务](../macos-installation-transaction.md)
- [macOS 社区 ad-hoc DMG Runbook](../runbooks/macos-release-carrier.md)
- [本周周志](../devlogs/2026-W32.md)
