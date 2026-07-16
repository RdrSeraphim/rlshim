#include <curl/curl.h>
#include <openssl/evp.h>
#include <array>
#include <cstdio>
#include <filesystem>
#include <memory>
#include <string>

#include "curl.h"
#include "logger.h"
#include "parse.h"

#ifdef _WIN32
#define popen _popen
#define pclose _pclose
#endif

namespace runelite {
    namespace {
        // Resolve ~/.runelite lazily so a missing $HOME surfaces as a handled
        // error (via establish_home) rather than a crash during static init.
        std::string runelite_dir() {
            const char* home = std::getenv("HOME");
            if (!home || *home == '\0')
                return {};
            return std::string(home) + "/.runelite";
        }
    }  // namespace

    std::string jar_path() {
        std::string dir = runelite_dir();
        if (dir.empty())
            return {};
        return dir + "/runelite.jar";
    }

    namespace {
        constexpr const char* RELEASE_API_URL =
            "https://api.github.com/repos/runelite/launcher/releases/latest";

        // Fetches a URL into `out`. Returns false on transport error or non-2xx.
        bool fetch_url_to_string(const std::string& url, std::string& out) {
            CURL* curl = curl_easy_init();
            if (!curl)
                return false;

            struct curl_slist* headers = nullptr;
            headers = curl_slist_append(headers, "Accept: application/vnd.github+json");
            headers = curl_slist_append(headers, "User-Agent: rlshim");

            curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
            curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
            curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
            curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, rlshim_write_callback);
            curl_easy_setopt(curl, CURLOPT_WRITEDATA, &out);

            CURLcode res = curl_easy_perform(curl);
            long http_code = 0;
            curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &http_code);
            curl_slist_free_all(headers);
            curl_easy_cleanup(curl);

            if (res != CURLE_OK) {
                logger::error("request to {} failed: {}", url, curl_easy_strerror(res));
                return false;
            }
            if (http_code < 200 || http_code >= 300) {
                logger::error("request to {} returned HTTP {}", url, http_code);
                return false;
            }
            return true;
        }

        bool download_url_to_file(const std::string& url, const std::string& path) {
            CURL* curl = curl_easy_init();
            if (!curl)
                return false;

            FILE* fp = fopen(path.c_str(), "wb");
            if (!fp) {
                logger::error("failed to open {} for writing", path);
                curl_easy_cleanup(curl);
                return false;
            }

            curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
            curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
            curl_easy_setopt(curl, CURLOPT_WRITEDATA, fp);

            CURLcode res = curl_easy_perform(curl);
            long http_code = 0;
            curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &http_code);
            fclose(fp);
            curl_easy_cleanup(curl);

            if (res != CURLE_OK || http_code < 200 || http_code >= 300) {
                logger::error("failed to download {} (HTTP {}): {}", url, http_code, curl_easy_strerror(res));
                std::filesystem::remove(path);
                return false;
            }
            return true;
        }

        // Returns the lowercase hex SHA-256 of the file at `path`, or empty on error.
        std::string sha256_file_hex(const std::string& path) {
            std::unique_ptr<FILE, int (*)(FILE*)> fp(fopen(path.c_str(), "rb"), fclose);
            if (!fp)
                return {};

            std::unique_ptr<EVP_MD_CTX, void (*)(EVP_MD_CTX*)> ctx(EVP_MD_CTX_new(), EVP_MD_CTX_free);
            if (!ctx || !EVP_DigestInit_ex(ctx.get(), EVP_sha256(), nullptr))
                return {};

            std::array<unsigned char, 65536> buf;
            size_t n;
            while ((n = fread(buf.data(), 1, buf.size(), fp.get())) > 0) {
                if (!EVP_DigestUpdate(ctx.get(), buf.data(), n))
                    return {};
            }
            if (ferror(fp.get()))
                return {};

            unsigned char hash[EVP_MAX_MD_SIZE];
            unsigned int hash_len = 0;
            if (!EVP_DigestFinal_ex(ctx.get(), hash, &hash_len))
                return {};

            static const char* hex = "0123456789abcdef";
            std::string out;
            out.reserve(hash_len * 2);
            for (unsigned int i = 0; i < hash_len; ++i) {
                out.push_back(hex[hash[i] >> 4]);
                out.push_back(hex[hash[i] & 0x0F]);
            }
            return out;
        }
    }  // namespace

    bool establish_jar() {
        std::string path = jar_path();
        if (path.empty()) {
            logger::error("$HOME is not set; cannot locate ~/.runelite");
            return false;
        }

        if (std::filesystem::exists(path))
            return true;

        // Resolve the latest release's RuneLite.jar asset and its published
        // SHA-256 via the GitHub API, so we can verify the bytes we execute
        // rather than trusting the download transport alone.
        std::string release_json;
        if (!fetch_url_to_string(RELEASE_API_URL, release_json)) {
            logger::error("failed to query the RuneLite release metadata");
            return false;
        }

        auto asset = parse::extract_release_asset(release_json, "RuneLite.jar");
        if (!asset) {
            logger::error("could not find RuneLite.jar (or its checksum) in the release metadata");
            return false;
        }

        if (!download_url_to_file(asset->download_url, path)) {
            logger::error("failed to download runelite.jar");
            return false;
        }

        std::string actual = sha256_file_hex(path);
        if (actual.empty()) {
            logger::error("failed to compute checksum of downloaded runelite.jar");
            std::filesystem::remove(path);
            return false;
        }

        if (actual != asset->sha256) {
            logger::error("runelite.jar checksum mismatch! expected {}, got {}. refusing to run it.", asset->sha256,
                          actual);
            std::filesystem::remove(path);
            return false;
        }

        logger::info("runelite.jar downloaded and checksum verified ✔");
        return true;
    }

    bool is_valid_java_installed() {
        std::array<char, 128> buf;
        std::string result;

        std::unique_ptr<FILE, int (*)(FILE*)> pipe(popen("java -version 2>&1", "r"), pclose);

        if (!pipe) {
            logger::error("failed to execute java -version");
            return false;
        }

        while (fgets(buf.data(), buf.size(), pipe.get()) != nullptr) {
            result += buf.data();
        }

        if (!parse::is_java_version_sufficient(result)) {
            logger::error("java 11+ is required; `java -version` reported: {}", result);
            return false;
        }

        return true;
    }

    bool establish_home() {
        std::string dir = runelite_dir();
        if (dir.empty()) {
            logger::error("$HOME is not set; cannot locate or create ~/.runelite");
            return false;
        }
        if (std::filesystem::exists(dir))
            return true;
        return std::filesystem::create_directories(dir);
    }
}  // namespace runelite