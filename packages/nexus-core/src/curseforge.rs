// curseforge — CurseForge provider (search, files, mod + modpack install).
//
// CurseForge requires a user supplied API key; it is read from the local
// config and only ever sent to api.curseforge.com.

use crate::engine::{http_client, slug, Artifact, DeltaSyncEngine, SandboxEnvironment};
use crate::state::InstanceRecord;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;

const API: &str = "https://api.curseforge.com/v1";
const GAME_ID: u32 = 432; // Minecraft

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfHit {
    pub id: u64,
    pub name: String,
    pub slug: String,
    pub summary: String,
    pub download_count: u64,
    pub icon_url: Option<String>,
    pub class_id: Option<u32>,
    pub latest_file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfFile {
    pub id: u64,
    pub mod_id: u64,
    pub display_name: String,
    pub file_name: String,
    pub download_url: Option<String>,
    pub release_type: u32,
    pub file_date: String,
    pub game_versions: Vec<String>,
}

fn class_id(project_type: &str) -> u32 {
    if project_type == "modpack" {
        4471
    } else {
        6
    }
}

fn loader_type(loader: Option<&str>) -> Option<u32> {
    match loader {
        Some("forge") => Some(1),
        Some("fabric") => Some(4),
        Some("quilt") => Some(5),
        Some("neoforge") => Some(6),
        _ => None,
    }
}

async fn get_json(url: &str, api_key: &str) -> Result<serde_json::Value, String> {
    if api_key.trim().is_empty() {
        return Err("ยังไม่ได้ใส่ CurseForge API key (ไปที่หน้าตั้งค่า)".to_string());
    }
    let resp = http_client()
        .get(url)
        .header("x-api-key", api_key.trim())
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("CurseForge request failed: {}", e))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("CurseForge read failed: {}", e))?;
    if !status.is_success() {
        return Err(format!(
            "CurseForge HTTP {}: {}",
            status,
            text.chars().take(180).collect::<String>()
        ));
    }
    serde_json::from_str(&text).map_err(|e| format!("CurseForge parse failed: {}", e))
}

fn urlencode(input: &str) -> String {
    let mut res = String::new();
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                res.push(b as char)
            }
            b' ' => res.push_str("%20"),
            _ => res.push_str(&format!("%{:02X}", b)),
        }
    }
    res
}

pub async fn search(
    query: &str,
    project_type: &str,
    api_key: &str,
    limit: usize,
) -> Result<Vec<CfHit>, String> {
    let url = format!(
        "{}/mods/search?gameId={}&classId={}&searchFilter={}&pageSize={}&sortField=2&sortOrder=desc",
        API,
        GAME_ID,
        class_id(project_type),
        urlencode(query),
        limit
    );
    let v = get_json(&url, api_key).await?;
    let data = v
        .get("data")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(data
        .iter()
        .map(|m| CfHit {
            id: m.get("id").and_then(|x| x.as_u64()).unwrap_or(0),
            name: m
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("?")
                .to_string(),
            slug: m
                .get("slug")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            summary: m
                .get("summary")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            download_count: m.get("downloadCount").and_then(|x| x.as_u64()).unwrap_or(0),
            icon_url: m
                .get("logo")
                .and_then(|l| l.get("thumbnailUrl"))
                .and_then(|x| x.as_str())
                .map(String::from),
            class_id: m.get("classId").and_then(|x| x.as_u64()).map(|x| x as u32),
            latest_file_name: m
                .get("latestFiles")
                .and_then(|f| f.as_array())
                .and_then(|a| a.first())
                .and_then(|f| f.get("fileName"))
                .and_then(|x| x.as_str())
                .map(String::from),
        })
        .collect())
}

