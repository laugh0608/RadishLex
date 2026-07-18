part of 'ffi_dynamic_native_binding.dart';

final class _RadishLexNativeApi {
  _RadishLexNativeApi(ffi.DynamicLibrary library)
    : contract = _RadishLexContractSymbols(library),
      userTerms = _RadishLexUserTermSymbols(library),
      deletedTerms = _RadishLexDeletedTermSymbols(library),
      dictionary = _RadishLexDictionarySymbols(library),
      importBatches = _RadishLexImportBatchSymbols(library),
      learning = _RadishLexLearningSymbols(library),
      sync = _RadishLexSyncSymbols(library),
      rank = _RadishLexRankSymbols(library),
      errors = _RadishLexErrorSymbols(library);

  static const _expectedContractVersion = 5;
  static const _expectedThreadPolicy = 1;
  static const _expectedPanicBoundary = 1;

  final _RadishLexContractSymbols contract;
  final _RadishLexUserTermSymbols userTerms;
  final _RadishLexDeletedTermSymbols deletedTerms;
  final _RadishLexDictionarySymbols dictionary;
  final _RadishLexImportBatchSymbols importBatches;
  final _RadishLexLearningSymbols learning;
  final _RadishLexSyncSymbols sync;
  final _RadishLexRankSymbols rank;
  final _RadishLexErrorSymbols errors;

  void validateContract() {
    final contractOut = calloc<_RadishLexFfiContract>();
    try {
      _callStatus(errors, (errorOut) => contract.load(contractOut, errorOut));
      final value = contractOut.ref;
      if (value.version != _expectedContractVersion ||
          value.sessionThreadPolicy != _expectedThreadPolicy ||
          value.panicBoundary != _expectedPanicBoundary) {
        throw const FfiManagerBridgeException(
          statusCode: 2,
          code: 'ffi_contract_mismatch',
          message:
              'RadishLex native library ABI contract does not match manager',
        );
      }
    } finally {
      calloc.free(contractOut);
    }
  }
}
