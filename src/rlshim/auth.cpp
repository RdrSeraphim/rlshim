#include "auth.h"
#include "cli.h"
#include "curl.h"
#include "gui.h"
#include "logger.h"

#include <curl/curl.h>
#include <libsecret/secret.h>
#include <cstddef>
#include <nlohmann/json.hpp>

#include <openssl/evp.h>
#include <openssl/rand.h>
#include <optional>
#include <string>
#include <vector>

using json = nlohmann::json;

extern "C" {
CURLcode curl_easy_impersonate(CURL* curl, const char* target, int default_headers);
}

namespace auth {
    std::optional<std::vector<auth_session>> read_session() {
        auto stored_creds = keyring::lookup("jagex_auth");
        if (!stored_creds) {
            return std::nullopt;
        }

        json creds_json;
        try {
            creds_json = json::parse(*stored_creds);
        } catch (const std::exception& e) {
            logger::error("failed to parse creds from keyring: {}", e.what());
            return std::nullopt;
        }

        try {
            auto creds = creds_json.get<std::vector<auth_session>>();
            return creds;
        } catch (const std::exception& e) {
            logger::error("failed to convert creds to auth_session: {}", e.what());
            return std::nullopt;
        }
    }

    bool save_session(const auth_session& creds) {
        json creds_json = std::vector<auth_session>{creds};
        std::string serialized = creds_json.dump();

        if (!keyring::store("jagex_auth", serialized)) {
            logger::error("failed to store creds in keyring");
            return false;
        }

        return true;
    }

    bool logout() {
        if (!keyring::clear("jagex_auth")) {
            return false;
        }
        logger::info("successfully logged out. credentials have been removed from the keyring.");
        return true;
    }

