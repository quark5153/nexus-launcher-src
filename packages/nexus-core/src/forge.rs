// forge — Forge / NeoForge loader support.
//
// Both projects ship an installer that resolves their own processor chain
// (mappings, patched client jar, module path) into a `versions/<id>/<id>.json`
// profile. Rather than re-implementing that pipeline we download the official
// installer, run it headless against the instance folder, then read back the
// generated profile and merge it exactly like the Fabric/Quilt profiles.

use crate::engine::{http_client, DEFAULT_JAVA};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const FORGE_PROMOS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
const NEO_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";
const NEO_VERSIONS: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
const FABRIC_LOADERS: &str = "https://meta.fabricmc.net/v2/versions/loader";
const QUILT_LOADERS: &str = "https://meta.quiltmc.org/v3/versions/loader";

/// NeoForge uses `21.1.x` style versions for Minecraft `1.21.1`.
fn neo_prefix(mc: &str) -> String {
    let mut parts = mc.split('.');
    let _major = parts.next();
    let minor = parts.next().unwrap_or("21");
    let patch = parts.next().unwrap_or("0");
    format!("{}.{}", minor, patch)
}

async fn text(url: &str) -> Result<String, String> {
    let resp = http_client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request failed ({}): {}", url, e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} from {}", resp.status(), url));
    }
    resp.text()
        .await
        .map_err(|e| format!("read failed ({}): {}", url, e))
}

/// Picks the recommended (falling back to latest) loader build for a version.
pub async fn resolve_version(loader: &str, mc: &str) -> Result<String, String> {
    if loader == "forge" {
        let raw = text(FORGE_PROMOS).await?;
        let v: Value = serde_json::from_str(&raw).map_err(|e| format!("promos parse: {}", e))?;
        let promos = v
            .get("promos")
            .and_then(|p| p.as_object())
            .ok_or_else(|| "promotions_slim.json ไม่มี promos".to_string())?;
        for key in [format!("{}-recommended", mc), format!("{}-latest", mc)] {
            if let Some(ver) = promos.get(&key).and_then(|x| x.as_str()) {
                return Ok(ver.to_string());
            }
        }
        return Err(format!("Forge ไม่มีบิลด์สำหรับ Minecraft {}", mc));
    }

    if loader == "neoforge" {
        let raw = text(NEO_VERSIONS).await?;
        let v: Value = serde_json::from_str(&raw).map_err(|e| format!("neo list parse: {}", e))?;
        let prefix = neo_prefix(mc);
        let list: Vec<String> = v
            .get("versions")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        return list
            .into_iter()
            .rev()
            .find(|ver| ver.starts_with(&format!("{}.", prefix)))
            .ok_or_else(|| format!("NeoForge ไม่มีบิลด์สำหรับ Minecraft {}", mc));
    }

    Err(format!("ไม่รู้จัก loader '{}'", loader))
}

