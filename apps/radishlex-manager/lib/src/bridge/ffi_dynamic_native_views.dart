part of 'ffi_dynamic_native_binding.dart';

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
