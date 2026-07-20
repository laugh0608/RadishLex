# Manager 同步 review-only 归档

本目录保存 2026-07 R06A 清理前的 Manager 同步命令设计与审批材料，仅用于历史追溯。

这些文档描述的 action preview、future command DTO、审批矩阵、migration review、fake replay、候选 C ABI 和 ADR 0006 均未形成产品接口，相关生产模型与测试资产已经删除。它们不属于当前阅读链，不应作为 M3 实现输入，也不应通过恢复文件的方式重新进入生产源码。

M3 开始真实同步实现时，应以当时的 `docs/privacy-sync.md`、`docs/sync-key-management.md`、`docs/production-recovery-flow.md`、`docs/sync-server-api-storage.md`、Rust sync/crypto API 和平台密钥 backend 为基础重新设计。当前 Manager UI 职责、secret 生命周期和停止线见 `docs/manager-sync-entry-boundary.md`。
