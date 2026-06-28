# GitHub Rulesets

本目录存放 RadishLex 的仓库规则模板。当前维护默认分支 `master` / `main` 的保护规则模板，`dev` 作为常态开发分支，不启用强制保护。

## 建议流程

1. 日常开发提交到 `dev` 或功能分支。
2. 功能、文档、规范类变更默认先合并到 `dev`。
3. 阶段性稳定后，再从 `dev` 发起到默认分支（当前为 `master`，如切换可适配 `main`）的 Pull Request。
4. 默认分支 PR 必须通过仓库检查。
5. 管理员如需绕过规则，也只能通过 Pull Request，不开放直接 push。

## 默认分支规则说明

- 禁止直接推送到受保护的默认分支（`master` / `main`）。
- 禁止 force push。
- 禁止删除分支。
- 仅允许通过 Pull Request 合并。
- 要求 1 个审批和已解决会话。
- 要求 `Repo Hygiene` 与 `Repository Baseline` 检查通过。
- `Repo Hygiene` 覆盖文本文件卫生、文档篇幅预算和 PR diff 空白检查。
- `Repository Baseline` 覆盖仓库必备文件、协作文件同步、ruleset / workflow 口径一致性和路径预算。
- GitHub 对 Actions required status checks 当前按 job 名匹配，因此 ruleset 中固定写 job 名。
- 允许 `merge` 与 `rebase` 两种合并方式，禁用 `squash`。
- 管理员仅可通过 Pull Request 方式绕过规则，不开放直接 push。

## dev 策略说明

- `dev` 是当前常态开发分支。
- 当前阶段不启用 branch protection。
- `push -> dev` 会自动触发 PR 同级检查，作为日常集成反馈。
- 当前支持 `pull_request -> dev`，但不把 `dev` 配成强制保护分支。
- 如后续进入多人并行开发，再评估是否对 `dev` 追加保护。

## 检查入口

- Windows：`pwsh ./scripts/check-text-files.ps1`
- Windows：`pwsh ./scripts/check-docs.ps1`
- Windows：`pwsh ./scripts/check-repo.ps1`
- Linux / macOS / Git Bash：`./scripts/check-text-files.sh`
- Linux / macOS / Git Bash：`./scripts/check-docs.sh`
- Linux / macOS / Git Bash：`./scripts/check-repo.sh`
- 提交前本地仍执行：`git diff --check`

默认分支 PR 的 GitHub Actions 会在 PR base/head 范围内执行 `git diff --check`，避免干净 checkout 中裸命令没有检查对象。

当前 ruleset 仍只要求仓库级检查名称；`Repository Baseline` 内部已经覆盖 Rust workspace 和 Go sync-server 测试。后续落地 Flutter manager、平台壳或独立发布矩阵后，应同步更新 `scripts/check-repo.*`、workflow 和 `master-protection.json` required checks。

## 应用方式

如果仓库还没有对应 ruleset，可以使用 GitHub CLI 或 REST API 导入：

```bash
gh api repos/<owner>/<repo>/rulesets --method POST --input .github/rulesets/master-protection.json
```

如果仓库中已存在旧 ruleset，建议改用 `PUT /repos/{owner}/{repo}/rulesets/{ruleset_id}` 更新。

`master-protection.json` 中的 `actor_id: 5` 按“RepositoryRole = Admin”模板生成，表示管理员只能通过 PR 绕过规则。

## 配套仓库设置

- 仓库 Merge options 中启用 `Rebase merging`。
- 仓库 Merge options 中启用 `Merge commits`。
- 关闭 `Squash merging`。
- 如后续增加 `CODEOWNERS`，再决定是否开启 code owner review。
