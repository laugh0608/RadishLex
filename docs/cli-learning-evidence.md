# 学习取证 CLI 参考

本文档面向需要复验本地个人化数据和候选排序的开发者，说明 `learn case-status` 与 `rime snapshot` 的输入、输出、数据边界和组合用法。本文不定义阶段进度、人工验收批次、平台安装流程或 SQLite 表结构；完整命令总览见 [CLI 说明](cli.md)，具体平台操作见对应 runbook。

## 命令职责

| 命令 | 观察对象 | 是否写 userdb | 是否选择候选 |
| --- | --- | --- | --- |
| `learn status` | 全库学习状态摘要 | 打开数据库时可能迁移或维护权限 | 否 |
| `learn case-status` | 一个精确学习身份及全库摘要 | 打开数据库时可能迁移或维护权限 | 否 |
| `rime snapshot` | fresh Rime 状态中的候选顺序，可选接入产品 ranker | 传入 `--rank-db` 时打开数据库可能迁移或维护权限 | 否 |

这里的“取证”表示输出具有稳定字段、明确归属和可重复判定条件，不表示命令是文件系统级只读。两个命令都必须使用明确归属的合成数据与路径，不得指向用户日常使用的 Rime 目录或来源不明的 userdb。

## `learn case-status`

### 用法

```bash
cargo run -p radishlex-ime-cli -- \
  learn case-status \
  --db /tmp/radishlex-userdb.sqlite \
  --input shi \
  --text 时 \
  --context editor
```

完整身份由以下字段组成：

- `--input` 与 `--text` 必填。
- `--reading` 可选；未传入与传入具体 reading 是不同身份。
- `--context` 只限定 ranker weight 的上下文，默认 `general`。

命令先按 userdb 规则规范化身份，再在同一个 SQLite deferred transaction 中读取全库摘要和目标记录，避免 term、ranker weight、tombstone 分属不同数据库时点。成功输出以 `learning_case_status: ready` 开始。

### 输出字段

```text
learning_case_status: ready
inspection_version: 1
identity:
  input: shi
  text: 时
  reading: <none>
  context: editor
aggregate:
  ...
term:
  present: true|false
  ...
ranker_weight:
  present: true|false
  ...
deleted_tombstone:
  present: true|false
  ...
p1_rows: omitted
```

- `inspection_version`：取证 DTO 与文本字段契约版本。消费者应先判断版本，不应依赖未声明的输出顺序推断兼容性。
- `identity`：规范化后的精确查询身份。
- `aggregate`：全库计数和各类最新活动时间；用于判断操作前后的增减量，不包含任何 P1 事件行。
- `term`：目标用户词条。存在时包含 `source`、`status`、`weight`、作为末写版本的 `version_ms`、`last_used_at_ms` 与 `restored_at_ms`。
- `ranker_weight`：目标身份在指定 context 下的摘要，包含 `frequency`、`last_used_at_ms`、`negative_score` 与 `updated_at_ms`。
- `deleted_tombstone`：精确身份的删除标记；存在时包含 `deleted_at_ms`。
- `p1_rows: omitted`：明确声明不返回 selection event 或 negative feedback 明细。`aggregate` 中的 P1 数量和时间仅用于局部一致性判断。

该 DTO 是 CLI/测试取证读模型，不是 manager 产品接口，也不经 FFI 暴露。产品 UI 继续使用受限的聚合状态和 P2 管理接口。

### 判定原则

- 学习选择应同时看到 selection aggregate 增量、目标 term 状态和目标 ranker weight 变化。
- 删除应同时看到目标 term 不再 active、目标 ranker weight 不再提供提升、tombstone 存在。
- 显式恢复应看到 tombstone 消失、term 恢复，并由后续真实选择重新建立所需 ranker 信号；不得把普通新增或导入当作恢复。
- 时间戳只用于同一用例内部的顺序和变化判定，不应用作跨机器全局序号。

## `rime snapshot`

### 用法

shared data 应按 [Rime Native Smoke Runbook](runbooks/rime-native-smoke.md)从来源锁装配，使用显式关闭 Rime 自有学习的产品 schema；未审阅的上游 schema 或配置会被 adapter 拒绝。

