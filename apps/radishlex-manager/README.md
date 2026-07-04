# RadishLex Manager

RadishLex Manager 是萝卜词核的 Flutter 管理端起步工程。

当前工程通过受控 `ManagerBridge` contract 接入合成 fixture，用于验证 Phase 4 第一批本地管理能力：

- 本地 userdb 词条管理视图。
- 本地学习状态摘要。
- `rank explain` 非敏感摘要。
- `sync preflight` 状态和生产不可用原因。
- 自部署服务端配置草案。

真实远端同步、恢复码、设备授权和平台私钥 backend 成功路径尚未开放。相关停止线见仓库根 `docs/manager-ui-boundary.md`。
当前 fixture bridge 只验证 manager 调用边界、snapshot 更新、删除词条 tombstone 展示、导入检查对话框和导出结果反馈；真实 userdb 读写等待后续 Dart FFI bridge 接线。

## 验证

```bash
../../scripts/check-manager.sh
```

如需在 macOS 桌面运行：

```bash
flutter run -d macos
```
