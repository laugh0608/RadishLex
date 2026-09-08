# RimeData 产品输入

本目录保存首个 macOS 产品候选可离线复验的 RimeData 输入、来源锁和逐资产许可证。它不保存用户 Rime 数据、编译后的词典、真实输入记录或构建产物。

- `data/default.yaml`：固定 schema list、5 项候选页与 macOS ASCII 切换行为。
- `data/radishlex_pinyin.schema.yaml`：RadishLex 维护的简体全拼产品 schema；不包含笔画反查和扩展符号表。
- `data/pinyin_simp.dict.yaml`：固定自官方 `rime/rime-pinyin-simp` 的 Apache-2.0 词典。
- `licenses/rime-pinyin-simp/`：上游 LICENSE 与 AUTHORS 原文。
- `product-rime-data.json`：来源 commit、文件 hash、运行时目标和许可证映射的唯一锁文件。

仓库门禁只接受锁文件列出的普通文件和 hash。产品构建从这些 committed 输入离线装配 RimeData，不在构建时下载数据，也不读取 `~/Library/Rime`、Squirrel 或 RadishLex 运行目录。

在仓库根可独立校验、装配和复验公开产品数据：

```bash
./scripts/prepare-rime-product-data.sh validate
./scripts/prepare-rime-product-data.sh assemble --output <isolated-output>/RimeData
./scripts/prepare-rime-product-data.sh verify --data-dir <isolated-output>/RimeData
```

输出目录必须是新建的隔离目录，不能指向用户或运行时 Rime 数据。完整双 bundle 构建、产物解释和失败处理见 [macOS 产品装配 Runbook](../../docs/runbooks/macos-product-assembly.md)。

产品 schema 使用 `enable_user_dict: false`，学习由 RadishLex userdb 独占。native adapter 还检查有效部署配置，旧编译配置或 custom patch 重新开启学习会拒绝创建 session。既有 Rime 学习数据保留且不迁移；基础词典仍用于候选生成，旧自学词/新词召回不再使用。具体合同与兼容限制见 [Rime adapter](../../docs/engine-rime-adapter.md#单一学习存储与有效配置)。
