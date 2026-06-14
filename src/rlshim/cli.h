#pragma once

#include <optional>
#include <string>
#include <vector>

#include "auth.h"

namespace cli {
    std::optional<std::string> prompt_for_url(const std::string& step_name,
                                              const std::string& instructions,
                                              const std::string& url_to_open,
                                              const std::string& url_placeholder);

    std::optional<auth::game_account> prompt_for_character(const std::vector<auth::game_account>& accounts);
}  // namespace cli
