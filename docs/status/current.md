# RadishLex 当前状态

本文是日常推进短入口，供维护者判断里程碑、验证基线、停止线和下一步。详细设计与历史进入边界文档、runbook 和周志。

## 当前判断

- 复核日期：2026-08-06（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品
- 当前主批次：M5-P05 Linux 安装维护；前置设计与边界已收口，安装实现尚未开始，停在 P05A metadata/rootfs assembly 前
- 已退出：M0-M3；M4 macOS build 38 单版本产品验收已冻结；M5-P01 第二平台决策与运行边界；M5-P02 Fcitx5 addon、共享 FFI 与开发构建；M5-P03 真实 Linux 桌面输入与隐私验收；M5-P04 Linux Manager 与同库个人化验收
- 真实用户同步：保持关闭；只允许合成数据与受控集成测试

## M5-P04 冻结基线

M5-P03/P04 已在 UTM Debian 13 ARM64 完成 Wayland/X11、GTK/Qt/Electron/Firefox/Terminal、候选交互、生命周期、断网、隐私、Linux Release Manager、同库学习、删除/恢复、导入导出与重启验收。生产 allowlist 只包含精确 `firefox-esr -> browser`；password、terminal、unknown 与 Qt `Sensitive` 不学习，GTK4 `PRIVATE` 继续按 unknown 失败关闭。

同一 guest/XDG userdb 的最终状态为 user terms 2、selection events 5、suppressed 0、deleted 1、import batches 2。Manager 导入检查零写入、普通导入防 tombstone 复活、P2 导出、Manager/Fcitx 与完整桌面会话重启均已通过；schema v9/WAL 与 `integrity_check=ok` 保持，无 busy、migration、projection、重复学习或状态丢失。

上述证据不再复跑。guest staging、backup、userdb、导入导出文件、公开合成输入和临时服务继续原样保留，不作为 P05 安装源、回滚源或清理目标。

## M5-P05 设计结论

完整边界见 [Linux 安装维护边界](../linux-installation-maintenance-boundary.md)，当前固定：

- 首个完整产品载体为 Debian 13 ARM64 的系统级单 package 本地 `.deb`，identity 为 `debian-local-deb-v1`；它不构成公开 apt repository、正式 Release 或通用 Linux 包。
- 不提供完整用户级程序安装；P02/P04 的 `FCITX_ADDON_DIRS`、用户 staging、autostart 副本和 transient service 仍是开发/验收机制。用户数据、配置和状态继续只在现有 XDG 用户域。
- CJK 字体走发行版 hard dependency：Debian profile 固定 `fonts-noto-cjk`，并以 `fonts-dejavu-core` 保证 Latin/数字；RadishLex payload 不携带字体，也不直接刷新系统 font cache。
- 系统 layout 固定绑定 Manager bundle、两个字节相同但非 hardlink 的 FFI、Fcitx addon、完整 RimeData/source/license、desktop entry、icon、Linux product manifest 和 root-owned transaction receipt。
- install、upgrade、repair、remove、rollback 都必须有显式 source/target、幂等 receipt/guard、静止检查、完整身份复验与失败关闭；默认 remove/purge 不遍历 home，保留全部用户 XDG 数据。
- 首批 upgrade/rollback 只接受 ABI v9、userdb schema v9、XDG/settings/privacy/Rime contract 完全相同的 release pair；跨数据 contract 升降级继续关闭。

## macOS 冻结参考

macOS `26.7.1 (38)` 保持冻结参考产品；安装、Manager、输入、repair、移除与数据保留证据不变。DMG SHA-256 为 `f171e74bdc0a429655a84b30429481bce3926b17076d09298feed77d9ce4ce4e`，未使用 Developer ID 或 Apple 公证；远端 draft 未发布、没有正式 Git tag。

build 38 的详细身份、事务、载体与实机流水由 [macOS 产品包边界](../macos-product-package-boundary.md)、[macOS 程序安装事务](../macos-installation-transaction.md)、[社区载体 runbook](../runbooks/macos-release-carrier.md) 和历史周志维护。真实跨发布升级、source rollback、Developer ID/公证和正式 Release 仍属于后续兼容与统一发布评审。

## 当前停止线

- 本会话只完成 P05 文档边界；不创建 `packaging/linux/`、package、安装脚本或事务代码，不运行 `dpkg`，不写 `/usr`、`/var`、用户 XDG、Fcitx profile/autostart 或 systemd 配置。
- 不启动、停止或重启 Fcitx/Manager/桌面会话，不安装依赖，不执行真实 repair/remove/rollback；所有实机与系统变更继续逐项授权。
- 不复跑 M5-P04 实机验收，不清理、reset、覆盖或改写其 guest staging、backup、userdb、导入导出文件和临时服务。
- 不发布 build 38 或 Linux package，不推送本地提交，不创建 tag/Release，不修改远端 draft 或仓库设置。
- 首个正式版本继续关闭真实用户同步；P0 永不学习/同步，P1 原始事件只留本地，P2 只允许端到端加密对象。
- 输入热路径保持完全本地；Fcitx addon 不复制 engine、ranker、userdb、privacy 或同步逻辑，安装协调层不打开用户数据库。
- M5 只推进 Linux Fcitx5，不并行实现 Android、Windows 或 iOS。
- 不自动清理 staging、backup、历史 operation、receipt 或身份绑定终态材料。

## 下一步顺位

1. 保持 P04 guest 与 macOS build 38 冻结现场不变。
2. 单独授权代码实现后进入 M5-P05A：新增 `packaging/linux/` metadata/layout，在临时 `DESTDIR` 形成 rootfs assembly，并建立纯自动 metadata/layout/ELF/RimeData/字体依赖负向门禁；仍不安装 `.deb`。
3. P05A 通过后再设计评审 P05B 的 `.deb`、receipt/guard、五类 operation、startup gate 与 ephemeral Debian matrix；不能在首批顺带写系统。
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
./scripts/check-manager-linux-product.sh
./scripts/build-linux-fcitx5-container.sh
./scripts/check-repo.sh
./scripts/check-docs.sh
git diff --check
```

P05A 规划中的 Linux metadata/layout 门禁尚未实现，不应写入当前可执行入口。真实系统安装、输入源变更、用户数据、公开上传和 Release 仍需对应授权。

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
