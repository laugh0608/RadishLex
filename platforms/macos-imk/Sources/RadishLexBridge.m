#import "RadishLexBridge.h"

#import <Carbon/Carbon.h>

NSErrorDomain const RLXBridgeErrorDomain = @"org.radishlex.inputmethod.bridge";
NSAttributedStringKey const RLXCandidateIndexAttributeName =
    @"org.radishlex.inputmethod.candidate-index";

@interface RLXCandidate ()
@property(nonatomic) NSUInteger index;
@property(nonatomic, copy) NSString *text;
@property(nonatomic, copy, nullable) NSString *reading;
@property(nonatomic, copy, nullable) NSString *annotation;
@property(nonatomic) uint32_t source;
@end
@implementation RLXCandidate
@end

@interface RLXSnapshot ()
@property(nonatomic, copy) NSString *schema;
@property(nonatomic, copy) NSString *preedit;
@property(nonatomic) NSUInteger cursor;
@property(nonatomic, copy) NSArray<RLXCandidate *> *candidates;
@end
@implementation RLXSnapshot
@end

@interface RLXKeyHandlingResult ()
@property(nonatomic, getter=isConsumed) BOOL consumed;
@property(nonatomic, copy, nullable) NSString *commit;
@property(nonatomic, strong, nullable) RLXSnapshot *snapshot;
@end
@implementation RLXKeyHandlingResult
@end

@interface RLXSessionBridge ()
@property(nonatomic) RadishLexSession *session;
@property(nonatomic, strong) NSThread *ownerThread;
@property(nonatomic, getter=isValid) BOOL valid;
@end

static void RLXAssignError(NSError **error, NSInteger code, NSString *message) {
  if (error != NULL) {
    *error = [NSError errorWithDomain:RLXBridgeErrorDomain
                                 code:code
                             userInfo:@{NSLocalizedDescriptionKey : message}];
  }
}

static void RLXCopyFFIError(NSError **error, RadishLexStatusCode status,
                            RadishLexError *ffiError) {
  NSString *message = @"RadishLex FFI call failed";
  if (ffiError != NULL) {
    status = radishlex_error_code(ffiError);
    const char *rawMessage = radishlex_error_message(ffiError);
    if (rawMessage != NULL) {
      NSString *copied = [NSString stringWithUTF8String:rawMessage];
      if (copied != nil) {
        message = copied;
      }
    }
    radishlex_error_free(ffiError);
  }
  RLXAssignError(error, status, message);
}

static NSString *_Nullable RLXCopyStringView(RadishLexStringView view,
                                              NSError **error) {
  if (view.len == 0) {
    return @"";
  }
  if (view.data == NULL) {
    RLXAssignError(error, RADISHLEX_STATUS_INTERNAL_ERROR,
                   @"FFI returned a null pointer for a non-empty UTF-8 view");
    return nil;
  }
  NSString *value = [[NSString alloc] initWithBytes:view.data
                                             length:view.len
                                           encoding:NSUTF8StringEncoding];
  if (value == nil) {
    RLXAssignError(error, RADISHLEX_STATUS_INTERNAL_ERROR,
                   @"FFI returned an invalid UTF-8 view");
  }
  return value;
}

NSUInteger RLXUTF16CursorForUTF8Offset(NSString *value, size_t utf8Offset,
                                      NSError **error) {
  NSData *utf8 = [value dataUsingEncoding:NSUTF8StringEncoding];
  if (utf8Offset > utf8.length) {
    RLXAssignError(error, RADISHLEX_STATUS_INTERNAL_ERROR,
                   @"Snapshot cursor exceeds the UTF-8 preedit length");
    return NSNotFound;
  }
  NSData *prefix = [utf8 subdataWithRange:NSMakeRange(0, utf8Offset)];
  NSString *prefixString = [[NSString alloc] initWithData:prefix
                                                  encoding:NSUTF8StringEncoding];
  if (prefixString == nil) {
    RLXAssignError(error, RADISHLEX_STATUS_INTERNAL_ERROR,
                   @"Snapshot cursor is not on a UTF-8 scalar boundary");
    return NSNotFound;
  }
  return prefixString.length;
}

