// auth — Microsoft account login (OAuth 2.0 device code) for Minecraft Java.
//
// The flow never needs a client secret: the user's own Azure application id is
// used for the public-client device-code grant, then the standard
// Xbox Live → XSTS → Minecraft Services chain mints a game session token.
// Only the long-lived refresh token is persisted, and it lives in SQLite,
// never in the JSON blob that is handed to the UI.

use crate::engine::http_client;
use crate::state::{Account, TokenSet};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const DEVICE_CODE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "XboxLive.signin offline_access";

/// Result of pre-flighting a client id from the settings page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCheck {
    pub ok: bool,
    pub code: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
    pub message: String,
}

/// Credentials the JVM needs for a signed-in launch.
#[derive(Debug, Clone)]
pub struct McAuth {
    pub access_token: String,
    pub uuid: String,
    pub name: String,
    pub xuid: String,
}

/// First AADSTS code in an Entra error payload, e.g. `AADSTS700016` → `700016`.
fn aad_code(value: &Value) -> Option<i64> {
    value
        .get("error_codes")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_i64())
        .or_else(|| {
            let desc = value.get("error_description")?.as_str()?;
            let rest = desc.split("AADSTS").nth(1)?;
            rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
        })
}

/// Plain-Thai explanation + the exact Azure switch to flip for common Entra errors.
/// These are configuration problems on the Azure side, so the message has to say
/// what to change rather than repeat the raw AADSTS text.
fn aad_hint(code: i64) -> Option<&'static str> {
    Some(match code {
        700016 | 700019 => "Azure app นี้ยังไม่เปิดให้ “บัญชี Microsoft ส่วนตัว” ล็อกอินได้\n\
แก้ที่ Azure Portal → App registrations → แอปของคุณ → Authentication →\n\
Supported account types = “Accounts in any organizational directory and personal Microsoft accounts” แล้วกด Save\n\
(บัญชี Minecraft เป็นบัญชี Microsoft ส่วนตัว ถ้าแอปเป็นแบบ single-tenant จะล็อกอินไม่ได้เลย)",
        7000218 => "แอปยังไม่เปิด public client flow\n\
แก้ที่ Azure Portal → App registrations → Authentication → Allow public client flows = Yes แล้วกด Save",
        700038 | 700022 | 700024 => "client id ไม่ถูกต้องหรือไม่มีแอปนี้อยู่\n\
คัดลอกค่า “Application (client) ID” จากหน้า Overview ของ App registration มาทั้งค่า (รูปแบบ xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)",
        90002 | 900021 | 50059 => "Microsoft หา tenant ของแอปไม่เจอ — แอปอาจอยู่คนละ cloud หรือถูกลบไปแล้ว ลองสร้าง App registration ใหม่ใน portal ปกติ",
        65001 | 65004 => "ยังไม่ได้ให้สิทธิ์ (consent) กับแอป ลองล็อกอินใหม่แล้วกด Accept",
        _ => return None,
    })
}

