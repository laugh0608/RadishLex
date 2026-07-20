#import "RadishLexCandidatePanel.h"

#if !RADISHLEX_CONTRACT_SMOKE
#error "Candidate panel inspection is available only to contract smoke builds."
#endif

@interface RLXCandidatePanel (ContractInspection)
@property(nonatomic, readonly) NSPanel *rlx_contractWindow;
@property(nonatomic, copy, readonly)
    NSArray<NSButton *> *rlx_contractCandidateButtons;
@property(nonatomic, readonly) NSStackView *rlx_contractCandidateStack;
@property(nonatomic, readonly) NSInteger rlx_contractSelectedIndex;
@property(nonatomic, weak, readonly)
    id<RLXCandidatePanelOwner> rlx_contractOwner;
@end
