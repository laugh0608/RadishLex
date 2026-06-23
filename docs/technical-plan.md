# RadishLex 技术方案

## 1. 项目背景

中文输入法的难点不只是拼音转汉字，还包括长期使用中的个人化适配：

- 用户常用的人名、项目名、地名、缩写和专有名词。
- 同音词和近音词的个人排序偏好。
- 不同应用场景下的候选差异，例如聊天、代码、文档、搜索框。
- 常用短句、语气、标点、emoji 和中英混输习惯。
- 用户纠错、删除、改选候选等负反馈。

主流商业输入法通常把这些能力和云服务绑定。RadishLex 的核心假设是：输入习惯应当属于用户本人，并且应该能自部署、可迁移、可审计、可删除。

## 2. 项目目标

RadishLex 的长期目标是做一个全平台中文输入系统：

- 覆盖 Windows、macOS、Linux、Android、iOS。
- 输入热路径本地运行，低延迟、可离线。
- 自部署后端负责同步、备份和多设备协作。
- Rust core 统一输入逻辑和个人化学习。
- Go server 提供简单可靠的私有云能力。
- Flutter 提供跨平台管理 UI。
- 底层输入引擎可替换，先接成熟引擎，长期演进到自研 Rust 引擎。

## 3. 总体架构

```text
Platform IME Shell
  - Windows TSF
  - macOS InputMethodKit
  - Linux Fcitx5 / IBus
  - Android InputMethodService
  - iOS Keyboard Extension
        |
        v
Rust Core
  - session state
  - composing buffer
  - candidate model
  - personalization
  - user dictionary
  - ranker
  - sync client
  - encryption
        |
        v
Engine Adapter
  - librime adapter in v1
  - native Rust engine in future
        |
        v
Local Storage
  - SQLite
  - encrypted profile data
  - local model files

Management UI
  - Flutter desktop/mobile app
  - optional egui engineering tools
        |
        v
Go Self-host Backend
  - device registry
  - encrypted blob storage
  - sync version history
  - backup and restore
  - package distribution
```

## 4. 技术栈分工

### Rust

Rust 是 RadishLex 的核心语言，负责所有需要跨平台复用、低延迟和高可靠性的逻辑。

Rust 负责：

- 输入会话状态机
- 拼音输入过程抽象
- 候选词数据结构
- 候选重排
- 用户词库
- 个人化学习
- 同步客户端
- 端到端加密
- FFI 边界
- CLI 调试工具

Rust 不负责：

- 直接实现所有平台系统输入法协议。
- 强行统一所有候选窗 UI。
- 在 v1 阶段从零实现完整中文输入引擎。

### Go

Go 负责自部署服务端。后端不参与每次按键，不做云端实时转换。

Go 负责：

- 用户和设备注册
- 设备密钥公钥登记
- 加密数据 blob 存储
- 同步版本号与冲突检测
- 词库和模型包分发
- 备份、恢复和审计日志
- Docker Compose 一键部署

Go 不负责：

- 解密用户输入数据。
- 参与候选排序。
- 保存明文输入历史。

### Flutter

Flutter 负责管理体验，而不是所有输入热路径。

Flutter 负责：

- 移动端设置 App
- 桌面管理器
- 同步状态
- 词库管理
- 学习记录可视化
- 隐私设置
- 后端连接配置

Flutter 不建议负责：

- iOS Keyboard Extension 的主键盘 UI。
- Android IME 的核心输入热路径。
- 桌面系统候选窗。

## 5. 平台落地策略

### Windows

- 系统输入法接入使用 TSF 薄壳。
- Rust core 通过 C ABI 或 CXX bridge 调用。
- 候选窗优先使用平台原生能力。
- 设置页使用 Flutter desktop。

风险：

- TSF / COM 工程复杂度高。
- 候选窗定位、焦点和兼容性需要大量实测。

建议：

- Windows 不作为第一个真实平台。
- 等 Rust core 和至少一个 Unix-like 平台稳定后再做。

### macOS

- 系统输入法接入使用 InputMethodKit。
- 外壳使用 Swift / Objective-C 薄层。
- Rust core 编译为 static lib 或 dynamic lib。
- 设置页使用 Flutter desktop。

风险：

- 输入法沙盒、候选窗、权限和安装流程要重点验证。

建议：

- macOS 适合作为第一批桌面平台之一。

### Linux

- 优先接 Fcitx5 插件。
- 其次考虑 IBus。
- Wayland 下尽量走输入法框架 panel，不自己硬造浮窗协议。
- 设置页使用 Flutter desktop 或 egui 工具。

