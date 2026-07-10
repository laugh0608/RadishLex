# RadishLex 当前状态

本文档是新会话和日常推进的唯一短入口，读者是需要快速判断当前阶段、验证基线、停止线和下一步的维护者与协作者。本文不包含历史流水、完整接口字段、长篇实现清单或操作 runbook；详细事实按链接进入专题和 devlog。

## 当前判断

- 复核日期：2026-07-10（Asia/Shanghai）
- 常态分支：`dev`
- 当前阶段：2026 年 7 月项目稳定化整改
- 已完成批次：R00 文档真相源与停止线收敛
- 下一主批次：R01 真实输入纵向链，尚未开始代码实现
- 并行质量批次：R06 首批 Clippy、CI 与 native bundle 门禁
- 第一真实平台：macOS InputMethodKit
- 真实用户同步：保持关闭

RadishLex 已落地 Rust workspace、Rime adapter、userdb、ranker、crypto/sync 原型、Go sync server 和 Flutter manager 原型，但还不是用户可安装使用的输入法 MVP。当前没有真实平台输入法，正常 manager 产品包也没有形成真实 FFI、持久化配置和平台文件访问闭环。

完整整改批次、停止线、验收证据和退出条件见 [项目稳定化整改总专题](../remediation/2026-07-project-stabilization.md)。

## R00 完成证据

- `AGENTS.md` 与 `CLAUDE.md` 已纠正过期阶段、代码状态、第一平台和文档入口，并继续保持同步。
- 本文已收敛为当前状态短入口，不再复制 manager sync review 流水。
- `docs/roadmap.md` 只保留阶段目标、交付物和退出标准。
- `docs/technical-plan.md` 已收敛为稳定架构、模块职责、输入链、平台和验证边界。
- `docs/repository-layout.md` 已改为实际目录、成熟度边界和明确的未落地目录。
- `docs/privacy-sync.md` 已移除实现流水，只保留长期数据分级、安全和同步边界。
- 整改总专题已登记 manager sync review 文档、Rust draft、Flutter 模型、fixture 和测试的保留、转化、删除或归档去向。

R00 只完成治理纠偏，不代表已修复审计发现的代码问题，也不代表 Phase 1–5 已达到产品退出标准。

## 已有工程证据

- 默认 Rust workspace、Go server 和跨语言加密对象 HTTP 测试可通过仓库基线。
- Flutter manager 的 format、analyze、widget tests 和开发期真实 FFI smoke 已有独立入口。
- userdb、ranker、crypto、sync、Go storage/API 和 manager 已有较丰富合成测试。
- 本地 Docker/HTTPS、备份恢复、外部 TLS 反代和升级回滚已有开发或实现级 smoke。
- Apple/Android 平台签名 backend 在能力不足时保持 unavailable，没有静默回退为生产密钥。

这些证据证明工程原型可继续演进，不证明真实平台输入、生产同步或产品发布已经完成。

## 已确认阻塞

- `ime-ffi` 丢弃 `KeyOutcome` 的 `consumed` 和即时 `commit`，平台无法正确分流按键和提交文本。
- 输入 session 未组合 engine、ranker、userdb 与 privacy policy，真实选择没有进入平台学习热路径。
- librime setup/initialize/finalize 尚未收口为进程级 runtime。
- userdb 用户意图缺少完整事务，ranker recency/frequency 语义需要修正。
- 同步 merge 未把本地 active state 作为同等输入，也缺少稳定设备 tie-break。
- 签名绑定、KDF 上限、secret 生命周期、HTTPS transport 和完整 sync orchestration 未达到开放条件。
- manager 默认 fixture fallback、native library 打包、持久化路径和文件权限尚未产品化。
- Rust Clippy 当前暴露公开 FFI 裸指针安全契约错误；常态 CI 尚未覆盖 Flutter、Go race、MSRV 和 native bundle。

## 当前停止线

- R01 完成前，不新增与真实输入链无关的 readiness、evidence、preview、approval、migration review 或 no-symbol 资产。
- R03 完成前，不开放真实远端上传、恢复码成功、设备授权成功或设备撤销执行路径。
- 第一平台达到可重复日常输入前，不启动第二平台实现。
- manager 产品模式不得把真实 FFI 失败静默伪装为 fixture 成功。
- 合成 fixture、local smoke 和设计草案不能单独作为阶段完成证据。
- 输入热路径继续保持本地和离线，不引入网络依赖。

## 下一步顺位

1. 在代码实现前更新 `docs/ffi-boundary.md`、`docs/engine-rime-adapter.md` 和平台边界，固定 KeyOutcome ABI、input runtime、librime 全局生命周期与 macOS InputMethodKit 薄壳。
2. 实施 R01：返回 `consumed`、可选 `commit` 和 snapshot，组合 engine/ranker/userdb/privacy runtime，并落地第一平台。
3. 并行启动 R06 首批门禁：修复 FFI Clippy，增加 Flutter、Go race、MSRV 和 macOS native bundle CI。
4. R01 通过真实平台验收后进入 R02 userdb/ranker 正确性，再按 R03 -> R04 -> R05 推进同步和 manager 产品化。

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

文档与文本：

```bash
./scripts/check-docs.sh
./scripts/check-text-files.sh
git diff --check
cmp -s AGENTS.md CLAUDE.md
```

native-rime、真实平台安装、Keychain/Android connected smoke、Docker 长流程和发布部署需要对应环境或人工授权，不属于普通文档变更的默认门禁。

## 最小阅读索引

- [整改总专题](../remediation/2026-07-project-stabilization.md)：当前批次、停止线、资产处置和退出条件。
- [技术方案](../technical-plan.md)：稳定架构、职责、平台与验证边界。
- [路线图](../roadmap.md)：长期阶段、交付物和退出标准。
- [仓库结构](../repository-layout.md)：实际目录与未落地边界。
- [隐私与同步](../privacy-sync.md)：数据分级、密钥、删除、恢复与威胁模型。
- [本周周志](../devlogs/2026-W28.md)：本周事实、验证和交接记录。
