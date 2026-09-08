# Rime Native Smoke Runbook

本文面向维护者，指导在隔离目录中验证 CLI、FFI 和 native adapter 的输入链。使用仓库锁定的 `radishlex_pinyin` 产品数据；不包含系统输入法安装、真实用户词库、GUI、平台实机验收或依赖安装。Rime 自有学习已禁用，任意 upstream schema 不再是本入口的默认输入。

## 前置与边界

- 已安装 librime、Python 3，Rust 依赖已缓存。macOS 可用 `brew --prefix librime` 只读确认路径；其他平台明确指定已安装库路径。
- 所有测试使用新的临时 user 目录与合成输入，串行执行；不使用真实 Rime 目录、冻结现场或历史失败目录。
- adapter 检查实际生效的 default/schema 配置；`learning_guard` 错误通常表示旧配置、custom patch 或不受支持的组件，不能靠启用 user_dict 解决。
- 基础候选来自 Rime，学习来自 RadishLex。原始 Rime Arrow/Space 高亮遵循 engine 顺序；平台可见候选通过 display index 选择，不能将两者序号混用。

## 标准流程（默认看这里）

在仓库根目录，使用本机实际安装路径：

```bash
export RIME_INCLUDE_DIR=/opt/homebrew/opt/librime/include
export RIME_LIB_DIR=/opt/homebrew/opt/librime/lib
export SMOKE="$(mktemp -d /tmp/radishlex-rime-smoke.XXXXXX)"
./scripts/prepare-rime-product-data.sh assemble --output "$SMOKE/shared"
mkdir "$SMOKE/behavior-user" "$SMOKE/peer-user" "$SMOKE/invalid-user" "$SMOKE/cli-user"
export RADISHLEX_RIME_SHARED_DATA="$SMOKE/shared"
export RADISHLEX_RIME_SCHEMA=radishlex_pinyin
export RADISHLEX_EXPECTED_CANDIDATE_PAGE_SIZE=5

cargo check --offline --locked -p radishlex-ime-cli --features native-rime
cargo test --offline --locked -p radishlex-ime-engine-rime --features native-rime

RADISHLEX_RIME_USER_DATA="$SMOKE/behavior-user" \
cargo test --offline --locked -p radishlex-ime-ffi --features native-rime \
  --test native_rime_behavior rime_session_native_smoke_uses_ffi_entrypoint \
  -- --ignored --exact --nocapture
RADISHLEX_RIME_USER_DATA="$SMOKE/peer-user" \
cargo test --offline --locked -p radishlex-ime-ffi --features native-rime \
  --test ffi_contract_and_dictionary rime_native_sessions_share_runtime_and_survive_peer_release \
  -- --ignored --exact --nocapture
RADISHLEX_RIME_USER_DATA="$SMOKE/invalid-user" \
cargo test --offline --locked -p radishlex-ime-ffi --features native-rime \
  --test ffi_contract_and_dictionary rime_session_native_invalid_schema_reports_engine_error \
  -- --ignored --exact --nocapture

cargo run --offline --locked -p radishlex-ime-cli --features native-rime -- \
  rime --schema radishlex_pinyin --shared-data "$SMOKE/shared" \
  --user-data "$SMOKE/cli-user" luobo
```

FFI smoke 自行部署 schema，覆盖稳定 input code、完整/分段选择、同库学习、删除恢复、普通/隐私/secure 策略、翻页和基础按键。异常 schema 在 session 创建前返回 `EngineError` 与 `learning_guard` 阶段；多 session 测试验证释放一个 session 后另一个仍可输入。默认 ignored 不算已执行。

CLI 可另用 `luobo 1` 检查非首候选、`luobo --key page-down 0` 检查翻页、`luobo 999` 检查越界失败。rank smoke 可在本次临时目录创建独立 SQLite，用 `dict add --db <path> --input luobo --text <当前确有的合成候选>` 添加词条，再用 `rime --rank-db <path> --context chat ...` 检查 engine_index、score、explain 与 commit_engine_index；不把新词添加误解成候选召回已实现。

