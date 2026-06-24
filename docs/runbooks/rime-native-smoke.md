# Rime Native Smoke Runbook

本文档用于指导开发者在 macOS 本机准备隔离的 `librime` smoke 环境，并运行 `radishlex-ime-cli rime` 验证真实 Rime adapter 链路。读者是需要复验 `ime-engine-rime` native 路径的维护者和协作者。本文不包含系统输入法安装、真实用户词库导入、Rime schema 版权评估、平台壳联调或长期 CI 配置。

## 目的

本 runbook 验证以下链路：

```text
radishlex-ime-cli rime
  -> ime-core::InputSession
  -> ime-engine-rime::RimeEngine
  -> librime C API
  -> luna_pinyin schema
  -> composition / candidates / commit
```

通过标准：

- `native-rime` feature 能链接本机 `librime`。
- CLI 能用隔离的 Rime 数据目录输出真实候选。
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

## 1. 设置环境变量

```bash
export RIME_PREFIX="$(brew --prefix librime)"
export RIME_INCLUDE_DIR="$RIME_PREFIX/include"
export RIME_LIB_DIR="$RIME_PREFIX/lib"

export SMOKE="$(mktemp -d /tmp/radishlex-rime-smoke.XXXXXX)"
mkdir -p "$SMOKE/src" "$SMOKE/shared" "$SMOKE/user" "$SMOKE/user/build"

echo "$SMOKE"
```

确认 `echo "$SMOKE"` 输出以 `/tmp/radishlex-rime-smoke.` 开头。

## 2. 下载公开 Rime 数据

这些数据仅用于本机 smoke，不进入 RadishLex 仓库。

```bash
git clone --depth 1 https://github.com/rime/rime-prelude "$SMOKE/src/rime-prelude"
git clone --depth 1 https://github.com/rime/rime-luna-pinyin "$SMOKE/src/rime-luna-pinyin"
git clone --depth 1 https://github.com/rime/rime-essay "$SMOKE/src/rime-essay"

cp "$SMOKE/src/rime-prelude"/*.yaml "$SMOKE/shared"/
cp "$SMOKE/src/rime-luna-pinyin"/*.yaml "$SMOKE/shared"/
cp "$SMOKE/src/rime-essay"/essay.txt "$SMOKE/shared"/
```

检查必要文件：

```bash
ls "$SMOKE/shared/default.yaml"
ls "$SMOKE/shared/symbols.yaml"
ls "$SMOKE/shared/luna_pinyin.schema.yaml"
ls "$SMOKE/shared/luna_pinyin.dict.yaml"
ls "$SMOKE/shared/essay.txt"
```

## 3. 部署隔离 Rime 用户数据

`rime_deployer --add-schema` 和 `--set-active-schema` 会写当前工作目录下的用户配置，因此必须先进入 `$SMOKE/user`。

```bash
cd "$SMOKE/user"
rime_deployer --add-schema luna_pinyin
rime_deployer --set-active-schema luna_pinyin
rime_deployer --build "$SMOKE/user" "$SMOKE/shared" "$SMOKE/user/build"
```

检查 build 产物：

```bash
find "$SMOKE/user/build" -maxdepth 1 -type f | sort
```

通常应能看到 `luna_pinyin` 相关的 schema / table / prism 产物。

## 4. 运行 RadishLex native smoke

```bash
cd /path/to/RadishLex

RIME_INCLUDE_DIR="$RIME_INCLUDE_DIR" \
RIME_LIB_DIR="$RIME_LIB_DIR" \
cargo check -p radishlex-ime-cli --features native-rime
```

类型检查通过后运行 CLI：

```bash
RIME_INCLUDE_DIR="$RIME_INCLUDE_DIR" \
RIME_LIB_DIR="$RIME_LIB_DIR" \
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime \
  --schema luna_pinyin \
  --shared-data "$SMOKE/shared" \
  --user-data "$SMOKE/user" \
  luobo
```

预期输出形态：

```text
schema: luna_pinyin
input: luobo
composition: ...
candidates:
  0. ...
commit: ...
```

具体候选文本取决于 Rime 数据版本；smoke 只要求能输出真实候选并完成 commit。

## 5. 记录结果

若 smoke 成功，应记录：

- `librime` 路径：`brew --prefix librime`
- schema：`luna_pinyin`
- smoke 目录：`$SMOKE`
- CLI 输出是否包含 `schema`、`composition`、`candidates`、`commit`
- 是否发现 candidate index 或 `select_keys` 行为异常

若输出中包含真实用户输入或真实用户词库内容，不要写入仓库日志。

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
    rm -rf "$SMOKE"
    ;;
  *)
    echo "Refuse to remove unexpected SMOKE path: $SMOKE"
    ;;
esac
```
