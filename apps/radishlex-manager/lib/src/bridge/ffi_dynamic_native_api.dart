part of 'ffi_dynamic_native_binding.dart';

final class _RadishLexNativeApi {
  _RadishLexNativeApi(ffi.DynamicLibrary library)
    : userTerms = _RadishLexUserTermSymbols(library),
      dictionary = _RadishLexDictionarySymbols(library),
      importBatches = _RadishLexImportBatchSymbols(library),
      learning = _RadishLexLearningSymbols(library),
      sync = _RadishLexSyncSymbols(library),
      rank = _RadishLexRankSymbols(library),
      errors = _RadishLexErrorSymbols(library);

  final _RadishLexUserTermSymbols userTerms;
  final _RadishLexDictionarySymbols dictionary;
  final _RadishLexImportBatchSymbols importBatches;
  final _RadishLexLearningSymbols learning;
  final _RadishLexSyncSymbols sync;
  final _RadishLexRankSymbols rank;
  final _RadishLexErrorSymbols errors;
}
