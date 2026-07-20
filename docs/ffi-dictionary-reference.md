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

`radishlex_userdb_terms_new/count/get/free` 返回 active / suppressed 用户词条。`RadishLexUserTermView.status` 使用 FFI 边界定义的 active/suppressed 数值常量；deleted 不混入该 list。ABI v5 在 view 末尾增加 `import_batch_id: i64` 与 `import_batch_id_present: u8`，用于关联最近一次实际写入该词条的本地导入批次。

`RadishLexUserTermView`：

```text
id: i64
input_code: RadishLexStringView
text: RadishLexStringView
reading: RadishLexStringView
reading_present: u8
source: u32
status: u32
weight: f64
created_at_ms: i64
updated_at_ms: i64
last_used_at_ms: i64
last_used_at_present: u8
import_batch_id: i64
import_batch_id_present: u8
```

稳定 source 常量：`engine_selection = 1`、`manual_import = 2`、`manual_add = 3`、`phrase_learning = 4`。稳定 status 常量：`active = 1`、`suppressed = 2`、`deleted = 3`；其中 deleted 只用于跨入口状态常量兼容，不会出现在 user term list。

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
- `import_batch_id` 只用于本地审计，不进入 P2 导出或同步 payload；v3 升级遗留或从未被实际导入写入的词条必须返回 `import_batch_id_present = 0`，dry run 不创建或改写关联。后续非导入学习不会伪造新的 batch id，既有导入关联仍作为审计来源保留。
- user-term view 的结构布局已随该字段升级到 ABI v5；manager 产品绑定同时校验 ABI contract 与必需 symbol 集。

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
- `dry_run = 0` 时先在事务内记录 import batch，再把实际插入或更新的词条关联到该 batch id；任一步失败都整体回滚。导入仍遵守 deleted tombstone，不复活用户已删除词条。

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