风险：

- 桌面环境差异大。
- X11 / Wayland 行为差异明显。

建议：

- Linux 适合作为第一个真实输入法端，因为调试成本低，开源社区友好。

### Android

- 键盘服务使用 Kotlin 的 InputMethodService 薄壳。
- Rust core 通过 NDK 编译为 `.so`。
- 设置 App 使用 Flutter。
- 键盘 UI v1 可用 Kotlin 原生实现，避免 Flutter engine 冷启动和内存成本。

风险：

- 输入法生命周期复杂。
- 不同厂商系统会有兼容性差异。

建议：

- Android 是移动端首选平台。

### iOS

- 键盘扩展使用 Swift / UIKit。
- Rust core 编译为 XCFramework。
- 设置 App 使用 Flutter。
- 默认离线可用；同步需要用户开启 full access。

风险：

- iOS 自定义键盘限制最多。
- 默认无网络能力。
- 安全输入框、电话键盘等场景会切回系统键盘。
- App Store 审核和隐私说明要求高。

建议：

- iOS 放在 Android 之后。
- 隐私可信和离线体验必须先做好。

## 6. 底层输入引擎策略

第一阶段建议接入 librime，理由是：

- 成熟。
- 跨平台。
- 中文输入能力完整。
- BSD-3-Clause 许可证相对友好。
- 已有大量输入方案和词库生态。

但 RadishLex 不应把 librime 作为不可替换核心。正确抽象是：

```text
trait Engine {
    fn reset(&mut self);
    fn push_key(&mut self, key: KeyEvent) -> EngineResult;
    fn candidates(&self) -> Vec<EngineCandidate>;
    fn commit(&mut self, index: usize) -> CommitResult;
    fn set_schema(&mut self, schema: SchemaId);
}
```

v1：

- `ime-engine-rime` 提供 librime adapter。
- Rust core 在候选结果之上做个人化重排和学习。

v2：

- 开始自研 Rust 拼音核心。
- 先实现全拼、双拼和基础词库。
- 不急于覆盖五笔、粤拼、仓颉等方案。

v3：

- Rust engine 逐步替代更多底层能力。
- librime 仍可作为兼容引擎保留。

## 7. 个人化学习设计

### 学习对象

- 用户词：专有名词、项目名、人名、缩写。
- 候选偏好：同音词排序。
- 短语习惯：常用二元、三元短语。
- 场景偏好：不同 App 或输入场景的权重差异。
- 标点习惯：中文标点、英文标点、空格习惯。
- 中英混输：代码、变量名、命令、英文缩写。
- emoji 和符号习惯。

### 正反馈

- 选择候选并提交。
- 连续多次选择同一候选。
- 输入后未立即删除。
- 在同一上下文反复使用同一短语。

### 负反馈

- 提交后立即退格删除。
- 改选同音候选。
- 删除整段刚输入内容。
- 手动从学习记录中移除。

### 排序因子

候选最终分数可以由这些因素组成：

```text
final_score =
  engine_score
  + user_word_boost
  + recency_boost
  + frequency_boost
  + app_context_boost
  + phrase_context_boost
  - negative_feedback_penalty
  - decay_penalty
```

### 可解释性

管理 UI 应展示：

- 最近学会的词。
- 高频词。
- 被降权的词。
- 某个词为什么排在前面。
- 哪些 App 允许学习。
- 哪些数据参与同步。

用户必须能：

- 删除单个词。
- 清空某个 App 的学习数据。
- 暂停学习。
- 开启临时隐私模式。
- 导出自己的词库。

## 8. 本地存储

建议本地存储使用 SQLite。

原因：

- 跨平台成熟。
- 易调试。
- 移动端可用。
- 支持事务。
- 适合用户词库、事件日志和同步元数据。

核心表草案：

```text
user_terms
  id
  text
  reading
  source
  weight
  created_at
  updated_at
  last_used_at

selection_events
  id
  session_id
  input_code
  selected_text
  candidate_index
  app_context
  created_at

negative_feedback
  id
  text
  reading
  reason
  app_context
  created_at

app_profiles
  id
  app_id
  learning_enabled
  sync_enabled
  profile_weight

sync_objects
  id
  object_type
  local_version
  remote_version
  encrypted_blob
  updated_at
```

## 9. 同步与隐私

### 后端定位

后端是私有同步服务，不是云输入服务。

后端保存：

