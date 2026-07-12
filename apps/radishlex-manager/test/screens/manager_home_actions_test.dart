import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:radishlex_manager/src/bridge/ffi_manager_sync_readiness_mapper.dart';
import 'package:radishlex_manager/src/bridge/manager_bridge.dart';
import 'package:radishlex_manager/src/data/manager_fixture.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';
import 'package:radishlex_manager/src/screens/manager/manager_home_actions.dart';

import '../fixtures/sync_readiness_bridge_fixtures.dart';

void main() {
  group('dictionary action result messages', () {
    test('summarize dry-run import results with combined skipped count', () {
      const result = DictionaryImportResult(
        filePath: '/tmp/radishlex-import.tsv',
        sourceName: 'manager-import',
        totalRecords: 10,
        importedTerms: 7,
        insertedTerms: 3,
        updatedTerms: 4,
        skippedDeletedTerms: 1,
        skippedDuplicateTerms: 2,
        dryRun: true,
      );

      expect(
        dictionaryImportResultMessage(result),
        '导入检查完成：7 / 10 条，新增 3，更新 4，跳过 3',
      );
    });

    test('summarize committed import results', () {
      const result = DictionaryImportResult(
        filePath: '/tmp/radishlex-import.tsv',
        sourceName: 'ops-import',
        totalRecords: 5,
        importedTerms: 5,
        insertedTerms: 4,
        updatedTerms: 1,
        skippedDeletedTerms: 0,
        skippedDuplicateTerms: 0,
        dryRun: false,
      );

      expect(
        dictionaryImportResultMessage(result),
        '导入完成：5 / 5 条，新增 4，更新 1，跳过 0',
      );
    });

    test('summarize export results without exposing file paths', () {
      const result = DictionaryExportResult(
        filePath: '/tmp/private/radishlex-export.json',
        exportedTerms: 3,
        format: 'dictionary.user_terms.v1',
        syncClass: 'P2 encrypted sync',
      );

      expect(
        dictionaryExportResultMessage(result),
        '导出完成：3 条，dictionary.user_terms.v1 / P2 encrypted sync',
      );
    });
  });

  group('manager bridge failure messages', () {
    test('prefer dictionary import category during inspect failures', () {
      final message = managerBridgeFailureMessage(
        const _TestBridgeFailure(code: 'invalid_argument'),
        ManagerBridgeOperation.inspectDictionaryImport,
      );

      expect(message, '检查导入词库失败：词库导入或本地 userdb 错误（invalid_argument）');
    });

    test('surface native library category before operation category', () {
      final message = managerBridgeFailureMessage(
        const _TestBridgeFailure(code: 'ffi_library_load_failed'),
        ManagerBridgeOperation.previewDiagnostics,
      );

      expect(message, '预览诊断摘要失败：native library 不可用（ffi_library_load_failed）');
    });

    test('keep settings draft failures in settings category', () {
      final message = managerBridgeFailureMessage(
        const _TestBridgeFailure(code: 'invalid_argument'),
        ManagerBridgeOperation.saveSettingsDraft,
      );

      expect(message, '保存设置草案失败：设置草案错误（invalid_argument）');
    });

    test('classify non-bridge exceptions as manager bridge errors', () {
      final message = managerBridgeFailureMessage(
        StateError('details must stay out of the UI'),
        ManagerBridgeOperation.exportDiagnostics,
      );

      expect(message, '导出诊断摘要失败：管理端 bridge 错误（manager_bridge_error）');
    });
  });

  group('manager action helpers', () {
    test('preserve imported readiness after settings draft save', () {
      final importedReadiness = managerSyncReadinessBridgeSnapshotFromJson(
        partiallyBlockedSyncReadinessBridgeJson(),
      );
      final bridgeSnapshot = _readySyncSnapshot();

      final saved = managerSnapshotAfterSettingsDraftSave(
        bridgeSnapshot: bridgeSnapshot,
        currentReadiness: importedReadiness,
      );
      final gate = managerSyncGateAuditForDraft(
        draft: saved.settings.draft,
        device: saved.sync.device,
        readinessBridgeSnapshot: saved.sync.readinessBridgeSnapshot,
      ).entryGate;

      expect(saved.sync.readinessBridgeSnapshot.source, 'fixture_readiness');
      expect(gate.readinessBridgeSource, 'fixture_readiness');
      expect(gate.readinessBlockedFlowSummary, 'recovery_setup, device_join');
      expect(gate.entryBlocker, 'recovery_record_missing');
      expect(gate.userSyncEnabled, isFalse);
    });

    test('keep default readiness on bridge snapshot after settings save', () {
      final bridgeSnapshot = _readySyncSnapshot();

      final saved = managerSnapshotAfterSettingsDraftSave(
        bridgeSnapshot: bridgeSnapshot,
        currentReadiness: managerDefaultSyncReadinessBridgeSnapshot,
      );

      expect(identical(saved, bridgeSnapshot), isTrue);
      expect(
        saved.sync.readinessBridgeSnapshot.source,
        managerSyncReadinessBridgeSourceDefault,
      );
    });

    test(
      'export diagnostics from current snapshot only for imported readiness',
      () {
        final importedReadiness = managerSyncReadinessBridgeSnapshotFromJson(
          partiallyBlockedSyncReadinessBridgeJson(),
        );
        final importedSnapshot = managerSnapshotWithSyncReadiness(
          _readySyncSnapshot(),
          importedReadiness,
        );
        final defaultSnapshot = _readySyncSnapshot();

        expect(
          managerShouldExportDiagnosticsFromCurrentSnapshot(importedSnapshot),
          isTrue,
        );
        expect(
          managerShouldExportDiagnosticsFromCurrentSnapshot(defaultSnapshot),
          isFalse,
        );

        final exportFile = File(
          '${Directory.systemTemp.path}/radishlex-manager-actions-${DateTime.now().microsecondsSinceEpoch}.txt',
        );
        addTearDown(() {
          if (exportFile.existsSync()) {
            exportFile.deleteSync();
          }
        });

        final result = exportManagerDiagnosticsFromCurrentSnapshot(
          filePath: exportFile.path,
          snapshot: importedSnapshot,
        );
        final text = exportFile.readAsStringSync();

        expect(result.lineCount, greaterThan(0));
        expect(
          text,
          contains('sync.readiness_bridge_source: fixture_readiness'),
        );
        expect(
          text,
          contains('sync.readiness_blocked_flows: recovery_setup, device_join'),
        );
        expect(text, isNot(contains('secret-token')));
        expect(text, isNot(contains('RADISHLEX-RECOVERY-CODE-SECRET')));
        expect(text, isNot(contains('payload_bytes=abcdef')));
      },
    );
  });
}