pub async fn list_versions(loader: &str, mc: &str) -> Result<Vec<String>, String> {
    if loader == "fabric" || loader == "quilt" {
        return list_loader_builds(loader, mc).await;
    }
    if loader == "forge" {
        let raw = text(FORGE_PROMOS).await?;
        let v: Value = serde_json::from_str(&raw).map_err(|e| format!("promos parse: {}", e))?;
        let mut out: Vec<String> = v
            .get("promos")
            .and_then(|p| p.as_object())
            .map(|promos| {
                promos
                    .iter()
                    .filter(|(k, _)| k.starts_with(&format!("{}-", mc)))
                    .filter_map(|(_, val)| val.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out.dedup();
        return Ok(out);
    }
    if loader == "neoforge" {
        let raw = text(NEO_VERSIONS).await?;
        let v: Value = serde_json::from_str(&raw).map_err(|e| format!("neo list parse: {}", e))?;
        let prefix = neo_prefix(mc);
        let mut out: Vec<String> = v
            .get("versions")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .filter(|s| s.starts_with(&format!("{}.", prefix)))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        return Ok(out);
    }
    Ok(Vec::new())
}

/// Every loader build published for one Minecraft version, newest first.
/// Fabric nests the build under `loader.version`, Quilt puts it at the top.
pub async fn list_loader_builds(loader: &str, mc: &str) -> Result<Vec<String>, String> {
    let base = match loader {
        "fabric" => FABRIC_LOADERS,
        "quilt" => QUILT_LOADERS,
        _ => return Err(format!("ไม่รู้จัก loader '{}'", loader)),
    };
    let raw = text(&format!("{}/{}", base, mc)).await?;
    let v: Value = serde_json::from_str(&raw).map_err(|e| format!("loader list parse: {}", e))?;
    let mut out: Vec<String> = v
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    entry
                        .get("loader")
                        .and_then(|l| l.get("version"))
                        .or_else(|| entry.get("version"))
                        .and_then(|x| x.as_str())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();
    out.dedup();
    Ok(out)
}

pub fn installer_url(loader: &str, mc: &str, ver: &str) -> String {
    if loader == "neoforge" {
        format!("{}/{}/neoforge-{}-installer.jar", NEO_MAVEN, ver, ver)
    } else {
        format!(
            "{}/{}-{}/forge-{}-{}-installer.jar",
            FORGE_MAVEN, mc, ver, mc, ver
        )
    }
}

fn find_profile(root: &Path, loader: &str) -> Option<(PathBuf, Value)> {
    let versions = root.join("versions");
    let rd = fs::read_dir(&versions).ok()?;
    let mut best: Option<(PathBuf, Value)> = None;
    for entry in rd.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.contains(loader) {
            continue;
        }
        let json_path = dir.join(format!("{}.json", name));
        let Ok(raw) = fs::read_to_string(&json_path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        best = match best {
            Some((path, prev)) => {
                let prev_ver = prev.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let new_ver = value.get("id").and_then(|v| v.as_str()).unwrap_or("");
                if new_ver > prev_ver {
                    Some((json_path, value))
                } else {
                    Some((path, prev))
                }
            }
            None => Some((json_path, value)),
        };
    }
    best
}

/// Installs (once per instance) Forge/NeoForge and returns its version profile.
pub async fn ensure_installed<F>(
    root: &Path,
    java: &str,
    loader: &str,
    mc: &str,
    wanted: Option<&str>,
    log: F,
) -> Result<Value, String>
where
    F: Fn(String) + Send + Sync,
{
    if let Some((_, profile)) = find_profile(root, loader) {
        log(format!(
            "[OK] ใช้ {} ที่ติดตั้งไว้แล้ว ({})",
            loader,
            profile
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
        ));
        return Ok(profile);
    }

    let version = match wanted.filter(|v| !v.is_empty()) {
        Some(v) => v.to_string(),
        None => resolve_version(loader, mc).await?,
    };
    let url = installer_url(loader, mc, &version);
    log(format!("[..] ดาวน์โหลดตัวติดตั้ง {} {}", loader, version));

    let bytes = http_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("ดาวน์โหลด installer ไม่สำเร็จ: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("อ่าน installer ไม่สำเร็จ: {}", e))?;

    fs::create_dir_all(root).map_err(|e| format!("สร้างโฟลเดอร์อินสแตนซ์: {}", e))?;
    let installer = root.join(format!("{}-installer.jar", loader));
    fs::write(&installer, &bytes).map_err(|e| format!("บันทึก installer: {}", e))?;

    // The modern installers expect a launcher_profiles.json to exist.
    let profiles = root.join("launcher_profiles.json");
    if !profiles.exists() {
        let _ = fs::write(&profiles, json!({"profiles": {}, "settings": {}}).to_string());
    }

    let java_bin = if java.trim().is_empty() {
        DEFAULT_JAVA.to_string()
    } else {
        java.to_string()
    };
    log(format!(
        "[..] กำลังรันตัวติดตั้ง {} (อาจใช้เวลา 1–3 นาที)",
        loader
    ));
    let root_owned = root.to_path_buf();
    let installer_owned = installer.clone();
    let status = tokio::task::spawn_blocking(move || {
        std::process::Command::new(&java_bin)
            .arg("-jar")
            .arg(&installer_owned)
            .arg("--installClient")
            .arg(&root_owned)
            .current_dir(&root_owned)
            .output()
    })
    .await
    .map_err(|e| format!("รัน installer ไม่สำเร็จ: {}", e))?
    .map_err(|e| format!("เรียก java ไม่สำเร็จ: {}", e))?;

    let _ = fs::remove_file(&installer);

    let Some((profile_path, profile)) = find_profile(root, loader) else {
        let stderr = String::from_utf8_lossy(&status.stderr);
        return Err(format!(
            "ติดตั้ง {} ไม่สำเร็จ (exit {}) {}",
            loader,
            status.status.code().unwrap_or(-1),
            stderr.chars().take(400).collect::<String>()
        ));
    };

    // Cache next to the instance so later launches skip the scan.
    let _ = fs::write(
        root.join("loader.json"),
        serde_json::to_string_pretty(&profile).unwrap_or_default(),
    );
    log(format!(
        "[OK] ติดตั้ง {} {} เสร็จ: {:?}",
        loader, version, profile_path
    ));
    Ok(profile)
}
