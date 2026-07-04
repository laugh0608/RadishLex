import 'dart:ffi' as ffi;
import 'dart:io' show Platform;

import 'package:ffi/ffi.dart';

import 'ffi_manager_native_models.dart';

part 'ffi_dynamic_native_api.dart';
part 'ffi_dynamic_native_types.dart';

final class DynamicRadishLexManagerNativeBinding
    implements RadishLexManagerNativeBinding {
  DynamicRadishLexManagerNativeBinding(ffi.DynamicLibrary library)
    : _api = _RadishLexNativeApi(library);

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
        _api,
        (errorOut) => _api.userdbTermsNew(dbPathPointer, errorOut),
      );
      try {
        final count = _api.userdbTermsCount(termsHandle);
        final terms = <NativeUserTermRecord>[];
        for (var index = 0; index < count; index += 1) {
          final termOut = calloc<_RadishLexUserTermView>();
          try {
            _callStatus(
              _api,
              (errorOut) =>
                  _api.userdbTermsGet(termsHandle, index, termOut, errorOut),
            );
            terms.add(_copyTermView(termOut.ref));
          } finally {
            calloc.free(termOut);
          }
        }
        return List.unmodifiable(terms);
      } finally {
        _api.userdbTermsFree(termsHandle);
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
              _api,
              (errorOut) => _api.userdbDeleteTerm(
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
          _api,
          (errorOut) => _api.userdbDictionaryInspect(
            filePathPointer,
            summaryOut,
            errorOut,
          ),
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
              _api,
              (errorOut) => _api.userdbDictionaryImport(
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
            _api,
            (errorOut) => _api.userdbDictionaryExport(
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
        _api,
        (errorOut) => _api.userdbImportBatchesNew(dbPathPointer, errorOut),
      );
      try {
        final count = _api.userdbImportBatchesCount(batchesHandle);
        final batches = <NativeImportBatchRecord>[];
        for (var index = 0; index < count; index += 1) {
          final batchOut = calloc<_RadishLexImportBatchView>();
          try {
            _callStatus(
              _api,
              (errorOut) => _api.userdbImportBatchesGet(
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
        _api.userdbImportBatchesFree(batchesHandle);
      }
    });
  }

  @override
  NativeLearningStatusSummary learningStatus(String dbPath) {
    return _withNativeString(dbPath, (dbPathPointer) {
      final summaryOut = calloc<_RadishLexLearningStatusSummary>();
      try {
        _callStatus(
          _api,
          (errorOut) =>
              _api.userdbLearningStatus(dbPathPointer, summaryOut, errorOut),
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
          _api,
          (errorOut) =>
              _api.userdbSyncPreflight(dbPathPointer, summaryOut, errorOut),
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
                _api,
                (errorOut) => _api.userdbRankExplainNew(
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
                    _api,
                    (errorOut) => _api.userdbRankExplainView(
                      explainHandle,
                      viewOut,
                      errorOut,
                    ),
                  );
                  return _copyRankExplainView(viewOut.ref);
                } finally {
                  calloc.free(viewOut);
                }
              } finally {
                _api.userdbRankExplainFree(explainHandle);
              }
            });
          });
        });
      });
    });
  }
}

NativeUserTermRecord _copyTermView(_RadishLexUserTermView term) {
  return NativeUserTermRecord(
    id: term.id,
    inputCode: _readStringView(term.inputCode),
    text: _readStringView(term.text),
    reading: _readOptionalStringView(term.reading, term.readingPresent),
    source: term.source,
    status: term.status,
    weight: term.weight,
    createdAtMs: term.createdAtMs,
    updatedAtMs: term.updatedAtMs,
    lastUsedAtMs: term.lastUsedAtMs,
    lastUsedAtPresent: term.lastUsedAtPresent != 0,
  );
}

