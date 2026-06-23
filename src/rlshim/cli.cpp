#include "cli.h"
#include <iostream>

namespace cli {
    std::optional<std::string> prompt_for_url(const std::string& step_name,
                                              const std::string& instructions,
                                              const std::string& url_to_open,
                                              const std::string& url_placeholder) {
        std::cout << "\n================ " << step_name << " ================\n\n";
        std::cout << instructions << "\n\n";
        std::cout << url_to_open << "\n\n";
        std::cout << "paste redirect URL here (" << url_placeholder << "): ";

        std::string pasted_url;
        std::getline(std::cin, pasted_url);

        if (pasted_url.empty()) {
            return std::nullopt;
        }

        return pasted_url;
    }

    std::optional<auth::game_account> prompt_for_character(const std::vector<auth::game_account>& accounts) {
        if (accounts.empty())
            return std::nullopt;
        if (accounts.size() == 1)
            return accounts[0];

        while (true) {
            std::cout << "\n================ choose your character ================\n\n";
            for (size_t i = 0; i < accounts.size(); ++i) {
                std::string displayName = accounts[i].displayName;
                if (displayName.empty()) {
                    displayName = "<no name set>";
                }

                std::cout << "[" << (i + 1) << "] " << displayName << "\n";
            }
            std::cout << "\nchoose a character (1-" << accounts.size() << "): ";

            std::string input;
            if (!std::getline(std::cin, input)) {
                return std::nullopt;
            }

            try {
                int choice = std::stoi(input);
                if (choice >= 1 && choice <= static_cast<int>(accounts.size())) {
                    return accounts[choice - 1];
                }
            } catch (...) {
            }
            std::cout << "invalid choice, please try again.\n";
        }
    }

}  // namespace cli
