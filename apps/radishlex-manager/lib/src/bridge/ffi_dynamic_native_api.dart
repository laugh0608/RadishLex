part of 'ffi_dynamic_native_binding.dart';

const _statusOk = 0;

final class _RadishLexNativeApi {
  _RadishLexNativeApi(ffi.DynamicLibrary library)
    : userdbTermsNew = library
          .lookupFunction<
            ffi.Pointer<_RadishLexUserTermList> Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            ffi.Pointer<_RadishLexUserTermList> Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_terms_new'),
      userdbTermsCount = library
          .lookupFunction<
            ffi.Size Function(ffi.Pointer<_RadishLexUserTermList>),
            int Function(ffi.Pointer<_RadishLexUserTermList>)
          >('radishlex_userdb_terms_count'),
      userdbTermsGet = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<_RadishLexUserTermList>,
              ffi.Size,
              ffi.Pointer<_RadishLexUserTermView>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<_RadishLexUserTermList>,
              int,
              ffi.Pointer<_RadishLexUserTermView>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_terms_get'),
      userdbTermsFree = library
          .lookupFunction<
            ffi.Void Function(ffi.Pointer<_RadishLexUserTermList>),
            void Function(ffi.Pointer<_RadishLexUserTermList>)
          >('radishlex_userdb_terms_free'),
      userdbDeleteTerm = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_delete_term'),
      userdbDictionaryInspect = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexDictionaryInspectSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexDictionaryInspectSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_dictionary_inspect'),
      userdbDictionaryImport = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Uint8,
              ffi.Pointer<_RadishLexDictionaryImportSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              int,
              ffi.Pointer<_RadishLexDictionaryImportSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_dictionary_import'),
      userdbDictionaryExport = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexDictionaryExportSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexDictionaryExportSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_dictionary_export'),
      userdbLearningStatus = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexLearningStatusSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexLearningStatusSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_learning_status'),
      userdbSyncPreflight = library
          .lookupFunction<
            ffi.Int32 Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexSyncPreflightSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            ),
            int Function(
              ffi.Pointer<ffi.Char>,
              ffi.Pointer<_RadishLexSyncPreflightSummary>,
              ffi.Pointer<ffi.Pointer<_RadishLexError>>,
            )
          >('radishlex_userdb_sync_preflight'),
      errorCode = library
          .lookupFunction<
            ffi.Int32 Function(ffi.Pointer<_RadishLexError>),
            int Function(ffi.Pointer<_RadishLexError>)
          >('radishlex_error_code'),
      errorMessage = library
          .lookupFunction<
            ffi.Pointer<ffi.Char> Function(ffi.Pointer<_RadishLexError>),
            ffi.Pointer<ffi.Char> Function(ffi.Pointer<_RadishLexError>)
          >('radishlex_error_message'),
      errorFree = library
          .lookupFunction<
            ffi.Void Function(ffi.Pointer<_RadishLexError>),
            void Function(ffi.Pointer<_RadishLexError>)
          >('radishlex_error_free');

  final ffi.Pointer<_RadishLexUserTermList> Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  userdbTermsNew;
  final int Function(ffi.Pointer<_RadishLexUserTermList>) userdbTermsCount;
  final int Function(
    ffi.Pointer<_RadishLexUserTermList>,
    int,
    ffi.Pointer<_RadishLexUserTermView>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  userdbTermsGet;
  final void Function(ffi.Pointer<_RadishLexUserTermList>) userdbTermsFree;
  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  userdbDeleteTerm;
  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<_RadishLexDictionaryInspectSummary>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  userdbDictionaryInspect;
  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    int,
    ffi.Pointer<_RadishLexDictionaryImportSummary>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  userdbDictionaryImport;
  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<_RadishLexDictionaryExportSummary>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  userdbDictionaryExport;
  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<_RadishLexLearningStatusSummary>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  userdbLearningStatus;
  final int Function(
    ffi.Pointer<ffi.Char>,
    ffi.Pointer<_RadishLexSyncPreflightSummary>,
    ffi.Pointer<ffi.Pointer<_RadishLexError>>,
  )
  userdbSyncPreflight;
  final int Function(ffi.Pointer<_RadishLexError>) errorCode;
  final ffi.Pointer<ffi.Char> Function(ffi.Pointer<_RadishLexError>)
  errorMessage;
  final void Function(ffi.Pointer<_RadishLexError>) errorFree;
}

