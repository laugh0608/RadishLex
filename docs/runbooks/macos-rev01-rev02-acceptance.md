# macOS REV-01 / REV-02 候选与验收

本文面向维护者与实机验收者，绑定本轮隐私和 SQLite 修复候选、仓库验证及待执行矩阵。它不授予系统安装、进程控制、输入源切换或真实数据操作权限，也不替代历史 M4 验收记录。

## 候选身份：2026-09-08

- 产品 `26.7.1 (39)`，产品源码提交 `5e9b0a8`，包含隐私修复 `40cd1bd` 与 SQLite 升级 `a03c69b`。包内 smoke 提交为 `1ec8859`；该测试和后续文档不进入本次产品二进制。
- 装配：`target/macos-product/26.7.1-39/`；最终本地载体：`target/macos-release/26.7.1-39/`，含 `Product/`、`InstallPayload/` 与 `RadishLex Installer.app`。
- 本机 native FFI 为 `arm64`，最低系统声明 macOS 13.0；不据 Manager 外壳的其他架构切片宣称 Intel 产品已验收。
- 构建工具：Rust 1.96.0、Xcode 26.6 / 17F113、Flutter 3.44.0 / Dart 3.12.0；librime 1.17.0。使用既有离线依赖，Cargo/pub 锁未变化；根 Rust 1.80 声明仍未验证，见 REV-06。
- `community-adhoc-v1`，未公证、未制作新 DMG、未发布。Installer 及双组件签名、双份 payload、ProductManifest、ReleaseIdentity、RimeData 与 native library manifest 均通过验证。
- `UpgradeSources` 为空，没有把历史 build 38 声明为升级源。此载体准备首次安装验收；不能用于直接证明或执行 build 38 → 39 升级。

以下 SHA-256 对应最终载体；完整 manifest 同时绑定其余资源、依赖与 helpers：

| 文件 | SHA-256 |
| --- | --- |
| `Product/ProductManifest.json` | `de1bf1ea937f742242975ad05a1d639e1d3899454e9acfb41eb28e089fe8362f` |
| Installer 内 `ReleaseIdentity.json` | `dfe8855fcaf4c7f15e27672bc01abfe982d173dd3c1e191febc3f503da2d3dde` |
| InputMethod FFI | `8a815ac6382c97d3f5f056fc8a3d19c8de967dc1f1af2a7cd7787cfd44f7b3cd` |
| Manager FFI | `791a76b345d4d9bd24201a138e7b5c535e1c0c25adca3601ccaf42ec8edd3d1f` |
| RimeData `SourceManifest.json` | `3a224aabcf0f7ca1e4c03042fd163a76b25a944bab88b652e412708be9c25583` |
| `radishlex_pinyin.schema.yaml` | `1b94cc5bd763b34dc86e782bdcfb46b9b14b1f8a9973af9e7864bd4754335bf1` |

