#import "RadishLexRuntime.h"

#import "RadishLexBridge.h"

@interface RLXProcessRuntime ()
@property(nonatomic, strong) NSHashTable<RLXSessionBridge *> *sessions;
@property(nonatomic) BOOL shutdown;
@end

@implementation RLXProcessRuntime

+ (instancetype)sharedRuntime {
  static RLXProcessRuntime *runtime;
  static dispatch_once_t onceToken;
  dispatch_once(&onceToken, ^{
    runtime = [[self alloc] init];
  });
  return runtime;
}

- (instancetype)init {
  self = [super init];
  if (self != nil) {
    _sessions = [NSHashTable weakObjectsHashTable];
  }
  return self;
}

- (nullable RLXSessionBridge *)createSessionWithError:(NSError **)error {
  if (![NSThread isMainThread] || self.shutdown) {
    if (error != NULL) {
      *error = [NSError errorWithDomain:RLXBridgeErrorDomain
                                   code:RADISHLEX_STATUS_INVALID_STATE
                               userInfo:@{NSLocalizedDescriptionKey :
                                              @"Input runtime is unavailable on this thread"}];
    }
    return nil;
  }

#if RADISHLEX_CONTRACT_SMOKE
  RLXSessionBridge *session = [[RLXSessionBridge alloc] initDemoWithError:error];
#else
  NSBundle *bundle = [NSBundle mainBundle];
  NSString *schema = [bundle objectForInfoDictionaryKey:@"RadishLexRimeSchema"];
  NSString *sharedRelative =
      [bundle objectForInfoDictionaryKey:@"RadishLexRimeSharedDataDirectory"];
  if (schema.length == 0 || sharedRelative.length == 0) {
    if (error != NULL) {
      *error = [NSError errorWithDomain:RLXBridgeErrorDomain
                                   code:RADISHLEX_STATUS_INVALID_STATE
                               userInfo:@{NSLocalizedDescriptionKey :
                                              @"Bundle is missing Rime runtime configuration"}];
    }
    return nil;
  }
  NSString *sharedData = [bundle.resourcePath stringByAppendingPathComponent:sharedRelative];
  NSArray<NSURL *> *applicationSupport =
      [[NSFileManager defaultManager] URLsForDirectory:NSApplicationSupportDirectory
                                             inDomains:NSUserDomainMask];
  NSURL *userDataURL = [[applicationSupport firstObject]
      URLByAppendingPathComponent:@"RadishLex/Rime" isDirectory:YES];
  NSError *directoryError = nil;
  if (userDataURL == nil ||
      ![[NSFileManager defaultManager] createDirectoryAtURL:userDataURL
                                withIntermediateDirectories:YES
                                                 attributes:nil
                                                      error:&directoryError]) {
    if (error != NULL) {
      *error = [NSError errorWithDomain:RLXBridgeErrorDomain
                                   code:RADISHLEX_STATUS_INVALID_STATE
                               userInfo:@{NSLocalizedDescriptionKey :
                                              @"Unable to prepare isolated Rime user data"}];
    }
    return nil;
  }
  NSNumber *deploy = [bundle objectForInfoDictionaryKey:@"RadishLexRimeDeployOnStart"];
  RLXSessionBridge *session =
      [[RLXSessionBridge alloc] initRimeWithSharedDataDirectory:sharedData
                                             userDataDirectory:userDataURL.path
                                                        schema:schema
                                                  logDirectory:nil
                                                 deployOnStart:deploy.boolValue
                                                         error:error];
#endif
  if (session != nil) {
    [self.sessions addObject:session];
  }
  return session;
}

- (void)forgetSession:(RLXSessionBridge *)session {
  [self.sessions removeObject:session];
}

- (BOOL)shutdownWithError:(NSError **)error {
  if (![NSThread isMainThread]) {
    if (error != NULL) {
      *error = [NSError errorWithDomain:RLXBridgeErrorDomain
                                   code:RADISHLEX_STATUS_INVALID_STATE
                               userInfo:@{NSLocalizedDescriptionKey :
                                              @"Runtime shutdown must run on the owner thread"}];
    }
    return NO;
  }
  for (RLXSessionBridge *session in self.sessions.allObjects) {
    [session invalidate];
  }
  [self.sessions removeAllObjects];

#if !RADISHLEX_CONTRACT_SMOKE
  RadishLexError *ffiError = NULL;
  RadishLexStatusCode status = radishlex_rime_runtime_shutdown(&ffiError);
  if (status != RADISHLEX_STATUS_OK) {
    NSString *message = @"Rime runtime shutdown failed";
    if (ffiError != NULL) {
      const char *rawMessage = radishlex_error_message(ffiError);
      if (rawMessage != NULL) {
        NSString *copied = [NSString stringWithUTF8String:rawMessage];
        if (copied != nil) message = copied;
      }
      radishlex_error_free(ffiError);
    }
    if (error != NULL) {
      *error = [NSError errorWithDomain:RLXBridgeErrorDomain
                                   code:status
                               userInfo:@{NSLocalizedDescriptionKey : message}];
    }
    return NO;
  }
#endif
  self.shutdown = YES;
  return YES;
}

@end
