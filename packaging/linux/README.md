# Linux 产品元数据与 rootfs

本目录保存 M5-P05A 的 Debian 13 ARM64 产品镜像、系统布局、control 模板和仓库自有桌面资产，读者是 Linux 产品装配与验证入口的维护者。这里不保存 `.deb`、构建缓存、签名材料、用户数据、事务 receipt 或可直接修改系统的安装命令；安装维护事务见 [`docs/linux-installation-maintenance-boundary.md`](../../docs/linux-installation-maintenance-boundary.md)。

## 真相源关系

- 根 `version.json` 是产品版本与 build number 的唯一人工真相源，`product.json` 只能保持严格镜像。
- `install-layout.json` 固定 `debian-system-v1` 的 component-to-path、预期 owner/mode、链接关系与禁止目标。
- `scripts/linux-product/product_metadata.py` 负责 metadata/layout 格式和确定性渲染；`source_contract.py` 单独负责跨 Rust、Dart、CMake、RimeData 与构建入口的一致性审计。
- `debian/control.in` 只形成可验证的 binary control 输入；M5-P05A 不调用 `dpkg-deb`，也不生成安装成功事实。
- `assets/radishlex.svg` 是 Linux Manager 与 Fcitx 图标的共同源码，rootfs 中必须复制为两个独立普通文件。
- RimeData 继续由 `packaging/rime/product-rime-data.json` 和 `scripts/rime-product/product_data.py` 独占来源、hash、许可证与装配语义。

## 当前入口

```bash
./scripts/check-linux-product-metadata.sh
./scripts/check-linux-product-layout.sh
```

第一个入口在所有受支持宿主验证 committed metadata 与负向测试。第二个入口默认运行平台无关 rootfs contract；只有显式提供真实 Linux Manager bundle 与 product-profile addon stage 时，才装配并复验临时 `DESTDIR`。两个入口都不读取用户 XDG、不写 `/usr` 或 `/var`，也不启动 Fcitx、Manager 或桌面会话。

2026-08-06 已使用 committed `e1ce740` 的全新 Debian 13.6 ARM64 Manager/addon 输入通过真实载荷模式；该证据包含路径映射、双 FFI、ELF closure、系统字体与临时 manifest，不生成 `.deb` 或安装 receipt。

## 字体边界

Manager 的中文、Latin 与数字文本继续使用系统 `fonts-noto-cjk` 和 `fonts-dejavu-core`。Flutter 因 `Icons.*` 生成的 `MaterialIcons-Regular.otf` 只承担图标字形，必须位于固定 `flutter_assets/fonts` 路径并由 `FontManifest.json` 与 `NOTICES.Z` 绑定；除此以外的 `.otf`、`.ttf` 或 `.ttc` 一律拒绝，产品也不携带 fontconfig 配置或调用 `fc-cache`。