    std::optional<auth_session> refresh_oauth_token(const std::string& refresh_token) {
        CURL* curl = curl_easy_init();
        if (!curl) {
            logger::error("failed to initialize curl for token refresh");
            return std::nullopt;
        }

        curl_easy_impersonate(curl, "chrome116", 1);
        curl_easy_setopt(curl, CURLOPT_ACCEPT_ENCODING, "");

        std::string response_body;
        std::string post_fields =
            "grant_type=refresh_token"
            "&client_id=com_jagex_auth_desktop_launcher"
            "&refresh_token=" +
            refresh_token;

        struct curl_slist* headers = nullptr;
        headers = curl_slist_append(headers, "Content-Type: application/x-www-form-urlencoded");
        headers = curl_slist_append(headers, "Accept: application/json");

        curl_easy_setopt(curl, CURLOPT_URL, "https://account.jagex.com/oauth2/token");
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, post_fields.c_str());
        curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
        curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, rlshim_write_callback);
        curl_easy_setopt(curl, CURLOPT_WRITEDATA, &response_body);

        CURLcode res = curl_easy_perform(curl);
        long http_code = 0;
        curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &http_code);

        curl_slist_free_all(headers);
        curl_easy_cleanup(curl);

        if (res != CURLE_OK) {
            logger::error("failed to refresh token: {}", curl_easy_strerror(res));
            return std::nullopt;
        }

        if (http_code != 200) {
            logger::error("failed to refresh token with http {}", http_code);
            return std::nullopt;
        }

        try {
            auto auth_json = json::parse(response_body);
            auth_session creds;
            creds.tokens.access_token = auth_json["access_token"].get<std::string>();
            creds.tokens.id_token = auth_json["id_token"].get<std::string>();
            creds.tokens.refresh_token = auth_json["refresh_token"].get<std::string>();

            if (auth_json.contains("expires_in")) {
                creds.tokens.expiry = auth_json["expires_in"].get<long>();
            }

            return creds;
        } catch (const std::exception& e) {
            logger::error("failed to parse token response: {}", e.what());
            return std::nullopt;
        }
    }

    std::string base64_url_encode(const std::vector<unsigned char>& input) {
        static const char b64_table[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        std::string out;
        out.reserve(((input.size() + 2) / 3) * 4);
        int val = 0;
        int valb = -6;
        for (unsigned char c : input) {
            val = (val << 8) + c;
            valb += 8;
            while (valb >= 0) {
                out.push_back(b64_table[(val >> valb) & 0x3F]);
                valb -= 6;
            }
        }
        if (valb > -6) {
            out.push_back(b64_table[((val << 8) >> (valb + 8)) & 0x3F]);
        }
        return out;
    }

    std::string generate_random_string(size_t length) {
        std::vector<unsigned char> bytes(length);
        RAND_bytes(bytes.data(), bytes.size());
        return base64_url_encode(bytes).substr(0, length);
    }

    std::string sha256_base64url(const std::string& input) {
        unsigned char hash[EVP_MAX_MD_SIZE];
        unsigned int lengthOfHash = 0;

        EVP_MD_CTX* context = EVP_MD_CTX_new();
        if (context != nullptr) {
            if (EVP_DigestInit_ex(context, EVP_sha256(), nullptr)) {
                EVP_DigestUpdate(context, input.c_str(), input.length());
                EVP_DigestFinal_ex(context, hash, &lengthOfHash);
            }
            EVP_MD_CTX_free(context);
        }

        std::vector<unsigned char> hash_vec(hash, hash + lengthOfHash);
        return base64_url_encode(hash_vec);
    }

    std::optional<std::string> get_consent_id_token(const std::string& first_id_token, bool use_gui) {
        std::string nonce = generate_random_string(32);
        std::string state = generate_random_string(32);

        CURL* curl = curl_easy_init();
        if (!curl)
            return std::nullopt;

        char* enc_id_token = curl_easy_escape(curl, first_id_token.c_str(), 0);
        char* enc_redirect = curl_easy_escape(curl, "http://localhost", 0);
        char* enc_response_type = curl_easy_escape(curl, "id_token code", 0);
        char* enc_scope = curl_easy_escape(curl, "openid offline", 0);

        std::string url = "https://account.jagex.com/oauth2/auth?";
        url += "id_token_hint=" + std::string(enc_id_token);
        url += "&nonce=" + nonce;
        url += "&prompt=consent";
        url += "&redirect_uri=" + std::string(enc_redirect);
        url += "&response_type=" + std::string(enc_response_type);
        url += "&state=" + state;
        url += "&client_id=1fddee4e-b100-4f4e-b2b0-097f9088f9d2";
        url += "&scope=" + std::string(enc_scope);

        curl_free(enc_id_token);
        curl_free(enc_redirect);
        curl_free(enc_response_type);
        curl_free(enc_scope);
        curl_easy_cleanup(curl);

        std::string instructions =
            "round two, the last one. this is just so Jagex knows you're not a robot and are eligible to receive a "
            "session ID.\n\n"
            "the process is a bit similar, open the link below, get through cloudflare (login shouldn't be "
            "necessary), get redirected to a localhost URL";

        std::string instructions_pt_2 = "like before, copy the full localhost URL and paste it here:";
        std::string url_placeholder = "http://localhost/#code=...";

        std::optional<std::string> pasted_url_opt;
        if (use_gui) {
            pasted_url_opt = gui::prompt_for_url("rlshim - initial login 2 electric boogaloo", instructions,
                                                 instructions_pt_2, url, url_placeholder);
        } else {
            pasted_url_opt =
                cli::prompt_for_url("initial login 2 electric boogaloo", instructions, url, url_placeholder);
        }

        if (!pasted_url_opt) {
            logger::error("No URL provided");
            return std::nullopt;
        }

        std::string location = *pasted_url_opt;
        std::string id_token_search = "id_token=";
        size_t id_token_pos = location.find(id_token_search);
        if (id_token_pos == std::string::npos) {
            logger::error("no id_token found in the pasted URL");
            return std::nullopt;
        }

        size_t start = id_token_pos + id_token_search.length();
        size_t end = location.find("&", start);
        if (end == std::string::npos) {
            end = location.length();
        }

        return location.substr(start, end - start);
    }

    std::optional<std::vector<game_account>> get_game_accounts(const std::string& session_id) {
        CURL* curl = curl_easy_init();
        if (!curl)
            return std::nullopt;
        curl_easy_impersonate(curl, "chrome116", 1);
        curl_easy_setopt(curl, CURLOPT_ACCEPT_ENCODING, "");

        std::string url = "https://auth.jagex.com/game-session/v1/accounts";

        struct curl_slist* headers = nullptr;
        headers = curl_slist_append(headers, "Accept: application/json");
        std::string auth_header = "Authorization: Bearer " + session_id;
        headers = curl_slist_append(headers, auth_header.c_str());

        std::string response_body;

        curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
        curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
        curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, rlshim_write_callback);
        curl_easy_setopt(curl, CURLOPT_WRITEDATA, &response_body);

        CURLcode res = curl_easy_perform(curl);

        long http_code = 0;
        curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &http_code);

        curl_slist_free_all(headers);
        curl_easy_cleanup(curl);

        if (res != CURLE_OK) {
            logger::error("failed to get game accounts: {}", curl_easy_strerror(res));
            return std::nullopt;
        }

        if (http_code != 200) {
            logger::error("failed to get game accounts with HTTP {}. Response: {}", http_code, response_body);
            return std::nullopt;
        }

        try {
            auto j = json::parse(response_body);
            std::vector<game_account> accounts;
            for (auto& item : j) {
                accounts.push_back(item.get<game_account>());
            }
            return accounts;
        } catch (const std::exception& e) {
            logger::error("failed to parse game accounts response: {}", e.what());
            return std::nullopt;
        }
    }

    std::optional<std::string> get_session_id(const std::string& id_token) {
        CURL* curl = curl_easy_init();
        if (!curl)
            return std::nullopt;
        curl_easy_impersonate(curl, "chrome116", 1);
        curl_easy_setopt(curl, CURLOPT_ACCEPT_ENCODING, "");

        std::string response_body;
        json payload = {{"idToken", id_token}};
        std::string post_fields = payload.dump();

        struct curl_slist* headers = nullptr;
        headers = curl_slist_append(headers, "Accept: application/json");
        headers = curl_slist_append(headers, "Content-Type: application/json");

        curl_easy_setopt(curl, CURLOPT_URL, "https://auth.jagex.com/game-session/v1/sessions");
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, post_fields.c_str());
        curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
        curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, rlshim_write_callback);
        curl_easy_setopt(curl, CURLOPT_WRITEDATA, &response_body);

        CURLcode res = curl_easy_perform(curl);
        long http_code = 0;
        curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &http_code);
        curl_slist_free_all(headers);
        curl_easy_cleanup(curl);

        if (res != CURLE_OK || http_code != 200) {
            logger::error("failed to get session id with HTTP {}. Response: {}", http_code, response_body);
            return std::nullopt;
        }

        try {
            auto j = json::parse(response_body);
            return j["sessionId"].get<std::string>();
        } catch (const json::parse_error& e) {
            logger::error("failed to parse session response: {}", e.what());
            return std::nullopt;
        }
    }

    std::optional<auth_session> do_interactive_login(bool use_gui) {
        std::string client_id = "com_jagex_auth_desktop_launcher";
        std::string redirect_uri = "https://secure.runescape.com/m=weblogin/launcher-redirect";
        std::string code_verifier = generate_random_string(64);
        std::string code_challenge = sha256_base64url(code_verifier);
        std::string state = generate_random_string(32);

        CURL* curl = curl_easy_init();
        char* enc_redirect = curl_easy_escape(curl, redirect_uri.c_str(), 0);
        char* enc_scope = curl_easy_escape(curl, "openid offline gamesso.token.create user.profile.read", 0);

        std::string auth_url = "https://account.jagex.com/oauth2/auth?";
        auth_url += "auth_method=&login_type=&flow=launcher&response_type=code";
        auth_url += "&client_id=" + client_id;
        auth_url += "&redirect_uri=" + std::string(enc_redirect);
        auth_url += "&code_challenge=" + code_challenge;
        auth_url += "&code_challenge_method=S256";
        auth_url += "&prompt=login";
        auth_url += "&scope=" + std::string(enc_scope);
        auth_url += "&state=" + state;

        curl_free(enc_redirect);
        curl_free(enc_scope);
        curl_easy_cleanup(curl);

        std::string instructions =
            "thank you for using rlshim!!\n\n"
            "apologies for the interruption, but this is your first time logging in, so "
            "we have to establish some credentials. once you've done it, you'll never have to do it again ...short of "
            "deleting them in your secrets manager.\n"
            "\n"
            "to start, open this link in your browser, get through cloudflare, login, any 2FA until you are "
            "redirected:";

        std::string instructions_pt_2 =
            "once you see \"Logging in to Jagex Launcher\", ignore/close any prompts for xdg-open, and copy the full "
            "link. paste that link here:";

        std::string url_placeholder = "https://secure.runescape.com/m=weblogin/launcher-redirect?code=...";

        std::optional<std::string> pasted_url_opt;
        if (use_gui) {
            pasted_url_opt = gui::prompt_for_url("rlshim - initial login", instructions, instructions_pt_2, auth_url,
                                                 url_placeholder);
        } else {
            pasted_url_opt = cli::prompt_for_url("rlshim - initial login", instructions, auth_url, url_placeholder);
        }

        if (!pasted_url_opt) {
            logger::error("No URL provided");
            return std::nullopt;
        }

        std::string pasted_url = *pasted_url_opt;

        size_t code_pos = pasted_url.find("code=");
        if (code_pos == std::string::npos) {
            logger::error("Could not find 'code=' in the pasted URL");
            return std::nullopt;
        }

        std::string code = pasted_url.substr(code_pos + 5);
        size_t ampersand_pos = code.find('&');
        if (ampersand_pos != std::string::npos) {
            code = code.substr(0, ampersand_pos);
        }

        CURL* tcurl = curl_easy_init();
        if (!tcurl)
            return std::nullopt;
        curl_easy_impersonate(tcurl, "chrome116", 1);
        curl_easy_setopt(tcurl, CURLOPT_ACCEPT_ENCODING, "");

        std::string response_body;
        std::string post_fields = "grant_type=authorization_code&client_id=" + client_id;
        post_fields += "&code=" + code;
        post_fields += "&code_verifier=" + code_verifier;
        post_fields += "&redirect_uri=" + redirect_uri;

        struct curl_slist* headers = nullptr;
        headers = curl_slist_append(headers, "Content-Type: application/x-www-form-urlencoded");
        headers = curl_slist_append(headers, "Accept: application/json");

        curl_easy_setopt(tcurl, CURLOPT_URL, "https://account.jagex.com/oauth2/token");
        curl_easy_setopt(tcurl, CURLOPT_POSTFIELDS, post_fields.c_str());
        curl_easy_setopt(tcurl, CURLOPT_HTTPHEADER, headers);
        curl_easy_setopt(tcurl, CURLOPT_WRITEFUNCTION, rlshim_write_callback);
        curl_easy_setopt(tcurl, CURLOPT_WRITEDATA, &response_body);

        CURLcode res = curl_easy_perform(tcurl);
        long http_code = 0;
        curl_easy_getinfo(tcurl, CURLINFO_RESPONSE_CODE, &http_code);
        curl_slist_free_all(headers);
        curl_easy_cleanup(tcurl);

        if (res != CURLE_OK || http_code != 200) {
            logger::error("failed to exchange code for tokens. HTTP {}: {}", http_code, response_body);
            return std::nullopt;
        }

        try {
            auto auth_json = json::parse(response_body);
            auth_session creds;
            creds.tokens.access_token = auth_json["access_token"].get<std::string>();
            creds.tokens.id_token = auth_json["id_token"].get<std::string>();
            creds.tokens.refresh_token = auth_json["refresh_token"].get<std::string>();

            auto consent_id_token_opt = get_consent_id_token(creds.tokens.id_token, use_gui);
            if (!consent_id_token_opt) {
                logger::error("failed to get consent id_token");
                return std::nullopt;
            }
            auto sid = get_session_id(*consent_id_token_opt);
            if (!sid) {
                logger::error("failed to fetch initial session id");
                return std::nullopt;
            }
            creds.session_id = *sid;

            return creds;
        } catch (const json::parse_error& e) {
            logger::error("failed to parse token exchange response: {}", e.what());
            return std::nullopt;
        }
    }

    namespace keyring {
        const SecretSchema* get_schema() {
            static const SecretSchema schema = {
                .name = "life.srp.rlshim",
                .flags = SECRET_SCHEMA_NONE,
                .attributes = {{"account", SECRET_SCHEMA_ATTRIBUTE_STRING}, {nullptr, SECRET_SCHEMA_ATTRIBUTE_STRING}}};

            return &schema;
        }

        bool store(const std::string& key, const std::string& value) {
            GError* error = nullptr;

            secret_password_store_sync(get_schema(), SECRET_COLLECTION_DEFAULT, "rlshim credentials", value.c_str(),
                                       nullptr, &error, "account", key.c_str(), nullptr);

            if (error != nullptr) {
                logger::error("failed to store secret: {}", error->message);
                g_error_free(error);
                return false;
            }

            return true;
        }

        std::optional<std::string> lookup(const std::string& key) {
            GError* error = nullptr;

            gchar* value = secret_password_lookup_sync(get_schema(), nullptr, &error, "account", key.c_str(), nullptr);

            if (error != nullptr) {
                logger::error("failed to lookup secret: {}", error->message);
                g_error_free(error);
                return std::nullopt;
            }

            if (value == nullptr) {
                return std::nullopt;
            }

            std::string str(value);
            secret_password_free(value);
            return str;
        }

        bool clear(const std::string& key) {
            GError* error = nullptr;

            secret_password_clear_sync(get_schema(), nullptr, &error, "account", key.c_str(), nullptr);

            if (error != nullptr) {
                logger::error("failed to clear secret: {}", error->message);
                g_error_free(error);
                return false;
            }

            return true;
        }
    }  // namespace keyring
}  // namespace auth