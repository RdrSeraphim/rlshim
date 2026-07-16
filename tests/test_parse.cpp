#define DOCTEST_CONFIG_IMPLEMENT_WITH_MAIN
#include <doctest/doctest.h>

#include "rlshim/parse.h"

using parse::base64_url_encode;
using parse::extract_query_param;
using parse::extract_release_asset;
using parse::is_java_version_sufficient;

namespace {
    // A trimmed-down but structurally faithful GitHub "get release" response,
    // with two assets so we exercise name-matching.
    const char* kReleaseJson = R"({
        "tag_name": "2.7.7",
        "assets": [
            {
                "name": "RuneLite.AppImage",
                "digest": "sha256:85a0d34abef6c08561018c3600c94db113125d69318fe71fc1fe4a7ea2d49c6c",
                "browser_download_url": "https://example.invalid/RuneLite.AppImage"
            },
            {
                "name": "RuneLite.jar",
                "digest": "sha256:a7ee00f0fb1133087c1f6b5219b47dc0d3eab2de7341409f6eed240db3d47855",
                "browser_download_url": "https://example.invalid/RuneLite.jar"
            }
        ]
    })";
}  // namespace

TEST_CASE("extract_query_param: basic query and fragment") {
    CHECK(extract_query_param("http://localhost/?code=abc123", "code") == "abc123");
    CHECK(extract_query_param("http://localhost/#code=abc123", "code") == "abc123");
    CHECK(extract_query_param("http://localhost/#id_token=xyz", "id_token") == "xyz");
}

TEST_CASE("extract_query_param: value terminated by ampersand vs end-of-string") {
    CHECK(extract_query_param("http://localhost/?code=abc&state=def", "code") == "abc");
    CHECK(extract_query_param("http://localhost/?state=def&code=abc", "code") == "abc");
    CHECK(extract_query_param("http://localhost/?code=abc", "code") == "abc");
}

TEST_CASE("extract_query_param: missing key") {
    CHECK(extract_query_param("http://localhost/?state=def", "code") == std::nullopt);
    CHECK(extract_query_param("", "code") == std::nullopt);
}

TEST_CASE("extract_query_param: does not match a key that is a substring of a longer key") {
    // The regression this refactor fixes: naive find("code=") matched "auth_code=".
    CHECK(extract_query_param("http://localhost/?auth_code=WRONG&code=RIGHT", "code") == "RIGHT");
    CHECK(extract_query_param("http://localhost/?auth_code=only", "code") == std::nullopt);
    CHECK(extract_query_param("http://localhost/#other_id_token=WRONG&id_token=RIGHT", "id_token") == "RIGHT");
}

TEST_CASE("extract_query_param: empty value") {
    CHECK(extract_query_param("http://localhost/?code=&state=x", "code") == "");
    CHECK(extract_query_param("http://localhost/?code=", "code") == "");
}

TEST_CASE("extract_query_param: value left percent-encoded (caller decodes)") {
    CHECK(extract_query_param("http://localhost/?code=a%2Bb%2Fc", "code") == "a%2Bb%2Fc");
}

TEST_CASE("is_java_version_sufficient: Java 8 and earlier rejected") {
    CHECK_FALSE(is_java_version_sufficient("openjdk version \"1.8.0_402\"\nOpenJDK Runtime Environment"));
    CHECK_FALSE(is_java_version_sufficient("java version \"1.7.0_80\""));
}

TEST_CASE("is_java_version_sufficient: Java 11+ accepted") {
    CHECK(is_java_version_sufficient("openjdk version \"11.0.21\" 2023-10-17"));
    CHECK(is_java_version_sufficient("openjdk version \"17.0.9\""));
    CHECK(is_java_version_sufficient("openjdk version \"21\" 2023-09-19"));
    CHECK(is_java_version_sufficient("java version \"21.0.1\" 2023-10-17 LTS"));
}

TEST_CASE("is_java_version_sufficient: Java 9 and 10 rejected") {
    CHECK_FALSE(is_java_version_sufficient("openjdk version \"9.0.4\""));
    CHECK_FALSE(is_java_version_sufficient("openjdk version \"10.0.2\""));
}

