part of 'ffi_dynamic_native_binding.dart';

final class _RadishLexContractSymbols {
  _RadishLexContractSymbols(ffi.DynamicLibrary library)
    : load = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<_RadishLexFfiContract>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<_RadishLexFfiContract>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_ffi_contract');

  final int Function(
    ffi.Pointer<_RadishLexFfiContract>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  load;
}

final class _RadishLexUserTermSymbols {
  _RadishLexUserTermSymbols(ffi.DynamicLibrary library)
    : newList = library
          .lookupFunction<
            ffi.Pointer<_RadishLexUserTermList> Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            ffi.Pointer<_RadishLexUserTermList> Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_terms_new'),
      count = library
          .lookupFunction<
            ffi.Size Function(ffi.Pointer<_RadishLexUserTermList>),
            int Function(ffi.Pointer<_RadishLexUserTermList>)
          >('radishlex_userdb_terms_count'),
      get = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<_RadishLexUserTermList>,
              ffi.Size,
              ffi.Pointer<_RadishLexUserTermView>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<_RadishLexUserTermList>,
              int,
              ffi.Pointer<_RadishLexUserTermView>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_terms_get'),
      free = library
          .lookupFunction<
            ffi.Void Function(ffi.Pointer<_RadishLexUserTermList>),
            void Function(ffi.Pointer<_RadishLexUserTermList>)
          >('radishlex_userdb_terms_free'),
      deleteTerm = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_delete_term'),
      restoreTerm = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_restore_term');

  final ffi.Pointer<_RadishLexUserTermList> Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  newList;
  final int Function(ffi.Pointer<_RadishLexUserTermList>) count;
  final int Function(
    ffi.Pointer<_RadishLexUserTermList>,
    int,
    ffi.Pointer<_RadishLexUserTermView>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  get;
  final void Function(ffi.Pointer<_RadishLexUserTermList>) free;
  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  deleteTerm;
  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  restoreTerm;
}

final class _RadishLexDeletedTermSymbols {
  _RadishLexDeletedTermSymbols(ffi.DynamicLibrary library)
    : newList = library
          .lookupFunction<
            ffi.Pointer<_RadishLexDeletedTermList> Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            ffi.Pointer<_RadishLexDeletedTermList> Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_deleted_terms_new'),
      count = library
          .lookupFunction<
            ffi.Size Function(ffi.Pointer<_RadishLexDeletedTermList>),
            int Function(ffi.Pointer<_RadishLexDeletedTermList>)
          >('radishlex_userdb_deleted_terms_count'),
      get = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<_RadishLexDeletedTermList>,
              ffi.Size,
              ffi.Pointer<_RadishLexDeletedTermView>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<_RadishLexDeletedTermList>,
              int,
              ffi.Pointer<_RadishLexDeletedTermView>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_deleted_terms_get'),
      free = library
          .lookupFunction<
            ffi.Void Function(ffi.Pointer<_RadishLexDeletedTermList>),
            void Function(ffi.Pointer<_RadishLexDeletedTermList>)
          >('radishlex_userdb_deleted_terms_free');

  final ffi.Pointer<_RadishLexDeletedTermList> Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  newList;
  final int Function(ffi.Pointer<_RadishLexDeletedTermList>) count;
  final int Function(
    ffi.Pointer<_RadishLexDeletedTermList>,
    int,
    ffi.Pointer<_RadishLexDeletedTermView>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  get;
  final void Function(ffi.Pointer<_RadishLexDeletedTermList>) free;
}

final class _RadishLexDictionarySymbols {
  _RadishLexDictionarySymbols(ffi.DynamicLibrary library)
    : inspect = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexDictionaryInspectSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexDictionaryInspectSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_dictionary_inspect'),
      importFile = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Uint8,
              ffi.Pointer<_RadishLexDictionaryImportSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              int,
              ffi.Pointer<_RadishLexDictionaryImportSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_dictionary_import'),
      exportFile = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexDictionaryExportSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexDictionaryExportSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_dictionary_export');

  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<_RadishLexDictionaryInspectSummary>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  inspect;
  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    int,
    ffi.Pointer<_RadishLexDictionaryImportSummary>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  importFile;
  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<_RadishLexDictionaryExportSummary>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  exportFile;
}

final class _RadishLexImportBatchSymbols {
  _RadishLexImportBatchSymbols(ffi.DynamicLibrary library)
    : newList = library
          .lookupFunction<
            ffi.Pointer<_RadishLexImportBatchList> Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            ffi.Pointer<_RadishLexImportBatchList> Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_import_batches_new'),
      count = library
          .lookupFunction<
            ffi.Size Function(ffi.Pointer<_RadishLexImportBatchList>),
            int Function(ffi.Pointer<_RadishLexImportBatchList>)
          >('radishlex_userdb_import_batches_count'),
      get = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<_RadishLexImportBatchList>,
              ffi.Size,
              ffi.Pointer<_RadishLexImportBatchView>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<_RadishLexImportBatchList>,
              int,
              ffi.Pointer<_RadishLexImportBatchView>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_import_batches_get'),
      free = library
          .lookupFunction<
            ffi.Void Function(ffi.Pointer<_RadishLexImportBatchList>),
            void Function(ffi.Pointer<_RadishLexImportBatchList>)
          >('radishlex_userdb_import_batches_free');

