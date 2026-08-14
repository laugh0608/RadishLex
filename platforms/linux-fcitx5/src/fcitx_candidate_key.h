#ifndef RADISHLEX_FCITX_CANDIDATE_KEY_H
#define RADISHLEX_FCITX_CANDIDATE_KEY_H

#include <fcitx-utils/key.h>

namespace radishlex::linux_fcitx5 {

bool candidateListHandlesKey(const fcitx::Key &key);

}  // namespace radishlex::linux_fcitx5

#endif