TEST_CASE("is_java_version_sufficient: vendor banners (Temurin/GraalVM)") {
    CHECK(is_java_version_sufficient(
        "openjdk version \"17.0.9\" 2023-10-17\nOpenJDK Runtime Environment Temurin-17.0.9+9"));
    CHECK(is_java_version_sufficient(
        "openjdk version \"21.0.1\" 2023-10-17\nGraalVM Runtime Environment GraalVM CE 21.0.1+12.1"));
}

TEST_CASE("is_java_version_sufficient: garbage / no java present") {
    CHECK_FALSE(is_java_version_sufficient(""));
    CHECK_FALSE(is_java_version_sufficient("java: command not found"));
    CHECK_FALSE(is_java_version_sufficient("some totally unrelated output"));
}

TEST_CASE("base64_url_encode: RFC 4648 vectors, url-safe alphabet, no padding") {
    auto enc = [](const std::string& s) {
        return base64_url_encode(std::vector<unsigned char>(s.begin(), s.end()));
    };
    CHECK(enc("") == "");
    CHECK(enc("f") == "Zg");
    CHECK(enc("fo") == "Zm8");
    CHECK(enc("foo") == "Zm9v");
    CHECK(enc("foob") == "Zm9vYg");
    CHECK(enc("fooba") == "Zm9vYmE");
    CHECK(enc("foobar") == "Zm9vYmFy");
}

TEST_CASE("extract_release_asset: picks the correct asset and strips sha256: prefix") {
    auto asset = extract_release_asset(kReleaseJson, "RuneLite.jar");
    REQUIRE(asset.has_value());
    CHECK(asset->download_url == "https://example.invalid/RuneLite.jar");
    CHECK(asset->sha256 == "a7ee00f0fb1133087c1f6b5219b47dc0d3eab2de7341409f6eed240db3d47855");
    // No "sha256:" prefix should leak through.
    CHECK(asset->sha256.find(':') == std::string::npos);
}

TEST_CASE("extract_release_asset: absent asset name") {
    CHECK(extract_release_asset(kReleaseJson, "RuneLite.dmg") == std::nullopt);
}

TEST_CASE("extract_release_asset: malformed / non-JSON input") {
    CHECK(extract_release_asset("", "RuneLite.jar") == std::nullopt);
    CHECK(extract_release_asset("not json", "RuneLite.jar") == std::nullopt);
    CHECK(extract_release_asset("{}", "RuneLite.jar") == std::nullopt);
    CHECK(extract_release_asset(R"({"assets": "wrong type"})", "RuneLite.jar") == std::nullopt);
}

TEST_CASE("extract_release_asset: asset present but missing digest is rejected") {
    const char* no_digest = R"({
        "assets": [
            {"name": "RuneLite.jar", "browser_download_url": "https://example.invalid/RuneLite.jar"}
        ]
    })";
    CHECK(extract_release_asset(no_digest, "RuneLite.jar") == std::nullopt);
}

TEST_CASE("extract_release_asset: non-sha256 digest algorithm is rejected") {
    const char* md5_digest = R"({
        "assets": [
            {"name": "RuneLite.jar", "digest": "md5:deadbeef", "browser_download_url": "https://example.invalid/RuneLite.jar"}
        ]
    })";
    CHECK(extract_release_asset(md5_digest, "RuneLite.jar") == std::nullopt);
}

TEST_CASE("base64_url_encode: emits '-' and '_' rather than '+' and '/'") {
    // Bytes chosen to force the 62nd/63rd alphabet entries.
    std::vector<unsigned char> bytes = {0xfb, 0xff, 0xbf};
    std::string out = base64_url_encode(bytes);
    CHECK(out.find('+') == std::string::npos);
    CHECK(out.find('/') == std::string::npos);
    CHECK(out.find('=') == std::string::npos);
    CHECK(out.find('-') != std::string::npos);
    CHECK(out.find('_') != std::string::npos);
}
