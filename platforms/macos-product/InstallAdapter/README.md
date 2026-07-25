# macOS 产品安装适配器

本文说明 `radishlex-macos-product-install` 的固定输入、manifest/code-signature 验证、staging 填充和核心组合边界，面向 Installer 与 M4-P03 维护者。本文不包含 Installer UI、真实用户安装、输入源操作、Developer ID 凭据、公证或数据升级编排；完整状态机见 [macOS 程序安装事务](../../../docs/macos-installation-transaction.md)。

## 职责

adapter 在 `ime-product-install` 的路径无关事务核心与 macOS 产品之间完成以下映射：

- 要求 authoritative current-user home、两个目标父目录和 Application Support data root 为 canonical、非 symlink、目标 uid 所有的稳定目录；
- 内嵌 committed `install-layout.json`，严格复验 InstallPayloadManifest、ProductManifest、双 component 映射和许可证记录；
- 对 bundle 内全部普通文件与内部相对 symlink 形成 `radishlex-bundle-tree-v1`；
- 使用 `codesign --verify --deep --strict -R=<designated requirement>` 验证发布要求，并把固定 code identity 字段散列为 receipt 可保存的 SHA-256；
- 使用 `ditto` 保留 macOS metadata 地填充核心固定 `staged.app`，同步完整 tree 后才记录 staged evidence；
- 在 source、installed target 和 restored source 阶段重新验证 tree/code identity。
- 实现 `InstallProgramValidationPort`，供独立协调组合层在每个数据 checkpoint 和程序恢复后复验双 bundle。
- 为 bridge 提供 Installer 自身 strict ad-hoc inspection、已安装产品与 receipt identity 交叉复验，以及显式 first-install action 后的固定目录 provisioning。

production 构造必须提供两个 component 的精确 ad-hoc designated requirement 集合；集合有界、排序、无重复且两端不重叠。strict ad-hoc 只证明包内 identity，不提供 Apple 发布者认证；单元测试通过注入的合成 verifier/copy port 覆盖平台编排，不产生发布证据。

first-install provisioning 不接受路径参数，也不递归猜测父链。它先验证 authoritative home、既有 `Library` 和 `Library/Application Support`，再只为缺失的 `Applications`、`Library/Input Methods` 与 RadishLex data root 创建 `0700` 目录并同步父目录；既有对象不 chmod、不覆盖，任何 owner/mode/symlink 漂移均失败。

## 失败语义

- payload/layout/product 字段、hash、tree、bundle ID 或发布身份漂移均失败关闭；
- fixed target、operation ID、guard 或 receipt product 不匹配时，不复制程序；
- `staged.app` 已完整复制但 receipt 尚未落盘时，重新复验并补记 evidence，不重复复制；
- 部分复制、签名失败或被替换的 staging 保持现场，不覆盖、不递归清理；
- adapter 的 tree/code evidence 与核心的 inode evidence 必须同时成立，任何一方不能替代另一方。

## 验证

```bash
./scripts/check-macos-install-adapter.sh
```

测试只使用私有合成 home、payload、bundle 和 Application Support，不访问真实 `~/Applications`、`~/Library/Input Methods`、系统设置、Keychain、签名身份或网络。
