import 'dart:convert';
import 'dart:io' show File;

import '../models/manager_models.dart';
import 'manager_bridge.dart';

class ManagerSettingsStore {
  ManagerSettingsStore({
    String? filePath,
    ManagerSettingsDraft fallbackDraft = const ManagerSettingsDraft.empty(),
  }) : filePath = filePath?.trim() ?? '',
       _memoryDraft = fallbackDraft.normalized();

  final String filePath;
  ManagerSettingsDraft _memoryDraft;

  bool get isPersistent => filePath.isNotEmpty;

  String get sourceLabel =>
      isPersistent ? 'RADISHLEX_MANAGER_SETTINGS_FILE configured' : 'in_memory';

  ManagerSettingsDraft load() {
    if (!isPersistent) {
      return _memoryDraft;
    }

    final file = File(filePath);
    if (!file.existsSync()) {
      return _memoryDraft;
    }

    try {
      final decoded = jsonDecode(file.readAsStringSync());
      if (decoded is! Map<String, Object?>) {
        throw const FormatException('settings root must be an object');
      }
      return _validateDraft(_draftFromJson(decoded).normalized());
    } on ManagerSettingsStoreException {
      rethrow;
    } on Object catch (error) {
      throw ManagerSettingsStoreException(
        code: 'settings_store_error',
        message: 'failed to read manager settings draft: $error',
      );
    }
  }

  ManagerSettingsDraft save(ManagerSettingsDraft draft) {
    final normalized = _validateDraft(draft.normalized());
    _memoryDraft = normalized;

    if (!isPersistent) {
      return normalized;
    }

    try {
      final file = File(filePath);
      file.parent.createSync(recursive: true);
      final temp = File('$filePath.tmp');
      temp.writeAsStringSync(
        const JsonEncoder.withIndent('  ').convert(_draftToJson(normalized)),
      );
      temp.renameSync(filePath);
      return normalized;
    } on Object catch (error) {
      throw ManagerSettingsStoreException(
        code: 'settings_store_error',
        message: 'failed to write manager settings draft: $error',
      );
    }
  }
}

class ManagerSettingsStoreException implements ManagerBridgeFailure {
  const ManagerSettingsStoreException({
    required this.code,
    required this.message,
  });

  @override
  int get statusCode => 2;

  @override
  final String code;

  @override
  final String message;
}

ManagerSettingsDraft _draftFromJson(Map<String, Object?> json) {
  final formatVersion = json['format_version'];
  if (formatVersion != 1) {
    throw const ManagerSettingsStoreException(
      code: 'settings_store_error',
      message: 'unsupported manager settings draft format',
    );
  }
  final deploymentEvidenceSource = _stringValue(
    json,
    'deployment_evidence_source',
  );

  return ManagerSettingsDraft(
    serverEndpoint: _stringValue(json, 'server_endpoint'),
    retainSyncConfig: _boolValue(json, 'retain_sync_config'),
    privacyMode: _boolValue(json, 'privacy_mode'),
    diagnosticsExport: _boolValue(json, 'diagnostics_export'),
    deploymentEvidenceRecorded:
        _boolValue(json, 'deployment_evidence_recorded') &&
        deploymentEvidenceSource.trim().isNotEmpty,
    accessTokenConfigured: _boolValue(json, 'access_token_configured'),
    deploymentEvidenceSource: deploymentEvidenceSource,
  );
}

Map<String, Object?> _draftToJson(ManagerSettingsDraft draft) {
  return {
    'format_version': 1,
    'server_endpoint': draft.serverEndpoint,
    'retain_sync_config': draft.retainSyncConfig,
    'privacy_mode': draft.privacyMode,
    'diagnostics_export': draft.diagnosticsExport,
    'deployment_evidence_recorded': draft.deploymentEvidenceRecorded,
    'access_token_configured': draft.accessTokenConfigured,
    'deployment_evidence_source': draft.deploymentEvidenceSource,
  };
}

ManagerSettingsDraft _validateDraft(ManagerSettingsDraft draft) {
  if (draft.deploymentEvidenceRecorded &&
      !isValidManagerDeploymentEvidenceSource(draft.deploymentEvidenceSource)) {
    throw const ManagerSettingsStoreException(
      code: 'invalid_argument',
      message: 'deployment evidence source must be a known non-secret label',
    );
  }

  final endpoint = draft.serverEndpoint.trim();
  if (endpoint.isEmpty) {
    return draft.copyWith(serverEndpoint: '');
  }

  final uri = Uri.tryParse(endpoint);
  if (uri == null ||
      !uri.hasScheme ||
      (uri.scheme != 'https' && uri.scheme != 'http') ||
      uri.host.isEmpty ||
      uri.userInfo.isNotEmpty ||
      uri.query.isNotEmpty ||
      uri.fragment.isNotEmpty) {
    throw const ManagerSettingsStoreException(
      code: 'invalid_argument',
      message:
          'server endpoint must be http(s) without user info, query, or fragment',
    );
  }

  return draft.copyWith(serverEndpoint: endpoint);
}

String _stringValue(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value == null) {
    return '';
  }
  if (value is String) {
    return value;
  }
  throw ManagerSettingsStoreException(
    code: 'settings_store_error',
    message: '$key must be a string',
  );
}

bool _boolValue(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value == null) {
    return false;
  }
  if (value is bool) {
    return value;
  }
  throw ManagerSettingsStoreException(
    code: 'settings_store_error',
    message: '$key must be a bool',
  );
}