static RLXSnapshot *_Nullable RLXCopySnapshot(const RadishLexSnapshot *snapshot,
                                               NSError **error) {
  if (snapshot == NULL) {
    RLXAssignError(error, RADISHLEX_STATUS_INTERNAL_ERROR,
                   @"FFI returned a null snapshot");
    return nil;
  }

  NSString *schema = RLXCopyStringView(radishlex_snapshot_schema(snapshot), error);
  NSString *preedit = RLXCopyStringView(radishlex_snapshot_preedit(snapshot), error);
  if (schema == nil || preedit == nil) {
    return nil;
  }
  NSUInteger cursor =
      RLXUTF16CursorForUTF8Offset(preedit, radishlex_snapshot_cursor(snapshot), error);
  if (cursor == NSNotFound) {
    return nil;
  }

  NSMutableArray<RLXCandidate *> *candidates = [NSMutableArray array];
  size_t count = radishlex_snapshot_candidate_count(snapshot);
  for (size_t index = 0; index < count; ++index) {
    RadishLexCandidateView view = {0};
    RadishLexError *ffiError = NULL;
    RadishLexStatusCode status =
        radishlex_snapshot_candidate(snapshot, index, &view, &ffiError);
    if (status != RADISHLEX_STATUS_OK) {
      RLXCopyFFIError(error, status, ffiError);
      return nil;
    }
    if (view.reading_present > 1 || view.annotation_present > 1) {
      RLXAssignError(error, RADISHLEX_STATUS_INTERNAL_ERROR,
                     @"FFI returned an invalid candidate presence flag");
      return nil;
    }
    NSString *text = RLXCopyStringView(view.text, error);
    NSString *reading = view.reading_present ? RLXCopyStringView(view.reading, error) : nil;
    NSString *annotation =
        view.annotation_present ? RLXCopyStringView(view.annotation, error) : nil;
    if (text == nil || (view.reading_present && reading == nil) ||
        (view.annotation_present && annotation == nil)) {
      return nil;
    }
    RLXCandidate *candidate = [[RLXCandidate alloc] init];
    candidate.index = view.index;
    candidate.text = text;
    candidate.reading = reading;
    candidate.annotation = annotation;
    candidate.source = view.source;
    [candidates addObject:candidate];
  }

  RLXSnapshot *copied = [[RLXSnapshot alloc] init];
  copied.schema = schema;
  copied.preedit = preedit;
  copied.cursor = cursor;
  copied.candidates = candidates;
  return copied;
}

@implementation RLXSessionBridge

+ (BOOL)validateFFIContract:(NSError **)error {
  RadishLexFfiContract contract = {0};
  RadishLexError *ffiError = NULL;
  RadishLexStatusCode status = radishlex_ffi_contract(&contract, &ffiError);
  if (status != RADISHLEX_STATUS_OK) {
    RLXCopyFFIError(error, status, ffiError);
    return NO;
  }
  if (contract.version != RADISHLEX_ABI_CONTRACT_VERSION ||
      contract.session_thread_policy != RADISHLEX_SESSION_THREAD_POLICY_OWNER_THREAD ||
      contract.panic_boundary != RADISHLEX_FFI_PANIC_BOUNDARY_CATCH_UNWIND) {
    RLXAssignError(error, RADISHLEX_STATUS_INVALID_STATE,
                   @"RadishLex FFI contract is incompatible with the macOS shell");
    return NO;
  }
  return YES;
}

- (nullable instancetype)initDemoWithError:(NSError **)error {
  self = [super init];
  if (self == nil) {
    return nil;
  }
  if (![[self class] validateFFIContract:error]) {
    return nil;
  }
  RadishLexError *ffiError = NULL;
  _session = radishlex_session_new(&ffiError);
  if (_session == NULL) {
    RLXCopyFFIError(error, RADISHLEX_STATUS_INVALID_STATE, ffiError);
    return nil;
  }
  _ownerThread = [NSThread currentThread];
  _valid = YES;
  return self;
}

