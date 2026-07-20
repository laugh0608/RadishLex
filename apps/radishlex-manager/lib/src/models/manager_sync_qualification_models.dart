import 'dart:typed_data';

class ManagerSyncQualificationRequest {
  ManagerSyncQualificationRequest({
    required this.endpoint,
    required Uint8List accessToken,
    required Uint8List? localCaDer,
    required this.timeoutMs,
  }) : accessToken = Uint8List.fromList(accessToken),
       localCaDer = localCaDer == null ? null : Uint8List.fromList(localCaDer);

  final String endpoint;
  final Uint8List accessToken;
  final Uint8List? localCaDer;
  final int timeoutMs;

  void clearTransientInputs() {
    accessToken.fillRange(0, accessToken.length, 0);
    final certificate = localCaDer;
    certificate?.fillRange(0, certificate.length, 0);
  }
}

abstract interface class ManagerSyncQualificationRun {
  ManagerSyncQualificationSnapshot poll();

  bool cancel();

  void dispose();
}

enum ManagerSyncQualificationState {
  created('created'),
  running('running'),
  cancelling('cancelling'),
  completed('completed'),
  failed('failed'),
  cancelled('cancelled');

  const ManagerSyncQualificationState(this.code);

  final String code;

  bool get isTerminal => switch (this) {
    completed || failed || cancelled => true,
    _ => false,
  };
}

enum ManagerSyncQualificationPhase {
  created('created', '准备启动'),
  validateRequest('validate_request', '校验一次性参数'),
  prepareWorkspace('prepare_workspace', '准备隔离临时工作区'),
  createDomain('create_domain', '创建合成同步域'),
  authorizeSecondDevice('authorize_second_device', '授权第二台合成设备'),
  clientAFirstSync('client_a_first_sync', '客户端 A 首轮同步'),
  prepareConflict('prepare_conflict', '构造版本冲突'),
  clientBMerge('client_b_merge', '客户端 B 合并上传'),
  clientAConflictRecovery('client_a_conflict_recovery', '客户端 A 冲突恢复'),
  verifyConvergence('verify_convergence', '验证双端收敛'),
  cleanup('cleanup', '清理临时资源'),
  complete('complete', '资格测试完成');

  const ManagerSyncQualificationPhase(this.code, this.label);

  final String code;
  final String label;
}

enum ManagerSyncQualificationErrorCode {
  none('none'),
  invalidRequest('invalid_request'),
  alreadyRunning('already_running'),
  unauthenticated('unauthenticated'),
  tlsRejected('tls_rejected'),
  transportTimeout('transport_timeout'),
  serverUnavailable('server_unavailable'),
  protocolRejected('protocol_rejected'),
  cryptoRejected('crypto_rejected'),
  localStorageFailed('local_storage_failed'),
  conflictNotObserved('conflict_not_observed'),
  convergenceFailed('convergence_failed'),
  cancelled('cancelled'),
  internal('internal');

  const ManagerSyncQualificationErrorCode(this.code);

  final String code;
}

class ManagerSyncQualificationSnapshot {
  const ManagerSyncQualificationSnapshot({
    required this.state,
    required this.phase,
    required this.discovered,
    required this.downloaded,
    required this.applied,
    required this.uploaded,
    required this.conflicts,
    required this.retries,
    required this.convergenceRounds,
    required this.temporaryFilesCleaned,
    required this.workerStopped,
    required this.transientInputsCleared,
    required this.errorCode,
    required this.errorPhase,
    required this.errorRetryable,
  });

  final ManagerSyncQualificationState state;
  final ManagerSyncQualificationPhase phase;
  final int discovered;
  final int downloaded;
  final int applied;
  final int uploaded;
  final int conflicts;
  final int retries;
  final int convergenceRounds;
  final bool temporaryFilesCleaned;
  final bool workerStopped;
  final bool transientInputsCleared;
  final ManagerSyncQualificationErrorCode errorCode;
  final ManagerSyncQualificationPhase? errorPhase;
  final bool errorRetryable;
}
