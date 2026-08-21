# 参与 RadishLex

本文说明参与 RadishLex 讨论、提案、代码、文档与验证工作的共同流程，面向外部贡献者和维护者。本文不记录当前里程碑、临时停止线、实机现场或发布承诺；这些易变信息以[当前状态](docs/status/current.md)及其引用文档为准。

感谢你关注 RadishLex。项目重视本地优先、隐私、可删除性、可复核证据和长期一致性；贡献应完整覆盖真实主路径，不把占位实现、局部演示或超出证据的结论当作交付。

## 贡献前先确认

1. 阅读[当前状态](docs/status/current.md)、[文档入口](docs/README.md)和与改动直接相关的最少专题。
2. 普通缺陷和变更提案请使用仓库 Issue chooser 中的对应表单；安全漏洞不要提交公开 Issue 或 Pull Request，请按[安全策略](SECURITY.md)私下报告。
3. 参与讨论、评审和其他项目交流时遵循[社区行为准则](CODE_OF_CONDUCT.md)。
4. 架构、公共协议、隐私、加密、许可证、平台接入、FFI ABI、安装事务或阶段边界变化，应先通过 Issue、设计讨论或 ADR 固定边界；小型缺陷、文案和不改变边界的验证补漏可以直接提出 PR。
5. 不提交真实输入历史、联系人、密码、支付或证件数据、用户词库、密钥、恢复码、生产凭据、未经脱敏的日志，或无法确认授权的第三方材料。

## 许可证与外部材料

本仓库采用 [RadishLex Source-Available License 1.0](LICENSE)，不是开放源码许可证。查看仓库不表示已经获得复制、修改、再分发、衍生开发或商业使用授权。外部贡献者在准备或提交需要复制、修改本仓库内容的补丁前，应先取得项目所有者的书面许可；公开反馈问题或提出原创建议本身不会扩大许可证授权。

提交 Pull Request、patch、Issue 附件或其他贡献，即表示你有权提交相关内容，并接受 `LICENSE` 第 4 节的贡献授权条款。除非另有书面约定，第三方代码、词库、模型、字体、图标、测试数据和其他资产必须标明来源与许可证，且不得复制许可证不兼容或来源不明的内容。

## 工作流

1. 普通变更从独立主题分支向 `dev` 发起 Pull Request；`master` 只接收阶段性 `dev` 晋级。
2. 主题分支可使用 `feature/*`、`fix/*`、`docs/*`、`proposal/*`、`experiment/*`、`test/*` 或 `chore/*`。
3. 不直接向 `master` push，不 force push 共享分支。`dev -> master` 合并后，按[仓库 Ruleset 说明](.github/rulesets/README.md)将 `master` 回流 `dev`。
4. 提交遵循 Conventional Commits，并使用贡献者自己的 Git 身份，不添加 AI 协作者署名。

示例：

```text
feat(ranker): explain contextual score factors
fix(sync): preserve tombstone across stale merge
docs(privacy): clarify P0 learning boundary
test(ffi): cover invalid UTF-8 ownership path
```

## 实现与数据边界

- 输入、候选、学习和 commit 热路径必须保持本地可用；不得把 RadishLex 改造成云端实时输入法 API。
- P0 数据永不学习或同步；P1 原始选择和上下文默认只在本地；P2 用户词、摘要和配置只作为端到端加密对象；P3 才可公开下载。
- 删除必须保留 tombstone 或等价的防复活语义，并考虑旧设备、冲突和备份恢复。
- 同步后端只存密文与必要 metadata，不参与按键处理或明文合并。
- 平台壳保持薄层，Manager 不进入输入热路径；FFI 必须明确 ABI、所有权、生命周期、线程、UTF-8、释放和错误语义。
- 安装、升级、修复、移除与回滚默认保留用户数据；系统状态未知、身份漂移或后置条件无法证明时应失败关闭并保留现场。
- fixture、截图、golden、日志和诊断只使用合成数据或脱敏聚合，不记录真实输入内容和秘密。
- 不复用或覆盖被当前状态和 runbook 标记为冻结的实机、数据库、staging、backup 或证据材料；系统写入、GUI、长期服务和真实平台验收必须遵循对应授权边界。

## Pull Request 说明

请使用仓库的 Pull Request 模板，并覆盖所有适用内容：

- 目标、范围、原因和明确非目标；
- 影响的 crate、服务、Manager、平台、数据级别、协议、ABI、安装事务或用户可见行为；
- 兼容性、迁移、失败模式、隐私和安全影响；
- 实际执行的验证命令及结果；
- 未验证内容、环境阻塞、已知风险和回滚方式；
- 需要人工、实机、实验室、发布或后续授权完成的事项。

不要把静态检查、合成测试、模拟执行器或分散现场证据表述为真实系统、连续事务、端到端安全或正式发布已经通过。结论必须与证据范围一致。

## 本地验证

文档改动至少执行：

```bash
./scripts/check-docs.sh
./scripts/check-text-files.sh
git diff --check
```

阶段性、高风险或跨模块交付还应执行：

```bash
./scripts/check-repo.sh
```

实现改动继续按范围执行相应门禁，例如 Rust 的 `cargo fmt --check`、`cargo check`、相关 `cargo test` 和 `clippy`，Go 的 `gofmt` 与 `go test ./...`，Flutter 的 `dart format`、`flutter analyze` 和相关测试，以及平台壳的 build/smoke。PR 只记录真实执行过的命令，并明确列出未执行项和残余风险。