- (nullable instancetype)initRimeWithSharedDataDirectory:(NSString *)sharedDataDirectory
                                       userDataDirectory:(NSString *)userDataDirectory
                                                  schema:(NSString *)schema
                                            logDirectory:(nullable NSString *)logDirectory
                                           deployOnStart:(BOOL)deployOnStart
                                                   error:(NSError **)error {
  self = [super init];
  if (self == nil) {
    return nil;
  }
  if (![[self class] validateFFIContract:error]) {
    return nil;
  }
  RadishLexRimeSessionOptions options = {
      .version = RADISHLEX_RIME_SESSION_OPTIONS_VERSION,
      .shared_data_dir = sharedDataDirectory.fileSystemRepresentation,
      .user_data_dir = userDataDirectory.fileSystemRepresentation,
      .schema = schema.UTF8String,
      .log_dir = logDirectory != nil ? logDirectory.fileSystemRepresentation : NULL,
      .deploy_on_start = deployOnStart ? 1 : 0,
  };
  RadishLexError *ffiError = NULL;
  _session = radishlex_session_new_rime(&options, &ffiError);
  if (_session == NULL) {
    RLXCopyFFIError(error, RADISHLEX_STATUS_INVALID_STATE, ffiError);
    return nil;
  }
  _ownerThread = [NSThread currentThread];
  _valid = YES;
  return self;
}

- (BOOL)ensureOwner:(NSError **)error {
  if (!self.valid || self.session == NULL) {
    RLXAssignError(error, RADISHLEX_STATUS_INVALID_STATE,
                   @"RadishLex session is not active");
    return NO;
  }
  if ([NSThread currentThread] != self.ownerThread) {
    RLXAssignError(error, RADISHLEX_STATUS_INVALID_STATE,
                   @"RadishLex session call is not on its owner thread");
    return NO;
  }
  return YES;
}

- (nullable RLXKeyHandlingResult *)handleEvent:(RadishLexKeyEvent)event
                                          error:(NSError **)error {
  if (![self ensureOwner:error]) {
    return nil;
  }
  RadishLexKeyResult *result = NULL;
  RadishLexError *ffiError = NULL;
  RadishLexStatusCode status =
      radishlex_session_handle_key_event(self.session, event, &result, &ffiError);
  if (status != RADISHLEX_STATUS_OK) {
    RLXCopyFFIError(error, status, ffiError);
    return nil;
  }
  uint8_t consumed = radishlex_key_result_consumed(result);
  uint8_t commitPresent = radishlex_key_result_commit_present(result);
  if (radishlex_key_result_version(result) != RADISHLEX_KEY_RESULT_VERSION ||
      consumed > 1 || commitPresent > 1) {
    radishlex_key_result_free(result);
    RLXAssignError(error, RADISHLEX_STATUS_INVALID_STATE,
                   @"RadishLex key result contract is incompatible with the macOS shell");
    return nil;
  }
  NSString *commit = commitPresent ? RLXCopyStringView(radishlex_key_result_commit(result), error)
                                   : nil;
  RLXSnapshot *snapshot = RLXCopySnapshot(radishlex_key_result_snapshot(result), error);
  radishlex_key_result_free(result);
  if ((commitPresent && commit == nil) || (snapshot == nil && commit == nil)) {
    return nil;
  }
  RLXKeyHandlingResult *copied = [[RLXKeyHandlingResult alloc] init];
  copied.consumed = consumed == 1;
  copied.commit = commit;
  copied.snapshot = snapshot;
  return copied;
}

- (nullable RLXSnapshot *)snapshotWithError:(NSError **)error {
  if (![self ensureOwner:error]) {
    return nil;
  }
  RadishLexError *ffiError = NULL;
  RadishLexSnapshot *snapshot = radishlex_session_snapshot_new(self.session, &ffiError);
  if (snapshot == NULL) {
    RLXCopyFFIError(error, RADISHLEX_STATUS_INVALID_STATE, ffiError);
    return nil;
  }
  RLXSnapshot *copied = RLXCopySnapshot(snapshot, error);
  radishlex_snapshot_free(snapshot);
  return copied;
}

