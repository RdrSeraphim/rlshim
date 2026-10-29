use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const CURL_IMP_VERSION: &str = "1.5.6";

fn main() {
    // Only link native app libraries on Linux targets
    println!("cargo:rerun-if-changed=build.rs");

    // 1. Link libsecret-1 and glib/gobject via pkg-config
    pkg_config::Config::new()
        .atleast_version("0.18")
        .probe("libsecret-1")
        .expect("Failed to find libsecret-1 via pkg-config");

    // 2. Download & link static libcurl-impersonate (includes BoringSSL, nghttp2, brotli, zstd)
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "x86_64".to_string());
    let rlshim_arch = if target_arch == "aarch64" || target_arch == "arm64" {
        "aarch64"
    } else {
        "x86_64"
    };

    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_else(|_| "gnu".to_string());
    let rlshim_libc = if target_env.contains("musl") {
        "musl"
    } else {
        "gnu"
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    // Share curl-impersonate across profiles/hashes under target/curl-impersonate-<arch>-linux-<libc>
    let target_root = out_dir
        .ancestors()
        .nth(4)
        .map(PathBuf::from)
        .unwrap_or_else(|| out_dir.clone());
    let curl_imp_dir = target_root.join(format!("curl-impersonate-{rlshim_arch}-linux-{rlshim_libc}"));
    let extracted_dir = curl_imp_dir.join("extracted");
    let static_lib = extracted_dir.join("libcurl-impersonate.a");

    if !static_lib.exists() {
        fs::create_dir_all(&extracted_dir).expect("Failed to create curl-impersonate directory");
        let bundled_archive = PathBuf::from("_bundled/libcurl-impersonate.tar.gz");
        let archive_path = if bundled_archive.exists() {
            bundled_archive
        } else {
            let downloaded = curl_imp_dir.join("libcurl-impersonate.tar.gz");
            let url = format!(
                "https://github.com/lexiforest/curl-impersonate/releases/download/v{CURL_IMP_VERSION}/libcurl-impersonate-v{CURL_IMP_VERSION}.{rlshim_arch}-linux-{rlshim_libc}.tar.gz"
            );

            let status = Command::new("curl")
                .args(["-fSL", &url, "-o"])
                .arg(&downloaded)
                .status()
                .expect("Failed to execute `curl` to download libcurl-impersonate");
            assert!(status.success(), "Failed to download {url}");
            downloaded
        };

        let tar_status = Command::new("tar")
            .args(["-xzf"])
            .arg(&archive_path)
            .arg("--no-same-owner")
            .arg("-C")
            .arg(&extracted_dir)
            .status()
            .expect("Failed to extract libcurl-impersonate archive");
        assert!(
            tar_status.success(),
            "Failed to extract libcurl-impersonate.tar.gz"
        );
    }

    println!("cargo:rustc-link-search=native={}", extracted_dir.display());
    println!("cargo:rustc-link-lib=static=curl-impersonate");
    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=dl");
    println!("cargo:rustc-link-lib=dylib=m");
    // BoringSSL inside libcurl-impersonate uses C++ runtime symbols
    println!("cargo:rustc-link-lib=dylib=stdc++");
}