fn aad_error_text(value: &Value, status: u16) -> String {
    let code = aad_code(value);
    let raw = value
        .get("error_description")
        // Microsoft Graph / Minecraft Services use `errorMessage` instead.
        .or_else(|| value.get("errorMessage"))
        .or_else(|| value.get("error_message"))
        .or_else(|| value.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown error");
    // Entra appends a long trace/correlation blob; keep the sentence people can act on.
    let first = raw.split("Trace ID").next().unwrap_or(raw).trim();
    // Minecraft Services only serves client ids Mojang has put on its allow list;
    // every other app registration is refused here — after Xbox Live and XSTS
    // already succeeded, so the message has to point at the review form.
    if raw.contains("Invalid app registration") {
        return "Mojang ยังไม่อนุมัติ (allow list) แอปนี้ — Minecraft Services ปฏิเสธ client id\n\n\
Minecraft ต้องอนุมัติแอปใหม่ทุกตัวที่เรียก Java game service API (ดู aka.ms/AppRegInfo)\n\
ยื่นคำขอที่ https://aka.ms/mce-reviewappid ด้วย Application (client) ID ของแอปนี้ แล้วรอผลตรวจ\n\n\
[AADSTS-free] Invalid app registration".to_string();
    }
    match code.and_then(aad_hint) {
        Some(hint) => format!("{}\n\n[AADSTS{}] {}", hint, code.unwrap_or(0), first),
        None => match code {
            Some(c) => format!("Microsoft ปฏิเสธ (HTTP {}, AADSTS{}): {}", status, c, first),
            None => format!("Microsoft ปฏิเสธ (HTTP {}): {}", status, first),
        },
    }
}

async fn post_json(url: &str, body: Value) -> Result<Value, String> {
    let resp = http_client()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed ({}): {}", url, e))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("read failed ({}): {}", url, e))?;
    let value: Value = serde_json::from_str(&text)
        .unwrap_or_else(|_| json!({ "raw": text.chars().take(300).collect::<String>() }));
    if !status.is_success() {
        return Err(aad_error_text(&value, status.as_u16()));
    }
    Ok(value)
}

/// Asks Entra for a device code; the raw payload is kept so callers can read
/// `error_codes` when it fails.
async fn request_device_code(client_id: &str) -> Result<Value, (Option<i64>, String)> {
    // Entra wants `application/x-www-form-urlencoded` here; a JSON body is
    // rejected with AADSTS900144 ("request body must contain client_id").
    let resp = http_client()
        .post(DEVICE_CODE)
        .form(&[("client_id", client_id.trim()), ("scope", SCOPE)])
        .send()
        .await
        .map_err(|e| {
            (
                None,
                format!("ต่อ login.microsoftonline.com ไม่ได้: {}", e),
            )
        })?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| (None, format!("อ่านผลไม่ได้: {}", e)))?;
    let value: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({ "raw": text }));
    if !status.is_success() {
        return Err((
            aad_code(&value),
            aad_error_text(&value, status.as_u16()),
        ));
    }
    Ok(value)
}

/// Pre-flight from the settings page: is this client id usable for the device
/// code flow at all? Catches the usual Azure mis-configuration before the user
/// types a code into the browser.
pub async fn check_client(client_id: &str) -> ClientCheck {
    if client_id.trim().is_empty() {
        return ClientCheck {
            ok: false,
            code: None,
            message: "ยังไม่ได้ใส่ Azure client id".to_string(),
        };
    }
    match request_device_code(client_id).await {
        Ok(_) => ClientCheck {
            ok: true,
            code: None,
            message: "client id ใช้งานได้ — กด “เข้าสู่ระบบด้วย Microsoft” ได้เลย".to_string(),
        },
        Err((code, message)) => ClientCheck {
            ok: false,
            code: code.map(|c| format!("AADSTS{}", c)),
            message,
        },
    }
}

/// Step 1 — ask Microsoft for a code the user types into their browser.
pub async fn device_start(client_id: &str) -> Result<DeviceStart, String> {
    if client_id.trim().is_empty() {
        return Err("ยังไม่ได้ใส่ Azure client id — ไปที่หน้าตั้งค่าแล้วกรอกก่อน".to_string());
    }
    let v = request_device_code(client_id).await.map_err(|(_, m)| m)?;

    let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Ok(DeviceStart {
        device_code: get("device_code"),
        user_code: get("user_code"),
        verification_uri: {
            let u = get("verification_uri");
            if u.is_empty() {
                "https://microsoft.com/link".to_string()
            } else {
                u
            }
        },
        interval: v.get("interval").and_then(|x| x.as_u64()).unwrap_or(5),
        expires_in: v.get("expires_in").and_then(|x| x.as_u64()).unwrap_or(900),
        message: get("message"),
    })
}

