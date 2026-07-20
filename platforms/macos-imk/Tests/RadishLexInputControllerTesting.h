#import "RadishLexInputController.h"

#if !RADISHLEX_CONTRACT_SMOKE
#error "Input controller test initialization is available only to contract smoke builds."
#endif

@interface RadishLexInputController (ContractInitialization)
- (instancetype)initForContractWithClient:(id)inputClient;
@end
