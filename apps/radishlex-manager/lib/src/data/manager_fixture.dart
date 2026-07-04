import '../models/manager_models.dart';

ManagerSnapshot createManagerFixture() {
  return ManagerSnapshot(
    generatedAt: '2026-07-04 10:42',
    dictionaryTerms: const [
      UserTerm(
        inputCode: 'luobo',
        text: '萝卜词核',
        reading: 'luo bo ci he',
        weight: 0.92,
        source: 'manual',
        lastUsed: '2026-07-04 10:42',
      ),
      UserTerm(
        inputCode: 'tongbu',
        text: '同步预检',
        reading: 'tong bu yu jian',
        weight: 0.76,
        source: 'selection',
        lastUsed: '2026-07-03 18:12',
      ),
      UserTerm(
        inputCode: 'bianjie',
        text: '边界清晰',
        reading: 'bian jie qing xi',
        weight: 0.71,
        source: 'import',
        lastUsed: '2026-07-02 21:03',
      ),
    ],
    deletedTerms: const [
      DeletedTerm(
        inputCode: 'demo',
        text: '演示词',
        reading: 'yan shi ci',
        deletedAt: '2026-07-03',
      ),
    ],
    learningSummary: const LearningSummary(
      userTerms: 3,
      deletedTerms: 1,
      selectionEvents: 128,
      suppressedTerms: 2,
      lastUpdated: '2026-07-04 10:42',
    ),
    explanations: const [
      RankerExplanation(
        inputCode: 'luobo',
        candidate: '萝卜词核',
        score: 0.92,
        signals: ['manual_user_term', 'frequency_boost', 'recent_selection'],
      ),
      RankerExplanation(
        inputCode: 'tongbu',
        candidate: '同步预检',
        score: 0.76,
        signals: ['selection_history', 'context_summary'],
      ),
    ],
    sync: SyncPreflightSummary(
      state: SyncUiState.backendUnavailable,
      serverEndpoint: 'https://sync.example.invalid',
      reason: 'android-keystore-v1 与 apple-keychain-v1 均未解除生产签名门禁',
      syncableObjects: 3,
      localOnlyEvents: 128,
      lastUpload: '未启用',
      lastDownload: '未启用',
      categories: const [
        SyncCategorySummary(name: 'dictionary.user_terms', count: 1),
        SyncCategorySummary(name: 'dictionary.deleted_terms', count: 1),
        SyncCategorySummary(name: 'ranker.weights', count: 1),
      ],
      device: const DeviceSecuritySummary(
        deviceId: 'device-demo-01',
        backendId: 'android-keystore-v1',
        capabilityStatus: 'unsupported_signature_algorithm',
        productionGate: 'blocked',
      ),
    ),
    settings: const ManagerSettings(
      privacyMode: false,
      diagnosticsExport: false,
      syncConfigured: true,
    ),
  );
}