class _TestBridgeFailure implements ManagerBridgeFailure {
  const _TestBridgeFailure({required this.code});

  @override
  int get statusCode => 4;

  @override
  final String code;

  @override
  String get message => 'private detail omitted';
}

ManagerSnapshot _readySyncSnapshot() {
  const readyDevice = DeviceSecuritySummary(
    deviceId: 'device-ready-01',
    backendId: 'test-production-ready',
    capabilityStatus: 'ready_for_test',
    productionGate: 'ready',
  );
  const draft = ManagerSettingsDraft(
    serverEndpoint: 'https://sync.example.invalid',
    retainSyncConfig: true,
    privacyMode: false,
    diagnosticsExport: false,
    deploymentEvidenceRecorded: true,
    accessTokenConfigured: true,
    deploymentEvidenceSource: managerDeploymentEvidenceExternalTls,
  );
  final fixture = createManagerFixture();
  final state = deriveManagerSyncUiState(draft: draft, device: readyDevice);

  return fixture.copyWith(
    sync: fixture.sync.copyWith(
      state: state,
      device: readyDevice,
      serverEndpoint: managerSyncEndpointLabel(draft),
      reason: managerSyncGateReason(
        state: state,
        draft: draft,
        device: readyDevice,
      ),
      readinessBridgeSnapshot: managerDefaultSyncReadinessBridgeSnapshot,
    ),
    settings: fixture.settings.copyWith(draft: draft, syncConfigured: true),
  );
}
