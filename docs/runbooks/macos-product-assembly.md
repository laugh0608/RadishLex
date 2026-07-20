# macOS 产品装配 Runbook

本文说明如何从仓库锁定输入构建并复验 macOS 双 bundle 产品装配目录，读者是维护产品构建、RimeData 和发布门禁的开发者。本文不执行系统安装、输入源注册、Developer ID 签名、公证、上传、用户数据迁移或发布；产品组成和兼容性规则以 [macOS 产品包边界](../macos-product-package-boundary.md) 为准。

## 产物边界

稳定入口 `./scripts/build-macos-product.sh` 生成：

```text
target/macos-product/<product-version>-<build-number>/
  Components/
    radishlex_manager.app/
    RadishLexInputMethod.app/
  LICENSE
  ProductManifest.json
```

该目录是未安装的产品装配结果，不是 `.pkg`、`.dmg` 或已公证安装包。默认构建使用 ad-hoc 签名；目录可以用于完整性和依赖复验，不能作为 Developer ID、notarization 或 Gatekeeper 发布证据。

## 环境前提

- macOS 13 或更高版本；
- 仓库要求的 Rust、Python 3、Flutter、Xcode command-line tools；
- 与当前 native 门禁兼容的 `librime` headers 和 libraries；
- `RIME_INCLUDE_DIR` 指向 headers 根目录，`RIME_LIB_DIR` 指向包含 `librime` 及其可打包依赖的目录；
- 工作区没有需要保留但尚未提交的同路径构建配置修改。

装配不联网获取 schema 或词典，不读取 `~/Library/Rime`、Squirrel 数据或 RadishLex Application Support。RimeData 只来自 [仓库锁定输入](../../packaging/rime/README.md)。

## 第一步：校验源码契约

在仓库根执行：

```bash
./scripts/check-macos-product-metadata.sh
```

该门禁会验证：

- `packaging/macos/product.json` 的字段和各版本声明一致；
- committed RimeData inventory、hash、来源和许可证映射一致；
- product manifest 与 RimeData 工具的拒绝路径测试通过。

这一步不构建 app bundle，也不修改系统状态。

需要单独检查 RimeData 时可执行：

```bash
./scripts/prepare-rime-product-data.sh validate
radish_rime_dir="$(mktemp -d /tmp/radishlex-rime-product.XXXXXX)"
./scripts/prepare-rime-product-data.sh assemble --output "${radish_rime_dir}/RimeData"
./scripts/prepare-rime-product-data.sh verify --data-dir "${radish_rime_dir}/RimeData"
```

临时目录只包含公开产品数据，不是用户 Rime 目录；复验后可按团队的临时文件清理方式处理。

## 第二步：构建产品装配目录

显式提供 native Rime 位置：

```bash
RIME_INCLUDE_DIR=<librime-include-dir> \
RIME_LIB_DIR=<librime-library-dir> \
./scripts/build-macos-product.sh
```

入口按以下顺序工作：

1. 校验产品元数据和 RimeData source lock；
2. 在 `target/macos-product/` 下创建隔离 RimeData；
3. 构建 product mode Manager bundle；
4. 构建 native-rime InputMethod bundle并递归收集非系统 dylib 与许可证；
5. 复制两个独立 bundle 和仓库许可证到 staging；
6. 生成并复验 `ProductManifest.json`；
7. 通过同文件系统原子 rename 发布版本化装配目录。

脚本不会安装或启动任何 bundle。目标版本目录已存在时会拒绝覆盖；需要比较重复构建时，应使用干净 worktree 或先把既有产物归档到明确位置，不能让脚本静默删除旧结果。

## 第三步：独立复验产物

从单一产品元数据推导目录，重新计算 manifest：

```bash
radish_product_version="$(python3 scripts/macos-product/product_manifest.py field product_version)"
radish_product_build="$(python3 scripts/macos-product/product_manifest.py field build_number)"
radish_product_dir="target/macos-product/${radish_product_version}-${radish_product_build}"

python3 scripts/macos-product/product_manifest.py verify \
  --manager-bundle "${radish_product_dir}/Components/radishlex_manager.app" \
  --input-method-bundle "${radish_product_dir}/Components/RadishLexInputMethod.app" \
  --license "${radish_product_dir}/LICENSE" \
  --manifest "${radish_product_dir}/ProductManifest.json"
```

再执行两个组件门禁：

```bash
./scripts/check-manager-product.sh
RIME_INCLUDE_DIR=<librime-include-dir> \
RIME_LIB_DIR=<librime-library-dir> \
RADISHLEX_RIME_SHARED_DATA="${radish_rime_dir}/RimeData" \
./scripts/check-macos-imk-native.sh
```

`RADISHLEX_RIME_SHARED_DATA` 必须来自 `prepare-rime-product-data.sh assemble` 生成的隔离目录，不得指向用户或运行时数据目录。

## 复验结果解释

成功装配至少证明：

- Manager/InputMethod 的版本、build、最低 macOS、bundle ID 与产品真相源一致；
- FFI ABI、userdb schema、RimeData/native manifest 版本一致；
- bundle 不依赖 Homebrew 或构建机绝对 library path；
- committed schema、词典、来源 manifest 和逐资产许可证进入 InputMethod；
- `ProductManifest.json` 能复算两个 bundle、内部安全 symlink 和许可证文件。

它不证明：

- bundle 已使用 Developer ID 或 Hardened Runtime 发布配置；
- Apple notarization、stapling 或 Gatekeeper 隔离环境验证通过；
- 安装、升级、回滚、移除和真实用户数据 migration 可用；
- 真实用户同步已经开放。

## 常见失败

- `product assembly already exists`：同版本目录已存在，脚本按设计拒绝覆盖；先确认是保留、归档还是在新 worktree 重建。
- `native bundle requires RIME_INCLUDE_DIR/RIME_LIB_DIR`：native 依赖位置未显式提供。
- `RimeData ... differs from source lock`：committed 数据、来源、许可证或 hash 与锁不一致；应审阅输入变更，不得跳过校验。
- `external absolute dependency`：某个 Mach-O 仍指向构建机路径；修正 dylib 收集或 install name，不能把本机路径加入 allowlist。
- `product manifest does not match assembled artifacts`：产物在 manifest 生成后被修改、缺失或版本不一致；重新从受控输入装配，不手改 manifest。
- codesign 验证失败：检查嵌套 dylib、主 executable 和外层 bundle 的签名顺序；不要把 `--deep --force` 当作发布修复策略。

任何需要安装输入法、修改系统设置、使用 Developer ID、提交公证或操作真实 Application Support 数据的后续步骤，都必须进入对应专用 runbook 并另行取得授权。
