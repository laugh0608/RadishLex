# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前产品里程碑、整改批次、验证基线、停止线和下一步的维护者与协作者。本文不记录历史流水、完整接口字段、长篇实现清单或操作 runbook；详细事实按链接进入路线图、整改专题和 devlog。

## 当前判断

- 复核日期：2026-07-12（Asia/Shanghai）
- 常态分支：`dev`
- 当前产品里程碑：M1 macOS 离线输入 Alpha
- 当前整改主批次：R01A 输入契约、进程级 runtime 与 macOS 基础输入
- 并行质量批次：R06A 首批 Clippy、CI 与 review-only 资产清理
- 已完成批次：R00 文档真相源与停止线收敛
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭；受控同步实现与测试可继续

RadishLex 已落地 Rust workspace、Rime adapter、userdb、ranker、crypto/sync 原型、Go sync server、Flutter manager 原型和 macOS InputMethodKit 开发薄壳。Apple Development v19 已完成一次用户级安装、启用和真实应用输入；为避免半成品长期占用日常输入源，验证后已切回系统拼音、停用 RadishLex 并将 bundle 移出用户输入法目录。它仍不是普通用户产品包；正常 manager 产品包也尚未形成真实 FFI、持久化配置和平台文件访问闭环。

长期产品交付顺序见 [产品交付路线图](../roadmap.md)，当前整改批次、停止线、资产处置和退出条件见 [项目稳定化整改专题](../remediation/2026-07-project-stabilization.md)。

## 本轮路线调整

- 产品主线为 M1 离线输入、M2 本地个人化、M3 加密同步、M4 发布候选；旧技术 Phase 不再决定交付顺序。
- 输入整改拆为 R01A 基础输入、R02L 本地正确性和 R01B 真实学习，避免 FFI、平台壳、数据库和 ranker 同批失控。
- 同步/设备管理与最终打包回归 M3/M4；R01A、R02L、R01B、R06A 退出后关闭临时整改专题。

## 已有工程证据

- 仓库基线已覆盖 Rust workspace、Go server、跨语言 HTTP、Flutter manager 与开发期真实 FFI；userdb、ranker、crypto、sync 和部署原型已有合成测试，但不作为产品阶段证据。
- `ime-ffi` ABI contract v3 已无损返回 `consumed`、可选即时 commit 和同事件 snapshot；候选选择复用同一 owned result，能表达分段候选只推进 composition 而不提交。输入侧 C header 已通过 C11 与 Objective-C 编译测试。
- librime 生命周期已收口到进程级 runtime；多 session、owner-thread、配置冲突、失败回滚、peer release 和最终 finalize 已有自动或 native smoke。
- `platforms/macos-imk/` 已形成可构建的 Objective-C InputMethodKit 薄壳、contract bundle 和 wrapper smoke；合成链覆盖 key normalization、未消费键、即时 commit、snapshot/candidate 复制、候选 index、reset、schema、owner-thread 与 teardown 释放顺序。
- 隔离 `rime-pinyin-simp` 的真实 FFI smoke 已覆盖 composition、完整与分段非首候选、Backspace、Escape、Enter、翻页、方向键高亮与 Space、multi-session 和不存在 schema 拒绝；adapter 以 deployed schema list、原生 current-page selection API 与选择后回读固定可用性。
- native 门禁覆盖 schema/license/data 清单、架构、FFI symbol、递归 dylib closure、逐库许可证/签名哈希和外部绝对依赖拒绝；不读取用户 Rime 目录。
- macOS build 现生成 `RadishLexInputMethod.app`，固定正式 Bundle ID `org.radishlex.inputmethod.macos`、单一 `org.radishlex.inputmethod.macos.Pinyin` mode、`LSUIElement`、简体中文 script/repertoire、图标和双语标签，并对全部 dylib、主程序和完整 bundle 执行可复验签名。
- Apple Development v19 已在用户级安装、加入并启用；TextEdit 已验证 Space 提交合成中文与 composition 存在时 `Command-N` 交还宿主，Codex 输入框已由开发者截图确认 5×1 原生横排候选可见。带 Control/Option/Command 的字符在进入 Rime 前保持未消费。

这些证据证明工程原型可继续演进，不证明真实平台输入、生产同步或产品发布已经完成。

## 已确认阻塞

- R01A 不安装 native 行为矩阵已闭合候选选择与主要编辑按键；真实应用仍缺稳定 client 切换、进程重启、断网、中英文混输及两个应用交叉复核，不能据此退出 R01A。正式 mode 当前保持停用，最后只做一次短时实机复核，不再长期占用系统输入源。
- 输入 session 未组合 engine、ranker、userdb 与 privacy policy，真实选择没有进入平台学习热路径。
- userdb 用户意图缺少统一事务、WAL/busy 策略；ranker recency/frequency 语义需要修正。
- 同步 merge、签名绑定、KDF 上限、secret 生命周期、HTTPS orchestration 和资源上限尚未达到真实用户开放条件。
- manager 默认 fixture fallback，native library 打包、持久化路径和文件权限尚未产品化。
- `cargo clippy --workspace --all-targets -- -D warnings` 已通过，Rust review-only manager sync command 模块已移除；PR workflow 与 ruleset 模板已加入独立 Rust Clippy、Flutter format/analyze/test 和 Go vet/race required checks，本机等价命令通过。Flutter review-only 模型/fixture 仍待清理，远端 ruleset 尚未应用或复验。

## 当前停止线

- R01A 完成前，不新增与真实输入链无关的 readiness、evidence、preview、approval、migration review、fake replay 或 no-symbol 资产。
- M3 退出前，不开放真实用户同步、非受控远端数据、恢复码产品成功路径、设备授权产品成功路径或设备撤销产品执行入口。
- 允许使用合成数据、loopback、短生命周期服务和受控集成测试实现同步成功路径，但这些证据不能解锁产品入口。
- 第一平台达到可重复日常输入前，不启动第二平台实现。
- manager 产品模式不得把真实 FFI 失败静默伪装为 fixture 成功；fixture 只能由显式 demo mode 启用并持续标识。
- 合成 fixture、local smoke、CLI 输出和设计草案不能单独作为产品阶段完成证据。
- 输入热路径继续保持本地和离线，不引入网络依赖。

## 下一步顺位

1. 下一次实机验证使用短时用户级安装，只复核 client 切换、进程重启、断网、中英文混输和两个应用交叉行为；完成即停用并移出输入法目录。
2. 将 app-scoped 自动化抓图不包含 InputMethodKit 独立候选浮层视为工具边界；候选可见性使用无敏感内容的全屏人工观察确认，按键结果仍以应用文本和脱敏日志交叉验证。
3. 短时矩阵通过后完成 R01A 退出判断，再经授权清理旧版本备份；失败则只修正真实平台链路。
4. R01A 等待人工动作期间继续 R06A：清理 Flutter review-only 生产模型、fixture 与配套文档，保留 disabled state、隐私模式和 secret 不持久化/不进入诊断断言；不启动第二平台。
5. R01A 退出后实施 R02L，修正 userdb 事务、SQLite 并发、recency、frequency 与删除语义；R02L 退出后再由 R01B 接入真实学习，之后关闭整改专题并进入 M3。

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
# 需要显式隔离 schema/shared data 与许可证
./scripts/check-macos-imk-native.sh
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
