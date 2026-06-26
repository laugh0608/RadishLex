## 变更说明

请简要说明本次 PR 的目标、范围和原因。

## 关联信息

- 关联 Issue / 任务：
- 目标分支：`dev` / `master`（如非 `dev`，请说明原因）
- 变更类型：
  - [ ] 功能
  - [ ] 修复
  - [ ] 重构
  - [ ] 文档
  - [ ] 配置 / 仓库治理
  - [ ] 测试 / 验证基线
  - [ ] 隐私 / 同步 / 加密
  - [ ] 平台壳 / FFI / Engine adapter

## 检查清单

- [ ] 本次改动符合当前阶段“方案冻结、仓库治理、Rust core 原型优先”的方向，或已明确说明为何需要例外
- [ ] 已优先从根因、长期维护性和系统一致性出发处理问题，而不是仅做最小修补
- [ ] 已执行与本次改动匹配的最小验证
- [ ] 如修改了架构、阶段边界、协议、隐私策略、平台接入方式或协作规范，已同步更新 `docs/` / `AGENTS.md` / `CLAUDE.md`
- [ ] 如属于本周重要推进，已追加到 `docs/devlogs/YYYY-Www.md`，或说明当前尚未建立周志入口
- [ ] 未直接向 `master` 提交常规功能改动
- [ ] 默认目标分支为 `dev`；只有阶段性集成、发布或明确收口事项才面向 `master`

## RadishLex 影响面

- Rust core / CLI：
  - [ ] 无
  - [ ] 有，已说明影响的 crate、输入会话、候选模型或提交链路
- Engine adapter / `librime`：
  - [ ] 无
  - [ ] 有，已说明 engine boundary、候选转换和底层状态隔离
- 用户词库 / ranker / 学习：
  - [ ] 无
  - [ ] 有，已说明正反馈、负反馈、排序因子和 explain 影响
- 隐私 / 同步 / 加密：
  - [ ] 无
  - [ ] 有，已说明 P0-P3 数据分级、密文同步、删除语义或设备授权影响
- Go server：
  - [ ] 无
  - [ ] 有，已说明 API、存储、版本、备份或部署影响
- Flutter manager：
  - [ ] 无
  - [ ] 有，已说明页面、状态、bridge 或配置影响
- 平台壳 / FFI：
  - [ ] 无
  - [ ] 有，已说明目标平台、系统权限、生命周期、候选窗或 ABI 影响

## 隐私与安全核对

- [ ] 未引入明文输入历史、明文用户词库、明文候选偏好或敏感上下文上传
- [ ] 未把 P0 数据纳入学习、日志、同步、fixture、截图或错误报告
- [ ] 如涉及删除语义，已考虑 tombstone / 防旧设备复活 / 备份恢复边界
- [ ] 如涉及设备授权或密钥，已说明恢复码、设备撤销、密钥轮换或重放防护影响
- [ ] 如涉及外部项目参考，遵守 clean-room 原则，未复制不兼容许可证源码、词库或测试数据

## 验证记录

请列出实际执行过的命令，并只保留真实跑过的内容：

```text
./scripts/check-text-files.sh
./scripts/check-docs.sh
./scripts/check-repo.sh
git diff --check
pwsh ./scripts/check-text-files.ps1
pwsh ./scripts/check-docs.ps1
pwsh ./scripts/check-repo.ps1
cargo fmt --check
cargo check
cargo test
go test ./...
dart format .
flutter analyze
flutter test
```

## 风险与回滚

- 已知风险：
- 未验证部分：
- 回滚方式：
- 是否涉及发布后额外操作：
  - [ ] 否
  - [ ] 是，请说明

## 后续事项

请说明当前未完成项、后续建议或需要额外跟踪的事项。
