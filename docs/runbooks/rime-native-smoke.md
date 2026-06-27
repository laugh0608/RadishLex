# Rime Native Smoke Runbook

本文档用于指导开发者在 macOS 本机准备隔离的 `librime` smoke 环境，并运行 `radishlex-ime-cli rime` 与 `ime-ffi` native smoke 验证真实 Rime adapter 链路。读者是需要复验 `ime-engine-rime` native 路径和 FFI Rime session 入口的维护者和协作者。本文不包含系统输入法安装、真实用户词库导入、Rime schema 版权评估、平台壳联调或长期 CI 配置。

## 目的

本 runbook 验证以下链路：

```text
radishlex-ime-cli rime
  -> ime-core::InputSession
  -> ime-engine-rime::RimeEngine
  -> librime C API
  -> luna_pinyin schema
  -> composition / candidates / commit

ime-ffi native smoke
  -> radishlex_session_new_rime
  -> RadishLexSession opaque handle
  -> ime-engine-rime::RimeEngine
  -> snapshot / candidates / commit
```

通过标准：

- `native-rime` feature 能链接本机 `librime`。
- CLI 能用隔离的 Rime 数据目录输出真实候选。
- `ime-ffi` 在显式 `native-rime` feature 下能通过 Rime session options 创建真实 Rime session。
- 不读取真实 Rime 用户目录，不使用真实输入历史或真实用户词库。

## 安全边界

- 不在仓库根目录创建 `private/`、`tmp/` 或 schema 数据目录。
- 不使用 `~/Library/Rime`、`~/.config/ibus/rime` 或其他真实输入法用户目录。
- 不提交下载的 Rime schema、词库、build 产物、日志或 smoke 输出。
- 只输入合成测试串，例如 `luobo`。
- 所有 smoke 数据放在 `mktemp` 创建的系统临时目录中。

macOS 上 `/tmp` 通常指向 `/private/tmp`，但命令中统一使用 `/tmp`，避免误把 `/private/tmp` 理解为仓库内目录。

## 前置条件

确认已安装 `librime`：

```bash
brew --prefix librime
ls "$(brew --prefix librime)/include/rime_api.h"
ls "$(brew --prefix librime)/lib/"*rime*
command -v rime_deployer
```

预期能看到：

- `rime_api.h`
- `librime.dylib`
- `rime_deployer`

## 标准流程（默认看这里）

如果 `/tmp/radishlex-rime-smoke.*` 已经删除，或不确定上一次 smoke 是否污染了用户数据，从本节重新准备。下面是当前唯一推荐的日常 smoke 路径；后面的“常见问题”和“可选清理”只用于排错和收尾。

### 1. 创建隔离目录并下载数据

在任意终端执行：

```bash
cd /Users/luobo/Code/RadishLex

export RIME_PREFIX="$(brew --prefix librime)"
export RIME_INCLUDE_DIR="$RIME_PREFIX/include"
export RIME_LIB_DIR="$RIME_PREFIX/lib"

export SMOKE="$(mktemp -d /tmp/radishlex-rime-smoke.XXXXXX)"
mkdir -p "$SMOKE/src" "$SMOKE/shared" "$SMOKE/user" "$SMOKE/user/build"

echo "$SMOKE"

git clone --depth 1 https://github.com/rime/rime-prelude "$SMOKE/src/rime-prelude"
git clone --depth 1 https://github.com/rime/rime-luna-pinyin "$SMOKE/src/rime-luna-pinyin"
git clone --depth 1 https://github.com/rime/rime-essay "$SMOKE/src/rime-essay"

cp "$SMOKE/src/rime-prelude"/*.yaml "$SMOKE/shared"/
cp "$SMOKE/src/rime-luna-pinyin"/*.yaml "$SMOKE/shared"/
cp "$SMOKE/src/rime-essay"/essay.txt "$SMOKE/shared"/
```

确认 `echo "$SMOKE"` 输出以 `/tmp/radishlex-rime-smoke.` 开头。下载的公开 Rime 数据只放在 `$SMOKE/src`，不进入 RadishLex 仓库。

### 2. 部署隔离 Rime 用户数据

必须进入 `$SMOKE/user` 后再执行 `rime_deployer --add-schema`，不要在仓库根目录执行：

```bash
cd "$SMOKE/user"
rime_deployer --add-schema luna_pinyin
rime_deployer --set-active-schema luna_pinyin
rime_deployer --build "$SMOKE/user" "$SMOKE/shared" "$SMOKE/user/build"
```

检查必要文件和 build 产物：