双 FFI 均内嵌 SQLite 3.51.3 source id `2026-03-13 10:38:09 737ae4a34738ffa0c3ff7f9bb18df914dd1cad163f28fd6b6e114a344fe6d618`，未发现旧 3.46.0 source id，Mach-O 未链接系统 SQLite。这里是包内二进制静态身份核对；实际 SQLite 版本查询、WAL 和旧库回归见 [REV-02](../remediation/product-review-2026-09.md#rev-02sqlite-wal-reset-修复版本)，不把静态扫描表述为应用内 SQL 查询。

## 已完成的候选检查

| 检查 | 结果与边界 |
| --- | --- |
| 完整仓库门禁、macOS/Linux 版本元数据与冻结 L6 合同 | 通过；当前 build 39 被旧 L6 target 资格正确拒绝 |
| 双组件与 Installer 构建、签名、资源/依赖/载荷身份 | 通过；只构建和读回文件，未启动 app |
| 包内 InputMethod FFI：普通、隐私、unknown、secure、sensitive、隐私→普通、普通→隐私→普通 | 7 个独立合成进程通过；普通场景 1 条事件/词条，其余 0；commit 保留、候选页 5 项、无 Rime 自有 userdb、SQLite 完整性通过 |
| 包内 Manager FFI smoke | 合成临时库 smoke 通过；装配与最终载体的 FFI hash 相同，未调用真实 Keychain |
| Manager analyze / test | `--no-pub`：无问题、99 项测试通过 |
| 旧 build 38 与冻结输入 | 104 项文件树 mode/hash/link 一致；Cargo.lock、pubspec.lock、L6 pair 文件及旧 manifest hash 不变 |

本轮 C probe 直接链接最终 `Product/Components/RadishLexInputMethod.app` 中的 dylib，使用同包 RimeData。上下文由测试显式注入，只验证 FFI/runtime 行为；不证明 macOS secure input、应用识别或控制器路由已复验。分段、重启、旧 Rime 合成库及删除恢复的较宽 native 覆盖来自上一修复批，也不替代本候选实机矩阵。

可重复的包内隐私检查（只创建新的私有临时目录，失败也保留证据）：

```bash
python3 scripts/macos-imk/check_bundled_privacy.py \
  --product-root target/macos-release/26.7.1-39/Product
```

入口先校验 ProductManifest，编译 [C probe](../../platforms/macos-imk/Tests/bundled_privacy_smoke.c)，分别运行七个场景，再只读核对刚生成的合成 SQLite。它不重建产品，不接受真实 userdb 作为输入，也不操作系统输入源。结果目录中的 `result.json` 绑定产品 manifest、FFI 与 probe 源码 hash。

## 实机前置条件与停止条件

1. 下一阶段先做只读现场盘点：明确验收用户/主机、固定双 bundle 路径、现有 build、外层 receipt/guard、相关进程与输入源状态；不读取真实 P1 明文记录，也不从旧授权推断可停进程或覆盖安装。
2. 以[产品包边界](../macos-product-package-boundary.md)和[Installer 边界](../macos-installer-app-boundary.md)为准，确认首次安装资格。已有程序、receipt、guard 或未知残留时停止，不能删除数据或把已有安装伪装为空用户域；需要升级则另行准备经过资格验证的历史 source 载体。
3. 确定独立、合格的验收账户与合成数据范围后，单独批准 Installer GUI、实际安装及双组件启动。固定目标为该用户的 `Applications/RadishLex Manager.app`、`Library/Input Methods/RadishLexInputMethod.app` 和 `Library/Application Support/RadishLex`；默认保留数据，禁止绕过 Installer 手工复制 app。
4. 系统设置、输入源切换、按键输入、退出/重启各按明确动作授权；任何自动点击或合成按键同样在此范围内。真实密码、联系人、证件等不得用作测试材料。
5. 安装后重新交叉核验 receipt 终态、双组件身份、固定路径与实际运行程序，再开始输入矩阵。发现身份漂移、事务未知、越界学习、commit 丢失或异常持久化，立即停止当前场景并保留现场，不自动 repair、retry 或 cleanup。

## 待执行实机矩阵

所有条目当前均为 **待执行**。仅对独立合成验收库记录必要计数/摘要；截图不得含真实输入历史。

| 场景 | 需要的实机证据 |
| --- | --- |
| 首次安装和固定路径启动 | Installer 完整终态、receipt 与双 bundle/进程身份一致、双端 startup gate 允许 |
| 普通拼音、选候选、翻页与中英切换 | 宿主收到准确 commit；正常选择进入 RadishLex userdb，重复选择/重开后的重排可解释 |
| 隐私模式与 composition 中双向切换 | P0 composition 不产生学习；退回普通模式后仅新合格 composition 可学习；输入与 commit 不丢失 |
| secure input / 敏感应用 / unknown 路由 | 记录系统实际是否把事件交给 IME；系统绕过与控制器收到受限上下文分别判定，不用 C 注入结果填充实机通过 |
| 分段候选和自动 commit | 完整文本、分段归属与最严格隐私状态跨段保持，无隐私尾段补学习 |
| Manager 与输入法共享状态 | 同一合成库的条目、删除、显式恢复及候选变化一致；删除后重启/迟到选择不复活 |
| 正常退出与重启 | 新普通学习保留；隐私输入无新增事件或 Rime 学习库；不使用 kill 模拟正常退出 |
| 旧合成库打开、备份恢复 | 3.46/schema 9 合成 fixture 在独立目标继续学习、保留删除语义，恢复后完整性通过；禁止覆盖真实库或冻结证据 |
| 输入质量对照 | 固定合成词集记录候选与排序变化；未有独立新词召回的行为如实记录，不宣称 Rime 自有学习收益仍然存在 |

本候选准备不关闭 REV-01/REV-02，也不改变 Linux M5 系统操作顺位；Linux 新 pair 与新制品、真实平台回归、输入质量和 MSRV 仍需各自闭合。原 build 38、P04、M4 历史数据与 L6 冻结资产不得用作本批可写测试目标。

## 证据位置

- 身份与冻结输入核对：`/private/tmp/radishlex-build39-preflight/candidate-identity.json`；前置快照在同目录。
- 装配、Installer、完整门禁与 artifact verify：`/private/tmp/radishlex-build39-{assembly-escalated,installer,check-repo-escalated,artifact-verify}.log`。
- 最终七场景：系统临时目录下 `radishlex-bundled-privacy-jw_2a6y3/result.json`，入口输出保存于 `/private/tmp/radishlex-build39-bundled-privacy-final.log`。
- Manager 包内 smoke：`/private/tmp/radishlex-build39-candidate-fe6xq27f/manager-smoke-escalated.log`；Flutter 日志为 `/private/tmp/radishlex-build39-flutter-{analyze,test}-escalated.log`。
- 沙盒失败日志一并保留：Flutter 缓存权限、仓库 Unix socket guard 和 Manager 本地同步资格初始化均按最小范围提权后通过；没有跳过检查或下载依赖。
