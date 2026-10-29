use crate::cli;
use crate::curl_ffi;
use crate::gui;
use crate::parse;
use crate::{log_error, log_info};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct GameAccount {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "userHash")]
    pub user_hash: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, Zeroize)]
#[serde(default)]
pub struct OAuthTokens {
    pub access_token: String,
    pub id_token: String,
    pub refresh_token: String,
    pub sub: String,
    pub expiry: i64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthSession {
    pub accounts: Vec<GameAccount>,
    pub tokens: OAuthTokens,
    pub session_id: String,
}

impl AuthSession {
    pub fn zeroize_sensitive(&mut self) {
        self.session_id.zeroize();
        self.tokens.refresh_token.zeroize();
        self.tokens.id_token.zeroize();
        self.tokens.access_token.zeroize();
    }
}

pub fn read_session() -> Option<Vec<AuthSession>> {
    let stored_creds = keyring::lookup("jagex_auth")?;

    let creds_json: Value = match serde_json::from_str(&stored_creds) {
        Ok(v) => v,
        Err(e) => {
            log_error!("failed to parse creds from keyring: {}", e);
            return None;
        }
    };

    match serde_json::from_value::<Vec<AuthSession>>(creds_json) {
        Ok(creds) => Some(creds),
        Err(e) => {
            log_error!("failed to convert creds to auth_session: {}", e);
            None
        }
    }
}

pub fn save_session(creds: &AuthSession) -> bool {
    let creds_vec = vec![creds.clone()];
    let Ok(serialized) = serde_json::to_string(&creds_vec) else {
        log_error!("failed to serialize session credentials");
        return false;
    };

    if !keyring::store("jagex_auth", &serialized) {
        log_error!("failed to store creds in keyring");
        return false;
    }

    true
}

pub fn logout() -> bool {
    if !keyring::clear("jagex_auth") {
        return false;
    }
    log_info!("successfully logged out. credentials have been removed from the keyring.");
    true
}

pub fn refresh_oauth_token(refresh_token: &str) -> Option<AuthSession> {
    let post_fields = format!(
        "grant_type=refresh_token&client_id=com_jagex_auth_desktop_launcher&refresh_token={refresh_token}"
    );
    let headers = [
        "Content-Type: application/x-www-form-urlencoded",
        "Accept: application/json",
    ];

    let resp = match curl_ffi::perform_request(
        "https://account.jagex.com/oauth2/token",
        Some(&post_fields),
        &headers,
        true,
        false,
    ) {
        Ok(r) => r,
        Err(err) => {
            log_error!("failed to refresh token: {}", err);
            return None;
        }
    };

    if resp.status != 200 {
        log_error!("failed to refresh token with http {}", resp.status);
        return None;
    }

    let auth_json: Value = match serde_json::from_str(&resp.body) {
        Ok(v) => v,
        Err(e) => {
            log_error!("failed to parse token response: {}", e);
            return None;
        }
    };

    let access_token = auth_json.get("access_token")?.as_str()?.to_string();
    let id_token = auth_json.get("id_token")?.as_str()?.to_string();
    let new_refresh_token = auth_json.get("refresh_token")?.as_str()?.to_string();
    let expiry = auth_json
        .get("expires_in")
        .and_then(Value::as_i64)
        .unwrap_or(0);

    Some(AuthSession {
        accounts: Vec::new(),
        tokens: OAuthTokens {
            access_token,
            id_token,
            refresh_token: new_refresh_token,
            sub: String::new(),
            expiry,
        },
        session_id: String::new(),
    })
}

fn generate_random_string(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    rand::thread_rng().fill_bytes(&mut bytes);
    let encoded = parse::base64_url_encode(&bytes);
    encoded.chars().take(length).collect()
}

fn sha256_base64url(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    parse::base64_url_encode(&digest)
}

pub fn get_consent_id_token(first_id_token: &str, dont_use_gui: bool) -> Option<String> {
    let nonce = generate_random_string(32);
    let state = generate_random_string(32);

    let enc_id_token = curl_ffi::url_escape(first_id_token)?;
    let enc_redirect = curl_ffi::url_escape("http://localhost")?;
    let enc_response_type = curl_ffi::url_escape("id_token code")?;
    let enc_scope = curl_ffi::url_escape("openid offline")?;

    let url = format!(
        "https://account.jagex.com/oauth2/auth?id_token_hint={enc_id_token}&nonce={nonce}&prompt=consent&redirect_uri={enc_redirect}&response_type={enc_response_type}&state={state}&client_id=1fddee4e-b100-4f4e-b2b0-097f9088f9d2&scope={enc_scope}"
    );

    let instructions = "round two, the last one. this is just so Jagex knows you're not a robot and are eligible to receive a session ID.\n\nthe process is a bit similar, open the link below, get through cloudflare (login shouldn't be necessary), get redirected to a localhost URL";
    let instructions_pt_2 = "like before, copy the full localhost URL and paste it here:";
    let url_placeholder = "http://localhost/#code=...";

    let pasted_url_opt = if dont_use_gui {
        cli::prompt_for_url(
            "initial login 2 electric boogaloo",
            instructions,
            &url,
            url_placeholder,
        )
    } else {
        gui::prompt_for_url(
            "rlshim - initial login 2 electric boogaloo",
            instructions,
            instructions_pt_2,
            &url,
            url_placeholder,
        )
    };

    let Some(pasted_url) = pasted_url_opt else {
        log_error!("No URL provided");
        return None;
    };

    let Some(id_token) = parse::extract_query_param(&pasted_url, "id_token") else {
        log_error!("no id_token found in the pasted URL");
        return None;
    };

    Some(id_token)
}

pub fn get_game_accounts(session_id: &str) -> Option<Vec<GameAccount>> {
    let url = "https://auth.jagex.com/game-session/v1/accounts";
    let auth_header = format!("Authorization: Bearer {session_id}");
    let headers = ["Accept: application/json", auth_header.as_str()];

    let resp = match curl_ffi::perform_request(url, None, &headers, true, false) {
        Ok(r) => r,
        Err(err) => {
            log_error!("failed to get game accounts: {}", err);
            return None;
        }
    };

    if resp.status != 200 {
        log_error!(
            "failed to get game accounts with HTTP {}. Response: {}",
            resp.status,
            resp.body
        );
        return None;
    }

    match serde_json::from_str::<Vec<GameAccount>>(&resp.body) {
        Ok(accounts) => Some(accounts),
        Err(e) => {
            log_error!("failed to parse game accounts response: {}", e);
            None
        }
    }
}

pub fn get_session_id(id_token: &str) -> Option<String> {
    let payload = json!({ "idToken": id_token }).to_string();
    let headers = ["Accept: application/json", "Content-Type: application/json"];

    let resp = curl_ffi::perform_request(
        "https://auth.jagex.com/game-session/v1/sessions",
        Some(&payload),
        &headers,
        true,
        false,
    )
    .ok()?;

    if resp.status != 200 {
        log_error!(
            "failed to get session id with HTTP {}. Response: {}",
            resp.status,
            resp.body
        );
        return None;
    }

    let j: Value = match serde_json::from_str(&resp.body) {
        Ok(v) => v,
        Err(e) => {
            log_error!("failed to parse session response: {}", e);
            return None;
        }
    };

    j.get("sessionId").and_then(Value::as_str).map(String::from)
}

pub fn do_interactive_login(dont_use_gui: bool) -> Option<AuthSession> {
    let client_id = "com_jagex_auth_desktop_launcher";
    let redirect_uri = "https://secure.runescape.com/m=weblogin/launcher-redirect";
    let code_verifier = generate_random_string(64);
    let code_challenge = sha256_base64url(&code_verifier);
    let state = generate_random_string(32);

    let enc_redirect = curl_ffi::url_escape(redirect_uri)?;
    let enc_scope = curl_ffi::url_escape("openid offline gamesso.token.create user.profile.read")?;

    let auth_url = format!(
        "https://account.jagex.com/oauth2/auth?auth_method=&login_type=&flow=launcher&response_type=code&client_id={client_id}&redirect_uri={enc_redirect}&code_challenge={code_challenge}&code_challenge_method=S256&prompt=login&scope={enc_scope}&state={state}"
    );

    let instructions = "thank you for using rlshim!!\n\napologies for the interruption, but this is your first time logging in, so we have to establish some credentials. once you've done it, you'll never have to do it again ...short of deleting them in your secrets manager.\n\nto start, open this link in your browser, get through cloudflare, login, any 2FA until you are redirected:";
    let instructions_pt_2 = "once you see \"Logging in to Jagex Launcher\", ignore/close any prompts for xdg-open, and copy the full link. paste that link here:";
    let url_placeholder = "https://secure.runescape.com/m=weblogin/launcher-redirect?code=...";

    let pasted_url_opt = if dont_use_gui {
        cli::prompt_for_url(
            "rlshim - initial login",
            instructions,
            &auth_url,
            url_placeholder,
        )
    } else {
        gui::prompt_for_url(
            "rlshim - initial login",
            instructions,
            instructions_pt_2,
            &auth_url,
            url_placeholder,
        )
    };

    let Some(pasted_url) = pasted_url_opt else {
        log_error!("No URL provided");
        return None;
    };

    let Some(code) = parse::extract_query_param(&pasted_url, "code") else {
        log_error!("Could not find 'code=' in the pasted URL");
        return None;
    };

    let post_fields = format!(
        "grant_type=authorization_code&client_id={client_id}&code={code}&code_verifier={code_verifier}&redirect_uri={redirect_uri}"
    );
    let headers = [
        "Content-Type: application/x-www-form-urlencoded",
        "Accept: application/json",
    ];

    let resp = match curl_ffi::perform_request(
        "https://account.jagex.com/oauth2/token",
        Some(&post_fields),
        &headers,
        true,
        false,
    ) {
        Ok(r) if r.status == 200 => r,
        Ok(r) => {
            log_error!(
                "failed to exchange code for tokens. HTTP {}: {}",
                r.status,
                r.body
            );
            return None;
        }
        Err(e) => {
            log_error!("failed to exchange code for tokens: {}", e);
            return None;
        }
    };

    let auth_json: Value = match serde_json::from_str(&resp.body) {
        Ok(v) => v,
        Err(e) => {
            log_error!("failed to parse token exchange response: {}", e);
            return None;
        }
    };

    let access_token = auth_json.get("access_token")?.as_str()?.to_string();
    let id_token = auth_json.get("id_token")?.as_str()?.to_string();
    let refresh_token = auth_json.get("refresh_token")?.as_str()?.to_string();

    let Some(consent_id_token) = get_consent_id_token(&id_token, dont_use_gui) else {
        log_error!("failed to get consent id_token");
        return None;
    };

    let Some(sid) = get_session_id(&consent_id_token) else {
        log_error!("failed to fetch initial session id");
        return None;
    };

    Some(AuthSession {
        accounts: Vec::new(),
        tokens: OAuthTokens {
            access_token,
            id_token,
            refresh_token,
            sub: String::new(),
            expiry: 0,
        },
        session_id: sid,
    })
}

pub mod vault {
    use crate::log_error;
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};
    use pbkdf2::pbkdf2_hmac;
    use rand::RngCore;
    use serde_json::{Map, Value};
    use sha2::Sha256;
    use std::env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    const MAGIC: &[u8; 8] = b"RLVAULT1";
    const IV_LEN: usize = 12;
    const TAG_LEN: usize = 16;
    const KEY_LEN: usize = 32;
    const SALT: &[u8] = b"life.srp.rlshim:vault:v1";

