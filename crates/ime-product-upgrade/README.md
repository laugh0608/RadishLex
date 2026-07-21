# ime-product-upgrade 组件说明

本文说明 `radishlex-ime-product-upgrade` 的职责、持久化对象、只读/写入边界和验证入口，面向维护产品数据升级协调器、FFI 与平台宿主的开发者。本文不包含安装器 UI、真实 Application Support 操作步骤、发布进度或历史验证流水；macOS 产品级状态机与回滚约束见 [数据升级协调器边界](../../docs/macos-data-upgrade-coordinator.md)。

## 组件定位

`ime-product-upgrade` 是产品数据升级协调核心。receipt/state 类型不依赖平台 API，当前文件系统编排、身份检查与 Unix socket guard 只在 Unix 构建中提供。它保存严格的 receipt、文件身份、SQLite 一致快照、settings 保留副本、隔离 migration candidate、启动门禁和双端 validation evidence。SQLite schema 与 migration SQL 仍由 `ime-userdb` 独占，macOS 固定路径、容量和进程检查由平台宿主提供。

```text
macOS product host
  ├─ fixed Application Support path / uid / capacity / quiescence
  └─ Manager + InputMethod validation evidence
                         │
                         ▼
                 ime-product-upgrade
                  ├─ receipt + guard
                  ├─ identity checks
                  ├─ snapshot/candidate orchestration
                  └─ startup decision
                         │
                         ▼
                     ime-userdb
                  schema / migration / SQLite checks
```

该 crate 不进入输入热路径，不停止进程，不接受任意产品数据布局，不解释用户词、学习事件、同步对象或 settings 业务含义，也不负责安装包、程序 bundle 切换或备份清理。

## 固定对象与权限

当前只支持 `application-support-v1`。经平台解析并验证的 data root 必须是目标 uid 所有、canonical、非 symlink 的 `0700` 目录。协调状态固定在：

```text
<data-root>/.radishlex-upgrade-v1/
  receipt.json
  receipt.json.tmp
  source-snapshot.sqlite3
  source-snapshot.sqlite3.tmp
  migration-candidate.sqlite3
  migration-candidate.sqlite3.tmp
  source-settings.json
  source-settings.json.tmp
```

状态目录必须为 `0700`，普通文件必须为 `0600`、单 link 普通文件。目录只接受上述白名单对象；未知文件、symlink、hardlink、owner/mode 漂移和中断临时对象均失败关闭并保留现场。

`UpgradeReceiptStore::open` 属于升级协调写路径：状态目录不存在时可以创建并持久化它。产品普通启动不能调用该入口，必须调用完全只读的 `inspect_startup_gate`。

## Receipt 与状态转换

receipt format 固定为 `radishlex-product-upgrade-receipt-v1`，最大 64 KiB，使用 canonical UTF-8 JSON 和单个末尾换行。operation ID 是 32 位小写十六进制；release、layout、schema 和既有 artifact identity 在同一 operation 内不可改写，artifact 只能追加。

正常状态只允许逐步推进：

```text
preflighted -> quiesced -> snapshot_ready -> candidate_migrated
  -> candidate_verified -> switch_prepared -> switched
  -> post_switch_verified -> completed
```

切换前失败只能进入 `aborted_preserved`；`switched` 后失败先进入 `rollback_required`，精确恢复后进入 `rolled_back`。`completed`、`aborted_preserved` 和 `rolled_back` 是终态，不能在同一 operation 内重新开始。

`UpgradeReceiptStore::persist` 要求调用方持有匹配的 `UpgradeProcessGuard`。写入顺序为同目录 `create_new` 临时文件、文件 `fsync`、身份复验、原子 rename、目录 `fsync` 和 canonical bytes 回读；不合法的跨状态替换、证据删除或字段漂移都会被拒绝。

## 主要能力

### 启动门禁

`inspect_startup_gate(data_root, expected_owner_id)` 不创建、删除、chmod、连接 socket 或清理任何对象：

- data root 不存在：`AllowedFirstLaunch`；
- data root 存在但状态目录/receipt 不存在：`AllowedNoUpgradeState`；
- receipt 为终态：`AllowedTerminalReceipt`；
- active guard 或非终态 receipt：阻止启动；
- 损坏 receipt、中断 artifact、未知对象、unsafe root/state 或身份漂移：`FailedClosed`。

调用方只能把三个 `Allowed*` decision 解释为允许。FFI status、result version、decision 或 error code 无法识别时必须失败关闭。

### 快照、settings 与候选

- `create_settings_backup` 从 receipt 已记录身份的固定 settings 创建原样私有副本，不解析或规范化 JSON。
- `create_userdb_snapshot` 使用 `ime-userdb` SQLite backup API 生成一致 standalone snapshot；空间预算为三份 logical snapshot 加 64 MiB reserve。
- `create_userdb_candidate` 只从已记录 snapshot 创建 candidate，并只对隔离临时文件调用 `UserDb::migrate_and_validate`。

三条写路径都在调用前后复验源对象身份，并按临时文件、内容生成、文件持久化、原子 rename、目录持久化、receipt identity、状态推进的顺序提交。任何阶段失败都不得修改原固定 `userdb.sqlite3`。

### 双端验证证据

`record_candidate_validation` 只接受 `UPGRADE_VALIDATION_EVIDENCE_VERSION = 1`：

- Manager evidence 必须包含目标 schema、管理查询和 settings 检查；
- InputMethod evidence 必须包含同一目标 schema、personalized runtime 和候选信号检查；
- 两端通过时，核心再次以 read-only current-schema connection 复验 candidate，才推进 `candidate_verified`；
- Manager 或 InputMethod 明确失败分别以稳定 failure code 进入 `aborted_preserved`。

receipt 不保存 host 输出、任意布尔数组、绝对路径、数据库正文或内容 hash。`candidate_verified` 状态本身是本 operation 已接受双端证据的持久化结论。

## 错误与隐私边界

`UpgradeFilesystemErrorCode` 和 `UpgradeFailureCode` 是调用方分支使用的稳定分类。面向 UI 或普通日志只能输出阶段、稳定 code 和非敏感处置建议；不得记录真实路径、SQL、用户词、输入历史、settings 正文、secret 或 validation host stderr。

发现不确定现场时应保留 receipt、原库、snapshot、candidate 和 settings 副本，不能通过删除状态目录或创建空库绕过门禁。恢复和备份删除必须由后续带版本、固定目标和身份复验的协调动作完成。

## 开发验证

```bash
cargo test -p radishlex-ime-product-upgrade --all-targets
cargo clippy -p radishlex-ime-product-upgrade --all-targets -- -D warnings
./scripts/check-macos-upgrade-preflight.sh
./scripts/check-manager-product.sh
./scripts/check-macos-imk.sh
```

测试只使用隔离合成目录。生产 preflight executable、Manager/InputMethod validation host 和真实产品不得在普通仓库门禁中对真实 Application Support 执行。
