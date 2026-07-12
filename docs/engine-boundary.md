# RadishLex Engine Boundary

本文档用于说明 RadishLex Rust core 与底层输入引擎之间的稳定边界，读者是实现 `ime-core`、engine adapter、CLI demo 和平台薄壳的开发者。本文只定义 v1 所需的领域模型、生命周期和验证口径，不包含当前批次状态、`librime` 绑定细节、平台输入法协议、候选排序权重公式或同步协议；`librime` adapter 细节见 `docs/engine-rime-adapter.md`，实时进度见 `docs/status/current.md`。

## 稳定定位

- `ime-core` 定义核心类型和 trait。
- `ime-core` 不依赖 `librime`、SQLite、Go server、Flutter 或平台原生 SDK。
- 测试可使用 test-only stub engine 验证生命周期，但不能把 stub 称为真实中文输入引擎。
- `ime-engine-rime` 只能通过 `ime-core` 暴露的 engine boundary 与 core 通信。
- 平台接入必须消费完整 `KeyOutcome`，不能在 FFI 或平台壳丢弃 `consumed`、即时 commit 或错误。
- 当前里程碑、实现证据和下一步只在 `docs/status/current.md` 与 devlog 维护。

## 边界原则

- Rust core 只处理 RadishLex 稳定领域模型，不散落底层 engine 私有概念。
- Engine adapter 负责把底层候选转换为 RadishLex candidate。
- 候选重排、学习事件、用户词库和同步只依赖 RadishLex candidate，不依赖底层 engine 的对象 ID。
- 平台壳只负责系统输入法生命周期、按键接收、候选窗展示和文本提交，不承载排序、学习、同步或隐私策略真相源。
- 输入热路径必须本地可用，engine boundary 不应引入网络调用。

## 核心模型

v1 的 `ime-core` 至少包含这些稳定模型：

- `KeyEvent`：平台或 CLI 传入的按键事件，包含字符键、命名键、修饰键和按键阶段。
- `KeyOutcome`：一次按键或候选选择的结果，明确表达是否消费以及可选 commit；没有 commit 不等于操作失败。
- `Composition`：当前预编辑文本和光标位置。
- `Candidate`：可展示候选，包含候选文本、读音、注释和来源。
- `Commit`：提交给宿主应用的文本，以及本次提交来源。
- `SchemaId`：当前输入方案标识。
- `SessionState`：一次状态快照，包含 composition、candidates 和 schema。

这些模型必须保持平台无关。平台私有生命周期、候选窗定位、TSF / InputMethodKit / Fcitx5 对象句柄、Android `InputConnection` 或 iOS extension 状态不能进入这些模型。

## Engine Trait

v1 engine boundary 的最小接口为：

```rust
pub trait Engine {
    fn reset(&mut self) -> CoreResult<()>;
    fn push_key(&mut self, key: KeyEvent) -> CoreResult<KeyOutcome>;
    fn composition(&self) -> CoreResult<Composition>;
    fn candidates(&self) -> CoreResult<Vec<Candidate>>;
    fn select_candidate(&mut self, index: usize) -> CoreResult<KeyOutcome>;
    fn set_schema(&mut self, schema: SchemaId) -> CoreResult<()>;
    fn schema(&self) -> CoreResult<SchemaId>;
}
```

接口语义：

- `reset` 清空当前输入会话状态，不删除用户词库或学习数据。
- `push_key` 只处理一个按键事件，返回该按键是否被输入法消费，以及是否产生提交。
- `composition` 返回当前预编辑文本。
- `candidates` 返回当前候选列表，列表顺序是进入 ranker 前的 engine 输出顺序。
- `select_candidate` 按当前页候选列表索引驱动底层引擎选择，并与 `push_key` 一样返回完整 `KeyOutcome`；分段候选可能消费选择并更新 composition，但不立即产生 commit。
- `set_schema` 切换输入方案，切换失败必须显式报错。
- `schema` 返回当前输入方案标识。

## Adapter 职责

`ime-engine-rime` 或后续自研 engine adapter 负责：

- 管理底层 engine session。
- 把底层 key 表示转换为 `KeyEvent` 可表达的结果，或在 adapter 内部完成不可泄露的映射。
- 把底层 composition 转换为 `Composition`。
- 把底层 candidate 转换为 `Candidate`。
- 把底层错误转换为 `CoreError` 或后续更细分错误类型。
- 屏蔽 C / C++ 指针、对象生命周期、线程限制和底层私有 ID。

Adapter 不负责：

- 写入用户词库。
- 直接更新候选重排权重。
- 做同步、加密或设备授权。
- 把底层 engine 私有对象暴露给 Flutter、Go server 或平台壳。

## 错误与生命周期

错误必须可诊断，不应静默吞掉：

- 空 schema id 是明确错误。
- composition cursor 必须落在 UTF-8 字符边界。
- candidate index 越界必须返回错误。
- 底层 engine 失败必须携带可读错误信息。

生命周期基本顺序：

```text
InputSession::new(engine)
  -> set_schema(schema)
  -> push_key(...)
  -> state()
  -> select_candidate(index)
  -> reset()
```

CLI demo、真实 engine adapter 和平台壳都应沿用这个生命周期。后续若 FFI 需要稳定 ABI，应在 `ime-ffi` 中包一层句柄管理，不直接把 Rust trait 暴露为 ABI。

## Clean-room 约束

Engine boundary 可以参考公开输入法行为和公开文档，但不能复制外部项目源码、私有函数结构、词库数据或测试数据。对 `librime` 的接入应作为兼容许可证 adapter，而不是把 `librime` 的内部结构固化进 `ime-core`。

调研流程应保持：

1. 观察公开行为。
2. 写行为规格。
3. 设计 RadishLex 自有 API。
4. 实现 adapter。
5. 用黑盒测试验证行为。

## 验证口径

进入下一步前，至少应满足：

- `cargo test -p radishlex-ime-core` 通过。
- `cargo run -p radishlex-ime-cli -- demo luobo` 能展示 composition、候选和 commit。
- `radishlex-ime-cli rime` 在启用 `native-rime` 且配置真实 `librime` 与隔离 schema 数据后，能展示真实 composition、候选和 commit。
- test-only stub engine 能完成 `push_key -> candidates -> select_candidate`。
- 核心类型不依赖平台 SDK 或真实底层 engine。
- 文档入口能指向本边界说明。

该验证证明 Rust core 边界和首条真实 Rime adapter 路径可复验；仍不证明完整中文输入质量、个人化学习、用户词库、ranker、同步或平台输入法可用。
