part of 'ffi_dynamic_native_binding.dart';

const _statusOk = 0;

ffi.Pointer<T> _callPointer<T extends ffi.NativeType>(
  _RadishLexErrorSymbols errors,
  ffi.Pointer<T> Function(ffi.Pointer<ffi.Pointer<_RadishLexError>>) call,
) {
  final errorOut = calloc<ffi.Pointer<_RadishLexError>>();
  try {
    final pointer = call(errorOut);
    if (pointer == ffi.nullptr) {
      _throwFfiError(errors, errorOut.value);
    }
    _freeUnexpectedSuccessError(errors, errorOut.value);
    return pointer;
  } finally {
    calloc.free(errorOut);
  }
}

void _callStatus(
  _RadishLexErrorSymbols errors,
  int Function(ffi.Pointer<ffi.Pointer<_RadishLexError>>) call,
) {
  final errorOut = calloc<ffi.Pointer<_RadishLexError>>();
  try {
    final status = call(errorOut);
    if (status != _statusOk) {
      _throwFfiError(errors, errorOut.value, fallbackStatus: status);
    }
    _freeUnexpectedSuccessError(errors, errorOut.value);
  } finally {
    calloc.free(errorOut);
  }
}

void _throwFfiError(
  _RadishLexErrorSymbols errors,
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

  final status = errors.code(error);
  final messagePointer = errors.message(error);
  final message = messagePointer == ffi.nullptr
      ? 'RadishLex FFI call failed without an error message'
      : messagePointer.cast<Utf8>().toDartString();
  errors.free(error);
  throw FfiManagerBridgeException(
    statusCode: status,
    code: _statusCodeLabel(status),
    message: message,
  );
}

void _freeUnexpectedSuccessError(
  _RadishLexErrorSymbols errors,
  ffi.Pointer<_RadishLexError> error,
) {
  if (error != ffi.nullptr) {
    errors.free(error);
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

T _withNativeBytes<T>(
  Uint8List value,
  T Function(ffi.Pointer<ffi.Uint8>, int) run,
) {
  final pointer = calloc<ffi.Uint8>(value.length);
  try {
    pointer.asTypedList(value.length).setAll(0, value);
    return run(pointer, value.length);
  } finally {
    pointer.asTypedList(value.length).fillRange(0, value.length, 0);
    calloc.free(pointer);
  }
}

T _withOptionalNativeBytes<T>(
  Uint8List? value,
  T Function(ffi.Pointer<ffi.Uint8>, int) run,
) {
  if (value == null) {
    return run(ffi.nullptr.cast<ffi.Uint8>(), 0);
  }
  return _withNativeBytes(value, run);
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