    pub fn get_vault_path() -> Option<PathBuf> {
        let home = env::var("HOME").ok().filter(|h| !h.is_empty())?;
        let dir = PathBuf::from(home).join(".runelite");
        let _ = fs::create_dir_all(&dir);
        Some(dir.join("rlshim_vault.enc"))
    }

    fn get_machine_entropy() -> String {
        let id = fs::read_to_string("/etc/machine-id")
            .or_else(|_| fs::read_to_string("/var/lib/dbus/machine-id"))
            .ok()
            .and_then(|s| s.split_whitespace().next().map(String::from))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                env::var("HOSTNAME").unwrap_or_else(|_| "rlshim-default-machine-seed".to_string())
            });

        let uid = unsafe { libc::getuid() };
        format!("{id}:{uid}")
    }

    fn derive_key() -> [u8; KEY_LEN] {
        let entropy = get_machine_entropy();
        let mut key = [0u8; KEY_LEN];
        pbkdf2_hmac::<Sha256>(entropy.as_bytes(), SALT, 50_000, &mut key);
        key
    }

    /// Matches C++ binary layout: MAGIC (8B) || IV (12B) || TAG (16B) || CIPHERTEXT
    fn encrypt_data(plaintext: &str) -> Option<Vec<u8>> {
        let key = derive_key();
        let cipher = Aes256Gcm::new_from_slice(&key).ok()?;

        let mut iv = [0u8; IV_LEN];
        rand::thread_rng().fill_bytes(&mut iv);
        let nonce = Nonce::from_slice(&iv);

        // `aes-gcm` returns `ciphertext || tag` (16-byte tag at the end)
        let cipher_with_tag = cipher.encrypt(nonce, plaintext.as_bytes()).ok()?;
        if cipher_with_tag.len() < TAG_LEN {
            return None;
        }
        let ct_len = cipher_with_tag.len() - TAG_LEN;
        let (ciphertext, tag) = cipher_with_tag.split_at(ct_len);

        let mut out = Vec::with_capacity(MAGIC.len() + IV_LEN + TAG_LEN + ct_len);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&iv);
        out.extend_from_slice(tag);
        out.extend_from_slice(ciphertext);
        Some(out)
    }

    fn decrypt_data(in_data: &[u8]) -> Option<String> {
        let header_len = MAGIC.len() + IV_LEN + TAG_LEN;
        if in_data.len() < header_len || &in_data[..MAGIC.len()] != MAGIC {
            return None;
        }

        let iv = &in_data[MAGIC.len()..MAGIC.len() + IV_LEN];
        let tag = &in_data[MAGIC.len() + IV_LEN..header_len];
        let ciphertext = &in_data[header_len..];

        let key = derive_key();
        let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
        let nonce = Nonce::from_slice(iv);

        // Reassemble `ciphertext || tag` expected by `aes-gcm`
        let mut combined = Vec::with_capacity(ciphertext.len() + TAG_LEN);
        combined.extend_from_slice(ciphertext);
        combined.extend_from_slice(tag);

        let plain_bytes = cipher.decrypt(nonce, combined.as_ref()).ok()?;
        String::from_utf8(plain_bytes).ok()
    }

    fn load_vault_json() -> Option<Value> {
        let path = get_vault_path()?;
        if !path.exists() {
            return None;
        }

        let data = fs::read(&path).ok()?;
        let Some(plaintext) = decrypt_data(&data) else {
            log_error!("failed to decrypt credentials vault");
            return None;
        };

        match serde_json::from_str(&plaintext) {
            Ok(v) => Some(v),
            Err(e) => {
                log_error!("failed to parse decrypted vault JSON: {}", e);
                None
            }
        }
    }

    fn save_vault_json(j: &Value) -> bool {
        let Some(path) = get_vault_path() else {
            return false;
        };
        let Ok(plaintext) = serde_json::to_string(j) else {
            return false;
        };
        let Some(encrypted) = encrypt_data(&plaintext) else {
            log_error!("failed to encrypt credentials vault");
            return false;
        };

        if fs::write(&path, &encrypted).is_err() {
            log_error!("failed to open vault file for writing: {}", path.display());
            return false;
        }

        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        true
    }

    pub fn store(key: &str, value: &str) -> bool {
        let mut j = load_vault_json().unwrap_or_else(|| Value::Object(Map::new()));
        if let Some(obj) = j.as_object_mut() {
            obj.insert(key.to_string(), Value::String(value.to_string()));
        }
        save_vault_json(&j)
    }

    pub fn lookup(key: &str) -> Option<String> {
        let j = load_vault_json()?;
        j.get(key)?.as_str().map(String::from)
    }

    pub fn clear(key: &str) -> bool {
        let Some(mut j) = load_vault_json() else {
            return true;
        };
        if let Some(obj) = j.as_object_mut() {
            obj.remove(key);
            if obj.is_empty() {
                if let Some(path) = get_vault_path() {
                    let _ = fs::remove_file(path);
                }
                return true;
            }
        }
        save_vault_json(&j)
    }
}

