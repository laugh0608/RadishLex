# macOS DMG、公证与 Gatekeeper Runbook

本文指导发布维护者把同一份已冻结 Developer ID Installer 形成签名 DMG，并取得可复验的 notarization、staple 与 Gatekeeper 证据。本文不授权申请/导出证书、写入公证凭据、上传、公开分发、安装产品、修改输入源或清理历史事务材料；执行真实签名和公证前仍需单独授权。

## 固定输入与输出

输入必须来自同一版本的发布构建：

```text
target/macos-release/<version>-<build>/
  Product/
  InstallPayload/
  RadishLex Installer.app/
```

`ProductManifest.json`、`InstallPayloadManifest.json`、`ReleaseIdentity.json`、三份 bundle designated requirement 与 Team ID 必须仍能精确复验。DMG 构建不接受路径参数，不从其他目录搜索 Installer，也不重签 Installer。

固定输出为：

```text
target/macos-release/<version>-<build>/
  RadishLex-<version>-<build>.dmg
  NotarizationSubmission.json
  ReleaseQualification.json
```

`NotarizationSubmission.json` 在 Apple 返回 `Accepted` 后以新文件原子写入，绑定 submission UUID、提交时 DMG SHA-256/size 和 Installer tree SHA-256。后续中断从该 receipt 读取 UUID、重新拉取 notary log，不重复上传。

staple 会修改 DMG，所以 `ReleaseQualification.json` 同时保留提交前 SHA-256 与最终分发 SHA-256。它只记录稳定状态、submission ID、hash 和 size，不保存 Keychain profile、凭据、notary 原始响应、log 正文、路径或签名正文。

## 前置条件

- macOS 13 或更高版本，Xcode command-line tools 中存在 `codesign`、`hdiutil`、`notarytool`、`stapler` 和 `spctl`；
- `RADISHLEX_DEVELOPER_ID_APPLICATION` 精确命中本机有效、证书 OU 为产品固定 Team `WF9UUN335P` 的 `Developer ID Application:` identity；
- Installer 发布根已由 `build-macos-release-installer.sh` 生成，所有嵌套 executable 具有 Hardened Runtime 与 trusted timestamp；
- notary 凭据已由维护者使用 `notarytool store-credentials` 存入 Keychain；执行时只提供 profile 名称，不向脚本传 Apple ID、password、API private key 或 issuer；
- 网络、Apple 服务和 timestamp/notary 资格已单独授权。

普通仓库门禁只运行 parser、shell 语法和缺失身份/凭据失败关闭测试，不调用 timestamp/notary 服务、不挂载真实发布 DMG，也不生成正向证据。

## A. 构建签名 DMG

在工作区干净且发布 Installer 已冻结后执行：

```bash
RADISHLEX_DEVELOPER_ID_APPLICATION="Developer ID Application: …" \
  ./scripts/build-macos-release-dmg.sh
```

入口按固定顺序：

1. 复验 Product、InstallPayload、ReleaseIdentity 与 Installer strict signature；
2. 只把 `RadishLex Installer.app` 复制到私有 staging；
3. 生成 volume name 为 `RadishLex Installer` 的 APFS/UDZO UDIF；
4. 使用同一 Developer ID Application 与 trusted timestamp 签名 DMG；
5. 执行 `codesign --verify --strict` 与 `hdiutil verify`；
6. 只读挂载，要求根目录精确只有一份 Installer app，并复验其 strict signature、内嵌 payload 与 release identity；
7. 通过原子 rename 写入固定 DMG 路径。

目标已存在时入口拒绝覆盖。任何失败只清理本次私有 staging，不修改发布 Installer、产品 assembly 或历史 source。

## B. 提交、staple 与 Gatekeeper

确认 Keychain profile 后执行：

```bash
RADISHLEX_NOTARY_KEYCHAIN_PROFILE="<stored-profile-name>" \
  ./scripts/notarize-macos-release-dmg.sh
```

入口只使用 `--keychain-profile`，并执行：

1. 再次复验 DMG signature 与 UDIF；
2. 若没有 submission receipt，使用 `notarytool submit --wait --output-format json` 提交；
3. 只接受字段集合已知、canonical UUID 且 `status=Accepted` 的结果，再原子持久化 receipt；
4. 按 receipt UUID 获取 notary log，要求 job ID、archive name、提交 SHA-256、`Accepted`、status code `0` 和空 issues 精确匹配；
5. 已有有效 ticket 时直接续跑，否则 staple；随后必须通过 `stapler validate`；
6. 重新验证签名与 UDIF，执行 DMG `spctl --type open --context context:primary-signature`；
7. 只读挂载并要求根对象精确唯一，比较挂载 Installer 与冻结 Installer 的完整 tree，复验 strict signature、内嵌 release identity，并执行 Installer `spctl --type execute`；
8. 最后写入并回读 `ReleaseQualification.json`。

已有 qualification 时入口只复验证据、当前 DMG 和 stapled ticket，成功后幂等返回。未知 submission 字段、非终态/Invalid 状态、log 缺字段、UUID/hash/name 漂移、issues、ticket 缺失、挂载内容漂移或任一 Gatekeeper 拒绝都会失败关闭，不生成 qualification。

## C. 隔离下载复验

本机 qualification 不能替代最终下载隔离证据。公开候选上传后，还必须在未建立本地产物信任的干净 macOS 用户环境：

1. 通过正式下载路径取得同名 DMG；
2. 计算 SHA-256，要求等于 `ReleaseQualification.json` 的 `distribution_sha256`；
3. 运行 `stapler validate` 与 DMG Gatekeeper assessment；
4. Finder 打开 DMG，人工启动 Installer，确认系统不显示未公证/来源不明阻断；
5. 在任何安装 mutation 前，核对 Installer 展示的版本、固定双目标和默认保留数据语义。

该阶段不得移除 quarantine xattr、使用 `spctl --add`、关闭 Gatekeeper、手工重签或复制出另一份 DMG来绕过拒绝。首次安装、修复、升级和移除继续按 [真实用户域验收 Runbook](macos-installer-user-domain-acceptance.md) 执行。

## 当前证据

截至 2026-07-25，仓库已经具备 DMG、公证、staple、双层 Gatekeeper 与稳定证据的失败关闭契约，普通门禁通过。当前机器没有可用 Developer ID Application identity，也没有真实上一发布 assembly，因此没有执行签名 DMG 构建、notary 上传、staple、Gatekeeper 正向验收或隔离下载；仓库不得据此宣称发布供应链完成。
