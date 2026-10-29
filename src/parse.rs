use regex::Regex;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub download_url: String,
    pub sha256: String, // lowercase hex, no "sha256:" prefix
}

/// Extracts the value of a query/fragment parameter (e.g. "code", "id_token")
/// from a redirect URL. Handles both '?query' and '#fragment' styles, values
/// terminated by '&' or end-of-string, and avoids matching a key that is only
/// a substring of a longer key (e.g. "code" must not match "auth_code").
pub fn extract_query_param(url: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=");
    let bytes = url.as_bytes();
    let mut search_from = 0;

    while search_from <= url.len() {
        let rel_pos = url[search_from..].find(&needle)?;
        let pos = search_from + rel_pos;

        let at_boundary = if pos == 0 {
            true
        } else {
            let prev = bytes[pos - 1];
            prev == b'?' || prev == b'#' || prev == b'&'
        };

        if at_boundary {
            let start = pos + needle.len();
            let end = url[start..]
                .find('&')
                .map(|offset| start + offset)
                .unwrap_or_else(|| url.len());
            return Some(url[start..end].to_string());
        }

        search_from = pos + needle.len();
    }

    None
}

/// Given the raw output of `java -version`, returns true iff the reported
/// major version is >= 11.
pub fn is_java_version_sufficient(version_output: &str) -> bool {
    let Ok(version_regex) = Regex::new(r#"version\s+"([0-9\._\-]+)""#) else {
        return false;
    };

    let Some(caps) = version_regex.captures(version_output) else {
        return false;
    };

    let version_str = &caps[1];

    // Java 8 and earlier report "1.8.x" — always insufficient.
    if version_str.starts_with("1.") {
        return false;
    }

    // Java 9+ reports "11.0.x", "17.0.x", "21", etc.
    let major_str = version_str.split('.').next().unwrap_or(version_str);
    major_str.parse::<i32>().map(|v| v >= 11).unwrap_or(false)
}

/// RFC 4648 §5 base64url encoding, no padding.
pub fn base64_url_encode(input: &[u8]) -> String {
    const B64_TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut val: i32 = 0;
    let mut valb: i32 = -6;

    for &c in input {
        val = (val << 8) + i32::from(c);
        valb += 8;
        while valb >= 0 {
            let idx = ((val >> valb) & 0x3F) as usize;
            out.push(B64_TABLE[idx] as char);
            valb -= 6;
        }
    }

    if valb > -6 {
        let idx = (((val << 8) >> (valb + 8)) & 0x3F) as usize;
        out.push(B64_TABLE[idx] as char);
    }

    out
}

/// Given the body of a GitHub "get release" API response, locate the asset
/// whose name equals `asset_name` and return its download URL and SHA-256
/// digest (with the "sha256:" prefix stripped).
pub fn extract_release_asset(release_json: &str, asset_name: &str) -> Option<ReleaseAsset> {
    let j: Value = serde_json::from_str(release_json).ok()?;
    let assets = j.get("assets")?.as_array()?;

    for asset in assets {
        if asset.get("name")?.as_str()? != asset_name {
            continue;
        }

        let download_url = asset
            .get("browser_download_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let sha256 = asset
            .get("digest")
            .and_then(Value::as_str)
            .and_then(|d| d.strip_prefix("sha256:"))
            .unwrap_or_default()
            .to_string();

        if download_url.is_empty() || sha256.is_empty() {
            return None;
        }

        return Some(ReleaseAsset {
            download_url,
            sha256,
        });
    }

    None
}

/// Returns the (unquoted, lowercased) value of `key` in an os-release
/// file, or None if absent.
fn os_release_value(os_release: &str, key: &str) -> Option<String> {
    for raw_line in os_release.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() != key {
            continue;
        }
        let mut val = v.trim();
        if val.len() >= 2
            && ((val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\'')))
        {
            val = &val[1..val.len() - 1];
        }
        return Some(val.to_ascii_lowercase());
    }
    None
}

/// Given the contents of an os-release(5) file, returns true iff it
/// describes a gaming/handheld-oriented distro that boots into a Steam
/// "gaming mode" session (SteamOS, Bazzite, ChimeraOS, HoloISO, ...).
/// Matches on ID, ID_LIKE and VARIANT_ID (e.g. Bazzite's "bazzite-deck").
pub fn is_gaming_os_release(os_release: &str) -> bool {
    const GAMING_IDS: &[&str] = &["steamos", "bazzite", "chimeraos", "holoiso", "steamfork"];

    let matches_gaming_id = |val: &str| {
        val.split_whitespace()
            .any(|token| GAMING_IDS.iter().any(|&id| id == token))
    };

    if let Some(id) = os_release_value(os_release, "ID") {
        if matches_gaming_id(&id) {
            return true;
        }
    }

    if let Some(id_like) = os_release_value(os_release, "ID_LIKE") {
        if matches_gaming_id(&id_like) {
            return true;
        }
    }

    // e.g. VARIANT_ID=bazzite-deck / bazzite-deck-gnome
    if let Some(variant) = os_release_value(os_release, "VARIANT_ID") {
        if variant.contains("deck") {
            return true;
        }
    }

    false
}