```bash
mkdir -m 700 /tmp/radishlex-rime-snapshot.<id>

RIME_INCLUDE_DIR=<include> \
RIME_LIB_DIR=<lib> \
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime snapshot \
  --schema radishlex_pinyin \
  --shared-data <isolated-shared-data> \
  --user-data /tmp/radishlex-rime-snapshot.<id> \
  --deploy-on-start 1 \
  --rank-db /tmp/radishlex-userdb.sqlite \
  --context editor \
  shi
```

`--deploy-on-start` 必须显式为 `0` 或 `1`，使 schema 部署行为进入证据。输入码只接受小写 ASCII 字母和 apostrophe；命令不接受候选索引或 `--key`，避免通过 CLI 参数引入选择、提交或导航动作。

### fresh user data 边界

`--user-data` 必须满足：

- 目录已存在、为空、由当前系统账户拥有且权限精确为 `0700`。
- 路径及祖先不得包含 symlink 或可被其他普通账户替换的目录成分。
- 权威账户目录来自系统账户记录，不信任调用进程覆盖的 `HOME`。
- 不得位于当前用户的 Rime、Squirrel、Input Methods 或 RadishLex Rime 数据路径及其后代。
- 安全检查得到的 canonical 路径就是传给 Rime 的实际路径；启动前目录设备号和 inode 不得漂移。

该约束隔离既有配置和构建缓存；adapter 另行保证 Rime 自有学习关闭。每次需要可归属的新快照都应准备新的 fresh empty 目录；Rime 启动后会在目录中生成配置或 build 数据，因此不能把同一目录再次当作 fresh 基线。

### 运行与输出

命令只向 engine 发送输入码字符。若任一字符意外产生 commit，命令失败，不输出成功快照；正常退出时显式关闭进程级 Rime runtime。

成功输出包含：

- `user_data_isolation: fresh_empty_at_start`：只证明 Rime 启动前满足隔离条件。
- `deploy_on_start`、`schema`、`input`、`composition`：本次 engine 配置与预编辑状态。
- `personalization_status`：未传 `--rank-db` 时为 `not_requested`；传入时为产品 runtime 返回的状态。
- `rank_context`：未接 ranker 时为 `<none>`，否则为显式 context。
- `candidates`：engine-only 模式下 display/engine index 相同；个人化模式同时输出 display index、engine index、最终分数和 explain。
- `selection_api_called: false`：CLI 没有调用候选选择 API。
- `commit_observed: false`：输入字符过程中没有观察到 commit。

`rime snapshot` 是 CLI 层的证据编排，不新增 `Engine` trait 能力，也不修改产品选择路径。它不承诺任意第三方自定义 schema 的内部按键绑定永远不会 commit；实际观察到 commit 时失败关闭。

传入 `--rank-db` 后，命令使用产品 `PersonalizedInputSession`。`UserDb::open` 仍可能执行 schema migration、PRAGMA 和文件权限维护，因此该参数只能指向本次复验明确归属的数据库。

## 组合取证

验证“RadishLex 学习影响候选”时，按同一精确身份组合证据：

1. 用 `learn case-status` 保存操作前的 aggregate、term、ranker weight 和 tombstone。
2. 用新的 fresh Rime user data 运行带 `--rank-db` 的 `rime snapshot`，保存操作前候选及 display/engine index。
3. 通过产品输入 runtime 执行被验证的真实选择、删除或显式恢复动作。
4. 再次运行 `learn case-status`，核对预期增减量和精确目标状态。
5. 再准备另一个 fresh Rime user data 运行相同 snapshot，核对候选顺序和 explain。
6. 最后再次读取 `case-status`，确认 snapshot 本身没有增加 selection event 或改变目标学习状态。

只有候选 UI 顺序不足以归属到 RadishLex：Rime 自身 user data、schema、shared data 和 deploy 结果都可能改变候选。相反，只有数据库计数也不足以证明排序结果。精确读模型、fresh engine 快照、同一产品 ranker 路径和操作前后增减量需要共同成立。

平台实机的固定用例、授权和停止条件见 [macOS R01B 本地个人化验收](runbooks/macos-r01b-personalization-acceptance.md)。