fn parse_file(v: &serde_json::Value) -> CfFile {
    CfFile {
        id: v.get("id").and_then(|x| x.as_u64()).unwrap_or(0),
        mod_id: v.get("modId").and_then(|x| x.as_u64()).unwrap_or(0),
        display_name: v
            .get("displayName")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        file_name: v
            .get("fileName")
            .and_then(|x| x.as_str())
            .unwrap_or("download.jar")
            .to_string(),
        download_url: v.get("downloadUrl").and_then(|x| x.as_str()).map(String::from),
        release_type: v.get("releaseType").and_then(|x| x.as_u64()).unwrap_or(1) as u32,
        file_date: v
            .get("fileDate")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        game_versions: v
            .get("gameVersions")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub async fn files(
    mod_id: u64,
    mc: &str,
    loader: Option<&str>,
    api_key: &str,
) -> Result<Vec<CfFile>, String> {
    let mut url = format!(
        "{}/mods/{}/files?gameVersion={}&pageSize=50",
        API,
        mod_id,
        urlencode(mc)
    );
    if let Some(t) = loader_type(loader) {
        url.push_str(&format!("&modLoaderType={}", t));
    }
    let v = get_json(&url, api_key).await?;
    let data = v
        .get("data")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(data.iter().map(parse_file).collect())
}

/// Newest file of a project regardless of game version (used for modpacks).
pub async fn latest_file(mod_id: u64, api_key: &str) -> Result<CfFile, String> {
    let url = format!("{}/mods/{}/files?pageSize=20", API, mod_id);
    let v = get_json(&url, api_key).await?;
    let data = v
        .get("data")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();
    data.first()
        .map(parse_file)
        .ok_or_else(|| "โปรเจกต์นี้ไม่มีไฟล์ให้ดาวน์โหลด".to_string())
}

fn cdn_url(file: &CfFile) -> String {
    format!(
        "https://edge.forgecdn.net/files/{}/{}/{}",
        file.id / 1000,
        file.id % 1000,
        urlencode(&file.file_name)
    )
}

async fn file_url(file: &CfFile, api_key: &str) -> String {
    if let Some(url) = &file.download_url {
        return url.clone();
    }
    // Ask the API for a fresh link; fall back to the CDN layout.
    let url = format!("{}/mods/{}/files/{}/download-url", API, file.mod_id, file.id);
    match get_json(&url, api_key).await {
        Ok(v) => v
            .get("data")
            .and_then(|d| d.as_str())
            .map(String::from)
            .unwrap_or_else(|| cdn_url(file)),
        Err(_) => cdn_url(file),
    }
}

async fn download(url: &str) -> Result<Vec<u8>, String> {
    let resp = http_client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("download failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("download HTTP {} for {}", resp.status(), url));
    }
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("download read failed: {}", e))
}

#[derive(Debug, Deserialize)]
struct CfPackManifest {
    minecraft: CfPackMinecraft,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    files: Vec<CfPackFile>,
    #[serde(default = "default_overrides")]
    overrides: String,
}

fn default_overrides() -> String {
    "overrides".to_string()
}

#[derive(Debug, Deserialize)]
struct CfPackMinecraft {
    version: String,
    #[serde(rename = "modLoaders", default)]
    mod_loaders: Vec<CfPackLoader>,
}

#[derive(Debug, Deserialize)]
struct CfPackLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Deserialize)]
struct CfPackFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
}

