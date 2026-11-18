//! DCWeather account bridge: sign the studio in to the DCWeather site with a device link (the
//! password never leaves the site), read its sponsor list, and post graphics to its Facebook page
//! through the site. The site address is `Settings::api_base`. The studio token is kept in the
//! Windows Credential Manager, never in a settings file.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::sync::Mutex;
use std::time::Duration;

fn site(base: &str) -> String {
    let b = base.trim().trim_end_matches('/');
    if b.is_empty() { "https://dcweather.xyz".into() } else { b.to_string() }
}

fn host(base: &str) -> String {
    site(base).split("://").nth(1).unwrap_or("dcweather").split('/').next().unwrap_or("dcweather").to_string()
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("OpenWX-Broadcaster/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}

async fn error_text(resp: reqwest::Response) -> String {
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    body.get("error").and_then(|e| e.as_str()).map(str::to_string).unwrap_or_else(|| format!("the site answered {status}"))
}

// ---------------------------------------------------------------- token ---

#[cfg(windows)]
mod vault {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn save(target: &str, secret: &str) -> bool {
        let mut t = wide(target);
        let mut user = wide("studio");
        let blob = secret.as_bytes().to_vec();
        let cred = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(t.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(user.as_mut_ptr()),
            ..Default::default()
        };
        unsafe { CredWriteW(&cred, 0).is_ok() }
    }

    pub fn load(target: &str) -> Option<String> {
        let t = wide(target);
        let mut out: *mut CREDENTIALW = std::ptr::null_mut();
        unsafe {
            CredReadW(PCWSTR(t.as_ptr()), CRED_TYPE_GENERIC, None, &mut out).ok()?;
            let c = &*out;
            let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec();
            CredFree(out as *const _);
            String::from_utf8(bytes).ok()
        }
    }

    pub fn delete(target: &str) {
        let t = wide(target);
        unsafe {
            let _ = CredDeleteW(PCWSTR(t.as_ptr()), CRED_TYPE_GENERIC, None);
        }
    }
}

#[cfg(not(windows))]
mod vault {
    pub fn save(_: &str, _: &str) -> bool {
        false
    }
    pub fn load(_: &str) -> Option<String> {
        None
    }
    pub fn delete(_: &str) {}
}

fn vault_key(base: &str) -> String {
    format!("OpenWX/DCWeather/{}", host(base))
}

fn token(base: &str) -> Result<String> {
    vault::load(&vault_key(base)).ok_or_else(|| anyhow!("Not signed in to DCWeather (Settings > Integrations)."))
}

/// True when this PC holds a studio token for the site (it may still have been revoked).
pub fn has_token(base: &str) -> bool {
    vault::load(&vault_key(base)).is_some()
}

pub fn sign_out(base: &str) {
    vault::delete(&vault_key(base));
    if let Ok(mut s) = STATUS.lock() {
        *s = None;
    }
}

/// "Signed in as Jane (Studio A)", remembered after the last successful check.
static STATUS: Mutex<Option<String>> = Mutex::new(None);

pub fn status() -> Option<String> {
    STATUS.lock().ok().and_then(|s| s.clone())
}

// ------------------------------------------------------------ device link ---

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub code: String,
    pub device_secret: String,
    pub approve_url: String,
    pub expires_in: u64,
}

/// Step 1: ask the site for a code. The user approves it at `approve_url` while signed in.
pub async fn start_link(base: String, device_name: String) -> Result<Link> {
    let resp = client()
        .post(format!("{}/api/studio/link/start", site(&base)))
        .json(&serde_json::json!({ "deviceName": device_name }))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!(error_text(resp).await));
    }
    Ok(resp.json().await?)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Poll {
    Pending,
    Expired,
    Approved,
}

/// Step 2: poll until approved; the token is stored in the Credential Manager.
pub async fn poll_link(base: String, device_secret: String) -> Result<Poll> {
    let resp = client()
        .post(format!("{}/api/studio/link/poll", site(&base)))
        .json(&serde_json::json!({ "deviceSecret": device_secret }))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!(error_text(resp).await));
    }
    let v: serde_json::Value = resp.json().await?;
    match v.get("status").and_then(|s| s.as_str()) {
        Some("approved") => {
            let token = v.get("token").and_then(|t| t.as_str()).ok_or_else(|| anyhow!("no token in the answer"))?;
            if !vault::save(&vault_key(&base), token) {
                return Err(anyhow!("couldn't save the sign-in in Windows Credential Manager"));
            }
            Ok(Poll::Approved)
        }
        Some("pending") => Ok(Poll::Pending),
        _ => Ok(Poll::Expired),
    }
}

// ------------------------------------------------------------------ API ---

/// Checks the token and returns "Name (Studio)". Clears the token if the site revoked it.
pub async fn check(base: String) -> Result<String> {
    let t = token(&base)?;
    let resp = client().get(format!("{}/api/studio/me", site(&base))).bearer_auth(t).send().await?;
    if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
        let msg = error_text(resp).await;
        sign_out(&base);
        return Err(anyhow!(msg));
    }
    if !resp.status().is_success() {
        return Err(anyhow!(error_text(resp).await));
    }
    let v: serde_json::Value = resp.json().await?;
    let who = format!(
        "{} ({})",
        v.pointer("/user/name").and_then(|x| x.as_str()).unwrap_or("?"),
        v.pointer("/studio/name").and_then(|x| x.as_str()).unwrap_or("studio")
    );
    if let Ok(mut s) = STATUS.lock() {
        *s = Some(who.clone());
    }
    Ok(who)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteSponsor {
    pub id: String,
    pub name: String,
    pub label: String,
    pub link: String,
    pub fb_page_id: String,
    pub logo_url: String,
    pub active: bool,
}

