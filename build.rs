use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const CURL_IMP_VERSION: &str = "1.5.6";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RLSHIM_CURL_IMP_DIR");

    pkg_config::Config::new()
        .atleast_version("0.18")
        .probe("libsecret-1")
        .expect("Failed to find libsecret-1 via pkg-config");

    let extracted_dir = if let Ok(prebuilt_dir) = env::var("RLSHIM_CURL_IMP_DIR") {
        PathBuf::from(prebuilt_dir)
    } else {
        let target_arch =
            env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "x86_64".to_string());
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
        let curl_imp_dir = out_dir.join("curl-impersonate");
        let extracted = curl_imp_dir.join("extracted");
        let static_lib = extracted.join("libcurl-impersonate.a");

        if !static_lib.exists() {
            fs::create_dir_all(&extracted).expect("Failed to create curl-impersonate directory");
            let archive_path = curl_imp_dir.join("libcurl-impersonate.tar.gz");
            let url = format!(
                "https://github.com/lexiforest/curl-impersonate/releases/download/v{CURL_IMP_VERSION}/libcurl-impersonate-v{CURL_IMP_VERSION}.{rlshim_arch}-linux-{rlshim_libc}.tar.gz"
            );

            let status = Command::new("curl")
                .args(["-fSL", &url, "-o"])
                .arg(&archive_path)
                .status()
                .expect("Failed to execute `curl` to download libcurl-impersonate");
            assert!(status.success(), "Failed to download {url}");

            let tar_status = Command::new("tar")
                .args(["-xzf"])
                .arg(&archive_path)
                .arg("-C")
                .arg(&extracted)
                .status()
                .expect("Failed to extract libcurl-impersonate archive");
            assert!(
                tar_status.success(),
                "Failed to extract libcurl-impersonate.tar.gz"
            );
        }
        extracted
    };

    println!("cargo:rustc-link-search=native={}", extracted_dir.display());
    println!("cargo:rustc-link-lib=static=curl-impersonate");
    println!("cargo:rustc-link-lib=dylib=pthread");
    println!("cargo:rustc-link-lib=dylib=dl");
    println!("cargo:rustc-link-lib=dylib=m");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}
