# RadishLex 当前状态

本文是日常推进短入口，供维护者判断里程碑、验证基线、停止线和下一步。详细设计与历史进入边界文档、runbook 和周志。

## 当前判断

- 复核日期：2026-08-04（Asia/Shanghai）
- 常态分支：`dev`；稳定主线：`master`
- 当前里程碑：M5 Linux Fcitx5 离线输入与个人化产品
- 当前主批次：M5-P04 Linux Manager、同库个人化与本地管理验收
- 已退出：M0-M3；M4 macOS build 38 单版本产品验收已冻结；M5-P01 第二平台决策与运行边界；M5-P02 Fcitx5 addon、共享 FFI 与开发构建；M5-P03 真实 Linux 桌面输入与隐私验收
- 真实用户同步：保持关闭；只允许合成数据与受控集成测试

## M5-P03 完成证据与 M5-P04 当前进度

M5-P02 已建立 C++17/CMake addon、ABI v9 owned projection、Fcitx input panel/session、共享 XDG resolver 和可持续 ARM64 构建。M5-P03 随后在 UTM Debian 13.6 ARM64 完成 GNOME Wayland/X11、GTK4/Qt6/Electron/Firefox/Terminal、候选交互、生命周期、整机断网和隐私实机验收；用户级开发装配不写 `/usr`，也不冒充 M5-P05 产品安装。

P03 的 password、terminal、unknown 与 Qt `Sensitive` 路径均未产生学习；GTK4 `PRIVATE` 因未传播敏感 bit 而继续按 unknown 失败关闭。候选内容、排序、display-index 选择、userdb 与隐私策略仍分别由 Rust 和独立平台 privacy 真相源负责，平台壳没有复制业务逻辑。

M5-P04 已落地 Linux Flutter runner、engine 前 `umask(0077)`、固定 bundle `.so`、共享 XDG/Manager runtime、严格 `privacy-mode.json` 原子读写/回滚，以及“runtime 写入—Manager C ABI 管理—新 runtime 观察”的同库自动证据。Debian ARM64 的 Flutter 3.44.0 Release bundle、ELF closure、`$ORIGIN/lib`、正式 ABI symbol、native-rime FFI 与 Dart smoke 已通过；首轮门禁暴露的两处 ABI 缩写漂移也已修正并纳入静态检查。

第二个子批建立了先 watch 后初读的 privacy 变更感知和失败关闭的应用粗分类。Password、Sensitive 与 Terminal 在读取 `InputContext::program()` 前返回；Terminal 保持 `context_known=0`。Wayland/X11 实机评审后，生产 allowlist 只加入精确 `firefox-esr -> browser`；其他 Firefox 变体、unknown 与 GTK4 `PRIVATE` 继续不读写个人化数据。

第三个子批增加默认关闭的 `RADISHLEX_APPLICATION_EVIDENCE`：只把源码内固定候选的精确匹配投影为 opaque token，只记录 capability 的稳定 on/off 变化；默认产品 addon 由门禁保证不含取证日志字符串。Wayland Firefox 实机得到 `browser_candidate_01`，进程会话与 GTK3 Fcitx frontend 证据支持原生 Wayland。2026-08-04 的 X11 对照再次得到同一 token；Firefox 为 `DISPLAY=:0`、`XDG_SESSION_TYPE=x11`、X11 client，并加载 GTK3 `im-fcitx5.so` 与 `libFcitx5GClient`，未设置 `WAYLAND_DISPLAY` 或 `MOZ_ENABLE_WAYLAND`。

Wayland/X11 两侧均产生 `password_on`/`password_off`；用户确认普通字段显示候选，密码字段只显示遮罩且无候选，完整往返前后 userdb v9 七项学习聚合保持全零。精确生产规则的宿主 contract 与 Debian ARM64 addon、ELF、runtime probe、六项 CTest 和默认二进制 evidence 排除已通过；guest 已停止 evidence unit 并恢复未改动的默认 production transient service，私有 evidence stage、P03 staging/backup 和旧脏工作副本均保留。

2026-08-04 的生产分类后纵向实机已证明同库学习与解释：Firefox 普通字段以公开合成 code `ba` 选择第二候选“把”后，userdb 的 active term、selection event 和 ranker weight 各增加一项；下一次输入同 code 时“把”提升到首位，Escape 取消未重复学习。Linux Release Manager 刷新读取同一 XDG userdb，并把该项解释为 `browser`，最终分数约 `1.79`，其中 user、frequency、recency 与 context 信号和输入侧排序一致。此前固定查询 `general` 的 Dart bridge 已改为查询六个允许的粗类别并明确显示 context；Linux 设置文案改为平台中立，主题字体 fallback 同时覆盖 Latin、数字与简体中文，ARM64 product 复验通过。M5-P04 仍未完成：Manager 写入 privacy 后的零增量、删除/恢复、导入导出、并发和重启仍待实机闭合。

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

1. 通过 Manager 开启 privacy 并保存，验证平台真相源读回为开启、addon 下一次公开合成学习机会零增量；关闭并保存后，一次新的公开合成选择只恢复一次预期增量。
2. 在同一 XDG userdb 上依次闭合删除不复活、explicit restore、导入检查/导入/导出，并对照 Fcitx 新 session 与 Manager 刷新结果。
3. 覆盖 Manager/Fcitx 双进程并发、WAL/busy、Manager 重启、Fcitx 重启和桌面会话重启，确认没有重复学习、迁移漂移或状态丢失。
4. M5-P05 需明确 Linux CJK 字体是随包交付还是由发行依赖保证；P04 当前系统字体 fallback 的实机通过不能替代安装产品依赖治理。
5. M5-P05 安装维护、P03 临时资产清理、远端推送和其他平台均不进入当前批次；对应动作继续单独授权。

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
