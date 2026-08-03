# RadishLex 当前状态

本文是日常推进短入口，供维护者判断里程碑、验证基线、停止线和下一步。详细设计与历史进入边界文档、runbook 和周志。

## 当前判断

- 复核日期：2026-08-03（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品
- 当前主批次：M5-P04 Linux Manager、同库个人化与本地管理验收
- 已退出：M0-M3；M4 macOS build 38 单版本产品验收已冻结；M5-P01 第二平台决策与运行边界；M5-P02 Fcitx5 addon、共享 FFI 与开发构建；M5-P03 真实 Linux 桌面输入与隐私验收
- 真实用户同步：保持关闭；只允许合成数据与受控集成测试

## M5-P03 完成证据与 M5-P04 当前进度

M5-P02 已建立 C++17/CMake addon、ABI v9 owned projection、Fcitx input panel/session、共享 XDG resolver 和可持续 ARM64 构建。M5-P03 随后在 UTM Debian 13.6 ARM64 完成 GNOME Wayland/X11、GTK4/Qt6/Electron/Firefox/Terminal、候选交互、生命周期、整机断网和隐私实机验收；用户级开发装配不写 `/usr`，也不冒充 M5-P05 产品安装。

P03 的 password、terminal、unknown 与 Qt `Sensitive` 路径均未产生学习；GTK4 `PRIVATE` 因未传播敏感 bit 而继续按 unknown 失败关闭。候选内容、排序、display-index 选择、userdb 与隐私策略仍分别由 Rust 和独立平台 privacy 真相源负责，平台壳没有复制业务逻辑。

M5-P04 已落地 Linux Flutter runner、engine 前 `umask(0077)`、固定 bundle `.so`、共享 XDG/Manager runtime、严格 `privacy-mode.json` 原子读写/回滚，以及“runtime 写入—Manager C ABI 管理—新 runtime 观察”的同库自动证据。Debian ARM64 的 Flutter 3.44.0 Release bundle、ELF closure、`$ORIGIN/lib`、正式 ABI symbol、native-rime FFI 与 Dart smoke 已通过；首轮门禁暴露的两处 ABI 缩写漂移也已修正并纳入静态检查。桌面 Manager 尚未启动，不能宣称 P04 完成。

第二个子批建立了先 watch 后初读的 privacy 变更感知和失败关闭的应用粗分类。Password、Sensitive 与 Terminal 在读取 `InputContext::program()` 前返回；Terminal 保持 `context_known=0`。生产 allowlist 仍为空，unknown 与 GTK4 `PRIVATE` 继续不读写个人化数据。

第三个子批增加默认关闭的 `RADISHLEX_APPLICATION_EVIDENCE`：只把源码内固定候选的精确匹配投影为 opaque token，只记录 capability 的稳定 on/off 变化；默认产品 addon 由门禁保证不含取证日志字符串。Wayland Firefox 实机得到 `browser_candidate_01`，其源码候选精确对应 `firefox-esr`；进程会话与 GTK3 Fcitx frontend 证据支持原生 Wayland，密码字段产生 `password_on`/`password_off`。用户确认普通字段显示候选，密码字段仅显示圆点且无候选，前后 userdb v9 学习聚合均为零。

Wayland 证据只闭合了评审矩阵的一半；同一 Firefox 身份、frontend 与密码传播仍须在 X11 复验，因此生产 allowlist 暂不加入任何项。guest 日终已恢复默认关闭取证的最新 addon 与生产临时服务，staging、backup 和旧脏工作副本保持原状；剩余主线是 X11 对照、生产 allowlist、桌面学习与 Manager 双进程验收。

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

1. 2026-08-04 首先在另行授权并保存现有工作的前提下停止当前临时服务，由用户注销、选择 GNOME on Xorg 并重新登录；AI 不自动切换会话或合成实体操作。
2. 在 X11 复用同一默认关闭、显式启用的取证模式和离线 Firefox 夹具，核对精确身份、frontend、普通/密码字段 capability 与 userdb 零增量；不记录输入正文、窗口标题或原始程序名。
3. 只有 Wayland/X11 两侧均通过时，才加入精确 `firefox-esr -> browser` 生产 allowlist，并复跑 classifier、Fcitx5、Manager 和默认产品二进制排除门禁；任一证据漂移都保持 unknown 失败关闭。
4. allowlist 闭合后再验证学习影响排序、Manager 刷新、privacy 零增量、删除不复活、显式恢复、导入导出、explain、WAL/并发及重启；每次 GUI、输入法与实体交互仍逐项授权。
5. M5-P05 安装维护、P03 临时资产清理、远端推送和其他平台均不进入明日首批；对应动作继续单独授权。

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