/// Given the contents of /sys/class/dmi/id/sys_vendor (or board_vendor),
/// returns true iff the hardware is made by Valve (Steam Deck, etc.).
pub fn is_valve_hardware_vendor(vendor: &str) -> bool {
    vendor.trim().eq_ignore_ascii_case("valve")
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELEASE_JSON: &str = r#"{
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
    }"#;

    #[test]
    fn extract_query_param_basic_query_and_fragment() {
        assert_eq!(
            extract_query_param("http://localhost/?code=abc123", "code").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            extract_query_param("http://localhost/#code=abc123", "code").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            extract_query_param("http://localhost/#id_token=xyz", "id_token").as_deref(),
            Some("xyz")
        );
    }

    #[test]
    fn extract_query_param_value_terminated_by_ampersand_vs_eos() {
        assert_eq!(
            extract_query_param("http://localhost/?code=abc&state=def", "code").as_deref(),
            Some("abc")
        );
        assert_eq!(
            extract_query_param("http://localhost/?state=def&code=abc", "code").as_deref(),
            Some("abc")
        );
        assert_eq!(
            extract_query_param("http://localhost/?code=abc", "code").as_deref(),
            Some("abc")
        );
    }

    #[test]
    fn extract_query_param_missing_key() {
        assert_eq!(
            extract_query_param("http://localhost/?state=def", "code"),
            None
        );
        assert_eq!(extract_query_param("", "code"), None);
    }

    #[test]
    fn extract_query_param_does_not_match_substring_key() {
        assert_eq!(
            extract_query_param("http://localhost/?auth_code=WRONG&code=RIGHT", "code").as_deref(),
            Some("RIGHT")
        );
        assert_eq!(
            extract_query_param("http://localhost/?auth_code=only", "code"),
            None
        );
        assert_eq!(
            extract_query_param(
                "http://localhost/#other_id_token=WRONG&id_token=RIGHT",
                "id_token"
            )
            .as_deref(),
            Some("RIGHT")
        );
    }

    #[test]
    fn extract_query_param_empty_value() {
        assert_eq!(
            extract_query_param("http://localhost/?code=&state=x", "code").as_deref(),
            Some("")
        );
        assert_eq!(
            extract_query_param("http://localhost/?code=", "code").as_deref(),
            Some("")
        );
    }

    #[test]
    fn extract_query_param_value_left_percent_encoded() {
        assert_eq!(
            extract_query_param("http://localhost/?code=a%2Bb%2Fc", "code").as_deref(),
            Some("a%2Bb%2Fc")
        );
    }

    #[test]
    fn is_java_version_sufficient_java_8_and_earlier_rejected() {
        assert!(!is_java_version_sufficient(
            "openjdk version \"1.8.0_402\"\nOpenJDK Runtime Environment"
        ));
        assert!(!is_java_version_sufficient("java version \"1.7.0_80\""));
    }

    #[test]
    fn is_java_version_sufficient_java_11_plus_accepted() {
        assert!(is_java_version_sufficient(
            "openjdk version \"11.0.21\" 2023-10-17"
        ));
        assert!(is_java_version_sufficient("openjdk version \"17.0.9\""));
        assert!(is_java_version_sufficient(
            "openjdk version \"21\" 2023-09-19"
        ));
        assert!(is_java_version_sufficient(
            "java version \"21.0.1\" 2023-10-17 LTS"
        ));
    }

    #[test]
    fn is_java_version_sufficient_java_9_and_10_rejected() {
        assert!(!is_java_version_sufficient("openjdk version \"9.0.4\""));
        assert!(!is_java_version_sufficient("openjdk version \"10.0.2\""));
    }

    #[test]
    fn is_java_version_sufficient_vendor_banners() {
        assert!(is_java_version_sufficient(
            "openjdk version \"17.0.9\" 2023-10-17\nOpenJDK Runtime Environment Temurin-17.0.9+9"
        ));
        assert!(is_java_version_sufficient(
            "openjdk version \"21.0.1\" 2023-10-17\nGraalVM Runtime Environment GraalVM CE 21.0.1+12.1"
        ));
    }

    #[test]
    fn is_java_version_sufficient_garbage() {
        assert!(!is_java_version_sufficient(""));
        assert!(!is_java_version_sufficient("java: command not found"));
        assert!(!is_java_version_sufficient("some totally unrelated output"));
    }

    #[test]
    fn base64_url_encode_rfc4648_vectors() {
        let enc = |s: &str| base64_url_encode(s.as_bytes());
        assert_eq!(enc(""), "");
        assert_eq!(enc("f"), "Zg");
        assert_eq!(enc("fo"), "Zm8");
        assert_eq!(enc("foo"), "Zm9v");
        assert_eq!(enc("foob"), "Zm9vYg");
        assert_eq!(enc("fooba"), "Zm9vYmE");
        assert_eq!(enc("foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_url_encode_emits_url_safe_chars() {
        let bytes = [0xfb, 0xff, 0xbf];
        let out = base64_url_encode(&bytes);
        assert!(!out.contains('+'));
        assert!(!out.contains('/'));
        assert!(!out.contains('='));
        assert!(out.contains('-'));
        assert!(out.contains('_'));
    }

    #[test]
    fn extract_release_asset_picks_correct_asset_and_strips_prefix() {
        let asset =
            extract_release_asset(RELEASE_JSON, "RuneLite.jar").expect("asset should exist");
        assert_eq!(asset.download_url, "https://example.invalid/RuneLite.jar");
        assert_eq!(
            asset.sha256,
            "a7ee00f0fb1133087c1f6b5219b47dc0d3eab2de7341409f6eed240db3d47855"
        );
        assert!(!asset.sha256.contains(':'));
    }

    #[test]
    fn extract_release_asset_absent_and_malformed() {
        assert_eq!(extract_release_asset(RELEASE_JSON, "RuneLite.dmg"), None);
        assert_eq!(extract_release_asset("", "RuneLite.jar"), None);
        assert_eq!(extract_release_asset("not json", "RuneLite.jar"), None);
        assert_eq!(extract_release_asset("{}", "RuneLite.jar"), None);
        assert_eq!(
            extract_release_asset(r#"{"assets": "wrong type"}"#, "RuneLite.jar"),
            None
        );
    }

    #[test]
    fn extract_release_asset_missing_or_wrong_digest() {
        let no_digest = r#"{
            "assets": [
                {"name": "RuneLite.jar", "browser_download_url": "https://example.invalid/RuneLite.jar"}
            ]
        }"#;
        assert_eq!(extract_release_asset(no_digest, "RuneLite.jar"), None);

        let md5_digest = r#"{
            "assets": [
                {"name": "RuneLite.jar", "digest": "md5:deadbeef", "browser_download_url": "https://example.invalid/RuneLite.jar"}
            ]
        }"#;
        assert_eq!(extract_release_asset(md5_digest, "RuneLite.jar"), None);
    }

    #[test]
    fn is_gaming_os_release_steamos() {
        let steamos = "NAME=\"SteamOS\"\nPRETTY_NAME=\"SteamOS\"\nVERSION_CODENAME=holo\nID=steamos\nID_LIKE=arch\n";
        assert!(is_gaming_os_release(steamos));
    }

    #[test]
    fn is_gaming_os_release_bazzite() {
        let bazzite =
            "NAME=\"Bazzite\"\nID=bazzite\nID_LIKE=\"fedora\"\nVARIANT_ID=bazzite-nvidia\n";
        let bazzite_deck =
            "NAME=\"Bazzite\"\nID=bazzite\nID_LIKE=\"fedora\"\nVARIANT_ID=bazzite-deck\n";
        assert!(is_gaming_os_release(bazzite));
        assert!(is_gaming_os_release(bazzite_deck));
    }

    #[test]
    fn is_gaming_os_release_derivative_via_id_like_or_deck_variant() {
        assert!(is_gaming_os_release(
            "ID=somefork\nID_LIKE=\"steamos arch\"\n"
        ));
        assert!(is_gaming_os_release(
            "ID=nobara\nID_LIKE=\"fedora\"\nVARIANT_ID='steam-deck'\n"
        ));
        assert!(is_gaming_os_release("ID=ChimeraOS\n"));
    }

    #[test]
    fn is_gaming_os_release_ordinary_desktop_distros_are_not_matched() {
        assert!(!is_gaming_os_release("NAME=\"Arch Linux\"\nID=arch\n"));
        assert!(!is_gaming_os_release(
            "NAME=\"Fedora Linux\"\nID=fedora\nVARIANT_ID=workstation\n"
        ));
        assert!(!is_gaming_os_release("ID=ubuntu\nID_LIKE=debian\n"));
        assert!(!is_gaming_os_release("# ID=steamos\nID=debian\n"));
        assert!(!is_gaming_os_release(""));
    }

    #[test]
    fn is_valve_hardware_vendor_test() {
        assert!(is_valve_hardware_vendor("Valve\n"));
        assert!(is_valve_hardware_vendor("valve"));
        assert!(!is_valve_hardware_vendor(
            "Micro-Star International Co., Ltd.\n"
        ));
        assert!(!is_valve_hardware_vendor(""));
    }
}
