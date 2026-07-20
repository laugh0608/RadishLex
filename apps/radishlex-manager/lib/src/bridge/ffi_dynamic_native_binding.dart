import 'dart:ffi' as ffi;
import 'dart:io' show Platform;

import 'package:ffi/ffi.dart';

import 'ffi_manager_native_models.dart';

part 'ffi_dynamic_native_api.dart';
part 'ffi_dynamic_native_calls.dart';
part 'ffi_dynamic_native_symbols.dart';
part 'ffi_dynamic_native_types.dart';
part 'ffi_dynamic_native_views.dart';

final class DynamicRadishLexManagerNativeBinding
    implements RadishLexManagerNativeBinding {
  DynamicRadishLexManagerNativeBinding(ffi.DynamicLibrary library)
    : _api = _RadishLexNativeApi(library) {
    _api.validateContract();
  }

  factory DynamicRadishLexManagerNativeBinding.open({String? libraryPath}) {
    final path = libraryPath?.trim();
    final library = path == null || path.isEmpty
        ? ffi.DynamicLibrary.open(_defaultLibraryName())
        : ffi.DynamicLibrary.open(path);
    return DynamicRadishLexManagerNativeBinding(library);
  }

  final _RadishLexNativeApi _api;

  @override
  List<NativeUserTermRecord> listUserTerms(String dbPath) {
    return _withNativeString(dbPath, (dbPathPointer) {
      final termsHandle = _callPointer<_RadishLexUserTermList>(
        _api.errors,
        (errorOut) => _api.userTerms.newList(dbPathPointer, errorOut),
      );
      try {
        final count = _api.userTerms.count(termsHandle);
        final terms = <NativeUserTermRecord>[];
        for (var index = 0; index < count; index += 1) {
          final termOut = calloc<_RadishLexUserTermView>();
          try {
            _callStatus(
              _api.errors,
              (errorOut) =>
                  _api.userTerms.get(termsHandle, index, termOut, errorOut),
            );
            terms.add(_copyTermView(termOut.ref));
          } finally {
            calloc.free(termOut);
          }
        }
        return List.unmodifiable(terms);
      } finally {
        _api.userTerms.free(termsHandle);
      }
    });
  }

  @override
  List<NativeDeletedTermRecord> listDeletedTerms(String dbPath) {
    return _withNativeString(dbPath, (dbPathPointer) {
      final termsHandle = _callPointer<_RadishLexDeletedTermList>(
        _api.errors,
        (errorOut) => _api.deletedTerms.newList(dbPathPointer, errorOut),
      );
      try {
        final count = _api.deletedTerms.count(termsHandle);
        final terms = <NativeDeletedTermRecord>[];
        for (var index = 0; index < count; index += 1) {
          final termOut = calloc<_RadishLexDeletedTermView>();
          try {
            _callStatus(
              _api.errors,
              (errorOut) =>
                  _api.deletedTerms.get(termsHandle, index, termOut, errorOut),
            );
            terms.add(_copyDeletedTermView(termOut.ref));
          } finally {
            calloc.free(termOut);
          }
        }
        return List.unmodifiable(terms);
      } finally {
        _api.deletedTerms.free(termsHandle);
      }
    });
  }

  @override
  void deleteUserTerm({
    required String dbPath,
    required String inputCode,
    required String text,
    required String? reading,
  }) {
    _withNativeString(dbPath, (dbPathPointer) {
      _withNativeString(inputCode, (inputCodePointer) {
        _withNativeString(text, (textPointer) {
          _withOptionalNativeString(reading, (readingPointer) {
            _callStatus(
              _api.errors,
              (errorOut) => _api.userTerms.deleteTerm(
                dbPathPointer,
                inputCodePointer,
                textPointer,
                readingPointer,
                errorOut,
              ),
            );
          });
        });
      });
    });
  }

  @override
  void restoreUserTerm({
    required String dbPath,
    required String inputCode,
    required String text,
    required String? reading,
  }) {
    _withNativeString(dbPath, (dbPathPointer) {
      _withNativeString(inputCode, (inputCodePointer) {
        _withNativeString(text, (textPointer) {
          _withOptionalNativeString(reading, (readingPointer) {
            _callStatus(
              _api.errors,
              (errorOut) => _api.userTerms.restoreTerm(
                dbPathPointer,
                inputCodePointer,
                textPointer,
                readingPointer,
                errorOut,
              ),
            );
          });
        });
      });
    });
  }

  @override
  NativeDictionaryInspectSummary inspectDictionaryImport(String filePath) {
    return _withNativeString(filePath, (filePathPointer) {
      final summaryOut = calloc<_RadishLexDictionaryInspectSummary>();
      try {
        _callStatus(
          _api.errors,
          (errorOut) =>
              _api.dictionary.inspect(filePathPointer, summaryOut, errorOut),
        );
        final summary = summaryOut.ref;
        return NativeDictionaryInspectSummary(
          formatVersion: summary.formatVersion,
          recordCount: summary.recordCount,
          syncClass: summary.syncClass,
        );
      } finally {
        calloc.free(summaryOut);
      }
    });
  }

  @override
  NativeDictionaryImportSummary importDictionaryFile({
    required String dbPath,
    required String filePath,
    required String? sourceName,
    required bool dryRun,
  }) {
    return _withNativeString(dbPath, (dbPathPointer) {
      return _withNativeString(filePath, (filePathPointer) {
        return _withOptionalNativeString(sourceName, (sourceNamePointer) {
          final summaryOut = calloc<_RadishLexDictionaryImportSummary>();
          try {
            _callStatus(
              _api.errors,
              (errorOut) => _api.dictionary.importFile(
                dbPathPointer,
                filePathPointer,
                sourceNamePointer,
                dryRun ? 1 : 0,
                summaryOut,
                errorOut,
              ),
            );
            return _copyDictionaryImportSummary(summaryOut.ref);
          } finally {
            calloc.free(summaryOut);
          }
        });
      });
    });
  }

  @override
  NativeDictionaryExportSummary exportDictionaryFile({
    required String dbPath,
    required String filePath,
  }) {
    return _withNativeString(dbPath, (dbPathPointer) {
      return _withNativeString(filePath, (filePathPointer) {
        final summaryOut = calloc<_RadishLexDictionaryExportSummary>();
        try {
          _callStatus(
            _api.errors,
            (errorOut) => _api.dictionary.exportFile(
              dbPathPointer,
              filePathPointer,
              summaryOut,
              errorOut,
            ),
          );
          final summary = summaryOut.ref;
          return NativeDictionaryExportSummary(
            formatVersion: summary.formatVersion,
            exportedTerms: summary.exportedTerms,
            syncClass: summary.syncClass,
          );
        } finally {
          calloc.free(summaryOut);
        }
      });
    });
  }

  @override
  List<NativeImportBatchRecord> listImportBatches(String dbPath) {
    return _withNativeString(dbPath, (dbPathPointer) {
      final batchesHandle = _callPointer<_RadishLexImportBatchList>(
        _api.errors,
        (errorOut) => _api.importBatches.newList(dbPathPointer, errorOut),
      );
      try {
        final count = _api.importBatches.count(batchesHandle);
        final batches = <NativeImportBatchRecord>[];
        for (var index = 0; index < count; index += 1) {
          final batchOut = calloc<_RadishLexImportBatchView>();
          try {
            _callStatus(
              _api.errors,
              (errorOut) => _api.importBatches.get(
                batchesHandle,
                index,
                batchOut,
                errorOut,
              ),
            );
            batches.add(_copyImportBatchView(batchOut.ref));
          } finally {
            calloc.free(batchOut);
          }
        }
        return List.unmodifiable(batches);
      } finally {
        _api.importBatches.free(batchesHandle);
      }
    });
  }

  @override
  NativeLearningStatusSummary learningStatus(String dbPath) {
    return _withNativeString(dbPath, (dbPathPointer) {
      final summaryOut = calloc<_RadishLexLearningStatusSummary>();
      try {
        _callStatus(
          _api.errors,
          (errorOut) =>
              _api.learning.status(dbPathPointer, summaryOut, errorOut),
        );
        return _copyLearningStatusSummary(summaryOut.ref);
      } finally {
        calloc.free(summaryOut);
      }
    });
  }

  @override
  NativeSyncPreflightSummary syncPreflight(String dbPath) {
    return _withNativeString(dbPath, (dbPathPointer) {
      final summaryOut = calloc<_RadishLexSyncPreflightSummary>();
      try {
        _callStatus(
          _api.errors,
          (errorOut) =>
              _api.sync.preflight(dbPathPointer, summaryOut, errorOut),
        );
        final summary = summaryOut.ref;
        return NativeSyncPreflightSummary(
          schemaVersion: summary.schemaVersion,
          plaintextPayload: summary.plaintextPayload != 0,
          syncableUserTerms: summary.syncableUserTerms,
          syncableRankerWeights: summary.syncableRankerWeights,
          syncableDeletedTerms: summary.syncableDeletedTerms,
          localSelectionEvents: summary.localSelectionEvents,
          localNegativeFeedback: summary.localNegativeFeedback,
          localImportBatches: summary.localImportBatches,
        );
      } finally {
        calloc.free(summaryOut);
      }
    });
  }

  @override
  NativeSyncProductStatus syncProductStatus() {
    final statusOut = calloc<_RadishLexManagerSyncProductStatus>();
    try {
      _callStatus(
        _api.errors,
        (errorOut) => _api.sync.productStatus(statusOut, errorOut),
      );
      final status = statusOut.ref;
      return NativeSyncProductStatus(
        version: status.version,
        signingBackend: status.signingBackend,
        signingAlgorithm: status.signingAlgorithm,
        signingCompiled: status.signingCompiled,
        signingRuntimeAvailable: status.signingRuntimeAvailable,
        signingCanCreate: status.signingCanCreate,
        signingCanSign: status.signingCanSign,
        signingExportable: status.signingExportable,
        signingHardwareBacked: status.signingHardwareBacked,
        signingUserPresenceRequired: status.signingUserPresenceRequired,
        signingBackupMigratable: status.signingBackupMigratable,
        signingProductQualified: status.signingProductQualified,
        keyAgreementBackend: status.keyAgreementBackend,
        keyAgreementCompiled: status.keyAgreementCompiled,
        keyAgreementRuntimeQualified: status.keyAgreementRuntimeQualified,
        keyAgreementProductQualified: status.keyAgreementProductQualified,
        productQualified: status.productQualified,
        userSyncEnabled: status.userSyncEnabled,
        blocker: status.blocker,
      );
    } finally {
      calloc.free(statusOut);
    }
  }

  @override
  NativeRankExplainSummary rankExplain({
    required String dbPath,
    required String inputCode,
    required String candidateText,
    required String? reading,
    required String contextKind,
  }) {
    return _withNativeString(dbPath, (dbPathPointer) {
      return _withNativeString(inputCode, (inputCodePointer) {
        return _withNativeString(candidateText, (candidateTextPointer) {
          return _withOptionalNativeString(reading, (readingPointer) {
            return _withNativeString(contextKind, (contextKindPointer) {
              final explainHandle = _callPointer<_RadishLexRankExplain>(
                _api.errors,
                (errorOut) => _api.rank.newExplain(
                  dbPathPointer,
                  inputCodePointer,
                  candidateTextPointer,
                  readingPointer,
                  contextKindPointer,
                  errorOut,
                ),
              );
              try {
                final viewOut = calloc<_RadishLexRankExplainView>();
                try {
                  _callStatus(
                    _api.errors,
                    (errorOut) =>
                        _api.rank.view(explainHandle, viewOut, errorOut),
                  );
                  return _copyRankExplainView(viewOut.ref);
                } finally {
                  calloc.free(viewOut);
                }
              } finally {
                _api.rank.free(explainHandle);
              }
            });
          });
        });
      });
    });
  }
}

String _defaultLibraryName() {
  if (Platform.isMacOS || Platform.isIOS) {
    return 'libradishlex_ime_ffi.dylib';
  }
  if (Platform.isWindows) {
    return 'radishlex_ime_ffi.dll';
  }
  return 'libradishlex_ime_ffi.so';
}