```bash
ls "$SMOKE/shared/default.yaml"
ls "$SMOKE/shared/symbols.yaml"
ls "$SMOKE/shared/luna_pinyin.schema.yaml"
ls "$SMOKE/shared/luna_pinyin.dict.yaml"
ls "$SMOKE/shared/essay.txt"
find "$SMOKE/user/build" -maxdepth 1 -type f | sort
```

### 3. 运行 native smoke

回到仓库根目录，先做 native feature 类型检查：

```bash
cd /Users/luobo/Code/RadishLex

RIME_INCLUDE_DIR="$RIME_INCLUDE_DIR" \
RIME_LIB_DIR="$RIME_LIB_DIR" \
cargo check -p radishlex-ime-cli --features native-rime
```

然后依次运行四条 smoke：

```bash
# 首候选提交，预期成功。
RIME_INCLUDE_DIR="$RIME_INCLUDE_DIR" \
RIME_LIB_DIR="$RIME_LIB_DIR" \
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime --schema luna_pinyin --shared-data "$SMOKE/shared" --user-data "$SMOKE/user" luobo

# 非首候选提交，预期提交当前候选列表里的 1 号候选。
RIME_INCLUDE_DIR="$RIME_INCLUDE_DIR" \
RIME_LIB_DIR="$RIME_LIB_DIR" \
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime --schema luna_pinyin --shared-data "$SMOKE/shared" --user-data "$SMOKE/user" luobo 1

# 翻页后提交当前页 0 号候选，预期成功。
RIME_INCLUDE_DIR="$RIME_INCLUDE_DIR" \
RIME_LIB_DIR="$RIME_LIB_DIR" \
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime --schema luna_pinyin --shared-data "$SMOKE/shared" --user-data "$SMOKE/user" luobo --key page-down 0

# 越界候选，预期失败且错误信息明确。
RIME_INCLUDE_DIR="$RIME_INCLUDE_DIR" \
RIME_LIB_DIR="$RIME_LIB_DIR" \
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime --schema luna_pinyin --shared-data "$SMOKE/shared" --user-data "$SMOKE/user" luobo 999
```

最后一条越界候选命令预期失败，返回非 0 退出码是正确结果；重点检查错误信息是否明确。

同一个 `$SMOKE/user` 目录不要并行运行多条 Rime smoke 命令。Rime userdb 使用本地数据库锁，并发打开同一隔离用户目录可能触发 LevelDB `LOCK` 或 corruption 报错；遇到这种情况应丢弃该 `$SMOKE`，重新创建隔离目录后串行重跑。

### 4. 运行 rank smoke

rank smoke 用于确认真实 Rime candidates 能进入 `ime-ranker`，并且重排后的候选能映射回原始 engine index 提交。

先创建临时 userdb，并把上一步 `luobo` 输出中确实存在的候选文本写入 userdb。下面的 `<candidate-text>` 需要替换为当前 smoke 输出里的候选文本，不要使用真实个人词库数据：

```bash
RANK_DB="$SMOKE/radishlex-userdb.sqlite"

cargo run -p radishlex-ime-cli -- \
  dict add \
  --db "$RANK_DB" \
  --input luobo \
  --text "<candidate-text>"
```

然后运行带 ranker 的 Rime smoke：

```bash
RIME_INCLUDE_DIR="$RIME_INCLUDE_DIR" \
RIME_LIB_DIR="$RIME_LIB_DIR" \
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime \
  --schema luna_pinyin \
  --shared-data "$SMOKE/shared" \
  --user-data "$SMOKE/user" \
  --rank-db "$RANK_DB" \
  --context chat \
  luobo
```

重点检查：

- 输出包含 `rank_context: chat`。
- candidates 行包含 `engine_index=<n>` 和 `score=<score>`。
- explain 行包含 `user_term`、`frequency`、`context`、`negative`、`suppressed`、`deleted`。
- 若发生提交，输出包含 `commit_engine_index: <n>`。
- `candidate-index` 在该模式下表示重排后的索引，而不是底层 engine 原始索引。

### 5. 运行 FFI native smoke

FFI native smoke 用于确认 `radishlex_session_new_rime` 能通过 C ABI options 创建真实 Rime session，并能完成按键输入、snapshot 候选读取和候选提交。该测试默认标记为 ignored，必须显式传入隔离 Rime 数据目录后运行。

```bash
RIME_INCLUDE_DIR="$RIME_INCLUDE_DIR" \
RIME_LIB_DIR="$RIME_LIB_DIR" \
RADISHLEX_RIME_SHARED_DATA="$SMOKE/shared" \
RADISHLEX_RIME_USER_DATA="$SMOKE/user" \
cargo test -p radishlex-ime-ffi --features native-rime \
  rime_session_native_smoke_uses_ffi_entrypoint -- --ignored
```