- (nullable RLXKeyHandlingResult *)selectCandidateAtIndex:(NSUInteger)index
                                                      error:(NSError **)error {
  if (![self ensureOwner:error]) {
    return nil;
  }
  RadishLexError *ffiError = NULL;
  RadishLexKeyResult *result = NULL;
  RadishLexStatusCode status =
      radishlex_session_select_candidate(self.session, index, &result, &ffiError);
  if (status != RADISHLEX_STATUS_OK || result == NULL) {
    RLXCopyFFIError(error, status, ffiError);
    return nil;
  }
  BOOL consumed = radishlex_key_result_consumed(result) == 1;
  BOOL commitPresent = radishlex_key_result_commit_present(result) == 1;
  NSString *commit = commitPresent ? RLXCopyStringView(radishlex_key_result_commit(result), error)
                                   : nil;
  RLXSnapshot *snapshot = RLXCopySnapshot(radishlex_key_result_snapshot(result), error);
  radishlex_key_result_free(result);
  if ((commitPresent && commit == nil) || snapshot == nil) return nil;
  RLXKeyHandlingResult *copied = [[RLXKeyHandlingResult alloc] init];
  copied.consumed = consumed;
  copied.commit = commit;
  copied.snapshot = snapshot;
  return copied;
}

- (BOOL)resetWithError:(NSError **)error {
  if (![self ensureOwner:error]) {
    return NO;
  }
  RadishLexError *ffiError = NULL;
  RadishLexStatusCode status = radishlex_session_reset(self.session, &ffiError);
  if (status != RADISHLEX_STATUS_OK) {
    RLXCopyFFIError(error, status, ffiError);
    return NO;
  }
  return YES;
}

- (BOOL)setSchema:(NSString *)schema error:(NSError **)error {
  if (![self ensureOwner:error]) {
    return NO;
  }
  RadishLexError *ffiError = NULL;
  RadishLexStatusCode status =
      radishlex_session_set_schema(self.session, schema.UTF8String, &ffiError);
  if (status != RADISHLEX_STATUS_OK) {
    RLXCopyFFIError(error, status, ffiError);
    return NO;
  }
  return YES;
}

- (void)invalidate {
  if (!self.valid || self.session == NULL || [NSThread currentThread] != self.ownerThread) {
    return;
  }
  radishlex_session_free(self.session);
  self.session = NULL;
  self.valid = NO;
}

- (void)dealloc {
  [self invalidate];
}

@end

static uint32_t RLXModifiers(NSEventModifierFlags flags) {
  uint32_t modifiers = 0;
  if (flags & NSEventModifierFlagShift) modifiers |= RADISHLEX_KEY_MOD_SHIFT;
  if (flags & NSEventModifierFlagControl) modifiers |= RADISHLEX_KEY_MOD_CONTROL;
  if (flags & NSEventModifierFlagOption) modifiers |= RADISHLEX_KEY_MOD_ALT;
  if (flags & NSEventModifierFlagCommand) modifiers |= RADISHLEX_KEY_MOD_META;
  return modifiers;
}

static uint32_t RLXNamedKey(unsigned short keyCode) {
  switch (keyCode) {
    case kVK_Space: return RADISHLEX_NAMED_KEY_SPACE;
    case kVK_Return:
    case kVK_ANSI_KeypadEnter: return RADISHLEX_NAMED_KEY_ENTER;
    case kVK_Delete: return RADISHLEX_NAMED_KEY_BACKSPACE;
    case kVK_Escape: return RADISHLEX_NAMED_KEY_ESCAPE;
    case kVK_Tab: return RADISHLEX_NAMED_KEY_TAB;
    case kVK_UpArrow: return RADISHLEX_NAMED_KEY_ARROW_UP;
    case kVK_DownArrow: return RADISHLEX_NAMED_KEY_ARROW_DOWN;
    case kVK_LeftArrow: return RADISHLEX_NAMED_KEY_ARROW_LEFT;
    case kVK_RightArrow: return RADISHLEX_NAMED_KEY_ARROW_RIGHT;
    case kVK_PageUp: return RADISHLEX_NAMED_KEY_PAGE_UP;
    case kVK_PageDown: return RADISHLEX_NAMED_KEY_PAGE_DOWN;
    case kVK_Shift:
    case kVK_RightShift: return RADISHLEX_NAMED_KEY_SHIFT;
    case kVK_Control:
    case kVK_RightControl: return RADISHLEX_NAMED_KEY_CONTROL;
    case kVK_Option:
    case kVK_RightOption: return RADISHLEX_NAMED_KEY_ALT;
    case kVK_Command:
    case kVK_RightCommand: return RADISHLEX_NAMED_KEY_META;
    default: return 0;
  }
}