/// Installs a CurseForge modpack zip (manifest.json + overrides).
pub async fn install_pack(
    bytes: &[u8],
    pack_slug: &str,
    pack_name: &str,
    api_key: &str,
    base_dir: &Path,
) -> Result<InstanceRecord, String> {
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| format!("zip ไม่ถูกต้อง: {}", e))?;

    let manifest_data = {
        let mut file = zip
            .by_name("manifest.json")
            .map_err(|_| "ไม่พบ manifest.json ในแพ็ก CurseForge".to_string())?;
        let mut s = String::new();
        file.read_to_string(&mut s)
            .map_err(|e| format!("อ่าน manifest: {}", e))?;
        s
    };
    let manifest: CfPackManifest =
        serde_json::from_str(&manifest_data).map_err(|e| format!("manifest parse: {}", e))?;

    let mc_version = manifest.minecraft.version.clone();
    let loader = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first());
    let (loader_kind, loader_ver) = match loader {
        Some(l) => {
            let (kind, ver) = l.id.split_once('-').unwrap_or((l.id.as_str(), ""));
            (Some(kind.to_string()), Some(ver.to_string()))
        }
        None => (None, None),
    };

    let instance_id = format!("pack-{}", slug(pack_slug));
    let sandbox = SandboxEnvironment::initialize(base_dir, "modpack", &instance_id)?;

    let overrides_prefix = format!("{}/", manifest.overrides);
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| format!("zip entry: {}", e))?;
        let name = file.name().to_string();
        let Some(rel) = name.strip_prefix(&overrides_prefix) else {
            continue;
        };
        if rel.is_empty() {
            continue;
        }
        let dest = sandbox.root_dir.join(rel);
        if file.is_dir() {
            let _ = fs::create_dir_all(&dest);
        } else {
            if let Some(parent) = dest.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mut out = fs::File::create(&dest)
                .map_err(|e| format!("เขียน override {:?}: {}", dest, e))?;
            std::io::copy(&mut file, &mut out).map_err(|e| format!("copy override: {}", e))?;
        }
    }

    let mut artifacts: Vec<Artifact> = Vec::new();
    for entry in &manifest.files {
        let url = format!("{}/mods/{}/files/{}", API, entry.project_id, entry.file_id);
        let v = match get_json(&url, api_key).await {
            Ok(v) => v,
            Err(err) => {
                eprintln!("[nexus] curseforge file {} skipped: {}", entry.file_id, err);
                continue;
            }
        };
        let Some(data) = v.get("data") else { continue };
        let file = parse_file(data);
        let link = file_url(&file, api_key).await;
        artifacts.push(Artifact {
            relative_path: format!("mods/{}", file.file_name),
            download_url: link,
            expected_sha1: String::new(),
            size_bytes: 0,
        });
    }

    DeltaSyncEngine::new()
        .sync_sandbox(&sandbox.root_dir, &artifacts)
        .await?;
    let _ = fs::write(sandbox.root_dir.join(".nexus_installed"), b"ok");

    Ok(InstanceRecord {
        id: instance_id,
        name: if pack_name.is_empty() {
            if manifest.name.is_empty() {
                pack_slug.to_string()
            } else {
                manifest.name.clone()
            }
        } else {
            pack_name.to_string()
        },
        provider: "modpack".to_string(),
        mc_version,
        loader: loader_kind,
        loader_version: loader_ver,
        modpack_id: Some(pack_slug.to_string()),
        modpack_version: Some(if manifest.version.is_empty() {
            "curseforge".to_string()
        } else {
            manifest.version.clone()
        }),
        update_url: Some(format!(
            "curseforge://{}/{}",
            manifest.author, manifest.name
        )),
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        last_played: None,
        ram_mb: None,
        java_path: None,
        extra_jvm: None,
        notes: None,
        favorite: false,
    })
}

/// Installs one mod file into an existing instance's `mods/` folder.
pub async fn install_mod(
    mod_id: u64,
    instance_root: &Path,
    mc: &str,
    loader: Option<&str>,
    api_key: &str,
) -> Result<String, String> {
    let list = files(mod_id, mc, loader, api_key).await?;
    let file = list
        .iter()
        .find(|f| f.release_type == 1 && f.game_versions.iter().any(|g| g == mc))
        .or_else(|| list.first())
        .ok_or_else(|| "ไม่พบไฟล์ที่เข้ากับเวอร์ชันนี้".to_string())?;
    let url = file_url(file, api_key).await;
    let bytes = download(&url).await?;
    let mods = instance_root.join("mods");
    fs::create_dir_all(&mods).map_err(|e| format!("สร้างโฟลเดอร์ mods: {}", e))?;
    fs::write(mods.join(&file.file_name), &bytes)
        .map_err(|e| format!("บันทึกมอด: {}", e))?;
    Ok(file.file_name.clone())
}

#[allow(dead_code)]
pub fn descriptor(project_id: u64) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("source".to_string(), "curseforge".to_string());
    m.insert("id".to_string(), project_id.to_string());
    m
}