pub mod keyring {
    use super::vault;
    use crate::{log_error, log_info, parse};
    use std::env;
    use std::ffi::{CStr, CString, c_char, c_int, c_void};
    use std::fs;
    use std::ptr;
    use std::sync::Mutex;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Backend {
        Unknown,
        LibSecret,
        EncryptedFile,
    }

    static BACKEND: Mutex<Backend> = Mutex::new(Backend::Unknown);

    #[repr(C)]
    struct GError {
        domain: u32,
        code: c_int,
        message: *const c_char,
    }

    #[repr(C)]
    struct GList {
        data: *mut c_void,
        next: *mut GList,
        prev: *mut GList,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct SecretSchemaAttribute {
        name: *const c_char,
        attr_type: c_int,
    }

    #[repr(C)]
    struct SecretSchema {
        name: *const c_char,
        flags: c_int,
        attributes: [SecretSchemaAttribute; 32],
        reserved: c_int,
        reserved1: *mut c_void,
        reserved2: *mut c_void,
        reserved3: *mut c_void,
        reserved4: *mut c_void,
        reserved5: *mut c_void,
        reserved6: *mut c_void,
        reserved7: *mut c_void,
    }

    unsafe impl Sync for SecretSchema {}

    const SECRET_SCHEMA_NONE: c_int = 0;
    const SECRET_SCHEMA_ATTRIBUTE_STRING: c_int = 0;
    const SECRET_SERVICE_LOAD_COLLECTIONS: c_int = 2;
    const SECRET_COLLECTION_NONE: c_int = 0;
    const SECRET_COLLECTION_DEFAULT: *const c_char = c"default".as_ptr();