- 设备列表。
- 加密后的同步对象。
- 对象版本。
- 备份快照。
- 模型和词库包。

后端不保存：

- 明文输入历史。
- 明文用户词库。
- 明文候选偏好。
- 明文上下文片段。

### 加密策略

建议采用端到端加密：

- 用户首次初始化生成主密钥。
- 每台设备生成设备密钥对。
- 同步对象在客户端加密。
- 服务端只保存密文 blob。
- 新设备加入需要已有设备授权或恢复码。

### 冲突策略

不同对象采用不同策略：

- 用户词：按词合并，权重做合成。
- 删除操作：使用 tombstone，避免被旧设备复活。
- 设置项：last-write-wins 或显式冲突提示。
- 事件日志：只追加，压缩后生成统计状态。

## 10. 服务端 API 草案

```text
POST /api/v1/devices/register
POST /api/v1/devices/authorize
GET  /api/v1/sync/objects
PUT  /api/v1/sync/objects/{id}
POST /api/v1/sync/batch
GET  /api/v1/backups
POST /api/v1/backups
POST /api/v1/backups/{id}/restore
GET  /api/v1/packages
GET  /api/v1/packages/{id}
GET  /api/v1/health
```

服务端 MVP：

- Go
- SQLite 默认
- Postgres 可选
- Docker Compose
- 单用户模式优先
- 后期支持多用户

## 11. UI 方案

### Flutter 管理界面

主要页面：

- 总览
- 当前输入方案
- 用户词库
- 学习记录
- 隐私模式
- 应用学习权限
- 同步状态
- 设备列表
- 后端连接
- 备份恢复
- 导入导出

设计原则：

- 管理 UI 不进入输入热路径。
- 输入法启用后，设置页面可以独立升级。
- 移动端和桌面端共享大部分页面。

### 平台候选窗

候选窗不强行统一：

- Windows 使用 TSF / Win32 原生能力。
- macOS 使用 Cocoa / InputMethodKit。
- Linux 走 Fcitx5 / IBus panel。
- Android 使用 Kotlin 原生 view。
- iOS 使用 Swift / UIKit。

这样可以降低兼容性风险。

## 12. Clean-room 实现原则

用户希望尽量不直接引用现成开源项目，必要时换语言照着实现。这里需要严格区分“借鉴行为”和“复制实现”。

允许：

- 阅读公开文档。
- 观察公开软件行为。
- 总结输入法交互规格。
- 自己设计数据结构和模块边界。
- 用兼容许可证的库作为可选 adapter。

不建议：

- 复制源码。
- 复制私有函数结构。
- 复制测试数据中带有版权风险的词库。
- 从 GPL/LGPL 项目搬实现进 permissive core。
- 把开源项目的实现细节逐行翻译成 Rust。

建议流程：

1. 调研已有项目。
2. 写行为规格文档。
3. 根据规格重新设计 Rust API。
4. 实现自己的模块。
5. 用黑盒测试验证行为，而不是复制源码测试。

## 13. 主要风险

### 输入质量风险

中文输入质量长期依赖词库、语言模型、纠错和排序。短期内自研引擎很难超过成熟项目。

应对：

- v1 接成熟引擎。
- 先做个人化层。
- 逐步自研替换。

### 平台集成风险

系统输入法接入比普通 App 难很多。

应对：

- 每个平台只写薄壳。
- 不追求 UI 完全统一。
- 优先 Linux / macOS / Android。

### iOS 限制风险

iOS 自定义键盘限制严格。

应对：

- iOS 放后期。
- 离线优先。
- 同步需要用户显式开启。

### 隐私信任风险

输入法天然敏感，任何上传行为都会降低信任。

应对：

- 默认不上传明文。
- 开源同步协议。
- 本地可视化学习记录。
- 提供一键清除和禁学名单。

## 14. 成功标准

MVP 成功标准：

- 能在 CLI 中输入拼音并得到候选。
- 能记录用户选择并影响下一次候选排序。
- 能删除学习到的词。
- 能导入/导出用户词库。
- 能连接自部署 Go 后端。
- 能端到端加密同步用户词库。
- 至少一个桌面或移动平台能作为真实输入法使用。

长期成功标准：

- 用户在多台设备上的词库和候选习惯自然一致。
- 用户可以完全掌控输入习惯数据。
- 服务端可以自部署且无需明文数据。
- 输入热路径稳定低延迟。
- 底层引擎可以从 librime 平滑演进到 Rust 自研。
