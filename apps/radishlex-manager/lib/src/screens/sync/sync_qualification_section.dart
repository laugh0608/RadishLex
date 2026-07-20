import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../../bridge/manager_bridge.dart';
import '../../models/manager_models.dart';
import '../manager_widgets.dart';

class SyncQualificationSection extends StatefulWidget {
  const SyncQualificationSection({
    super.key,
    required this.endpoint,
    required this.onStart,
  });

  final String endpoint;
  final ManagerSyncQualificationRun Function(
    ManagerSyncQualificationRequest request,
  )
  onStart;

  @override
  State<SyncQualificationSection> createState() =>
      _SyncQualificationSectionState();
}

class _SyncQualificationSectionState extends State<SyncQualificationSection> {
  ManagerSyncQualificationRun? _run;
  ManagerSyncQualificationSnapshot? _snapshot;
  Timer? _pollTimer;
  String? _failure;

  bool get _running => _run != null;

  @override
  void dispose() {
    _pollTimer?.cancel();
    final run = _run;
    if (run != null) {
      try {
        run.cancel();
      } on Object {
        // dispose remains the ownership stop line even if cancellation failed.
      } finally {
        run.dispose();
      }
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final snapshot = _snapshot;
    final endpointAllowed = managerSyncQualificationEndpointAllowed(
      widget.endpoint,
    );
    return ManagerSection(
      title: '本地合成同步资格测试',
      trailing: ManagerStatusBadge(
        icon: snapshot?.state == ManagerSyncQualificationState.completed
            ? Icons.verified_outlined
            : Icons.science_outlined,
        label: snapshot?.state.code ?? (_running ? 'running' : 'not_run'),
        tone: snapshot?.state == ManagerSyncQualificationState.completed
            ? ManagerBadgeTone.success
            : ManagerBadgeTone.warning,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            '只连接 loopback HTTPS，使用隔离合成 P2 数据；结果不成为 readiness 或部署证据，也不会开启用户同步。',
          ),
          const SizedBox(height: 12),
          ManagerKeyValueRow(
            label: 'endpoint gate',
            value: endpointAllowed ? 'loopback_https_allowed' : 'blocked',
          ),
          if (snapshot != null) ...[
            ManagerKeyValueRow(
              label: 'phase',
              value: '${snapshot.phase.code} / ${snapshot.phase.label}',
            ),
            ManagerKeyValueRow(
              label: 'objects',
              value:
                  'discover ${snapshot.discovered}, download ${snapshot.downloaded}, '
                  'apply ${snapshot.applied}, upload ${snapshot.uploaded}',
            ),
            ManagerKeyValueRow(
              label: 'conflict / retry / convergence',
              value:
                  '${snapshot.conflicts} / ${snapshot.retries} / ${snapshot.convergenceRounds}',
            ),
            ManagerKeyValueRow(
              label: 'cleanup',
              value:
                  'files=${snapshot.temporaryFilesCleaned}, '
                  'worker=${snapshot.workerStopped}, '
                  'transient=${snapshot.transientInputsCleared}',
            ),
            ManagerKeyValueRow(label: 'error', value: snapshot.errorCode.code),
          ],
          if (_failure != null) ...[
            const SizedBox(height: 8),
            Text(
              _failure!,
              key: const Key('sync-qualification-failure'),
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          ],
          const SizedBox(height: 14),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              FilledButton.icon(
                key: const Key('sync-qualification-start-button'),
                onPressed: !_running && endpointAllowed ? _start : null,
                icon: const Icon(Icons.play_arrow_outlined),
                label: const Text('运行本地资格测试'),
              ),
              OutlinedButton.icon(
                key: const Key('sync-qualification-cancel-button'),
                onPressed: _running ? _cancel : null,
                icon: const Icon(Icons.stop_circle_outlined),
                label: const Text('取消运行'),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _start() async {
    final input = await showDialog<_SyncQualificationInput>(
      context: context,
      builder: (context) =>
          _SyncQualificationDialog(endpoint: widget.endpoint.trim()),
    );
    if (!mounted || input == null) {
      return;
    }

    ManagerSyncQualificationRequest? request;
    Uint8List? ca;
    try {
      ca = await _readOptionalCa(input.localCaPath);
      request = ManagerSyncQualificationRequest(
        endpoint: widget.endpoint.trim(),
        accessToken: input.accessToken,
        localCaDer: ca,
        timeoutMs: input.timeoutMs,
      );
      final run = widget.onStart(request);
      if (!mounted) {
        run.cancel();
        run.dispose();
        return;
      }
      setState(() {
        _run = run;
        _snapshot = null;
        _failure = null;
      });
      _poll();
      _pollTimer = Timer.periodic(
        const Duration(milliseconds: 250),
        (_) => _poll(),
      );
    } on Object catch (error) {
      if (mounted) {
        setState(() {
          _failure = describeManagerBridgeFailure(
            error,
            ManagerBridgeOperation.startSyncQualification,
          ).userMessage;
        });
      }
    } finally {
      request?.clearTransientInputs();
      ca?.fillRange(0, ca.length, 0);
      input.clear();
    }
  }

  void _poll() {
    final run = _run;
    if (run == null) {
      return;
    }
    try {
      final snapshot = run.poll();
      if (!mounted) {
        return;
      }
      setState(() {
        _snapshot = snapshot;
      });
      if (snapshot.state.isTerminal) {
        _closeRun(run);
      }
    } on Object catch (error) {
      if (mounted) {
        setState(() {
          _failure = describeManagerBridgeFailure(
            error,
            ManagerBridgeOperation.startSyncQualification,
          ).userMessage;
        });
      }
      _closeRun(run);
    }
  }

  void _cancel() {
    final run = _run;
    if (run == null) {
      return;
    }
    try {
      run.cancel();
      _poll();
    } on Object catch (error) {
      setState(() {
        _failure = describeManagerBridgeFailure(
          error,
          ManagerBridgeOperation.startSyncQualification,
        ).userMessage;
      });
      _closeRun(run);
    }
  }

  void _closeRun(ManagerSyncQualificationRun run) {
    _pollTimer?.cancel();
    _pollTimer = null;
    run.dispose();
    if (mounted) {
      setState(() {
        if (identical(_run, run)) {
          _run = null;
        }
      });
    }
  }
}

bool managerSyncQualificationEndpointAllowed(String endpoint) {
  final uri = Uri.tryParse(endpoint.trim());
  if (uri == null ||
      uri.scheme != 'https' ||
      uri.userInfo.isNotEmpty ||
      uri.hasQuery ||
      uri.hasFragment ||
      (uri.path.isNotEmpty && uri.path != '/')) {
    return false;
  }
  return uri.host == 'localhost' ||
      uri.host == '127.0.0.1' ||
      uri.host == '::1';
}

Future<Uint8List?> _readOptionalCa(String path) async {
  if (path.isEmpty) {
    return null;
  }
  final bytes = await File(path).readAsBytes();
  if (bytes.isEmpty || bytes.length > 64 * 1024) {
    throw ArgumentError('local CA DER must contain 1..65536 bytes');
  }
  return bytes;
}

final class _SyncQualificationInput {
  _SyncQualificationInput({
    required this.accessToken,
    required this.localCaPath,
    required this.timeoutMs,
  });

  final Uint8List accessToken;
  final String localCaPath;
  final int timeoutMs;

  void clear() {
    accessToken.fillRange(0, accessToken.length, 0);
  }
}

class _SyncQualificationDialog extends StatefulWidget {
  const _SyncQualificationDialog({required this.endpoint});

  final String endpoint;

  @override
  State<_SyncQualificationDialog> createState() =>
      _SyncQualificationDialogState();
}

class _SyncQualificationDialogState extends State<_SyncQualificationDialog> {
  final _tokenController = TextEditingController();
  final _caPathController = TextEditingController();
  int _timeoutMs = 30_000;
  String? _validationError;

  @override
  void dispose() {
    _tokenController.clear();
    _caPathController.clear();
    _tokenController.dispose();
    _caPathController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('本地合成同步资格测试'),
      content: SizedBox(
        width: 520,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('目标：${widget.endpoint}'),
            const SizedBox(height: 12),
            TextField(
              key: const Key('sync-qualification-token-field'),
              controller: _tokenController,
              obscureText: true,
              enableSuggestions: false,
              autocorrect: false,
              decoration: const InputDecoration(
                labelText: '一次性 bearer token',
                helperText: '仅用于本次调用，不写入设置或诊断',
              ),
            ),
            const SizedBox(height: 12),
            TextField(
              key: const Key('sync-qualification-ca-path-field'),
              controller: _caPathController,
              decoration: const InputDecoration(
                labelText: '本地 CA DER 路径（可选）',
                helperText: '读取后只传递证书 bytes，不传递路径',
              ),
            ),
            const SizedBox(height: 12),
            DropdownButtonFormField<int>(
              key: const Key('sync-qualification-timeout-field'),
              initialValue: _timeoutMs,
              decoration: const InputDecoration(labelText: '总超时'),
              items: const [
                DropdownMenuItem(value: 15_000, child: Text('15 秒')),
                DropdownMenuItem(value: 30_000, child: Text('30 秒')),
                DropdownMenuItem(value: 60_000, child: Text('60 秒')),
              ],
              onChanged: (value) {
                if (value != null) {
                  _timeoutMs = value;
                }
              },
            ),
            if (_validationError != null) ...[
              const SizedBox(height: 8),
              Text(
                _validationError!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          key: const Key('sync-qualification-submit-button'),
          onPressed: _submit,
          child: const Text('开始'),
        ),
      ],
    );
  }

  void _submit() {
    final token = _tokenController.text;
    final validToken =
        token.length >= 32 &&
        token.length <= 4096 &&
        token.codeUnits.every((byte) => byte >= 0x21 && byte <= 0x7e);
    if (!validToken) {
      setState(() {
        _validationError = 'token 必须为 32..4096 字节的可见 ASCII';
      });
      return;
    }
    final input = _SyncQualificationInput(
      accessToken: Uint8List.fromList(ascii.encode(token)),
      localCaPath: _caPathController.text.trim(),
      timeoutMs: _timeoutMs,
    );
    _tokenController.clear();
    _caPathController.clear();
    Navigator.of(context).pop(input);
  }
}