    static SCHEMA: SecretSchema = {
        let mut attrs = [SecretSchemaAttribute {
            name: ptr::null(),
            attr_type: SECRET_SCHEMA_ATTRIBUTE_STRING,
        }; 32];
        attrs[0] = SecretSchemaAttribute {
            name: c"account".as_ptr(),
            attr_type: SECRET_SCHEMA_ATTRIBUTE_STRING,
        };
        SecretSchema {
            name: c"life.srp.rlshim".as_ptr(),
            flags: SECRET_SCHEMA_NONE,
            attributes: attrs,
            reserved: 0,
            reserved1: ptr::null_mut(),
            reserved2: ptr::null_mut(),
            reserved3: ptr::null_mut(),
            reserved4: ptr::null_mut(),
            reserved5: ptr::null_mut(),
            reserved6: ptr::null_mut(),
            reserved7: ptr::null_mut(),
        }
    };

    unsafe extern "C" {
        fn g_error_free(error: *mut GError);
        fn g_object_unref(object: *mut c_void);
        fn g_list_append(list: *mut GList, data: *mut c_void) -> *mut GList;
        fn g_list_free(list: *mut GList);

        fn secret_service_get_sync(
            flags: c_int,
            cancellable: *mut c_void,
            error: *mut *mut GError,
        ) -> *mut c_void;
        fn secret_collection_for_alias_sync(
            service: *mut c_void,
            alias: *const c_char,
            flags: c_int,
            cancellable: *mut c_void,
            error: *mut *mut GError,
        ) -> *mut c_void;
        fn secret_collection_get_locked(collection: *mut c_void) -> c_int;
        fn secret_service_unlock_sync(
            service: *mut c_void,
            objects: *mut GList,
            cancellable: *mut c_void,
            unlocked: *mut *mut GList,
            error: *mut *mut GError,
        ) -> c_int;

        fn secret_password_store_sync(
            schema: *const SecretSchema,
            collection: *const c_char,
            label: *const c_char,
            password: *const c_char,
            cancellable: *mut c_void,
            error: *mut *mut GError,
            ...
        ) -> c_int;
        fn secret_password_lookup_sync(
            schema: *const SecretSchema,
            cancellable: *mut c_void,
            error: *mut *mut GError,
            ...
        ) -> *mut c_char;
        fn secret_password_clear_sync(
            schema: *const SecretSchema,
            cancellable: *mut c_void,
            error: *mut *mut GError,
            ...
        ) -> c_int;
        fn secret_password_free(password: *mut c_char);
    }

