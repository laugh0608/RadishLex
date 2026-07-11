# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前产品里程碑、整改批次、验证基线、停止线和下一步的维护者与协作者。本文不记录历史流水、完整接口字段、长篇实现清单或操作 runbook；详细事实按链接进入路线图、整改专题和 devlog。

## 当前判断

- 复核日期：2026-07-11（Asia/Shanghai）
- 常态分支：`dev`
- 当前产品里程碑：M1 macOS 离线输入 Alpha
- 当前整改主批次：R01A 输入契约、进程级 runtime 与 macOS 基础输入
- 并行质量批次：R06A 首批 Clippy、CI 与 review-only 资产清理
- 已完成批次：R00 文档真相源与停止线收敛
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；受控同步实现与测试可继续

RadishLex 已落地 Rust workspace、Rime adapter、userdb、ranker、crypto/sync 原型、Go sync server、Flutter manager 原型和 macOS InputMethodKit 开发薄壳，但还不是用户可安装使用的输入法。macOS 当前只有不安装系统输入法的 contract bundle 与 wrapper smoke；正常 manager 产品包也没有形成真实 FFI、持久化配置和平台文件访问闭环。

长期产品交付顺序见 [产品交付路线图](../roadmap.md)，当前整改批次、停止线、资产处置和退出条件见 [项目稳定化整改专题](../remediation/2026-07-project-stabilization.md)。

## 本轮路线调整

- 旧的技术 Phase 顺序不再作为产品推进主线；交付改为 M1 离线输入、M2 本地个人化、M3 加密同步、M4 发布候选。
- 原 R01 拆为 R01A 基础输入、R02L 本地正确性和 R01B 真实学习闭环，避免一次批次同时改 FFI、librime、平台壳、数据库和 ranker。
- 同步收敛、生产同步、manager 同步 UI 与最终产品包回归 M3/M4 正常路线，不再作为 2026-07 临时整改的关闭条件。
- Flutter manager 分层交付：M2 先完成本地词库、学习和隐私管理；M3 再完成同步与设备管理；M4 完成产品打包。
- R01A、R02L、R01B 与 R06A 退出后关闭整改专题，后续按路线图推进，不继续维护永久整改状态。

## 已有工程证据

- 默认 Rust workspace、Go server 和跨语言加密对象 HTTP 测试可通过仓库基线。
- Flutter manager 的 format、analyze、widget tests 和开发期真实 FFI smoke 已有独立入口。
- userdb、ranker、crypto、sync、Go storage/API 和 manager 已有较丰富合成测试。
- 本地 Docker/HTTPS、备份恢复、外部 TLS 反代和升级回滚已有开发或实现级 smoke。
- Apple/Android 平台签名 backend 在能力不足时保持 unavailable，没有静默回退为生产密钥。
- `ime-ffi` ABI contract v2 已无损返回 `consumed`、可选即时 commit 和同事件 snapshot；输入侧 C header 已通过 C11 与 Objective-C 编译测试。
- librime setup / initialize / explicit shutdown / finalize 已收口到进程级 runtime；多 session、零 session 间隙、配置冲突、失败回滚、peer release 和最终 finalize 已有自动测试或 gated native smoke。
- `platforms/macos-imk/` 已形成可构建的 Objective-C InputMethodKit 薄壳、contract bundle 和 wrapper smoke；合成链覆盖 key normalization、未消费键、即时 commit、snapshot/candidate 复制、候选 index、reset、schema、owner-thread 与 teardown 释放顺序。

这些证据证明工程原型可继续演进，不证明真实平台输入、生产同步或产品发布已经完成。

## 已确认阻塞

- macOS native-rime bundle 尚需一份显式隔离且来源合规的 shared data/schema；安装、启用、两个真实应用输入框 smoke 和移除尚未获授权执行。
- 输入 session 未组合 engine、ranker、userdb 与 privacy policy，真实选择没有进入平台学习热路径。
- userdb 用户意图缺少统一事务、WAL/busy 策略；ranker recency/frequency 语义需要修正。
- 同步 merge、签名绑定、KDF 上限、secret 生命周期、HTTPS orchestration 和资源上限尚未达到真实用户开放条件。
- manager 默认 fixture fallback，native library 打包、持久化路径和文件权限尚未产品化。
- 严格 Rust Clippy 当前失败；常态 CI 尚未覆盖 Flutter、Go race 和完整 native bundle 门禁。

## 当前停止线

- R01A 完成前，不新增与真实输入链无关的 readiness、evidence、preview、approval、migration review、fake replay 或 no-symbol 资产。
- M3 退出前，不开放真实用户同步、非受控远端数据、恢复码产品成功路径、设备授权产品成功路径或设备撤销产品执行入口。
- 允许使用合成数据、loopback、短生命周期服务和受控集成测试实现同步成功路径，但这些证据不能解锁产品入口。
- 第一平台达到可重复日常输入前，不启动第二平台实现。
- manager 产品模式不得把真实 FFI 失败静默伪装为 fixture 成功；fixture 只能由显式 demo mode 启用并持续标识。
- 合成 fixture、local smoke、CLI 输出和设计草案不能单独作为产品阶段完成证据。
- 输入热路径继续保持本地和离线，不引入网络依赖。

## 下一步顺位

1. 准备来源合规、与用户现有输入法隔离的 Rime shared data/schema，执行 `build-bundle.sh native` 并复核开发依赖加载；不得隐式读取真实用户 Rime 目录。
2. 按 macOS 开发 runbook 评审安装、启用、两个应用输入 smoke 与移除步骤；取得明确授权后再执行并完成 R01A。
3. 真实 smoke 重点复验连续输入、非首候选、翻页、取消、Backspace、Enter、中英文混输、未消费快捷键、client 切换与进程重启。
4. 并行实施 R06A：修复严格 Clippy，增加 Flutter 与 Go race CI，清理 review-only Rust/Flutter 生产资产。
5. R01A 退出后实施 R02L，修正 userdb 事务、SQLite 并发、recency、frequency 与删除语义。
6. R02L 退出后实施 R01B，让真实选择在隐私策略约束下持久化并影响后续候选；之后关闭整改专题并进入 M3。

## 验证入口

常态仓库基线：

```bash
./scripts/check-repo.sh
```

Flutter manager：

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
```

macOS InputMethodKit（不安装）：

```bash
./scripts/check-macos-imk.sh
```

文档与文本：

```bash
./scripts/check-docs.sh
./scripts/check-text-files.sh
git diff --check
cmp -s AGENTS.md CLAUDE.md
```

native-rime、真实平台安装、Keychain/Android connected smoke、Docker 长流程和发布部署需要对应环境或人工授权，不属于普通文档变更的默认门禁。

## 最小阅读索引

- [整改专题](../remediation/2026-07-project-stabilization.md)：当前批次、停止线、资产处置和退出条件。
- [产品交付路线图](../roadmap.md)：产品里程碑、交付物和退出标准。
- [技术方案](../technical-plan.md)：稳定架构、职责、平台与验证边界。
- [macOS InputMethodKit](../macos-inputmethodkit-boundary.md)：第一平台的 runtime、按键链、目录和验收边界。
- [仓库结构](../repository-layout.md)：实际目录与未落地边界。
- [隐私与同步](../privacy-sync.md)：数据分级、密钥、删除、恢复与威胁模型。
- [本周周志](../devlogs/2026-W28.md)：本周事实、验证和交接记录。
