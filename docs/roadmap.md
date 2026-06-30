# RadishLex 阶段路线图

## Phase 0: 方案冻结

目标：

- 冻结项目名、定位和边界。
- 明确 v1 不重写完整中文输入引擎。
- 明确 Rust core / Go server / Flutter manager 的职责。
- 明确平台薄壳策略。

交付：

- 根 README
- 技术方案
- 仓库结构草案
- 隐私与同步设计

## Phase 1: Rust Core 原型

周期建议：第 1 个月

目标：

- 建立 Rust workspace。
- 定义输入会话、候选、提交、按键事件等核心类型。
- 实现 CLI demo。
- 接入底层引擎 adapter 接口。
- 完成基础单元测试。

交付：

- `ime-core`
- `ime-cli`
- `ime-engine-rime` 原型
- CLI 可输入拼音并返回候选

退出标准：

- CLI 能完成 compose -> candidates -> commit。
- 核心类型不依赖任何具体平台。

## Phase 2: 个人化学习

周期建议：第 2 个月

目标：

- 建立本地 userdb。
- 实现候选重排。
- 实现选择事件记录。
- 实现负反馈。
- 实现词库导入导出。

交付：

- `ime-userdb`
- `ime-ranker`
- SQLite schema
- 用户词管理 CLI

退出标准：

- 用户连续选择某个候选后，该候选排序可提升。
- 用户删除或降权某个词后，该词不会被旧权重立即复活。
- Rime candidates 能进入 ranker，并输出可解释排序与提交映射。
- 用户词库导入导出、导入检查、删除 tombstone 和同步前置计数均可复验。
- FFI 管理入口只暴露受控 userdb、dictionary、learning status 和 sync preflight 摘要，不导出 P1 明细或明文同步 payload。
- 进入 Phase 3 代码前，必须完成 `ime-crypto` 本地 envelope / key / hash 测试、同步密钥与设备生命周期 Rust 模型、删除合并测试、生产级 envelope 组装边界、恢复码 KDF ADR、恢复码 KDF Rust 模型、签名 / 设备密钥存储 ADR、签名 / 设备密钥存储 Rust 模型、真实 P2 payload 解析、合并结果写回 userdb 的执行器、Go server API / storage 边界设计、生产恢复流程设计，以及平台私钥存储 backend ADR。

## Phase 3: 自部署同步

周期建议：第 3 个月

目标：

- 实现 Go 单用户同步服务。
- 实现端到端加密 blob 同步；`ime-crypto` 本地 envelope、key、nonce、AAD、ciphertext hash 和篡改失败测试必须先于 Go server 上传下载。
- 实现设备注册。
- 实现备份恢复。
- 同步密钥、设备授权、撤销、key epoch、签名模型和客户端合并写回 userdb 的 Rust 测试已经完成。
- Go server API / storage、生产恢复流程、生产部署边界和平台私钥存储 backend 边界已由专题文档固定；平台私钥存储 backend capability / unavailable backend 的 Rust 模型和测试已经落地，`apple-keychain-v1` 首个平台 runbook 已固定，macOS Keychain backend 已在 `apple-keychain` feature 下接线并通过非 smoke 测试，真实 Keychain smoke 已执行但阻塞于 `ed25519-v1` 创建。Go server 已起步 metadata / storage / API / runtime 验证模型，并已补 storage conformance tests、SQLite-backed repository、local object storage staged transaction 接线、Ed25519 签名验签门禁、device wrapping encrypted key bytes 承载、recovery wrapped material 读取接口、recovery latest handler、domain / device / join request metadata handler、authorization handler、encrypted object version 上传 / metadata 读取 / payload 下载 handler、request id、panic recovery、非持久审计 hook、SQLite audit_events 写入、`cmd/radishlex-sync-server`、runtime 配置装配、HTTP timeout、对象大小门禁、脱敏 audit logger、本机 smoke runbook、短生命周期双设备 HTTP smoke 测试、Docker Compose 本地 / 部署态入口和容器实际启动 smoke；本地 compose 提供 `https://localhost:7319` 并通过 Caddy internal TLS 到达 sync-server，部署态 compose 提供 HTTP 上游 `http://127.0.0.1:7319`，两者使用同一个对外端口，并附外部 Nginx TLS 终止示例。Rust `ime-sync` 已起步 remote client DTO / transport trait 和 std-only `http://` HTTP transport，覆盖 signed encrypted object upload request、metadata 读取、binary payload 下载、stale conflict latest metadata、server error code 映射、payload length mismatch、真实 HTTP request / response 传递和错误脱敏；Rust HTTP transport 直连 Go server 的短生命周期跨语言测试已覆盖 domain 初始化、signed encrypted object 上传、metadata / payload 读取和 stale conflict；Rust 侧两客户端 userdb harness 已覆盖 P2 payload 加密上传、另一客户端下载解密、合并写回和冲突后 v2 上传；Rust userdb 两客户端真实 Go HTTP 测试已覆盖设备授权、三类 P2 对象上传下载、客户端解密写回、stale conflict 和 v2 重新上传。下一步若进入代码，应判断 Apple 平台签名 / Keychain 存储策略，或推进生产访问控制 / 备份恢复演练的实现证据。

交付：

- `server/sync-server`
- Docker Compose
- `ime-sync`
- `ime-crypto`

退出标准：

- 两台客户端能同步加密用户词库。
- 服务端无法读取明文词库。

## Phase 4: 管理 UI

周期建议：第 4 个月

目标：

- Flutter 管理界面。
- 词库查看、删除、导入、导出。
- 同步状态查看。
- 隐私模式开关。

交付：

- `apps/radishlex-manager`
- Flutter desktop/mobile 基础页面
- Rust core bridge

退出标准：

- 用户能通过 UI 管理已学习词。
- 用户能配置自部署后端。

## Phase 5: 第一个真实平台

周期建议：第 5-6 个月

建议首选：

- Linux Fcitx5 插件，或者
- macOS InputMethodKit 原型。

目标：

- 在真实输入框中使用 RadishLex。
- 输入热路径调用 Rust core。
- 管理 UI 可查看学习结果。

退出标准：

- 至少一个平台可日常打字。
- 学习记录能影响真实输入候选。

## Phase 6: Android

周期建议：第 7-8 个月

目标：

- Kotlin InputMethodService 薄壳。
- Rust core via NDK。
- Flutter 设置 App。
- 本地学习和同步可用。

退出标准：

- Android 真机可作为系统输入法使用。
- 基础拼音、候选、提交、退格、符号页可用。

## Phase 7: Windows

周期建议：第 9-10 个月

目标：

- TSF 薄壳。
- Rust core 集成。
- 基础候选窗。
- 安装与启用流程。

退出标准：

- Windows 桌面应用中可稳定输入中文。
- 候选窗定位和焦点行为通过常见应用验证。

## Phase 8: iOS

周期建议：第 11-12 个月

目标：

- Swift Keyboard Extension。
- Rust core via XCFramework。
- Flutter 设置 App。
- 离线可用。
- full access 同步模式。

退出标准：

- iOS 真机可用。
- 未开启 full access 时不影响基础输入。
- 开启 full access 后可同步密文数据。

## Phase 9: 自研 Rust 引擎

周期建议：第 2 年开始

目标：

- 自研全拼引擎。
- 自研双拼支持。
- 自研词库编译格式。
- 自研语言模型和纠错。

策略：

- 不一次性替代 librime。
- 先做最小可用全拼。
- 用同一套 Engine trait 并行对比。
- 逐步迁移高价值路径。