    fn try_libsecret(silent: bool) -> bool {
        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let service = secret_service_get_sync(
                SECRET_SERVICE_LOAD_COLLECTIONS,
                ptr::null_mut(),
                &mut error,
            );

            if !error.is_null() {
                if !silent {
                    let msg = CStr::from_ptr((*error).message).to_string_lossy();
                    log_error!("secret service not available: {}", msg);
                }
                g_error_free(error);
                return false;
            }

            if service.is_null() {
                if !silent {
                    log_error!("no secret service provider found on D-Bus");
                }
                return false;
            }

            let collection = secret_collection_for_alias_sync(
                service,
                SECRET_COLLECTION_DEFAULT,
                SECRET_COLLECTION_NONE,
                ptr::null_mut(),
                &mut error,
            );

            if !error.is_null() {
                if !silent {
                    let msg = CStr::from_ptr((*error).message).to_string_lossy();
                    log_error!("failed to access default keyring collection: {}", msg);
                }
                g_error_free(error);
                g_object_unref(service);
                return false;
            }

            if collection.is_null() {
                if !silent {
                    log_error!("no default keyring collection found. is your keyring set up?");
                }
                g_object_unref(service);
                return false;
            }

            let locked = secret_collection_get_locked(collection) != 0;
            if locked {
                if !silent {
                    log_info!("default keyring collection is locked, attempting to unlock...");
                }
                let to_unlock = g_list_append(ptr::null_mut(), collection);
                let mut unlocked: *mut GList = ptr::null_mut();

                secret_service_unlock_sync(
                    service,
                    to_unlock,
                    ptr::null_mut(),
                    &mut unlocked,
                    &mut error,
                );
                g_list_free(to_unlock);

                if !error.is_null() {
                    if !silent {
                        let msg = CStr::from_ptr((*error).message).to_string_lossy();
                        log_error!("failed to unlock keyring: {}", msg);
                    }
                    g_error_free(error);
                    g_object_unref(collection);
                    g_object_unref(service);
                    return false;
                }

                let was_unlocked = !unlocked.is_null();
                g_list_free(unlocked);

                if !was_unlocked {
                    if !silent {
                        log_error!("keyring unlock was dismissed or failed");
                    }
                    g_object_unref(collection);
                    g_object_unref(service);
                    return false;
                }

                if !silent {
                    log_info!("keyring unlocked successfully");
                }
            }

            g_object_unref(collection);
            g_object_unref(service);
            true
        }
    }

