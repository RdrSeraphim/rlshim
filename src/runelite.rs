use crate::curl_ffi;
use crate::parse;
use crate::{log_error, log_info};
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const RELEASE_API_URL: &str = "https://api.github.com/repos/runelite/launcher/releases/latest";

fn runelite_dir() -> Option<PathBuf> {
    let home = env::var("HOME").ok().filter(|h| !h.is_empty())?;
    Some(PathBuf::from(home).join(".runelite"))
}

pub fn jar_path() -> String {
    runelite_dir()
        .map(|d| d.join("runelite.jar").to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn characters_path() -> String {
    runelite_dir()
        .map(|d| d.join("rlshim_char").to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn fetch_url_to_string(url: &str) -> Option<String> {
    let headers = ["Accept: application/vnd.github+json", "User-Agent: rlshim"];
    match curl_ffi::perform_request(url, None, &headers, false, true) {
        Ok(resp) if (200..300).contains(&resp.status) => Some(resp.body),
        Ok(resp) => {
            log_error!("request to {} returned HTTP {}", url, resp.status);
            None
        }
        Err(err) => {
            log_error!("request to {} failed: {}", url, err);
            None
        }
    }
}

fn download_url_to_file(url: &str, path: &Path) -> bool {
    match curl_ffi::download_to_file(url, path) {
        Ok(status) if (200..300).contains(&status) => true,
        Ok(status) => {
            log_error!("failed to download {} (HTTP {})", url, status);
            let _ = fs::remove_file(path);
            false
        }
        Err(err) => {
            log_error!("failed to download {}: {}", url, err);
            let _ = fs::remove_file(path);
            false
        }
    }
}

fn sha256_file_hex(path: &Path) -> String {
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(_) => return String::new(),
        }
    }
    hex::encode(hasher.finalize())
}

pub fn read_saved_character() -> String {
    let path = characters_path();
    if path.is_empty() {
        log_error!("$HOME is not set; cannot locate ~/.runelite");
        return String::new();
    }

    let Ok(file) = File::open(&path) else {
        return String::new();
    };
    let mut reader = BufReader::new(file);
    let mut name = String::new();
    let _ = reader.read_line(&mut name);
    name.trim_end_matches(&['\r', '\n'][..]).to_string()
}

pub fn save_character(display_name: &str) -> bool {
    let path = characters_path();
    if path.is_empty() {
        log_error!("$HOME is not set; cannot locate ~/.runelite");
        return false;
    }

    let Ok(mut file) = File::create(&path) else {
        log_error!("failed to open {} for writing", path);
        return false;
    };

    if writeln!(file, "{display_name}").is_err() {
        log_error!("failed to save character to {}", path);
        return false;
    }

    true
}

pub fn establish_jar() -> bool {
    let path_str = jar_path();
    if path_str.is_empty() {
        log_error!("$HOME is not set; cannot locate ~/.runelite");
        return false;
    }
    let path = Path::new(&path_str);
    if path.exists() {
        return true;
    }

    let Some(release_json) = fetch_url_to_string(RELEASE_API_URL) else {
        log_error!("failed to query the RuneLite release metadata");
        return false;
    };

    let Some(asset) = parse::extract_release_asset(&release_json, "RuneLite.jar") else {
        log_error!("could not find RuneLite.jar (or its checksum) in the release metadata");
        return false;
    };

    if !download_url_to_file(&asset.download_url, path) {
        log_error!("failed to download runelite.jar");
        return false;
    }

    let actual = sha256_file_hex(path);
    if actual.is_empty() {
        log_error!("failed to compute checksum of downloaded runelite.jar");
        let _ = fs::remove_file(path);
        return false;
    }

    if actual != asset.sha256 {
        log_error!(
            "runelite.jar checksum mismatch! expected {}, got {}. refusing to run it.",
            asset.sha256,
            actual
        );
        let _ = fs::remove_file(path);
        return false;
    }

    log_info!("runelite.jar downloaded and checksum verified ✔");
    true
}

pub fn is_valid_java_installed() -> bool {
    let output = match Command::new("java").arg("-version").output() {
        Ok(out) => out,
        Err(_) => {
            log_error!("failed to execute java -version");
            return false;
        }
    };

    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    if !parse::is_java_version_sufficient(&combined) {
        log_error!(
            "java 11+ is required; `java -version` reported: {}",
            combined.trim()
        );
        return false;
    }

    true
}

pub fn establish_home() -> bool {
    let Some(dir) = runelite_dir() else {
        log_error!("$HOME is not set; cannot locate or create ~/.runelite");
        return false;
    };
    if dir.exists() {
        return true;
    }
    fs::create_dir_all(dir).is_ok()
}
