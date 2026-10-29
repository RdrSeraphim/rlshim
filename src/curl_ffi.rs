use std::ffi::{CStr, CString, c_char, c_int, c_long, c_void};
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[allow(non_camel_case_types)]
type CURL = c_void;
#[allow(non_camel_case_types)]
type CURLcode = c_int;

const CURLE_OK: CURLcode = 0;

const CURLOPT_WRITEDATA: c_int = 10001;
const CURLOPT_URL: c_int = 10002;
const CURLOPT_POSTFIELDS: c_int = 10015;
const CURLOPT_HTTPHEADER: c_int = 10023;
const CURLOPT_ACCEPT_ENCODING: c_int = 10102;
const CURLOPT_WRITEFUNCTION: c_int = 20011;
const CURLOPT_FOLLOWLOCATION: c_int = 52;

const CURLINFO_RESPONSE_CODE: c_int = 0x200002;

#[repr(C)]
struct CurlSlist {
    data: *mut c_char,
    next: *mut CurlSlist,
}

unsafe extern "C" {
    fn curl_easy_init() -> *mut CURL;
    fn curl_easy_cleanup(curl: *mut CURL);
    fn curl_easy_perform(curl: *mut CURL) -> CURLcode;
    fn curl_easy_setopt(curl: *mut CURL, option: c_int, ...) -> CURLcode;
    fn curl_easy_getinfo(curl: *mut CURL, info: c_int, ...) -> CURLcode;
    fn curl_easy_strerror(code: CURLcode) -> *const c_char;
    fn curl_easy_escape(curl: *mut CURL, string: *const c_char, length: c_int) -> *mut c_char;
    fn curl_free(p: *mut c_void);
    fn curl_slist_append(list: *mut CurlSlist, string: *const c_char) -> *mut CurlSlist;
    fn curl_slist_free_all(list: *mut CurlSlist);
    fn curl_easy_impersonate(
        curl: *mut CURL,
        target: *const c_char,
        default_headers: c_int,
    ) -> CURLcode;
}

extern "C" fn write_to_vec_callback(
    ptr: *const c_char,
    size: usize,
    nmemb: usize,
    userdata: *mut c_void,
) -> usize {
    let total = size.saturating_mul(nmemb);
    if total == 0 || ptr.is_null() || userdata.is_null() {
        return 0;
    }
    unsafe {
        let buffer = &mut *(userdata as *mut Vec<u8>);
        let slice = std::slice::from_raw_parts(ptr as *const u8, total);
        buffer.extend_from_slice(slice);
    }
    total
}

extern "C" fn write_to_file_callback(
    ptr: *const c_char,
    size: usize,
    nmemb: usize,
    userdata: *mut c_void,
) -> usize {
    let total = size.saturating_mul(nmemb);
    if total == 0 || ptr.is_null() || userdata.is_null() {
        return 0;
    }
    unsafe {
        let file = &mut *(userdata as *mut File);
        let slice = std::slice::from_raw_parts(ptr as *const u8, total);
        if file.write_all(slice).is_err() {
            return 0;
        }
    }
    total
}

pub struct HttpResponse {
    pub status: i64,
    pub body: String,
}

pub fn url_escape(input: &str) -> Option<String> {
    unsafe {
        let curl = curl_easy_init();
        if curl.is_null() {
            return None;
        }
        let c_input = CString::new(input).ok()?;
        let escaped_ptr = curl_easy_escape(curl, c_input.as_ptr(), 0);
        if escaped_ptr.is_null() {
            curl_easy_cleanup(curl);
            return None;
        }
        let escaped = CStr::from_ptr(escaped_ptr).to_string_lossy().into_owned();
        curl_free(escaped_ptr as *mut c_void);
        curl_easy_cleanup(curl);
        Some(escaped)
    }
}

pub fn perform_request(
    url: &str,
    post_fields: Option<&str>,
    headers: &[&str],
    impersonate_chrome: bool,
    follow_location: bool,
) -> Result<HttpResponse, String> {
    unsafe {
        let curl = curl_easy_init();
        if curl.is_null() {
            return Err("failed to initialize curl".to_string());
        }

        if impersonate_chrome {
            let target = CString::new("chrome116").unwrap();
            curl_easy_impersonate(curl, target.as_ptr(), 1);
            let empty = CString::new("").unwrap();
            curl_easy_setopt(curl, CURLOPT_ACCEPT_ENCODING, empty.as_ptr());
        }

        if follow_location {
            curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1i64 as c_long);
        }

        let c_url = CString::new(url).map_err(|e| e.to_string())?;
        curl_easy_setopt(curl, CURLOPT_URL, c_url.as_ptr());

        let c_post = if let Some(fields) = post_fields {
            let s = CString::new(fields).map_err(|e| e.to_string())?;
            curl_easy_setopt(curl, CURLOPT_POSTFIELDS, s.as_ptr());
            Some(s)
        } else {
            None
        };

        let mut slist: *mut CurlSlist = std::ptr::null_mut();
        for &h in headers {
            if let Ok(c_h) = CString::new(h) {
                slist = curl_slist_append(slist, c_h.as_ptr());
            }
        }
        if !slist.is_null() {
            curl_easy_setopt(curl, CURLOPT_HTTPHEADER, slist);
        }

        let mut response_bytes: Vec<u8> = Vec::new();
        curl_easy_setopt(
            curl,
            CURLOPT_WRITEFUNCTION,
            write_to_vec_callback as *const c_void,
        );
        curl_easy_setopt(
            curl,
            CURLOPT_WRITEDATA,
            &mut response_bytes as *mut Vec<u8> as *mut c_void,
        );

        let res = curl_easy_perform(curl);
        let mut http_code: c_long = 0;
        curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &mut http_code);

        if !slist.is_null() {
            curl_slist_free_all(slist);
        }
        drop(c_post);
        curl_easy_cleanup(curl);

        if res != CURLE_OK {
            let err_str = CStr::from_ptr(curl_easy_strerror(res))
                .to_string_lossy()
                .into_owned();
            return Err(err_str);
        }

        let body = String::from_utf8_lossy(&response_bytes).into_owned();
        Ok(HttpResponse {
            status: http_code as i64,
            body,
        })
    }
}

pub fn download_to_file(url: &str, dest: &Path) -> Result<i64, String> {
    let mut file =
        File::create(dest).map_err(|_| format!("failed to open {} for writing", dest.display()))?;

    unsafe {
        let curl = curl_easy_init();
        if curl.is_null() {
            return Err("failed to initialize curl".to_string());
        }

        let c_url = CString::new(url).map_err(|e| e.to_string())?;
        curl_easy_setopt(curl, CURLOPT_URL, c_url.as_ptr());
        curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1i64 as c_long);
        curl_easy_setopt(
            curl,
            CURLOPT_WRITEFUNCTION,
            write_to_file_callback as *const c_void,
        );
        curl_easy_setopt(
            curl,
            CURLOPT_WRITEDATA,
            &mut file as *mut File as *mut c_void,
        );

        let res = curl_easy_perform(curl);
        let mut http_code: c_long = 0;
        curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &mut http_code);
        curl_easy_cleanup(curl);

        if res != CURLE_OK {
            let err_str = CStr::from_ptr(curl_easy_strerror(res))
                .to_string_lossy()
                .into_owned();
            return Err(err_str);
        }

        Ok(http_code as i64)
    }
}