/// Step 2 — poll until the user finishes signing in.
/// `Ok(None)` means "still waiting", `Ok(Some(_))` means an account is ready.
pub async fn device_poll(client_id: &str, device_code: &str) -> Result<Option<Account>, String> {
    let resp = http_client()
        .post(TOKEN)
        .form(&[
            ("client_id", client_id.trim()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", device_code),
        ])
        .send()
        .await
        .map_err(|e| format!("token request failed: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("token read failed: {}", e))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("token parse failed: {}", e))?;

    if let Some(err) = v.get("error").and_then(|x| x.as_str()) {
        return match err {
            "authorization_pending" | "slow_down" => Ok(None),
            "expired_token" => Err("โค้ดหมดอายุแล้ว กดเข้าสู่ระบบใหม่".to_string()),
            "authorization_declined" => Err("คุณยกเลิกการเข้าสู่ระบบ".to_string()),
            other => Err(match aad_code(&v).and_then(aad_hint) {
                Some(hint) => format!("{}\n\n[AADSTS{}] {}", hint, aad_code(&v).unwrap_or(0), other),
                None => format!(
                    "{}: {}",
                    other,
                    v.get("error_description")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                ),
            }),
        };
    }

    let refresh = v
        .get("refresh_token")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "ไม่มี refresh_token ในการตอบกลับ".to_string())?
        .to_string();
    let ms_access = v
        .get("access_token")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "ไม่มี access_token ในการตอบกลับ".to_string())?
        .to_string();

    let auth = chain(&ms_access).await?;
    let account_id = format!("ms-{}", auth.uuid.replace('-', ""));
    crate::state::save_tokens(
        &account_id,
        &TokenSet {
            refresh_token: refresh,
            uuid: auth.uuid.clone(),
            name: auth.name.clone(),
            xuid: auth.xuid.clone(),
        },
    );

    Ok(Some(Account {
        id: account_id,
        username: auth.name.clone(),
        kind: "Microsoft".to_string(),
        skin_seed: auth.name.chars().map(|c| c as u32).sum::<u32>() % 997,
        uuid: Some(auth.uuid),
    }))
}

fn refresh_token_of(account_id: &str) -> Result<TokenSet, String> {
    crate::state::load_tokens(account_id)
        .ok_or_else(|| "ไม่พบข้อมูลล็อกอินของบัญชีนี้ กรุณาเข้าสู่ระบบใหม่".to_string())
}

/// Exchanges the stored refresh token for a fresh Microsoft access token.
pub async fn refresh_ms(client_id: &str, account_id: &str) -> Result<String, String> {
    let tokens = refresh_token_of(account_id)?;
    let resp = http_client()
        .post(TOKEN)
        .form(&[
            ("client_id", client_id.trim()),
            ("grant_type", "refresh_token"),
            ("refresh_token", tokens.refresh_token.as_str()),
            ("scope", SCOPE),
        ])
        .send()
        .await
        .map_err(|e| format!("refresh request failed: {}", e))?;
    let text = resp
        .text()
        .await
        .map_err(|e| format!("refresh read failed: {}", e))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("refresh parse failed: {}", e))?;

    if let Some(err) = v.get("error").and_then(|x| x.as_str()) {
        return Err(match aad_code(&v).and_then(aad_hint) {
            Some(hint) => format!("รีเฟรชเซสชันไม่สำเร็จ — {}\n\n[AADSTS{}] {}", hint, aad_code(&v).unwrap_or(0), err),
            None => format!(
                "รีเฟรชเซสชันไม่สำเร็จ ({}): {}",
                err,
                v.get("error_description")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
            ),
        });
    }
    if let Some(new_refresh) = v.get("refresh_token").and_then(|x| x.as_str()) {
        let mut updated = tokens.clone();
        updated.refresh_token = new_refresh.to_string();
        crate::state::save_tokens(account_id, &updated);
    }
    v.get("access_token")
        .and_then(|x| x.as_str())
        .map(String::from)
        .ok_or_else(|| "ไม่มี access_token ในการรีเฟรช".to_string())
}