    fn in_gamescope_session() -> bool {
        if let Ok(desktop) = env::var("XDG_CURRENT_DESKTOP") {
            if desktop.contains("gamescope") {
                return true;
            }
        }
        if let Ok(gs) = env::var("GAMESCOPE_WAYLAND_DISPLAY") {
            if !gs.is_empty() {
                return true;
            }
        }
        false
    }

    fn should_prefer_vault() -> bool {
        // Escape hatch for unusual setups; not required for normal use.
        if let Ok(forced) = env::var("RLSHIM_CREDENTIAL_BACKEND") {
            if forced == "vault" {
                return true;
            }
            if forced == "keyring" || forced == "libsecret" {
                return false;
            }
            if !forced.is_empty() {
                log_error!(
                    "ignoring unknown RLSHIM_CREDENTIAL_BACKEND value: {}",
                    forced
                );
            }
        }

        // Inside a Flatpak, /etc/os-release describes the runtime, and the
        // host's file is exposed at /run/host/os-release instead.
        for path in &[
            "/run/host/os-release",
            "/etc/os-release",
            "/usr/lib/os-release",
        ] {
            if let Ok(contents) = fs::read_to_string(path) {
                if contents.is_empty() {
                    continue;
                }
                if parse::is_gaming_os_release(&contents) {
                    log_info!("detected a gaming-mode distro via {}", path);
                    return true;
                }
                break; // only consult the first os-release that exists
            }
        }

        // Steam Deck hardware, whatever distro it's running.
        for path in &[
            "/sys/class/dmi/id/sys_vendor",
            "/sys/class/dmi/id/board_vendor",
        ] {
            if let Ok(contents) = fs::read_to_string(path) {
                if parse::is_valve_hardware_vendor(&contents) {
                    log_info!("detected Valve hardware via {}", path);
                    return true;
                }
            }
        }

        false
    }

