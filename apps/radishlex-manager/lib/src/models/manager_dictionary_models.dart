class UserTerm {
  const UserTerm({
    required this.inputCode,
    required this.text,
    required this.reading,
    required this.weight,
    required this.source,
    required this.lastUsed,
    this.status = 'active',
    this.importBatchId,
  });

  final String inputCode;
  final String text;
  final String reading;
  final double weight;
  final String source;
  final String lastUsed;
  final String status;
  final int? importBatchId;

  UserTermKey get key =>
      UserTermKey(inputCode: inputCode, text: text, reading: reading);
}

class UserTermKey {
  const UserTermKey({
    required this.inputCode,
    required this.text,
    required this.reading,
  });

  final String inputCode;
  final String text;
  final String reading;

  @override
  bool operator ==(Object other) {
    return other is UserTermKey &&
        other.inputCode == inputCode &&
        other.text == text &&
        other.reading == reading;
  }

  @override
  int get hashCode => Object.hash(inputCode, text, reading);
}

class DeletedTerm {
  const DeletedTerm({
    required this.inputCode,
    required this.text,
    required this.reading,
    required this.deletedAt,
    this.reason = 'manual_delete',
  });

  final String inputCode;
  final String text;
  final String reading;
  final String deletedAt;
  final String reason;

  UserTermKey get key =>
      UserTermKey(inputCode: inputCode, text: text, reading: reading);
}

class DictionaryImportBatchSummary {
  const DictionaryImportBatchSummary({
    required this.id,
    required this.sourceName,
    required this.totalRecords,
    required this.importedTerms,
    required this.insertedTerms,
    required this.updatedTerms,
    required this.skippedDeletedTerms,
    required this.skippedDuplicateTerms,
    required this.createdAt,
    required this.notes,
  });

  final int id;
  final String sourceName;
  final int totalRecords;
  final int importedTerms;
  final int insertedTerms;
  final int updatedTerms;
  final int skippedDeletedTerms;
  final int skippedDuplicateTerms;
  final String createdAt;
  final String notes;
}

class DictionaryImportPreview {
  const DictionaryImportPreview({
    required this.filePath,
    required this.format,
    required this.recordCount,
    required this.syncClass,
  });

  final String filePath;
  final String format;
  final int recordCount;
  final String syncClass;
}

class DictionaryImportResult {
  const DictionaryImportResult({
    required this.filePath,
    required this.sourceName,
    required this.totalRecords,
    required this.importedTerms,
    required this.insertedTerms,
    required this.updatedTerms,
    required this.skippedDeletedTerms,
    required this.skippedDuplicateTerms,
    required this.dryRun,
  });

  final String filePath;
  final String sourceName;
  final int totalRecords;
  final int importedTerms;
  final int insertedTerms;
  final int updatedTerms;
  final int skippedDeletedTerms;
  final int skippedDuplicateTerms;
  final bool dryRun;
}

class DictionaryExportResult {
  const DictionaryExportResult({
    required this.filePath,
    required this.exportedTerms,
    required this.format,
    required this.syncClass,
  });

  final String filePath;
  final int exportedTerms;
  final String format;
  final String syncClass;
}