/// Xbox Live → XSTS → Minecraft Services → profile.
async fn chain(ms_access: &str) -> Result<McAuth, String> {
    let xbl = post_json(
        "https://user.auth.xboxlive.com/user/authenticate",
        json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", ms_access)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
    )
    .await
    .map_err(|e| format!("Xbox Live ปฏิเสธ: {}", e))?;

    let xbl_token = xbl
        .get("Token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Xbox Live ไม่ส่ง Token กลับมา".to_string())?
        .to_string();
    let uhs = xbl
        .get("DisplayClaims")
        .and_then(|d| d.get("xui"))
        .and_then(|x| x.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("uhs"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Xbox Live ไม่ส่ง user hash กลับมา".to_string())?
        .to_string();

    let xsts = post_json(
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )
    .await
    .map_err(|e| {
        format!(
            "XSTS ปฏิเสธ ({}) — บัญชีอาจยังไม่เคยสร้าง Xbox profile หรือติดข้อจำกัดอายุ",
            e
        )
    })?;

    let xsts_token = xsts
        .get("Token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "XSTS ไม่ส่ง Token กลับมา".to_string())?
        .to_string();
    let xuid = xsts
        .get("DisplayClaims")
        .and_then(|d| d.get("xui"))
        .and_then(|x| x.as_array())
        .and_then(|a| a.first())
        .and_then(|f| f.get("xid"))
        .and_then(|v| v.as_str())
        .unwrap_or("0")
        .to_string();

    let mc = post_json(
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        json!({ "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token) }),
    )
    .await
    .map_err(|e| format!("Minecraft Services ปฏิเสธ: {}", e))?;

    let access_token = mc
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Minecraft Services ไม่ส่ง access token กลับมา".to_string())?
        .to_string();

    let profile = http_client()
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&access_token)
        .send()
        .await
        .map_err(|e| format!("profile request failed: {}", e))?;
    if !profile.status().is_success() {
        return Err(
            "บัญชีนี้ยังไม่ได้ซื้อ Minecraft Java Edition หรือยังไม่ได้ตั้งชื่อผู้เล่น".to_string(),
        );
    }
    let pv: Value = profile
        .json()
        .await
        .map_err(|e| format!("profile parse failed: {}", e))?;

    Ok(McAuth {
        access_token,
        uuid: pv
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        name: pv
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Player")
            .to_string(),
        xuid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact body Minecraft Services returned for a client id that Mojang has
    /// not allow-listed (observed 2026-10-23 with a freshly registered app).
    #[test]
    fn minecraft_app_registration_403_explains_the_allow_list() {
        let body = json!({
            "path": "/authentication/login_with_xbox",
            "errorMessage": "Invalid app registration, see https://aka.ms/AppRegInfo for more information"
        });
        let msg = aad_error_text(&body, 403);
        assert!(msg.contains("mce-reviewappid"), "missing review form: {msg}");
        assert!(msg.contains("allow list"), "missing explanation: {msg}");
        assert!(!msg.contains("unknown error"), "raw fallback leaked: {msg}");
    }

    #[test]
    fn entra_errors_keep_their_hint() {
        let body = json!({
            "error": "invalid_client",
            "error_codes": [7000218],
            "error_description": "AADSTS7000218: The request body must contain client_assertion. Trace ID: abc"
        });
        let msg = aad_error_text(&body, 401);
        assert!(msg.contains("Allow public client flows = Yes"), "{msg}");
        assert!(msg.contains("AADSTS7000218"), "{msg}");
        assert!(!msg.contains("Trace ID"), "trace blob not trimmed: {msg}");
    }
}

/// Fresh Minecraft session token for a stored Microsoft account.
pub async fn launch_auth(client_id: &str, account_id: &str) -> Result<McAuth, String> {
    let ms_access = refresh_ms(client_id, account_id).await?;
    chain(&ms_access).await
}