BOOL RLXNormalizeKeyEvent(NSEvent *event, RadishLexKeyEvent *eventOut) {
  if (eventOut == NULL) return NO;
  if (event.type != NSEventTypeKeyDown && event.type != NSEventTypeKeyUp &&
      event.type != NSEventTypeFlagsChanged) return NO;
  uint32_t namedKey = RLXNamedKey(event.keyCode);
  uint32_t phase = event.type == NSEventTypeKeyUp ? RADISHLEX_KEY_PHASE_RELEASE
                                                  : RADISHLEX_KEY_PHASE_PRESS;
  if (event.type == NSEventTypeFlagsChanged) {
    uint32_t modifier = namedKey == RADISHLEX_NAMED_KEY_SHIFT ? RADISHLEX_KEY_MOD_SHIFT
        : namedKey == RADISHLEX_NAMED_KEY_CONTROL ? RADISHLEX_KEY_MOD_CONTROL
        : namedKey == RADISHLEX_NAMED_KEY_ALT ? RADISHLEX_KEY_MOD_ALT
        : namedKey == RADISHLEX_NAMED_KEY_META ? RADISHLEX_KEY_MOD_META : 0;
    if (modifier == 0) return NO;
    phase = (RLXModifiers(event.modifierFlags) & modifier) ? RADISHLEX_KEY_PHASE_PRESS
                                                           : RADISHLEX_KEY_PHASE_RELEASE;
  }
  if (namedKey != 0) {
    *eventOut = (RadishLexKeyEvent){RADISHLEX_KEY_KIND_NAMED, 0, namedKey,
                                    RLXModifiers(event.modifierFlags), phase};
    return YES;
  }
  NSString *characters = event.charactersIgnoringModifiers;
  if (characters.length == 0) return NO;
  __block uint32_t scalar = 0;
  __block NSUInteger scalarCount = 0;
  [characters enumerateSubstringsInRange:NSMakeRange(0, characters.length)
                                  options:NSStringEnumerationByComposedCharacterSequences
                               usingBlock:^(NSString *substring, NSRange substringRange,
                                            NSRange enclosingRange, BOOL *stop) {
    (void)substringRange; (void)enclosingRange;
    scalarCount += 1;
    if (substring.length == 1) {
      scalar = [substring characterAtIndex:0];
    } else if (substring.length == 2) {
      unichar high = [substring characterAtIndex:0];
      unichar low = [substring characterAtIndex:1];
      if (CFStringIsSurrogateHighCharacter(high) && CFStringIsSurrogateLowCharacter(low)) {
        scalar = CFStringGetLongCharacterForSurrogatePair(high, low);
      } else {
        scalar = 0;
      }
    }
    *stop = scalarCount > 1;
  }];
  if (scalarCount != 1 || scalar < 0x20 || scalar == 0x7f ||
      (scalar >= NSUpArrowFunctionKey && scalar <= NSModeSwitchFunctionKey)) return NO;
  *eventOut = (RadishLexKeyEvent){RADISHLEX_KEY_KIND_CHAR, scalar, 0,
                                  RLXModifiers(event.modifierFlags), phase};
  return YES;
}

NSAttributedString *RLXAttributedCandidate(RLXCandidate *candidate) {
  return [[NSAttributedString alloc]
      initWithString:candidate.text
          attributes:@{RLXCandidateIndexAttributeName : @(candidate.index)}];
}

NSNumber *_Nullable RLXCandidateIndexFromAttributedString(NSAttributedString *candidate) {
  if (candidate.length == 0) return nil;
  return [candidate attribute:RLXCandidateIndexAttributeName atIndex:0 effectiveRange:NULL];
}