FFI native 异常 smoke 用于确认错误 schema 会通过 C ABI 映射为稳定 `EngineError`，并保留 `select_schema` 阶段信息：

```bash
RIME_INCLUDE_DIR="$RIME_INCLUDE_DIR" \
RIME_LIB_DIR="$RIME_LIB_DIR" \
RADISHLEX_RIME_SHARED_DATA="$SMOKE/shared" \
RADISHLEX_RIME_USER_DATA="$SMOKE/user" \
cargo test -p radishlex-ime-ffi --features native-rime \
  rime_session_native_invalid_schema_reports_engine_error -- --ignored
```

重点检查：

- 测试结果为 `ok`。
- 命令只使用 `$SMOKE/shared` 和 `$SMOKE/user`，不读取真实 Rime 用户目录。
- 正常 smoke 不验证平台壳、系统输入法候选窗或 Flutter bridge。
- 异常 smoke 预期错误码为 `EngineError`，错误消息包含 `select_schema` 和缺失 schema 名。

### 6. 记录结论

记录时不要复制整段命令输出，只保留可复验事实：

- `librime` 路径：`brew --prefix librime`
- schema：`luna_pinyin`
- smoke 目录：`$SMOKE`
- `cargo check -p radishlex-ime-cli --features native-rime` 是否通过
- 首候选、非首候选、翻页后候选是否都能按当前输出候选提交
- 越界候选索引是否返回明确错误
- rank smoke 是否输出 `rank_context`、`engine_index`、explain 和 `commit_engine_index`
- FFI native smoke 是否通过 `rime_session_native_smoke_uses_ffi_entrypoint`
- FFI native 异常 smoke 是否通过 `rime_session_native_invalid_schema_reports_engine_error`
- 是否发现 candidate index 或 `select_keys` 行为异常

候选文本取决于 Rime 数据版本和 `$SMOKE/user` 内的学习状态。同一个 `$SMOKE` 目录内重复提交候选后，后续候选顺序可能变化；如果需要稳定复现首轮结果，重新创建一个新的 `$SMOKE` 目录。

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
- 该 smoke 覆盖 `radishlex_session_new_rime -> radishlex_session_push_key -> radishlex_session_snapshot_new -> radishlex_session_commit_candidate`。
- 本次 smoke 继续使用隔离 Rime shared / user data，不使用真实个人词库或真实 Rime 用户目录。

若运行时报 `dyld` 找不到 `librime`，执行：

```bash
export DYLD_LIBRARY_PATH="$RIME_LIB_DIR${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
```

然后重新运行对应 `cargo run` 命令。

## 常见问题

### 找不到 `rime_api.h`

确认环境变量：

```bash
echo "$RIME_INCLUDE_DIR"
ls "$RIME_INCLUDE_DIR/rime_api.h"
```

`RIME_INCLUDE_DIR` 必须指向包含 `rime_api.h` 的目录。

### 找不到 `librime.dylib`

确认：

```bash
echo "$RIME_LIB_DIR"
ls "$RIME_LIB_DIR/"*rime*
```

如果运行时报 `dyld` 找不到动态库，可临时设置：

```bash
export DYLD_LIBRARY_PATH="$RIME_LIB_DIR${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
```

然后重新执行 `cargo run`。

### `select_schema` 失败

优先检查：

```bash
ls "$SMOKE/shared/luna_pinyin.schema.yaml"
ls "$SMOKE/user/build"
```

然后重新执行：

```bash
cd "$SMOKE/user"
rime_deployer --add-schema luna_pinyin
rime_deployer --set-active-schema luna_pinyin
rime_deployer --build "$SMOKE/user" "$SMOKE/shared" "$SMOKE/user/build"
```

### 没有候选

检查 `shared` 目录是否至少包含：

```text
default.yaml
symbols.yaml
luna_pinyin.schema.yaml
luna_pinyin.dict.yaml
essay.txt
```

如果文件存在但仍无候选，保留完整命令输出，用于判断是 Rime 数据部署问题还是 RadishLex adapter 转换问题。

## 可选清理

确认 `$SMOKE` 是本次 `mktemp` 创建的临时目录后再清理：

```bash
case "$SMOKE" in
  /tmp/radishlex-rime-smoke.*|/private/tmp/radishlex-rime-smoke.*)
    /bin/rm -rf -- "$SMOKE"
    ;;
  *)
    echo "Refuse to remove unexpected SMOKE path: $SMOKE"
    ;;
esac
```

如果本机将 `rm` alias 到 `trash`，也可以在确认路径后手动执行：

```bash
trash "$SMOKE"
```