/// The site's sponsor list (the portal stays the place to edit it).
pub async fn sponsors(base: String) -> Result<Vec<RemoteSponsor>> {
    let resp = client().get(format!("{}/api/studio/sponsors", site(&base))).bearer_auth(token(&base)?).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!(error_text(resp).await));
    }
    let v: serde_json::Value = resp.json().await?;
    Ok(serde_json::from_value(v.get("sponsors").cloned().unwrap_or_default()).unwrap_or_default())
}

/// Downloads each active sponsor's logo into the media library and returns sponsors for the
/// studio's lineup. Sponsors already in `existing` (same name) keep their own logo and line.
pub async fn import_sponsors(base: String, existing: Vec<crate::config::Sponsor>) -> Result<Vec<crate::config::Sponsor>> {
    let list = sponsors(base).await?;
    let c = client();
    let mut out = Vec::new();
    for s in list.into_iter().filter(|s| s.active && !s.name.trim().is_empty()) {
        if existing.iter().any(|e| e.name.eq_ignore_ascii_case(s.name.trim())) {
            continue;
        }
        let mut logo_media = None;
        if !s.logo_url.is_empty() {
            if let Ok(resp) = c.get(&s.logo_url).send().await {
                let ext = match resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("") {
                    t if t.contains("svg") => "svg",
                    t if t.contains("jpeg") => "jpg",
                    t if t.contains("webp") => "webp",
                    _ => "png",
                };
                if let Ok(bytes) = resp.bytes().await {
                    let clean: String = s.name.chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' }).collect();
                    let tmp = std::env::temp_dir().join(format!("dcweather-sponsor-{clean}.{ext}"));
                    if std::fs::write(&tmp, &bytes).is_ok() {
                        logo_media = crate::library::import_media(&[tmp.clone()]).ok().and_then(|v| v.into_iter().next());
                        let _ = std::fs::remove_file(&tmp);
                    }
                }
            }
        }
        out.push(crate::config::Sponsor { name: s.name.trim().to_string(), tagline: s.label.trim().to_string(), logo_media });
    }
    Ok(out)
}

/// Posts an image to the site's Facebook page. `sponsor_id` adds the site's "Brought to you by"
/// line and tags the sponsor's page. Returns the Facebook post id.
pub async fn post_image(base: String, bytes: Vec<u8>, file: String, message: String, sponsor_id: Option<String>) -> Result<String> {
    let mime = if file.ends_with(".jpg") { "image/jpeg" } else { "image/png" };
    let mut form = reqwest::multipart::Form::new()
        .text("message", message)
        .part("image", reqwest::multipart::Part::bytes(bytes).file_name(file).mime_str(mime)?);
    if let Some(id) = sponsor_id.filter(|s| !s.is_empty()) {
        form = form.text("sponsorId", id);
    }
    let resp = client().post(format!("{}/api/studio/post", site(&base))).bearer_auth(token(&base)?).multipart(form).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!(error_text(resp).await));
    }
    let v: serde_json::Value = resp.json().await?;
    Ok(v.get("facebookPostId").and_then(|x| x.as_str()).unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_and_vault_key() {
        assert_eq!(site(" https://dcweather.xyz/ "), "https://dcweather.xyz");
        assert_eq!(site(""), "https://dcweather.xyz");
        assert_eq!(vault_key("http://localhost:3000"), "OpenWX/DCWeather/localhost:3000");
    }

    /// Against a running site that auto-approves links:
    /// DCWX_TEST_SITE=http://127.0.0.1:3999 cargo test --release dcweather::tests::live_bridge -- --ignored
    #[test]
    #[ignore]
    fn live_bridge() {
        let base = std::env::var("DCWX_TEST_SITE").expect("DCWX_TEST_SITE");
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            sign_out(&base);
            let link = start_link(base.clone(), "Test PC".into()).await.unwrap();
            assert_eq!(link.code.len(), 9);
            let mut state = Poll::Pending;
            for _ in 0..20 {
                tokio::time::sleep(Duration::from_millis(1500)).await;
                state = poll_link(base.clone(), link.device_secret.clone()).await.unwrap();
                if state != Poll::Pending {
                    break;
                }
            }
            assert_eq!(state, Poll::Approved);
            assert!(has_token(&base));
            let who = check(base.clone()).await.unwrap();
            assert!(who.contains("Bridge test"), "{who}");
            sponsors(base.clone()).await.unwrap();
            sign_out(&base);
            assert!(!has_token(&base));
            assert!(check(base.clone()).await.is_err());
        });
    }

    #[cfg(windows)]
    #[test]
    fn credential_round_trip() {
        let key = "OpenWX/DCWeather/test-only";
        assert!(vault::save(key, "abc123"));
        assert_eq!(vault::load(key).as_deref(), Some("abc123"));
        vault::delete(key);
        assert_eq!(vault::load(key), None);
    }
}
