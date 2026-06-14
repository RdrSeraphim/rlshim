#ifndef RLSHIM_AUTH_H
#define RLSHIM_AUTH_H

#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <vector>

namespace auth {
    struct game_account {
        std::string accountId;
        std::string displayName;
        std::string userHash;
    };

    struct oauth_tokens {
        std::string access_token;
        std::string id_token;
        std::string refresh_token;
        std::string sub;
        long expiry = 0;
    };

    struct auth_session {
        std::vector<game_account> accounts;
        oauth_tokens tokens;
        std::string session_id;
    };

    NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE_WITH_DEFAULT(game_account, accountId, displayName, userHash)
    NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE_WITH_DEFAULT(oauth_tokens, access_token, id_token, refresh_token, sub, expiry)
    NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE_WITH_DEFAULT(auth_session, accounts, tokens, session_id)

    std::optional<std::vector<auth_session>> read_session();
    bool save_session(const auth_session& creds);

    std::optional<auth_session> refresh_oauth_token(const std::string& refresh_token);
    std::optional<std::string> get_consent_id_token(const std::string& id_token, bool use_gui);
    std::optional<std::string> get_session_id(const std::string& id_token);
    std::optional<auth_session> do_interactive_login(bool use_gui);
    std::optional<std::vector<game_account>> get_game_accounts(const std::string& session_id);
    bool logout();

    namespace keyring {
        bool store(const std::string& key, const std::string& value);
        std::optional<std::string> lookup(const std::string& key);
        bool clear(const std::string& key);
    }  // namespace keyring
}  // namespace auth

#endif  // RLSHIM_AUTH_H