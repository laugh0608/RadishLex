# Linux 产品元数据、rootfs 与 Debian 载体

本目录保存 M5-P05A/P05B 的 Debian 13 ARM64 产品镜像、系统布局、control/artifact contract、L6 matrix 和仓库自有桌面资产，读者是 Linux 产品装配与验证入口的维护者。这里不提交生成的 `.deb`、构建缓存、签名材料、用户数据、事务 receipt 或可直接修改系统的安装命令；安装维护事务见 [`docs/linux-installation-maintenance-boundary.md`](../../docs/linux-installation-maintenance-boundary.md)。

## 真相源关系

- 根 `version.json` 是产品版本与 build number 的唯一人工真相源，`product.json` 只能保持严格镜像。
- `install-layout.json` 固定 `debian-system-v1` 的 component-to-path、预期 owner/mode、链接关系与禁止目标。
- `scripts/linux-product/product_metadata.py` 负责 metadata/layout 格式和确定性渲染；`source_contract.py` 单独负责跨 Rust、Dart、CMake、RimeData 与构建入口的一致性审计。
- `debian/control.in` 形成可验证的 binary control 输入，`debian/artifact.json` 固定 canonical ar/USTAR、文件名、epoch、control inventory 与 dependency-analysis profile；两者都不表示安装成功。
- `l6-matrix.json` 固定 `debian13-arm64-ephemeral-v1` 的 guest、不同 commit release pair、六步主序列、八个 crash checkpoint、probe/evidence 与逐 mutation 授权；它不是可执行安装脚本。
- `l6-release-pair.json` 固定 S2 terminal source `55351f2` revision 1 的 package/evidence chain anchor、target clean descendant revision 2、共享 data contract 与 production/acceptance executable profile；它不包含生成物或运行授权。
- `assets/radishlex.svg` 是 Linux Manager 与 Fcitx 图标的共同源码，rootfs 中必须复制为两个独立普通文件。
- RimeData 继续由 `packaging/rime/product-rime-data.json` 和 `scripts/rime-product/product_data.py` 独占来源、hash、许可证与装配语义。

## 当前入口

```bash
./scripts/check-linux-product-metadata.sh
./scripts/check-linux-product-layout.sh
./scripts/check-linux-deb-artifact.sh
./scripts/check-linux-l6-contract.sh
./scripts/check-linux-l6-controller.sh
./scripts/check-linux-l6-release-pair.sh
```

第一个入口在所有受支持宿主验证 committed metadata 与负向测试。第二个入口默认运行平台无关 rootfs contract；只有显式提供真实 Linux Manager bundle 与 product-profile addon stage 时，才装配并复验临时 `DESTDIR`。第三个入口验证 canonical `.deb`、依赖输出、诊断分类、篡改拒绝和重复构建 contract；第四个入口只验证 L6 matrix 与负向边界；第五个入口编译隔离的 acceptance identity 并验证八点合成中断/恢复、进程组顺序与 evidence redaction；第六个入口以 synthetic AArch64 ELF/artifact evidence 验证 prior-terminal source anchor、target-only build、compile identity、canonical envelope 与发布后重哈希。真实 Debian 13 ARM64 构建环境另以 `build-linux-l6-release-pair.sh --source-package ABSOLUTE_FILE --source-artifact-evidence ABSOLUTE_FILE --target-root ABSOLUTE_CLEAN_PATH --output ABSENT_ABSOLUTE_PATH` 形成私有 pair；它不创建 guest、不安装 package、不运行维护 CLI。所有检查入口都不读取用户 XDG、不写 `/usr`、`/var` 或 dpkg database，也不启动 Fcitx、Manager 或桌面会话。

2026-08-06 已使用 committed `e1ce740` 的全新 Debian 13.6 ARM64 Manager/addon 输入通过真实载荷模式；随后 committed `ce74981` 对该产品 rootfs 连续生成两份逐字节相同的 `radishlex_26.7.1+38-1_arm64.deb` 与 evidence。包 SHA-256 为 `b56ba9494e715df847a778a59a09bea2ccef091ce023a596849c13c9f1db27cd`；只执行结构、解包与 package database 只读检查，未安装 package，也未生成 receipt。

2026-08-08 已在 Debian 13 ARM64 从 source `55351f2` revision 1 与 target `e5b6da1` revision 2 的独立 clean root 断网生成并复验真实 release pair。canonical record SHA-256 为 `a9bcf35762b460a23ad9bc062611f8d5edb57e7303861bbcb99e1efb40703dfd`，source/target package 分别为 `b41e32db76388ad18cdeb60e4b40fb8e28710556df87d53bfa5b275ff2ce028c`、`8209c0161609fde3b798628e5c3460e6237c8618f2d26f1452063540c7541295`。该 pair 仍是未安装的私有 L6 输入；没有运行 maintenance/acceptance CLI、`dpkg` 或产品进程。

随后第三套 pair 的 source `09ed1228…bec` 已真实安装并形成 terminal S1/S2。第四套证明同版本重建 source 不能替代 terminal artifact；builder 因此改为精确冻结 committed source并只构建 clean target。第五套 transaction 在 target validation 失败后自动恢复 source，terminal 为 `rolled_back`。第六套 source `09ed1228…bec`/target `b211d940…d09c` 的 upgrade形成 target `completed` 与冻结 S3；首次 repair 在 dpkg 前因旧 production ELF 的 target-only staged relation 缺口 `aborted_preserved`，源码已修复。第五套与repair clone只作取证；下一步需先形成绑定既有 target package/evidence 的修复后 ARM64 maintenance handoff，不能用同版本重建 package替代 installed artifact。

仓库中较早的 revision 1 P05A/P05B evidence 只保留为历史未安装载体，不能替代上述 source revision 1/target revision 2 的 L6 pair record，也不能作为 package transaction、startup 或系统安装证据。

## 字体边界

Manager 的中文、Latin 与数字文本继续使用系统 `fonts-noto-cjk` 和 `fonts-dejavu-core`。Flutter 因 `Icons.*` 生成的 `MaterialIcons-Regular.otf` 只承担图标字形，必须位于固定 `flutter_assets/fonts` 路径并由 `FontManifest.json` 与 `NOTICES.Z` 绑定；除此以外的 `.otf`、`.ttf` 或 `.ttc` 一律拒绝，产品也不携带 fontconfig 配置或调用 `fc-cache`。
