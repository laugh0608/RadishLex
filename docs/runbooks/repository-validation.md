# 仓库门禁与测试运行

本文供维护者安排本机验证、核对覆盖及处理验证中断；不授权真实安装、GUI、冻结资产或生产资格实验。当前批次状态与实测结果见 current 和当周周志。

## 入口与覆盖

完整验收使用 `./scripts/check-repo.sh`；它自动应用 `run-low-impact.sh` 的单编译任务、单测试线程、单 Go 并发及低优先级设置。`--plan` 只显示阶段与 Rust 命令，不执行验证。各平台 `check-*.sh` 无参数入口仍执行完整专项。

完整仓库入口顺序为文档/治理自检、Rust 格式和 macOS/core Clippy、原生及 Linux repository-only 合同、Go、workspace 默认目标及 doc-test、额外 feature 测试。Linux 专项保留自己的 check/test/Clippy 顺序；本次重点去除 macOS 长矩阵的重复，未声称所有短测试均已去重。

| 原专项 | 仓库完整入口中的执行位置 | 独立入口 |
| --- | --- | --- |
| product-install core、macOS install adapter | 默认测试由 workspace `--all-targets` 覆盖，原 scoped Clippy 前置 | 无参数仍执行 scoped Clippy 与测试 |
| macOS install coordinator | 默认测试由 workspace 覆盖；`qualification-harness` 的 `cancellation_archive` 单独执行；两种 Clippy 保留 | 默认及指定 feature 测试均保留 |
| Installer Driver/Executor/Bridge | 默认测试由 workspace 覆盖；各 scoped Clippy 前置；AppKit 构建、双向映射与 presentation 合同保留 | 三包与原生完整专项 |
| macOS upgrade coordinator | 默认 lib/integration 由 workspace 覆盖；doc-test 单列；两种 Clippy 及 harness `--no-run` 保留 | 默认测试与 harness 编译、原生 host 检查保留 |
| 其余 workspace 包与 doc-test | `cargo test --locked --workspace --all-targets` 和独立 `--doc` 各一次 | 各包原有入口不变 |
| Linux L6 checkpoint feature | 原 Linux 专项继续独立执行 | 不并入默认 feature，也不替代真实 Linux 验收 |

命令清单集中于 `scripts/validation_suites.py`，平台入口与仓库入口共用，避免两套命令漂移。workspace 默认测试具有 Cargo 的正常 feature 合并语义；不以它代替显式 feature 合同或 scoped Clippy。不得全 workspace 开启 `qualification-harness`、`--all-features` 或 `--ignored`；真实程序资格和人工入口不在普通门禁内。observer 的三项容量入口只按其合同另测。

`--phase` 只用于拆分专项静态/测试/原生阶段；输出明确标为 partial gate。`check-repo.sh` 的任何 `--skip-*` 运行也只报告局部检查，不能用于最终验收。完整结论只在同次全部必要阶段成功后产生；runner 没有复用旧门禁结果或断点恢复机制，旧日志不能充当当前通过凭据。Cargo/Go 自身的正常构建与测试缓存保持原有行为。

## 耗时与测试构建

启动时显示阶段顺序与已知耗时参考；每条外部命令打印开始、结束和耗时，长命令每 30 秒报告仍在执行。参考值来自既往运行，不是时限或硬性完成承诺；冷编译和机器负载可能增加耗时。先做静态检查，避免数小时矩阵后才发现格式或 Clippy 错误。

测试构建继续只对锁定 `sha2 0.10.9` 使用 opt-level 3，另对 `radishlex-ime-product-upgrade` 使用 opt-level 1，并显式保留 debug assertions、overflow checks。其余业务 crate 保持默认测试配置，生产 dev/release profile 不变。此优化针对重复 proof 解码与验证的执行成本，不缓存 hash、不省略 fsync/实物复验、不共享可变 fixture，也不减少故障点。

## 子进程与中断

POSIX 下每条验证命令创建自己的进程组，保持降载环境继承。正常返回、失败、SIGINT 或 SIGTERM 后，runner 只对该组发送 TERM 并等待，必要时发送 KILL；不按进程名扫描、不碰其他任务。明确脱离该组的程序不在自动管理保证内，验证程序不得自行 daemonize。非 POSIX 仅能管理直接子进程，不能宣称同等进程树收尾。

HTTP 集成 fixture 先 `go build` 到本次合成目录，再直接启动二进制，持有真正服务的 `Child`；正常测试显式 stop/wait 后才成功，readiness 或断言 panic 也回收服务。构建失败和测试失败保留材料；完整门禁中断时 runner 负责回收整个本次命令组，未知目录及失败证据不自动删除。

中断的返回码和状态必须保留为未完成。下一次先核对后台退出及本次残留，按对应 fixture 合同决定能否新建样本；固定 observer 根仍采用已存在即拒绝规则。进程收尾不能扩展为测试目录、冻结数据或全局缓存清理。

## 修改门禁时的验证

先运行 `python3 -m unittest discover -s scripts -p 'test_repository_*.py'`，覆盖计划中默认目标/doc-test 唯一性、feature 保留、独立入口完整性、部分结果措辞及正常/失败/中断收尾。再运行完整仓库门禁及改动涉及的专项，记录旧/新耗时、编译时间与执行时间的区别。

本机降载与证据表述以[协作细则](../agent-collaboration.md#验证与证据口径)为准；最新未完项见[当前状态](../status/current.md)。
