#include "parse.h"

#include <regex>

#include <nlohmann/json.hpp>

namespace parse {
    std::optional<std::string> extract_query_param(const std::string& url, const std::string& key) {
        const std::string needle = key + "=";

        // Scan for occurrences of "key=" and accept one only when it sits at a
        // parameter boundary: start of the query/fragment, or immediately after
        // a '?', '#' or '&'. This prevents "code=" from matching "auth_code=".
        size_t search_from = 0;
        while (true) {
            size_t pos = url.find(needle, search_from);
            if (pos == std::string::npos) {
                return std::nullopt;
            }

            bool at_boundary = pos == 0;
            if (!at_boundary) {
                char prev = url[pos - 1];
                at_boundary = prev == '?' || prev == '#' || prev == '&';
            }

            if (at_boundary) {
                size_t start = pos + needle.length();
                size_t end = url.find('&', start);
                if (end == std::string::npos) {
                    end = url.length();
                }
                return url.substr(start, end - start);
            }

            search_from = pos + needle.length();
        }
    }

    bool is_java_version_sufficient(const std::string& version_output) {
        std::regex version_regex("version\\s+\"([0-9\\._\\-]+)\"");
        std::smatch match;
        if (!std::regex_search(version_output, match, version_regex)) {
            return false;
        }

        std::string version_str = match[1];

        // Java 8 and earlier report "1.8.x" — always insufficient.
        if (version_str.starts_with("1.")) {
            return false;
        }

        // Java 9+ reports "11.0.x", "17.0.x", "21", etc.
        size_t dot_pos = version_str.find('.');
        std::string major_str = dot_pos != std::string::npos ? version_str.substr(0, dot_pos) : version_str;

        try {
            return std::stoi(major_str) >= 11;
        } catch (...) {
            return false;
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

    std::optional<release_asset> extract_release_asset(const std::string& release_json,
                                                       const std::string& asset_name) {
        nlohmann::json j;
        try {
            j = nlohmann::json::parse(release_json);
        } catch (const std::exception&) {
            return std::nullopt;
        }

        if (!j.contains("assets") || !j["assets"].is_array()) {
            return std::nullopt;
        }

        for (const auto& asset : j["assets"]) {
            if (!asset.contains("name") || asset["name"] != asset_name) {
                continue;
            }

            release_asset result;
            if (asset.contains("browser_download_url") && asset["browser_download_url"].is_string()) {
                result.download_url = asset["browser_download_url"].get<std::string>();
            }
            if (asset.contains("digest") && asset["digest"].is_string()) {
                std::string digest = asset["digest"].get<std::string>();
                const std::string prefix = "sha256:";
                if (digest.starts_with(prefix)) {
                    result.sha256 = digest.substr(prefix.length());
                }
            }

            if (result.download_url.empty() || result.sha256.empty()) {
                return std::nullopt;
            }
            return result;
        }

        return std::nullopt;
    }
}  // namespace parse
