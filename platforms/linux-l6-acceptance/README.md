# RadishLex Linux L6 acceptance controller

本目录承载 M5-P05B 的 compile-isolated L6 crash checkpoint、完整测试进程组终止证明与 canonical 脱敏 evidence。读者是 L6 matrix 执行器和 Linux package transaction 维护者。它不是 production installer，不提供日常维护入口，也不授权打开 VM、执行 `dpkg`、写系统目录或清理现场。

`radishlex-linux-l6-acceptance` 是独立 workspace crate 和二进制身份。它唯一通过 `radishlex-linux-product-install/l6-acceptance-checkpoints` 编译 feature 取得八个 checkpoint；production `radishlex-linux-maintenance` 默认 feature 为空，既不识别 acceptance 参数，也不能由环境变量、路径覆盖或普通运行参数打开 checkpoint。

八个 checkpoint 与 [`packaging/linux/l6-matrix.json`](../../packaging/linux/l6-matrix.json) 一一对应。worker 在目标位置通过继承 pipe 发送有界 typed notification 后停住；controller 不读取或轮询 receipt，而是终止 worker 创建的完整独立 process group，等待 `SIGKILL`，再连续扫描 `/proc/*/stat` 证明 group member 为零且没有遗留 `dpkg` child。worker 内仍调用 production fixed-path observer/executor，不能替换 `/usr/bin/dpkg`，不能使用 shell、`PATH`、`--force-*` 或手工修改 receipt/dpkg status。

controller command 只接受 matrix scenario、repository/guest/snapshot 的非敏感 identity、`--authorized-l6-crash`，以及 `--` 后已经通过 production parser 的 `start` command。production command 自身仍必须同时具有 `--authorized-system-mutation`、`--preserve-user-data`、精确 operation ID 与 root-owned canonical artifact pair。每个 crash case 和后续 `resume` 仍需按 runbook 分别授权；本仓库门禁不会运行该二进制。

checkpoint evidence format 固定为 `radishlex-linux-l6-checkpoint-evidence-v1`。它只记录 matrix/build/scenario/checkpoint、repository/guest/snapshot identity、operation ID 的 SHA-256、授权分类、fault 分类、`SIGKILL`/空 process group/无 dpkg child 结论和预期终态；不记录 operation ID 原值、PID、绝对 artifact/staging 路径、`/proc` 或 dpkg 原文、命令诊断和用户数据。Linux 执行时只写固定 `/var/tmp/radishlex-l6-evidence/checkpoints`，目录/文件要求 root ownership、`0700`/`0600`、单 link 与 create-new；测试没有 production 路径 override。

L6 handoff 另由 [`packaging/linux/l6-release-pair.json`](../../packaging/linux/l6-release-pair.json) 与 `build-linux-l6-release-pair.sh` 冻结：source `55351f2` revision 1、target revision 2，target 的 production maintenance binary 必须不含 acceptance markers，只有本 crate 的独立 ELF 同时含 build identity/授权 marker。`radishlex-linux-l6-release-pair-evidence-v1` 记录两份 package 与两个 ELF 的 SHA-256/size/commit/build profile，不记录本机路径、operation ID、PID、proc/dpkg 原文或用户数据；第六套已形成target terminal与S3。后续[`l6-maintenance-refresh.json`](../../packaging/linux/l6-maintenance-refresh.json)只承载较新production ELF；`b891ed1`新clone没有复制、构建或运行本acceptance executable，其唯一production调用因健康repair短路而未进入dpkg。真实repair、controller和crash matrix仍未闭合。

仓库验证入口：

```bash
./scripts/check-linux-l6-controller.sh
./scripts/check-linux-l6-release-pair.sh
./scripts/check-linux-l6-maintenance-refresh.sh
```

这些入口只编译或解析隔离身份并运行 Rust/Python 合成正负向、恢复、参数授权、进程组顺序和 evidence 格式测试，不连接 guest、不执行 maintenance CLI、不调用真实 `dpkg`，也不证明 L6 已运行。真实步骤与停止线见 [Linux L6 Debian package matrix runbook](../../docs/runbooks/linux-l6-package-matrix.md)。
