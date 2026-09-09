# macOS Installer Driver

本文说明独立 Installer App 的只读状态投影与显式动作授权契约，面向 Installer UI、外层安装事务和产品门禁维护者。本文不包含真实用户目录写入、程序停止、输入源修改、Developer ID、公证或 DMG 发布。

`radishlex-macos-installer-driver` 只组合 `ime-product-install` 的只读状态检查，不直接执行程序切换。它把 receipt/guard 结果投影为版本化的：

- `phase`、primary/secondary action、稳定 error 与 receipt state；
- 0–10 的持久化进度步骤；
- 手动切到中立输入源并关闭 Manager 的提示；
- 默认移除仅删除程序、保留 Application Support 的数据策略。

UI 不得解析 `receipt.json`、读取 `HOME`、接受自定义路径或根据 bundle 缺失猜测 operation。无 receipt 或前一 receipt 已终态时，平台必须以固定目标和产品身份形成 `InstallerProductSituation`；completed install/upgrade/repair 会按当前真实产品重新提供 upgrade/repair/remove，receipt 与实际产品矛盾、身份不可得或已安装版本更新时失败关闭。

`authorize_installer_action` 会重新验证当前 snapshot 是否仍提供该动作。除 refresh 外，所有动作都要求显式确认；upgrade、repair、retry、remove 和 prepared 后的 confirm 还要求确认已手动切换输入源并关闭 Manager，移除另须确认保留数据。切换前中止恢复要求显式动作、数据保留、中立输入源与 Manager 关闭四项确认。确认只形成 `AuthorizedInstallerIntent`，不能替代公开平台 API 和固定 preflight 的重新取证。

`AbortPreSwitchUpgrade` 由 Executor 的双 receipt 只读检查补充到 snapshot；driver 本身不读取数据 receipt 或恢复证据。授权 intent 绑定 operation、root 的 device/inode/owner/mode 与 source/target manifest digest、build，执行器在 guard 内重新核验。证据已存在的专用恢复只提供本动作；缺失或不合法证据不能回退普通 resume。完整范围见 [Installer 边界](../../../docs/macos-installer-app-boundary.md#显式切换前中止恢复)。

```bash
cargo test --locked -p radishlex-macos-installer-driver --all-targets
cargo clippy --locked -p radishlex-macos-installer-driver --all-targets -- -D warnings
```
