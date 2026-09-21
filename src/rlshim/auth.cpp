#include "auth.h"
#include "cli.h"
#include "curl.h"
#include "gui.h"
#include "logger.h"
#include "parse.h"

#include <curl/curl.h>
#include <libsecret/secret.h>
#include <cstddef>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <nlohmann/json.hpp>
#include <sys/stat.h>
#include <unistd.h>

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

    std::string generate_random_string(size_t length) {
        std::vector<unsigned char> bytes(length);
        RAND_bytes(bytes.data(), bytes.size());
        return parse::base64_url_encode(bytes).substr(0, length);
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
        return parse::base64_url_encode(hash_vec);
    }

    std::optional<std::string> get_consent_id_token(const std::string& first_id_token, bool dont_use_gui) {
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
        if (dont_use_gui) {
            pasted_url_opt =
                cli::prompt_for_url("initial login 2 electric boogaloo", instructions, url, url_placeholder);
        } else {
            pasted_url_opt = gui::prompt_for_url("rlshim - initial login 2 electric boogaloo", instructions,
                                                 instructions_pt_2, url, url_placeholder);
        }

        if (!pasted_url_opt) {
            logger::error("No URL provided");
            return std::nullopt;
        }

        auto id_token = parse::extract_query_param(*pasted_url_opt, "id_token");
        if (!id_token) {
            logger::error("no id_token found in the pasted URL");
            return std::nullopt;
        }

        return id_token;
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

    std::optional<auth_session> do_interactive_login(bool dont_use_gui) {
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
        if (dont_use_gui) {
            pasted_url_opt = cli::prompt_for_url("rlshim - initial login", instructions, auth_url, url_placeholder);
        } else {
            pasted_url_opt = gui::prompt_for_url("rlshim - initial login", instructions, instructions_pt_2, auth_url,
                                                 url_placeholder);
        }

        if (!pasted_url_opt) {
            logger::error("No URL provided");
            return std::nullopt;
        }

        auto code_opt = parse::extract_query_param(*pasted_url_opt, "code");
        if (!code_opt) {
            logger::error("Could not find 'code=' in the pasted URL");
            return std::nullopt;
        }
        std::string code = *code_opt;

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

            auto consent_id_token_opt = get_consent_id_token(creds.tokens.id_token, dont_use_gui);
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

    namespace vault {
        static const char MAGIC[8] = {'R', 'L', 'V', 'A', 'U', 'L', 'T', '1'};
        constexpr size_t IV_LEN = 12;
        constexpr size_t TAG_LEN = 16;
        constexpr size_t KEY_LEN = 32;

        static std::string get_vault_path() {
            const char* home = std::getenv("HOME");
            if (!home || *home == '\0') {
                return {};
            }
            std::filesystem::path dir = std::filesystem::path(home) / ".runelite";
            std::error_code ec;
            std::filesystem::create_directories(dir, ec);
            return (dir / "rlshim_vault.enc").string();
        }

        static std::string get_machine_entropy() {
            std::string id;
            std::ifstream file("/etc/machine-id");
            if (!file.is_open()) {
                file.open("/var/lib/dbus/machine-id");
            }
            if (file.is_open()) {
                file >> id;
            }
            if (id.empty()) {
                const char* hostname = std::getenv("HOSTNAME");
                id = hostname ? hostname : "rlshim-default-machine-seed";
            }
            uid_t uid = getuid();
            return id + ":" + std::to_string(uid);
        }

        static bool derive_key(unsigned char* key_out, size_t key_len) {
            std::string entropy = get_machine_entropy();
            static const unsigned char salt[] = "life.srp.rlshim:vault:v1";
            int res = PKCS5_PBKDF2_HMAC(
                entropy.c_str(), static_cast<int>(entropy.length()),
                salt, static_cast<int>(sizeof(salt) - 1),
                50000,
                EVP_sha256(),
                static_cast<int>(key_len), key_out);
            return res == 1;
        }

        static bool encrypt_data(const std::string& plaintext, std::vector<unsigned char>& out_data) {
            unsigned char key[KEY_LEN];
            if (!derive_key(key, KEY_LEN)) {
                return false;
            }

            unsigned char iv[IV_LEN];
            if (RAND_bytes(iv, static_cast<int>(IV_LEN)) != 1) {
                return false;
            }

            EVP_CIPHER_CTX* ctx = EVP_CIPHER_CTX_new();
            if (!ctx) return false;

            if (EVP_EncryptInit_ex(ctx, EVP_aes_256_gcm(), nullptr, nullptr, nullptr) != 1 ||
                EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_IVLEN, static_cast<int>(IV_LEN), nullptr) != 1 ||
                EVP_EncryptInit_ex(ctx, nullptr, nullptr, key, iv) != 1) {
                EVP_CIPHER_CTX_free(ctx);
                return false;
            }

            std::vector<unsigned char> ciphertext(plaintext.size() + 16);
            int out_len1 = 0;
            if (EVP_EncryptUpdate(ctx, ciphertext.data(), &out_len1,
                                   reinterpret_cast<const unsigned char*>(plaintext.data()),
                                   static_cast<int>(plaintext.size())) != 1) {
                EVP_CIPHER_CTX_free(ctx);
                return false;
            }

            int out_len2 = 0;
            if (EVP_EncryptFinal_ex(ctx, ciphertext.data() + out_len1, &out_len2) != 1) {
                EVP_CIPHER_CTX_free(ctx);
                return false;
            }

            unsigned char tag[TAG_LEN];
            if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_GET_TAG, static_cast<int>(TAG_LEN), tag) != 1) {
                EVP_CIPHER_CTX_free(ctx);
                return false;
            }

            EVP_CIPHER_CTX_free(ctx);

            size_t total_cipher_len = static_cast<size_t>(out_len1 + out_len2);

            out_data.clear();
            out_data.reserve(sizeof(MAGIC) + IV_LEN + TAG_LEN + total_cipher_len);
            out_data.insert(out_data.end(), MAGIC, MAGIC + sizeof(MAGIC));
            out_data.insert(out_data.end(), iv, iv + IV_LEN);
            out_data.insert(out_data.end(), tag, tag + TAG_LEN);
            out_data.insert(out_data.end(), ciphertext.begin(), ciphertext.begin() + total_cipher_len);

            return true;
        }

        static bool decrypt_data(const std::vector<unsigned char>& in_data, std::string& out_plaintext) {
            if (in_data.size() < sizeof(MAGIC) + IV_LEN + TAG_LEN) {
                return false;
            }

            if (std::memcmp(in_data.data(), MAGIC, sizeof(MAGIC)) != 0) {
                return false;
            }

            const unsigned char* iv = in_data.data() + sizeof(MAGIC);
            const unsigned char* tag = iv + IV_LEN;
            const unsigned char* ciphertext = tag + TAG_LEN;
            size_t cipher_len = in_data.size() - (sizeof(MAGIC) + IV_LEN + TAG_LEN);

            unsigned char key[KEY_LEN];
            if (!derive_key(key, KEY_LEN)) {
                return false;
            }

            EVP_CIPHER_CTX* ctx = EVP_CIPHER_CTX_new();
            if (!ctx) return false;

            if (EVP_DecryptInit_ex(ctx, EVP_aes_256_gcm(), nullptr, nullptr, nullptr) != 1 ||
                EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_IVLEN, static_cast<int>(IV_LEN), nullptr) != 1 ||
                EVP_DecryptInit_ex(ctx, nullptr, nullptr, key, iv) != 1) {
                EVP_CIPHER_CTX_free(ctx);
                return false;
            }

            std::vector<unsigned char> plaintext(cipher_len + 16);
            int out_len1 = 0;
            if (EVP_DecryptUpdate(ctx, plaintext.data(), &out_len1, ciphertext, static_cast<int>(cipher_len)) != 1) {
                EVP_CIPHER_CTX_free(ctx);
                return false;
            }

            if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_TAG, static_cast<int>(TAG_LEN),
                                   const_cast<unsigned char*>(tag)) != 1) {
                EVP_CIPHER_CTX_free(ctx);
                return false;
            }

            int out_len2 = 0;
            int ret = EVP_DecryptFinal_ex(ctx, plaintext.data() + out_len1, &out_len2);
            EVP_CIPHER_CTX_free(ctx);

            if (ret <= 0) {
                return false;
            }

            out_plaintext.assign(reinterpret_cast<char*>(plaintext.data()), static_cast<size_t>(out_len1 + out_len2));
            return true;
        }

        static std::optional<nlohmann::json> load_vault_json() {
            std::string path = get_vault_path();
            if (path.empty() || !std::filesystem::exists(path)) {
                return std::nullopt;
            }

            std::ifstream file(path, std::ios::binary);
            if (!file.is_open()) {
                return std::nullopt;
            }

            std::vector<unsigned char> data((std::istreambuf_iterator<char>(file)),
                                             std::istreambuf_iterator<char>());
            file.close();

            std::string plaintext;
            if (!decrypt_data(data, plaintext)) {
                logger::error("failed to decrypt credentials vault");
                return std::nullopt;
            }

            try {
                return nlohmann::json::parse(plaintext);
            } catch (const std::exception& e) {
                logger::error("failed to parse decrypted vault JSON: {}", e.what());
                return std::nullopt;
            }
        }

        static bool save_vault_json(const nlohmann::json& j) {
            std::string path = get_vault_path();
            if (path.empty()) return false;

            std::string plaintext = j.dump();
            std::vector<unsigned char> encrypted;
            if (!encrypt_data(plaintext, encrypted)) {
                logger::error("failed to encrypt credentials vault");
                return false;
            }

            std::ofstream file(path, std::ios::binary | std::ios::trunc);
            if (!file.is_open()) {
                logger::error("failed to open vault file for writing: {}", path);
                return false;
            }

            file.write(reinterpret_cast<const char*>(encrypted.data()), encrypted.size());
            file.close();

            chmod(path.c_str(), S_IRUSR | S_IWUSR);  // 0600: read/write by owner only
            return true;
        }

        static bool store(const std::string& key, const std::string& value) {
            auto opt = load_vault_json();
            nlohmann::json j = opt.value_or(nlohmann::json::object());
            j[key] = value;
            return save_vault_json(j);
        }

        static std::optional<std::string> lookup(const std::string& key) {
            auto opt = load_vault_json();
            if (!opt || !opt->contains(key)) {
                return std::nullopt;
            }
            return (*opt)[key].get<std::string>();
        }

        static bool clear(const std::string& key) {
            auto opt = load_vault_json();
            if (!opt) return true;
            nlohmann::json j = *opt;
            j.erase(key);
            if (j.empty()) {
                std::string path = get_vault_path();
                std::error_code ec;
                std::filesystem::remove(path, ec);
                return true;
            }
            return save_vault_json(j);
        }
    }  // namespace vault

    namespace keyring {
        enum class Backend {
            UNKNOWN,
            LIBSECRET,
            ENCRYPTED_FILE
        };

        static Backend s_backend = Backend::UNKNOWN;

        static bool try_libsecret(bool silent = false) {
            GError* error = nullptr;
            SecretService* service = secret_service_get_sync(SECRET_SERVICE_LOAD_COLLECTIONS, nullptr, &error);

            if (error != nullptr) {
                if (!silent) logger::error("secret service not available: {}", error->message);
                g_error_free(error);
                return false;
            }

            if (service == nullptr) {
                if (!silent) logger::error("no secret service provider found on D-Bus");
                return false;
            }

            SecretCollection* collection = secret_collection_for_alias_sync(service, SECRET_COLLECTION_DEFAULT,
                                                                            SECRET_COLLECTION_NONE, nullptr, &error);

            if (error != nullptr) {
                if (!silent) logger::error("failed to access default keyring collection: {}", error->message);
                g_error_free(error);
                g_object_unref(service);
                return false;
            }

            if (collection == nullptr) {
                if (!silent) logger::error("no default keyring collection found. is your keyring set up?");
                g_object_unref(service);
                return false;
            }

            bool locked = secret_collection_get_locked(collection);

            if (locked) {
                if (!silent) logger::info("default keyring collection is locked, attempting to unlock...");

                GList* to_unlock = g_list_append(nullptr, collection);
                GList* unlocked = nullptr;

                secret_service_unlock_sync(service, to_unlock, nullptr, &unlocked, &error);
                g_list_free(to_unlock);

                if (error != nullptr) {
                    if (!silent) logger::error("failed to unlock keyring: {}", error->message);
                    g_error_free(error);
                    g_object_unref(collection);
                    g_object_unref(service);
                    return false;
                }

                bool was_unlocked = unlocked != nullptr;
                g_list_free(unlocked);

                if (!was_unlocked) {
                    if (!silent) logger::error("keyring unlock was dismissed or failed");
                    g_object_unref(collection);
                    g_object_unref(service);
                    return false;
                }

                if (!silent) logger::info("keyring unlocked successfully");
            }

            g_object_unref(collection);
            g_object_unref(service);

            return true;
        }

        bool is_available() {
            if (s_backend == Backend::LIBSECRET) {
                return true;
            }
            if (s_backend == Backend::ENCRYPTED_FILE) {
                return true;
            }

            if (try_libsecret(true)) {
                s_backend = Backend::LIBSECRET;
                return true;
            }

            std::string vp = vault::get_vault_path();
            if (!vp.empty()) {
                logger::info("system keyring not available; using machine-encrypted credential vault ✔");
                s_backend = Backend::ENCRYPTED_FILE;
                return true;
            }

            return false;
        }

        const SecretSchema* get_schema() {
            static const SecretSchema schema = {
                .name = "life.srp.rlshim",
                .flags = SECRET_SCHEMA_NONE,
                .attributes = {{"account", SECRET_SCHEMA_ATTRIBUTE_STRING}, {nullptr, SECRET_SCHEMA_ATTRIBUTE_STRING}}};

            return &schema;
        }

        bool store(const std::string& key, const std::string& value) {
            if (s_backend == Backend::ENCRYPTED_FILE) {
                return vault::store(key, value);
            }

            // libsecret path
            GError* error = nullptr;
            secret_password_store_sync(get_schema(), SECRET_COLLECTION_DEFAULT, "rlshim credentials", value.c_str(),
                                       nullptr, &error, "account", key.c_str(), nullptr);

            if (error != nullptr) {
                logger::error("failed to store secret in keyring: {}", error->message);
                g_error_free(error);
                return false;
            }

            // Also mirror to encrypted vault as a backup for Game Mode / headless sessions
            vault::store(key, value);

            return true;
        }

        std::optional<std::string> lookup(const std::string& key) {
            if (s_backend == Backend::ENCRYPTED_FILE) {
                return vault::lookup(key);
            }

            // libsecret path
            GError* error = nullptr;
            gchar* value = secret_password_lookup_sync(get_schema(), nullptr, &error, "account", key.c_str(), nullptr);

            if (error != nullptr) {
                logger::error("failed to lookup secret from keyring: {}", error->message);
                g_error_free(error);
                return vault::lookup(key);
            }

            if (value == nullptr) {
                // Not found in libsecret; check if encrypted vault has it (e.g. created in Game Mode)
                return vault::lookup(key);
            }

            std::string str(value);
            secret_password_free(value);
            return str;
        }

        bool clear(const std::string& key) {
            bool ok = true;
            if (s_backend == Backend::LIBSECRET) {
                GError* error = nullptr;
                secret_password_clear_sync(get_schema(), nullptr, &error, "account", key.c_str(), nullptr);
                if (error != nullptr) {
                    logger::error("failed to clear secret from keyring: {}", error->message);
                    g_error_free(error);
                    ok = false;
                }
            }
            vault::clear(key);
            return ok;
        }
    }  // namespace keyring
}  // namespace auth