ffi.Pointer<T> _callPointer<T extends ffi.NativeType>(
  _RadishLexNativeApi api,
  ffi.Pointer<T> Function(ffi.Pointer<ffi.Pointer<_RadishLexError>>) call,
) {
  final errorOut = calloc<ffi.Pointer<_RadishLexError>>();
  try {
    final pointer = call(errorOut);
    if (pointer == ffi.nullptr) {
      _throwFfiError(api, errorOut.value);
    }
    _freeUnexpectedSuccessError(api, errorOut.value);
    return pointer;
  } finally {
    calloc.free(errorOut);
  }
}

void _callStatus(
  _RadishLexNativeApi api,
  int Function(ffi.Pointer<ffi.Pointer<_RadishLexError>>) call,
) {
  final errorOut = calloc<ffi.Pointer<_RadishLexError>>();
  try {
    final status = call(errorOut);
    if (status != _statusOk) {
      _throwFfiError(api, errorOut.value, fallbackStatus: status);
    }
    _freeUnexpectedSuccessError(api, errorOut.value);
  } finally {
    calloc.free(errorOut);
  }
}

void _throwFfiError(
  _RadishLexNativeApi api,
  ffi.Pointer<_RadishLexError> error, {
  int? fallbackStatus,
}) {
  if (error == ffi.nullptr) {
    final status = fallbackStatus ?? 255;
    throw FfiManagerBridgeException(
      statusCode: status,
      code: _statusCodeLabel(status),
      message: 'RadishLex FFI call failed without an error object',
    );
  }

  final status = api.errorCode(error);
  final messagePointer = api.errorMessage(error);
  final message = messagePointer == ffi.nullptr
      ? 'RadishLex FFI call failed without an error message'
      : messagePointer.cast<Utf8>().toDartString();
  api.errorFree(error);
  throw FfiManagerBridgeException(
    statusCode: status,
    code: _statusCodeLabel(status),
    message: message,
  );
}

void _freeUnexpectedSuccessError(
  _RadishLexNativeApi api,
  ffi.Pointer<_RadishLexError> error,
) {
  if (error != ffi.nullptr) {
    api.errorFree(error);
  }
}

T _withNativeString<T>(String value, T Function(ffi.Pointer<ffi.Char>) run) {
  final pointer = value.toNativeUtf8(allocator: calloc);
  try {
    return run(pointer.cast<ffi.Char>());
  } finally {
    calloc.free(pointer);
  }
}

T _withOptionalNativeString<T>(
  String? value,
  T Function(ffi.Pointer<ffi.Char>) run,
) {
  if (value == null) {
    return run(ffi.nullptr.cast<ffi.Char>());
  }
  return _withNativeString(value, run);
}

String _readStringView(_RadishLexStringView view) {
  if (view.len == 0) {
    return '';
  }
  if (view.data == ffi.nullptr) {
    throw const FfiManagerBridgeException(
      statusCode: 2,
      code: 'invalid_state',
      message: 'RadishLex FFI returned a non-empty null string view',
    );
  }
  return view.data.cast<Utf8>().toDartString(length: view.len);
}

String? _readOptionalStringView(_RadishLexStringView view, int present) {
  return present == 0 ? null : _readStringView(view);
}

String _statusCodeLabel(int status) {
  switch (status) {
    case 0:
      return 'ok';
    case 1:
      return 'invalid_argument';
    case 2:
      return 'invalid_state';
    case 3:
      return 'engine_error';
    case 4:
      return 'userdb_error';
    case 5:
      return 'ranker_error';
    case 6:
      return 'sync_error';
    default:
      return 'internal_error';
  }
}
