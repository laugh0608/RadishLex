# FFI Dictionary Reference

本文档是 `ime-ffi` 用户词库文件与导入批次的字段级 ABI 参考，读者是维护 Rust FFI、Flutter manager bridge 和对应测试的开发者。本文不定义跨语言所有权总原则、输入热路径、当前产品阶段或同步协议；通用 ABI、错误、线程和释放规则见 `docs/ffi-boundary.md`。

## 数据边界

dictionary file FFI 只处理用户明确管理的 P2 用户词条 TSV，不导出 P1 选择事件、负反馈明细、上下文统计或 ranker 权重摘要。

当前常量：

```text
RADISHLEX_DICTIONARY_FORMAT_USER_TERMS_V1 = 1
RADISHLEX_SYNC_CLASS_P2_ENCRYPTED_SYNC = 2
```

## 本地词条与 deleted tombstone view

`radishlex_userdb_terms_new/count/get/free` 返回 active / suppressed 用户词条。`RadishLexUserTermView.status` 使用 FFI 边界定义的 active/suppressed 数值常量；deleted 不混入该 list。

`radishlex_userdb_deleted_terms_new/count/get/free` 返回独立的只读 tombstone handle。`RadishLexDeletedTermView`：

```text
input_code: RadishLexStringView
text: RadishLexStringView
reading: RadishLexStringView
reading_present: u8
deleted_at_ms: i64
reason: RadishLexStringView
```

规则：

- input code、text、reading 与 reason view 都借用自 `RadishLexDeletedTermList*`，绑定层必须复制后再调用 `radishlex_userdb_deleted_terms_free`。
- deleted list 只暴露 explicit restore 所需 identity、删除时间和非敏感 reason 分类，不暴露 P1 原始事件、上下文、SQL row ID 或 tombstone 内部版本。
- `radishlex_userdb_restore_term` 仍是唯一恢复入口；普通导入、新增、学习或刷新不得自动恢复 deleted/suppressed 词条。
- 新增 deleted list symbols 没有改变既有 ABI v4 结构布局；manager 产品绑定同时校验 ABI contract 与必需 symbol 集。

## Inspect 与 export

`RadishLexDictionaryInspectSummary`：

```text
format_version: u32
record_count: usize
sync_class: u32
```

`RadishLexDictionaryExportSummary`：

```text
format_version: u32
exported_terms: usize
sync_class: u32
```

规则：

- `radishlex_userdb_dictionary_inspect` 只读取导入文件并返回格式版本、记录数和同步分类，不打开 userdb。
- `radishlex_userdb_dictionary_export` 必须显式传入 SQLite 路径和输出文件路径，只导出 active / suppressed 用户词条字段。

## Import

`RadishLexDictionaryImportSummary`：

```text
import_batch_id: i64
import_batch_id_present: u8
total_records: usize
imported_terms: usize
inserted_terms: usize
updated_terms: usize
skipped_deleted_terms: usize
skipped_duplicate_terms: usize
dry_run: u8
```

规则：

- `radishlex_userdb_dictionary_import` 必须显式传入 SQLite 路径、输入文件路径、可选 source name 和 `dry_run` 的 `0 / 1` 值。
- `dry_run = 1` 时复用实际导入分类逻辑，但不写入词条或 import batch。
- `dry_run = 0` 时写入词条并记录 import batch；导入仍遵守 deleted tombstone，不复活用户已删除词条。

## Import batch view

`RadishLexImportBatchView`：

```text
id: i64
source_name: RadishLexStringView
total_records: usize
imported_terms: usize
inserted_terms: usize
updated_terms: usize
skipped_deleted_terms: usize
skipped_duplicate_terms: usize
created_at_ms: i64
notes: RadishLexStringView
notes_present: u8
```

规则：

- `radishlex_userdb_import_batches_new` 返回只读 `RadishLexImportBatchList*`，由 `radishlex_userdb_import_batches_free` 释放。
- import batch view 中的 string view 借用自 batch list handle，平台端只能在 list 释放前读取，不得缓存裸指针。
- 所有布尔字段只接受 `0` 或 `1`；未知 format version 或 sync class 必须明确拒绝。
- 错误消息、诊断和测试输出不得包含导入文件正文、真实本机路径或 P1 原始事件。
