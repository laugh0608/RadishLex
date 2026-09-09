#import "RadishLexInputController.h"
#import "RadishLexBridge.h"

#if !RADISHLEX_CONTRACT_SMOKE
#error "Input controller test initialization is available only to contract smoke builds."
#endif

@interface RadishLexInputController (ContractInitialization)
- (instancetype)initForContractWithClient:(id)inputClient;
- (instancetype)initForContractWithClient:(id)inputClient
                                 session:(RLXSessionBridge *)session;
@property(nonatomic) BOOL contractPrivacyMode;
@property(nonatomic) BOOL contractSecureInput;
@end
