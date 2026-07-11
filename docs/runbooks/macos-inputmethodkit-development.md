# macOS InputMethodKit 开发构建与人工 smoke

本文档供执行 RadishLex M1 macOS 开发 bundle 构建、授权后安装、真实应用输入和回滚的维护者使用。它不包含发布签名、公证、普通用户分发、Rime schema 下载、真实用户词库迁移或 M2 学习验证；稳定边界见 `docs/macos-inputmethodkit-boundary.md`。

## 无系统改动的默认验证

```bash
./scripts/check-macos-imk.sh
```

该命令只在 `target/` 下构建 contract bundle 和 smoke executable，不写 `~/Library/Input Methods`，不注册输入源，不启动或重启输入法服务。contract bundle 使用合成 demo engine，只证明 wrapper、bundle 和 FFI 调用链，不证明真实 Rime 或 InputMethodKit 可用。

## native bundle 前提

1. 使用已有、显式指定的 `librime` include/lib；构建脚本不安装依赖。
2. 准备来源与许可证已确认的 shared data/schema，并复制到与任何真实用户输入法目录无关的隔离目录。
3. schema id 只允许 ASCII 字母、数字、点、下划线和连字符。
4. smoke 只使用合成词，不记录窗口正文、输入历史、联系人或其他敏感信息。

```bash
RIME_INCLUDE_DIR=<include> \
RIME_LIB_DIR=<lib> \
RADISHLEX_RIME_SHARED_DATA=<isolated-shared-data> \
RADISHLEX_RIME_SCHEMA=<schema-id> \
./platforms/macos-imk/build-bundle.sh native
```

产物为 `target/macos-imk/native/RadishLex.inputmethod`。构建完成后先用 `plutil -lint`、`otool -L` 和 `nm -gU` 检查 plist、`@rpath/libradishlex_ime_ffi.dylib`、`librime` 开发依赖与三个关键 FFI symbol，再考虑安装。

## 授权停止线

以下步骤会修改本机输入法状态，未取得当次明确授权时必须停止：

- 写入 `~/Library/Input Methods`；
- 使用系统设置启用输入源；
- 启动、终止或重启输入法相关进程；
- 在真实应用输入框执行 smoke；
- 删除已安装 bundle 或修改系统输入源配置。

取得授权后，执行者应先记录准备安装的生成 bundle 路径与回滚目标，再把 bundle 复制到用户级 Input Methods 目录。启用动作优先通过系统设置人工完成，不把注册、服务重启或系统数据库修改写进自动脚本。

## 人工 smoke

至少在两个不同的 macOS 应用输入框中使用合成输入复验：

1. composition、首候选和非首候选提交；
2. 数字选择、空格、翻页、方向键、Backspace、Escape 和 Enter；
3. 中英文混输与未消费普通键/快捷键回到宿主；
4. input client 切换、停用再启用和进程重启；
5. 断网前后输入行为一致；
6. 退出时没有活动 session 阻止 runtime shutdown。

只记录通过/失败、错误类别和脱敏环境信息，不记录输入正文或截图中的敏感内容。

## 回滚

完成 smoke 后停用 RadishLex 输入源，再移除本次安装的用户级 bundle。若输入法进程仍持有旧 bundle，按授权范围终止对应开发进程后复核；不要删除 shared data 来源目录、用户其他输入法目录或任何非本次生成的数据。最终确认系统设置中不再启用 RadishLex，仓库外只保留执行者明确要求保留的隔离开发数据。
