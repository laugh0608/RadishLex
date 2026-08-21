# GitHub Rulesets

本目录存放 RadishLex 的仓库规则模板。当前维护默认分支 `master` / `main` 的保护规则模板，`dev` 作为常态开发分支，不启用强制保护。

## 建议流程

1. 日常开发提交到 `dev` 或功能分支。
2. 功能、文档、规范类变更默认先合并到 `dev`。
3. 直接 push 到 `dev` 不触发 GitHub Actions；以 `dev` 或 `master` 为目标的 Pull Request 触发 `PR Checks`。
4. 以 `dev` 为目标的检查为协作反馈，不作为当前未保护 `dev` 的强制门禁。
5. 阶段性稳定后，从 `dev` 发起到 `master` 的 Pull Request；聚合 `Candidate Quality` 和已解决会话共同构成合并门禁。
6. PR 合并后，在下一批常规开发前把最新 `master` merge 回 `dev` 并推送，确认 `master` 是 `dev` 的祖先；该回同步 push 不触发 Actions。
7. 只有创建 `v*-dev`、`v*-test` 或 `v*-release` tag 时才触发 `Release Checks`。
8. 管理员如需绕过规则，也只能通过 Pull Request，不开放直接 push。

## 默认分支规则说明

- 禁止直接推送到受保护的默认分支（`master` / `main`）。
- 禁止 force push。
- 禁止删除分支。
- 仅允许通过 Pull Request 合并。
- 单人维护阶段不要求额外审批，但仍要求已解决会话。
- 仅要求聚合检查 `Candidate Quality` 通过，并启用 strict/up-to-date policy；它会汇总 `Repo Hygiene`、`Repository Baseline`、`Rust Clippy`、`Flutter Manager` 与 `Go Quality` 五个组件结果。
- `Repo Hygiene` 覆盖文本、文档预算和 PR diff 空白检查；`Repository Baseline` 额外覆盖必需治理文件、Issue Forms、Markdown 相对链接及检查器单测，其余 job 分别覆盖 portable baseline、严格 Rust lint、Flutter format/analyze/test 和 Go vet/race。
- 允许 `merge` 与 `rebase` 两种合并方式，禁用 `squash`。
- 管理员仅可通过 Pull Request 方式绕过规则，不开放直接 push。

## dev 策略说明

- `dev` 是当前常态开发分支。
- 当前阶段不启用 branch protection。
- 直接 push 到 `dev` 不触发 GitHub Actions；以 `dev` 为目标的 Pull Request 会触发完整 `PR Checks`，供其他开发者和并行分支协作使用。
- 因 `dev` 当前未保护，这些检查不会强制阻止合并；提交者仍须记录风险匹配的本地验证。
- 每次 `dev -> master` PR 合并后，必须把最新 `master` merge 回 `dev`；该回同步是阶段性 PR 的收尾，不是可选的反向功能流。
- `dev` 是共享分支，回同步不使用 rebase 或 force push；完成后应确认 `git merge-base --is-ancestor origin/master dev` 返回成功。
- 如后续进入多人并行开发，再评估是否对 `dev` 追加保护。

## 检查入口

- Windows：`pwsh ./scripts/check-text-files.ps1`
- Windows：`pwsh ./scripts/check-docs.ps1`
- Windows：`pwsh ./scripts/check-repo.ps1`
- Linux / macOS / Git Bash：`./scripts/check-text-files.sh`
- Linux / macOS / Git Bash：`./scripts/check-docs.sh`
- Linux / macOS / Git Bash：`./scripts/check-repo.sh`
- 提交前本地仍执行：`git diff --check`

`PR Checks` 只监听目标为 `dev` 或 `master` 的 Pull Request，并在 PR base/head 范围执行 `git diff --check`。普通分支 push、直接 push 到 `dev`、PR 合并后的 `master` push 和手动 dispatch 都不会触发该工作流。

native macOS bundle 仍使用独立平台门禁，不并入 Ubuntu portable baseline。发布 tag 只触发 `Release Checks`，不重复触发 `PR Checks`。

## 应用方式

如果仓库还没有对应 ruleset，可以使用 GitHub CLI 或 REST API 导入：

```bash
gh api repos/<owner>/<repo>/rulesets --method POST --input .github/rulesets/master-protection.json
```

如果仓库中已存在旧 ruleset，建议改用 `PUT /repos/{owner}/{repo}/rulesets/{ruleset_id}` 更新。

远端 ruleset 只绑定稳定的 `Candidate Quality` context，并启用 strict/up-to-date policy；五个组件及其聚合依赖由仓库基线校验。更新本目录模板不会自动修改 GitHub 远端设置，远端变更仍须独立授权并复核。

本目录模板还包含 Conventional Commits 的远端校验规则。若现有远端 ruleset 尚未启用该规则，而本次只授权调整 Actions 触发策略或 required checks，应基于远端当前配置构造精确 PUT payload，不要直接用完整模板扩大远端治理范围。

`master-protection.json` 中的 `actor_id: 5` 按“RepositoryRole = Admin”模板生成，表示管理员只能通过 PR 绕过规则。

## 配套仓库设置

- 仓库 Merge options 中启用 `Rebase merging`。
- 仓库 Merge options 中启用 `Merge commits`。
- 关闭 `Squash merging`。
- 如后续增加 `CODEOWNERS`，再决定是否开启 code owner review。
- 如果后续形成稳定的多人评审安排，再提高 `required_approving_review_count`；单人阶段不应把管理员 bypass 当作每次合并的常规路径。
