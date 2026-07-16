#ifndef RLSHIM_PARSE_H
#define RLSHIM_PARSE_H

#include <optional>
#include <string>
#include <vector>

namespace parse {
    // Extracts the value of a query/fragment parameter (e.g. "code", "id_token")
    // from a redirect URL. Handles both '?query' and '#fragment' styles, values
    // terminated by '&' or end-of-string, and avoids matching a key that is only
    // a substring of a longer key (e.g. "code" must not match "auth_code").
    // Returns the raw (still percent-encoded) value, or nullopt if not present.
    std::optional<std::string> extract_query_param(const std::string& url, const std::string& key);

    // Given the raw output of `java -version` (which is emitted on stderr),
    // returns true iff the reported major version is >= 11.
    bool is_java_version_sufficient(const std::string& version_output);

    // RFC 4648 §5 base64url encoding, no padding.
    std::string base64_url_encode(const std::vector<unsigned char>& input);

    struct release_asset {
        std::string download_url;
        std::string sha256;  // lowercase hex, no "sha256:" prefix
    };

    // Given the body of a GitHub "get release" API response, locate the asset
    // whose name equals `asset_name` and return its download URL and SHA-256
    // digest. Returns nullopt if the JSON is malformed, the asset is absent, or
    // the asset carries no sha256 digest. The "sha256:" prefix GitHub prepends
    // to the digest is stripped.
    std::optional<release_asset> extract_release_asset(const std::string& release_json,
                                                       const std::string& asset_name);
}  // namespace parse

#endif  // RLSHIM_PARSE_H
