# RadishLex

RadishLex 是一个以 Rust 为输入核心、Go 为自部署同步后端、Flutter 为管理界面的源代码可见中文输入系统，中文定位为“萝卜词核”。

项目目标不是再做一个单平台输入法外壳，而是建立本地优先、隐私可信、可解释、可删除、可自部署同步，并能长期学习个人输入习惯的中文输入基础设施。

许可条款以仓库根 [LICENSE](LICENSE) 为准，当前采用 RadishLex Source-Available License。

## 设计原则

- **本地优先**：候选生成、候选重排、学习和常用输入路径必须离线可用。
- **服务端不可信**：后端只保存密文对象和必要 metadata，不进入每次按键热路径。
- **可解释学习**：用户能查看、删除、导出、暂停或限制输入法学到的内容。
- **引擎可替换**：v1 接入成熟底层引擎，Rust core 不依赖其私有实现。
- **平台薄壳**：系统输入法端只处理生命周期、按键、候选展示、文本提交和 FFI。
- **删除优先**：tombstone 防止旧事件、旧设备和旧备份复活用户已删除内容。

## 技术栈

- **Rust**：输入会话、engine adapter、候选重排、用户词库、学习、同步客户端、加密和 FFI。
- **Go**：单用户自部署同步服务、设备、密文对象、版本、备份恢复和审计。
- **Flutter**：本地词库、学习、隐私、同步、设备和诊断管理界面。
- **平台原生薄壳**：macOS InputMethodKit、Linux Fcitx5/IBus、Android IME、Windows TSF、iOS Keyboard Extension。

当前工程成熟度、停止线和下一步只在 [当前状态](docs/status/current.md) 维护。仓库已有 Rust、Go 和 Flutter 工程原型，但真实平台输入法与产品发布闭环仍按路线推进。

## 稳定入口

- [当前状态](docs/status/current.md)：当前批次、验证基线、停止线和下一步。
- [技术方案](docs/technical-plan.md)：稳定架构、职责、输入链和平台策略。
- [阶段路线图](docs/roadmap.md)：长期阶段、交付物和退出标准。
- [仓库结构](docs/repository-layout.md)：实际目录、模块职责和未落地边界。
- [隐私与同步](docs/privacy-sync.md)：数据分级、密钥、删除、恢复和威胁模型。
- [Engine Boundary](docs/engine-boundary.md)：核心 engine 契约。
- [Rime Adapter](docs/engine-rime-adapter.md)：librime adapter 与 native smoke。
- [个人化学习](docs/personalization-learning.md)：userdb、ranker、反馈和词库管理。
- [FFI Boundary](docs/ffi-boundary.md)：C ABI、所有权、线程和错误语义。
- [同步密钥管理](docs/sync-key-management.md)：设备、授权、恢复、撤销和 key epoch。
- [Sync Server API/Storage](docs/sync-server-api-storage.md)：Go API、metadata、blob 和错误语义。
- [Manager Boundary](docs/manager-ui-boundary.md)：Flutter manager 职责与数据可见性。

更细的 ADR、runbook 和协议专题从上述入口按任务进入。临时整改或发布专题只有被 `docs/status/current.md` 引用时才进入日常阅读链。

## 开发验证入口

仓库常态基线：

```bash
./scripts/check-repo.sh
```

Rust CLI demo：

```bash
cargo run -p radishlex-ime-cli -- demo luobo
```

本地词库和学习：

```bash
cargo run -p radishlex-ime-cli -- dict add \
  --db /tmp/radishlex-userdb.sqlite --input luobo --text 萝卜
cargo run -p radishlex-ime-cli -- learn select \
  --db /tmp/radishlex-userdb.sqlite --input luobo --text 萝卜
cargo run -p radishlex-ime-cli -- rank explain \
  --db /tmp/radishlex-userdb.sqlite --input luobo --candidate 萝卜
```

真实 Rime adapter 需要本机 `librime` 和隔离 schema 数据：

```bash
cargo run -p radishlex-ime-cli --features native-rime -- \
  rime --schema luna_pinyin --shared-data <path> --user-data <path> luobo
```

详细命令见 [CLI 说明](docs/cli.md)，本机 Rime 环境见 [Rime Native Smoke Runbook](docs/runbooks/rime-native-smoke.md)。

Flutter manager：

```bash
./scripts/check-manager.sh
./scripts/check-manager-ffi-smoke.sh
```

`check-manager-ffi-smoke.sh` 使用临时 SQLite、settings 和合成词库验证开发期真实 Dart FFI bridge；它不证明正常产品包已经携带 native library，也不连接真实同步服务。

Go sync server：

```bash
(cd server/sync-server && go test ./...)
cargo test -p radishlex-ime-sync
cargo test -p radishlex-ime-userdb --test two_client_go_http_sync
docker compose -f deploy/sync-server/docker-compose.local.yaml config
```

部署、备份恢复、连接健康和证据校验见：

- [Compose Runbook](docs/runbooks/sync-server-compose.md)
- [Local Smoke Runbook](docs/runbooks/sync-server-local-smoke.md)
- [Production Deployment Runbook](docs/runbooks/sync-server-production-deployment.md)

真实 Keychain、Android Keystore、平台输入法安装、Docker 长流程和发布部署可能修改外部环境，必须按对应 runbook 和人工授权执行。

## MVP 边界

v1 不重写完整中文输入引擎。拼音切分、基础候选和长句转换可由成熟底层引擎提供，RadishLex 聚焦：

- 稳定 Rust 输入核心与 engine adapter；
- 用户词库、候选重排和个人化学习；
- 可解释、可删除、可暂停的本地数据；
- 自部署端到端加密同步；
- 真实 manager 产品运行态；
- 至少一个可日常使用的真实平台输入法。

第一真实平台固定为 macOS InputMethodKit。第二平台只有在第一平台达到退出标准后再启动。

## 非目标

- 不做云端实时输入法 API。
- 不默认上传原始输入流、P1 原始事件或明文用户词库。
- 不用 Flutter 或 egui 强行统一系统候选窗。
- 不在 v1 阶段重写完整拼音引擎。
- 不复制外部项目源码结构、实现细节或受限词库。
- 不同时展开全部桌面和移动平台。

## 一句话

RadishLex 是一个把个人输入习惯留在自己手里的源代码可见中文输入系统。
