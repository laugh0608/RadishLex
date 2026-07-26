import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:radishlex_manager/src/models/manager_models.dart';
import 'package:radishlex_manager/src/screens/sync/sync_qualification_section.dart';

void main() {
  testWidgets('qualification modal clears transient request after start', (
    tester,
  ) async {
    ManagerSyncQualificationRequest? captured;
    final run = _CompletedRun();
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(
            child: SyncQualificationSection(
              endpoint: 'https://localhost:9443',
              onStart: (request) {
                captured = request;
                return run;
              },
            ),
          ),
        ),
      ),
    );

    await tester.tap(find.byKey(const Key('sync-qualification-start-button')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('sync-qualification-token-field')),
      'qualification-token-visible-ascii-1234567890',
    );
    await tester.tap(find.byKey(const Key('sync-qualification-submit-button')));
    await tester.pumpAndSettle();

    expect(captured, isNotNull);
    expect(captured!.accessToken.every((byte) => byte == 0), isTrue);
    expect(run.disposed, isTrue);
    expect(find.text('completed'), findsOneWidget);
    expect(
      find.text('files=true, worker=true, transient=true'),
      findsOneWidget,
    );
    expect(find.byKey(const Key('sync-enable-button')), findsNothing);
  });

  testWidgets('non-loopback endpoint keeps qualification disabled', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SyncQualificationSection(
            endpoint: 'https://sync.example.invalid',
            onStart: (_) => _CompletedRun(),
          ),
        ),
      ),
    );

    final button = tester.widget<FilledButton>(
      find.byKey(const Key('sync-qualification-start-button')),
    );
    expect(button.onPressed, isNull);
    expect(find.text('blocked'), findsOneWidget);
  });

  testWidgets('running qualification can be cancelled', (tester) async {
    final run = _CancellableRun();
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(
            child: SyncQualificationSection(
              endpoint: 'https://127.0.0.1:9443',
              onStart: (_) => run,
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.byKey(const Key('sync-qualification-start-button')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('sync-qualification-token-field')),
      'qualification-token-visible-ascii-1234567890',
    );
    await tester.tap(find.byKey(const Key('sync-qualification-submit-button')));
    await tester.pump(const Duration(milliseconds: 20));
    await tester.tap(find.byKey(const Key('sync-qualification-cancel-button')));
    await tester.pumpAndSettle();

    expect(run.cancelled, isTrue);
    expect(run.disposed, isTrue);
    expect(find.text('cancelled'), findsWidgets);
  });
}

class _CompletedRun implements ManagerSyncQualificationRun {
  bool disposed = false;

  @override
  bool cancel() => false;

  @override
  void dispose() {
    disposed = true;
  }

  @override
  ManagerSyncQualificationSnapshot poll() => _terminalSnapshot(
    state: ManagerSyncQualificationState.completed,
    phase: ManagerSyncQualificationPhase.complete,
    errorCode: ManagerSyncQualificationErrorCode.none,
  );
}

class _CancellableRun implements ManagerSyncQualificationRun {
  bool cancelled = false;
  bool disposed = false;

  @override
  bool cancel() {
    cancelled = true;
    return true;
  }

  @override
  void dispose() {
    disposed = true;
  }

  @override
  ManagerSyncQualificationSnapshot poll() {
    if (cancelled) {
      return _terminalSnapshot(
        state: ManagerSyncQualificationState.cancelled,
        phase: ManagerSyncQualificationPhase.clientAFirstSync,
        errorCode: ManagerSyncQualificationErrorCode.cancelled,
      );
    }
    return const ManagerSyncQualificationSnapshot(
      state: ManagerSyncQualificationState.running,
      phase: ManagerSyncQualificationPhase.clientAFirstSync,
      discovered: 1,
      downloaded: 0,
      applied: 0,
      uploaded: 0,
      conflicts: 0,
      retries: 0,
      convergenceRounds: 0,
      temporaryFilesCleaned: false,
      workerStopped: false,
      transientInputsCleared: false,
      errorCode: ManagerSyncQualificationErrorCode.none,
      errorPhase: null,
      errorRetryable: false,
    );
  }
}

ManagerSyncQualificationSnapshot _terminalSnapshot({
  required ManagerSyncQualificationState state,
  required ManagerSyncQualificationPhase phase,
  required ManagerSyncQualificationErrorCode errorCode,
}) {
  return ManagerSyncQualificationSnapshot(
    state: state,
    phase: phase,
    discovered: 3,
    downloaded: 3,
    applied: 3,
    uploaded: 3,
    conflicts: 1,
    retries: 1,
    convergenceRounds: 2,
    temporaryFilesCleaned: true,
    workerStopped: true,
    transientInputsCleared: true,
    errorCode: errorCode,
    errorPhase: errorCode == ManagerSyncQualificationErrorCode.none
        ? null
        : phase,
    errorRetryable: false,
  );
}
