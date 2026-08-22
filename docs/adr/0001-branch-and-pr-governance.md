# ADR 0001: `dev` / `master` 分支拓扑与 PR 闭环

本文档用于固定 RadishLex 的长期分支职责、阶段性 PR 方向和合并后的回同步要求，读者是维护者、审阅者与自动化协作者。本文不规定具体功能批次、发布版本号、GitHub 权限账号或临时冲突处理流水；当前产品阶段见 `docs/status/current.md`，远端保护模板见 `.github/rulesets/`。

## 状态

Accepted

## 背景

RadishLex 使用 `dev` 承载常态开发，使用 `master` 承载阶段性稳定主线。若只反复执行 `dev -> master` PR，而不把合并后的 `master` 回同步到 `dev`，Git 历史会长期保留 master 独有的 merge commit 或 rebase 后提交：

- `master` 不再是 `dev` 的祖先，分支图持续分叉。
- 下一次 PR 的提交计数、merge-base 和拓扑判断容易混入历史噪音。
- master 上通过 PR 进入的治理、热修复或合并结果可能没有进入下一轮开发基线。
- 冲突会被推迟到下一次阶段性 PR，增加收口成本。

因此，`dev -> master` 不是单向完成动作；每次阶段性合并都必须以 `master -> dev` 回同步完成拓扑闭环。

## 决策

### 分支职责

- `dev` 是常态开发与日常集成分支。
- `master` 是受保护的稳定主线，不承载常规直接开发。
- 功能、修复、文档和治理改动默认先进入 `dev`；功能分支如存在，也默认先合入 `dev`。
- 阶段性稳定、发布准备或明确收口时，从 `dev` 发起到 `master` 的 Pull Request。
- `master` 上的紧急修复仍应通过 Pull Request 进入，不以直接 push 绕过保护。

### 阶段性 PR 闭环

每次 `dev -> master` PR 合并后，必须在开始下一批常规开发前完成以下动作：

1. 刷新远端 `master` 与 `dev` 引用。
2. 在 `dev` 上合并最新 `master`，保留 master 合并结果的祖先关系。
3. 解决冲突并执行与冲突范围匹配的验证；无内容冲突时至少复核分支状态和拓扑。
4. 将完成回同步的 `dev` 推送到远端。
5. 确认 `origin/master` 已成为 `dev` / `origin/dev` 的祖先，再开始下一批常规提交。

推荐的拓扑验证是：

```bash
git merge-base --is-ancestor origin/master dev
git rev-list --left-right --count origin/master...dev
```

第一条命令应返回成功；第二条的左侧计数应为 `0`。右侧可以大于 `0`，表示回同步后 `dev` 已继续产生新提交。

### 回同步方式

- 当前 `dev` 是共享常态分支，回同步默认使用 merge，不对已推送的 `dev` 做 rebase 或 force push。
- 回同步只负责把已经进入稳定主线的历史带回 `dev`，不夹带新的功能修改或无关重构。
- 当前 `dev` 未启用强制保护时，可以在本地合并 `origin/master` 后推送 `dev`；如果未来 `dev` 启用保护，则改用 `master -> dev` 同步 PR，但闭环要求不变。
- 不因完成 `dev -> master` PR 删除长期 `dev` 分支。
- 若 master PR 使用 rebase merge，回同步仍然必须执行，因为 master 上的稳定提交 hash 与 dev 原提交不同。

### 自动化触发策略

- 直接 push 到 `dev` 不触发 GitHub Actions；日常直接推进继续依赖风险匹配的本地验证。
- 以 `dev` 或 `master` 为目标的 Pull Request 触发同一套 `PR Checks`。`dev` 检查为协作反馈，`master` 检查由 ruleset 配置为 strict required checks。
- `master` 禁止直接 push，只能通过聚合 `Candidate Quality` 和会话解决门禁的 Pull Request 进入；单人维护阶段不要求额外审批，合并后的 `master` push 不重复触发 `PR Checks`。
- `Release Checks` 只监听 `v*-dev`、`v*-test` 与 `v*-release` tag。普通分支 push、非发布 tag 和 PR 不触发发布工作流。
- 准备阶段性 `dev -> master` PR 时，仍须先在本地执行完整仓库门禁并在 PR 中记录真实结果，不能只依赖远端检查发现问题。

### 紧急修复与冲突

- 紧急修复通过专用分支向 `master` 提交 PR 后，同样必须立即回同步到 `dev`。
- 回同步出现冲突时，在 `dev` 上显式解决并验证，不通过覆盖 master、丢弃一侧历史或 force push 隐藏冲突。
- 未完成回同步时，不把下一次 `dev -> master` PR 视为已准备完成；应先恢复 `master` 是 `dev` 祖先的拓扑。

## 后果

收益：

- `master` 的每次稳定合并都会成为下一轮开发的明确祖先。
- PR merge-base、提交范围和冲突时点保持可预测。
- master 上的治理与紧急修复不会遗漏在 dev 之外。
- 无需通过破坏性 rebase 维持共享开发分支。
- 日常 `dev` 连续提交不会重复消耗远端 Actions；其他开发者的 `dev` PR 与稳定主线 PR 仍有可追踪的远端验证记录。

代价：

- 每次阶段性 PR 合并后会多一个明确的回同步动作，通常也会保留一个 merge commit。
- 合并完成不能视为本轮 Git 收尾；必须继续完成 dev 推送和祖先关系复核。
- 如果未来启用 dev 保护，回同步需要额外的同步 PR 与 required checks。
- `dev` 直接 push 不获得远端检查反馈，因此直接提交者必须承担本地验证责任；如希望强制所有开发者通过检查，需要另行启用 `dev` 保护。