NativeDictionaryImportSummary _copyDictionaryImportSummary(
  _RadishLexDictionaryImportSummary summary,
) {
  return NativeDictionaryImportSummary(
    importBatchId: summary.importBatchId,
    importBatchIdPresent: summary.importBatchIdPresent != 0,
    totalRecords: summary.totalRecords,
    importedTerms: summary.importedTerms,
    insertedTerms: summary.insertedTerms,
    updatedTerms: summary.updatedTerms,
    skippedDeletedTerms: summary.skippedDeletedTerms,
    skippedDuplicateTerms: summary.skippedDuplicateTerms,
    dryRun: summary.dryRun != 0,
  );
}

NativeLearningStatusSummary _copyLearningStatusSummary(
  _RadishLexLearningStatusSummary summary,
) {
  return NativeLearningStatusSummary(
    schemaVersion: summary.schemaVersion,
    plaintextPayload: summary.plaintextPayload != 0,
    p1RawDetails: summary.p1RawDetails != 0,
    contextStats: summary.contextStats != 0,
    activeUserTerms: summary.activeUserTerms,
    suppressedUserTerms: summary.suppressedUserTerms,
    rankerWeights: summary.rankerWeights,
    deletedTermTombstones: summary.deletedTermTombstones,
    selectionEvents: summary.selectionEvents,
    negativeFeedback: summary.negativeFeedback,
    importBatches: summary.importBatches,
    latestUserTermUpdatedAtMs: summary.latestUserTermUpdatedAtMs,
    latestUserTermUpdatedAtPresent: summary.latestUserTermUpdatedAtPresent != 0,
    latestSelectionEventAtMs: summary.latestSelectionEventAtMs,
    latestSelectionEventAtPresent: summary.latestSelectionEventAtPresent != 0,
    latestNegativeFeedbackAtMs: summary.latestNegativeFeedbackAtMs,
    latestNegativeFeedbackAtPresent:
        summary.latestNegativeFeedbackAtPresent != 0,
    latestDeletedTermAtMs: summary.latestDeletedTermAtMs,
    latestDeletedTermAtPresent: summary.latestDeletedTermAtPresent != 0,
    latestImportBatchAtMs: summary.latestImportBatchAtMs,
    latestImportBatchAtPresent: summary.latestImportBatchAtPresent != 0,
    latestActivityAtMs: summary.latestActivityAtMs,
    latestActivityAtPresent: summary.latestActivityAtPresent != 0,
  );
}

NativeImportBatchRecord _copyImportBatchView(_RadishLexImportBatchView batch) {
  return NativeImportBatchRecord(
    id: batch.id,
    sourceName: _readStringView(batch.sourceName),
    totalRecords: batch.totalRecords,
    importedTerms: batch.importedTerms,
    insertedTerms: batch.insertedTerms,
    updatedTerms: batch.updatedTerms,
    skippedDeletedTerms: batch.skippedDeletedTerms,
    skippedDuplicateTerms: batch.skippedDuplicateTerms,
    createdAtMs: batch.createdAtMs,
    notes: _readOptionalStringView(batch.notes, batch.notesPresent),
    notesPresent: batch.notesPresent != 0,
  );
}

NativeRankExplainSummary _copyRankExplainView(_RadishLexRankExplainView view) {
  return NativeRankExplainSummary(
    inputCode: _readStringView(view.inputCode),
    candidateText: _readStringView(view.candidateText),
    reading: _readOptionalStringView(view.reading, view.readingPresent),
    readingPresent: view.readingPresent != 0,
    contextKind: _readStringView(view.contextKind),
    originalIndex: view.originalIndex,
    finalScore: view.finalScore,
    engineOrderFactor: view.engineOrderFactor,
    userTermBoost: view.userTermBoost,
    frequencyBoost: view.frequencyBoost,
    recencyBoost: view.recencyBoost,
    contextBoost: view.contextBoost,
    negativeFeedbackPenalty: view.negativeFeedbackPenalty,
    suppressedPenalty: view.suppressedPenalty,
    deletedPenalty: view.deletedPenalty,
  );
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