    fn migrate_libsecret_to_vault() {
        let Some(vp) = vault::get_vault_path() else {
            return;
        };
        if vp.exists() || in_gamescope_session() {
            return;
        }

        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let val_ptr = secret_password_lookup_sync(
                &SCHEMA,
                ptr::null_mut(),
                &mut error,
                c"account".as_ptr(),
                c"jagex_auth".as_ptr(),
                ptr::null::<c_char>(),
            );
            if !error.is_null() {
                g_error_free(error);
                return;
            }
            if val_ptr.is_null() {
                return;
            }

            let value = CStr::from_ptr(val_ptr).to_string_lossy().into_owned();
            secret_password_free(val_ptr);

            if vault::store("jagex_auth", &value) {
                log_info!(
                    "migrated saved credentials from the system keyring to the encrypted vault"
                );
            }
        }
    }

    fn resolve_backend() {
        let mut backend = BACKEND.lock().unwrap();
        if *backend != Backend::Unknown {
            return;
        }

        if should_prefer_vault() {
            if vault::get_vault_path().is_some() {
                log_info!(
                    "using machine-encrypted credential vault for consistency across gaming/desktop mode ✔"
                );
                *backend = Backend::EncryptedFile;
                drop(backend);
                migrate_libsecret_to_vault();
            }
            return;
        }

        if try_libsecret(true) {
            *backend = Backend::LibSecret;
            return;
        }

        if vault::get_vault_path().is_some() {
            log_info!("system keyring not available; using machine-encrypted credential vault ✔");
            *backend = Backend::EncryptedFile;
        }
    }

    pub fn is_available() -> bool {
        resolve_backend();
        *BACKEND.lock().unwrap() != Backend::Unknown
    }

    pub fn store(key: &str, value: &str) -> bool {
        resolve_backend();
        let backend = *BACKEND.lock().unwrap();
        if backend == Backend::EncryptedFile {
            return vault::store(key, value);
        }

        let Ok(c_key) = CString::new(key) else {
            return false;
        };
        let Ok(c_val) = CString::new(value) else {
            return false;
        };

        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            secret_password_store_sync(
                &SCHEMA,
                SECRET_COLLECTION_DEFAULT,
                c"rlshim credentials".as_ptr(),
                c_val.as_ptr(),
                ptr::null_mut(),
                &mut error,
                c"account".as_ptr(),
                c_key.as_ptr(),
                ptr::null::<c_char>(),
            );

            if !error.is_null() {
                let msg = CStr::from_ptr((*error).message).to_string_lossy();
                log_error!("failed to store secret in keyring: {}", msg);
                g_error_free(error);
                return false;
            }
        }

        // Mirror to encrypted vault as backup for Game Mode / headless sessions
        vault::store(key, value);
        true
    }

    pub fn lookup(key: &str) -> Option<String> {
        resolve_backend();
        let backend = *BACKEND.lock().unwrap();
        if backend == Backend::EncryptedFile {
            return vault::lookup(key);
        }

        let c_key = CString::new(key).ok()?;
        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let val_ptr = secret_password_lookup_sync(
                &SCHEMA,
                ptr::null_mut(),
                &mut error,
                c"account".as_ptr(),
                c_key.as_ptr(),
                ptr::null::<c_char>(),
            );

            if !error.is_null() {
                let msg = CStr::from_ptr((*error).message).to_string_lossy();
                log_error!("failed to lookup secret from keyring: {}", msg);
                g_error_free(error);
                return vault::lookup(key);
            }

            if val_ptr.is_null() {
                return vault::lookup(key);
            }

            let result = CStr::from_ptr(val_ptr).to_string_lossy().into_owned();
            secret_password_free(val_ptr);
            Some(result)
        }
    }

    pub fn clear(key: &str) -> bool {
        resolve_backend();
        let backend = *BACKEND.lock().unwrap();
        let mut ok = true;

        // In vault mode, still remove any old keyring copy (best effort) so
        // stale tokens don't linger -- but never poke the secret service
        // from gaming mode, where an unlock prompt would be invisible.
        let clear_libsecret = backend == Backend::LibSecret
            || (backend == Backend::EncryptedFile && !in_gamescope_session());
        if clear_libsecret {
            if let Ok(c_key) = CString::new(key) {
                unsafe {
                    let mut error: *mut GError = ptr::null_mut();
                    secret_password_clear_sync(
                        &SCHEMA,
                        ptr::null_mut(),
                        &mut error,
                        c"account".as_ptr(),
                        c_key.as_ptr(),
                        ptr::null::<c_char>(),
                    );
                    if !error.is_null() {
                        if backend == Backend::LibSecret {
                            let msg = CStr::from_ptr((*error).message).to_string_lossy();
                            log_error!("failed to clear secret from keyring: {}", msg);
                            ok = false;
                        }
                        g_error_free(error);
                    }
                }
            }
        }

        vault::clear(key);
        ok
    }
}