两套学习存储、旧合成 Rime userdb、上下文往返、独立进程重启和配置漂移必须另跑 [隐私隔离回归](rime-privacy-probe.md)。本入口不能替代那些隐私断言，也不证明真实平台键盘/密码路由通过。

## 记录与排错

记录 source commit、librime 版本与路径、schema 来源锁、临时目录和实际执行入口。新旧测试目录不得混用；失败时保留现场。头文件/动态库缺失时核对 include/lib 路径，缺少依赖另行授权安装；配置失败时回读本次 `user/build` 与 custom patch，使用新目录重做隔离部署，不编辑真实用户配置来绕过 guard。

清理只针对确认属于本次且不再作为证据的临时目录，并遵循任务授权；本 runbook 不自动清理历史目录。

## 历史 smoke 事实（不用于现行重跑）

下列原始记录使用当时的 `luna_pinyin` 和旧学习合同，只证明当时范围，保留原样；不将其命令或目录作为当前输入。

2026-06-25 本机日志 `log/log-202606251903.txt` 的结论：

- `cargo check -p radishlex-ime-cli --features native-rime` 通过。
- 首候选 smoke 输出 `composition: luo bo`，可提交当前 0 号候选。
- 非首候选 smoke 可提交当前 1 号候选。
- `--key page-down 0` 可翻页并提交翻页后的当前 0 号候选。
- `luobo 999` 返回明确错误：`candidate index 999 is out of range for 5 candidates`。

2026-06-25 本机 rank smoke 结论：

- `librime` 路径为 `/opt/homebrew/opt/librime`，schema 为 `luna_pinyin`。
- 有效隔离目录为 `/tmp/radishlex-rime-smoke.HpbV0l`；公开 Rime schema 数据、Rime user data 和临时 rank userdb 均位于该目录下。
- `cargo check -p radishlex-ime-cli --features native-rime` 通过。
- 基础 `luobo` smoke 输出 `composition: luo bo`，当前候选包含 `蘿蔔`、`落泊`、`蘿菠`、`羅柏`、`洛伯`，首候选提交 `蘿蔔`。
- `luobo 1` 可提交当前 1 号候选 `落泊`；`luobo --key page-down 0` 可提交翻页后的当前 0 号候选 `落薄`；`luobo 999` 返回明确越界错误。
- rank userdb 使用 `/tmp/radishlex-rime-smoke.HpbV0l/radishlex-userdb.sqlite`，通过 `dict add --input luobo --text 落泊` 写入当前 Rime 输出中真实存在的候选文本。
- `rime --rank-db /tmp/radishlex-rime-smoke.HpbV0l/radishlex-userdb.sqlite --context chat luobo` 输出 `rank_context: chat`，候选行包含 `engine_index` 和 `score`，explain 行包含 `user_term`、`frequency`、`context`、`negative`、`suppressed`、`deleted`，提交结果包含 `commit_engine_index: 1`。
- 本次 smoke 未使用真实个人词库、真实输入历史、真实 Rime 用户目录或仓库内临时数据目录。

2026-06-27 本机 FFI native smoke 结论：

- `cargo test -p radishlex-ime-ffi --features native-rime` 在 Homebrew `librime` 1.17.0 环境下通过，默认跳过 ignored native smoke。
- `RADISHLEX_RIME_SHARED_DATA=/tmp/radishlex-rime-smoke.HpbV0l/shared RADISHLEX_RIME_USER_DATA=/tmp/radishlex-rime-smoke.HpbV0l/user cargo test -p radishlex-ime-ffi --features native-rime rime_session_native_smoke_uses_ffi_entrypoint -- --ignored` 通过。
- 该时点 smoke 覆盖普通 `radishlex_session_new_rime -> key result -> snapshot -> select_candidate`；当前同名 smoke 已按上文升级为 personalized Rime session，并继续区分完整候选提交与分段候选只更新 composition 的结果。
- 本次 smoke 继续使用隔离 Rime shared / user data，不使用真实个人词库或真实 Rime 用户目录。