  final ffi.Pointer<_RadishLexImportBatchList> Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  newList;
  final int Function(ffi.Pointer<_RadishLexImportBatchList>) count;
  final int Function(
    ffi.Pointer<_RadishLexImportBatchList>,
    int,
    ffi.Pointer<_RadishLexImportBatchView>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  get;
  final void Function(ffi.Pointer<_RadishLexImportBatchList>) free;
}

final class _RadishLexLearningSymbols {
  _RadishLexLearningSymbols(ffi.DynamicLibrary library)
    : status = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexLearningStatusSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexLearningStatusSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_learning_status');

  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<_RadishLexLearningStatusSummary>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  status;
}

final class _RadishLexSyncSymbols {
  _RadishLexSyncSymbols(ffi.DynamicLibrary library)
    : preflight = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexSyncPreflightSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexSyncPreflightSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_sync_preflight');

  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<_RadishLexSyncPreflightSummary>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  preflight;
}

final class _RadishLexRankSymbols {
  _RadishLexRankSymbols(ffi.DynamicLibrary library)
    : newExplain = library
          .lookupFunction<
            ffi.Pointer<_RadishLexRankExplain> Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            ffi.Pointer<_RadishLexRankExplain> Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_rank_explain_new'),
      view = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<_RadishLexRankExplain>,
              ffi.Pointer<_RadishLexRankExplainView>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<_RadishLexRankExplain>,
              ffi.Pointer<_RadishLexRankExplainView>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_rank_explain_view'),
      free = library
          .lookupFunction<
            ffi.Void Function(ffi.Pointer<_RadishLexRankExplain>),
            void Function(ffi.Pointer<_RadishLexRankExplain>)
          >('radishlex_userdb_rank_explain_free');

  final ffi.Pointer<_RadishLexRankExplain> Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  newExplain;
  final int Function(
    ffi.Pointer<_RadishLexRankExplain>,
    ffi.Pointer<_RadishLexRankExplainView>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  view;
  final void Function(ffi.Pointer<_RadishLexRankExplain>) free;
}

final class _RadishLexErrorSymbols {
  _RadishLexErrorSymbols(ffi.DynamicLibrary library)
    : code = library
          .lookupFunction<
            ffi.Int32 Function(ffi.Pointer<_RadishLexError>),
            int Function(ffi.Pointer<_RadishLexError>)
          >('radishlex_error_code'),
      message = library
          .lookupFunction<
            ffi.Pointer<ffi.Char> Function(ffi.Pointer<_RadishLexError>),
            ffi.Pointer<ffi.Char> Function(ffi.Pointer<_RadishLexError>)
          >('radishlex_error_message'),
      free = library
          .lookupFunction<
            ffi.Void Function(ffi.Pointer<_RadishLexError>),
            void Function(ffi.Pointer<_RadishLexError>)
          >('radishlex_error_free');

  final int Function(ffi.Pointer<_RadishLexError>) code;
  final ffi.Pointer<ffi.Char> Function(ffi.Pointer<_RadishLexError>) message;
  final void Function(ffi.Pointer<_RadishLexError>) free;
}
