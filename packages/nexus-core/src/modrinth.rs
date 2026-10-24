// modrinth — Modrinth provider: search, version pick, mod/modpack install,
// dependency resolution and pack update checks.

use crate::engine::{http_client, slug, Artifact, DeltaSyncEngine, SandboxEnvironment};
use crate::state::InstanceRecord;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const MODRINTH_API: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub project_id: String,
    pub project_type: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub categories: Vec<String>,
    pub client_side: String,
    pub server_side: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    hits: Vec<Hit>,
    #[serde(default)]
    total_hits: u64,
}

/// One page of search results, plus how many results the query has in total.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPage {
    pub hits: Vec<Hit>,
    pub total_hits: u64,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVersionFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
    pub hashes: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub dependency_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<ProjectVersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub date_published: String,
}

/// `modrinth.index.json` inside a .mrpack. The format is camelCase
/// (`formatVersion`, `versionId`, `fileSize`), so without the rename every
/// pack failed to parse — which looked like "modpacks do not work".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrpackIndex {
    #[serde(default)]
    pub format_version: u32,
    #[serde(default = "default_game")]
    pub game: String,
    #[serde(default)]
    pub version_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub files: Vec<MrpackFile>,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
}

fn default_game() -> String {
    "minecraft".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrpackFile {
    pub path: String,
    #[serde(default)]
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub downloads: Vec<String>,
    #[serde(default)]
    pub file_size: u64,
}

pub async fn search(
    query: &str,
    project_type: &str,
    limit: usize,
    offset: usize,
) -> Result<SearchPage, String> {
    let client = http_client();
    let facet = if !project_type.is_empty() {
        format!("&facets=[[\"project_type:{}\"]]", project_type)
    } else {
        String::new()
    };
    let url = format!(
        "{}/search?query={}&limit={}&offset={}{}",
        MODRINTH_API,
        urlencoding(query),
        limit,
        offset,
        facet
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Modrinth search error: {}", e))?;
    let text = resp
        .text()
        .await
        .map_err(|e| format!("Read search response: {}", e))?;
    let res: SearchResponse =
        serde_json::from_str(&text).map_err(|e| format!("Parse search response: {}", e))?;
    Ok(SearchPage {
        hits: res.hits,
        total_hits: res.total_hits,
        offset,
    })
}

pub async fn fetch_versions(project_id_or_slug: &str) -> Result<Vec<ProjectVersion>, String> {
    let client = http_client();
    let url = format!("{}/project/{}/version", MODRINTH_API, project_id_or_slug);
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Modrinth fetch versions: {}", e))?;
    let text = resp
        .text()
        .await
        .map_err(|e| format!("Read versions response: {}", e))?;
    let list: Vec<ProjectVersion> =
        serde_json::from_str(&text).map_err(|e| format!("Parse versions: {}", e))?;
    Ok(list)
}

pub async fn fetch_version(id: &str) -> Result<ProjectVersion, String> {
    let url = format!("{}/version/{}", MODRINTH_API, id);
    let resp = http_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Modrinth version request: {}", e))?;
    let text = resp
        .text()
        .await
        .map_err(|e| format!("Read version response: {}", e))?;
    serde_json::from_str(&text).map_err(|e| format!("Parse version: {}", e))
}

/// Best file for a concrete game version + loader.
pub fn pick_for(
    versions: &[ProjectVersion],
    mc: &str,
    loader: Option<&str>,
) -> Option<ProjectVersion> {
    let loader = loader.unwrap_or("vanilla");
    let matching: Vec<&ProjectVersion> = versions
        .iter()
        .filter(|v| v.game_versions.iter().any(|g| g == mc))
        .filter(|v| loader == "vanilla" || v.loaders.iter().any(|l| l == loader))
        .collect();
    let pool = if matching.is_empty() {
        versions.iter().collect::<Vec<_>>()
    } else {
        matching
    };
    pool.into_iter()
        .max_by(|a, b| {
            let rank = |v: &ProjectVersion| v.date_published.clone();
            rank(a).cmp(&rank(b))
        })
        .cloned()
}

fn primary_file(version: &ProjectVersion) -> Option<&ProjectVersionFile> {
    version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
}

async fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let resp = http_client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("download failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("download HTTP {}", resp.status()));
    }
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("download read failed: {}", e))
}

/// Downloads a mod plus its required dependencies into `mods/`.
pub async fn install_mod_with_deps<F>(
    slug_or_id: &str,
    instance: &InstanceRecord,
    root: &Path,
    log: F,
) -> Result<String, String>
where
    F: Fn(String) + Send + Sync,
{
    let versions = fetch_versions(slug_or_id).await?;
    let version = pick_for(&versions, &instance.mc_version, instance.loader.as_deref())
        .ok_or_else(|| "ไม่พบเวอร์ชันที่เข้ากับโปรไฟล์นี้".to_string())?;
    let file = primary_file(&version)
        .ok_or_else(|| "ไม่พบไฟล์ดาวน์โหลด".to_string())?
        .clone();

    let mods = root.join("mods");
    fs::create_dir_all(&mods).map_err(|e| format!("สร้างโฟลเดอร์ mods: {}", e))?;
    let bytes = download_file(&file.url).await?;
    fs::write(mods.join(&file.filename), &bytes).map_err(|e| format!("บันทึกมอด: {}", e))?;
    log(format!("[OK] ติดตั้ง {} ({})", version.name, file.filename));

    let mut installed = 0;
    for dep in &version.dependencies {
        if dep.dependency_type != "required" {
            continue;
        }
        let dep_version = match &dep.version_id {
            Some(id) => match fetch_version(id).await {
                Ok(v) => v,
                Err(err) => {
                    log(format!("[..] ข้าม dependency {}: {}", id, err));
                    continue;
                }
            },
            None => continue,
        };
        let Some(dep_file) = primary_file(&dep_version) else {
            continue;
        };
        if mods.join(&dep_file.filename).exists() {
            continue;
        }
        match download_file(&dep_file.url).await {
            Ok(data) => {
                let _ = fs::write(mods.join(&dep_file.filename), &data);
                installed += 1;
                log(format!("[OK] dependency: {}", dep_file.filename));
            }
            Err(err) => log(format!("[ERR] dependency ล้มเหลว: {}", err)),
        }
    }
    if installed > 0 {
        log(format!("[OK] ติดตั้ง dependency เพิ่ม {} ไฟล์", installed));
    }
    Ok(file.filename)
}

pub async fn install_mrpack(
    mrpack_bytes: &[u8],
    pack_slug: &str,
    pack_name: &str,
    update_url: Option<String>,
    base_dir: &Path,
) -> Result<InstanceRecord, String> {
    let reader = std::io::Cursor::new(mrpack_bytes);
    let mut zip =
        zip::ZipArchive::new(reader).map_err(|e| format!("Invalid mrpack zip: {}", e))?;

    let index_data = {
        let mut index_file = zip
            .by_name("modrinth.index.json")
            .map_err(|_| "mrpack missing modrinth.index.json".to_string())?;
        let mut s = String::new();
        index_file
            .read_to_string(&mut s)
            .map_err(|e| format!("Read index: {}", e))?;
        s
    };

    let index: MrpackIndex = serde_json::from_str(&index_data)
        .map_err(|e| format!("Parse modrinth.index.json: {}", e))?;

    let mc_version = index
        .dependencies
        .get("minecraft")
        .cloned()
        .unwrap_or_else(|| "1.21.1".to_string());

    let (loader, loader_ver) = if let Some(v) = index.dependencies.get("fabric-loader") {
        (Some("fabric".to_string()), Some(v.clone()))
    } else if let Some(v) = index.dependencies.get("quilt-loader") {
        (Some("quilt".to_string()), Some(v.clone()))
    } else if let Some(v) = index.dependencies.get("neoforge") {
        (Some("neoforge".to_string()), Some(v.clone()))
    } else if let Some(v) = index.dependencies.get("forge") {
        (Some("forge".to_string()), Some(v.clone()))
    } else {
        (None, None)
    };

    let instance_id = format!("pack-{}", slug(pack_slug));
    let sandbox = SandboxEnvironment::initialize(base_dir, "modpack", &instance_id)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| format!("Zip entry: {}", e))?;
        let name = file.name().to_string();
        let dest_rel = if let Some(stripped) = name.strip_prefix("overrides/") {
            Some(stripped.to_string())
        } else if let Some(stripped) = name.strip_prefix("client-overrides/") {
            Some(stripped.to_string())
        } else {
            None
        };

        if let Some(rel) = dest_rel {
            if rel.is_empty() {
                continue;
            }
            let dest_path = sandbox.root_dir.join(&rel);
            if file.is_dir() {
                let _ = fs::create_dir_all(&dest_path);
            } else {
                if let Some(parent) = dest_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let mut out = fs::File::create(&dest_path).map_err(|e| {
                    format!("Create override file {}: {}", dest_path.display(), e)
                })?;
                std::io::copy(&mut file, &mut out)
                    .map_err(|e| format!("Write override file: {}", e))?;
            }
        }
    }

    let mut artifacts = Vec::new();
    for f in &index.files {
        if let Some(dl_url) = f.downloads.first() {
            let sha1 = f.hashes.get("sha1").cloned().unwrap_or_default();
            artifacts.push(Artifact {
                relative_path: f.path.clone(),
                download_url: dl_url.clone(),
                expected_sha1: sha1,
                size_bytes: f.file_size,
            });
        }
    }

    let sync_engine = DeltaSyncEngine::new();
    sync_engine
        .sync_sandbox(&sandbox.root_dir, &artifacts)
        .await?;

    let _ = fs::write(sandbox.root_dir.join(".nexus_installed"), b"ok");

    Ok(InstanceRecord {
        id: instance_id,
        name: if !pack_name.is_empty() {
            pack_name.to_string()
        } else if !index.name.is_empty() {
            index.name.clone()
        } else {
            pack_slug.to_string()
        },
        provider: "modpack".to_string(),
        mc_version,
        loader,
        loader_version: loader_ver,
        modpack_id: Some(pack_slug.to_string()),
        modpack_version: Some(index.version_id),
        update_url,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        last_played: None,
        ram_mb: None,
        java_path: None,
        extra_jvm: None,
        notes: None,
        favorite: false,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub instance_id: String,
    pub name: String,
    pub current: String,
    pub latest: String,
    pub url: String,
}

/// Compares each installed pack with the newest Modrinth version it can run.
pub async fn check_updates(instances: &[InstanceRecord]) -> Vec<UpdateInfo> {
    let mut out = Vec::new();
    for inst in instances {
        let (Some(slug), Some(current)) = (&inst.modpack_id, &inst.modpack_version) else {
            continue;
        };
        if inst.provider != "modpack" {
            continue;
        }
        let Ok(versions) = fetch_versions(slug).await else {
            continue;
        };
        let Some(latest) = pick_for(&versions, &inst.mc_version, inst.loader.as_deref()) else {
            continue;
        };
        if &latest.version_number == current || &latest.id == current {
            continue;
        }
        let url = primary_file(&latest)
            .map(|f| f.url.clone())
            .unwrap_or_default();
        out.push(UpdateInfo {
            instance_id: inst.id.clone(),
            name: inst.name.clone(),
            current: current.clone(),
            latest: latest.version_number.clone(),
            url,
        });
    }
    out
}

/// Downloads and applies a pack update in place (data in `saves/` is kept).
pub async fn apply_update(
    info: &UpdateInfo,
    base_dir: &Path,
    log: impl Fn(String),
) -> Result<InstanceRecord, String> {
    if info.url.is_empty() {
        return Err("ไม่พบไฟล์อัปเดตของแพ็กนี้".to_string());
    }
    log(format!("[..] ดาวน์โหลดอัปเดต {} → {}", info.current, info.latest));
    let bytes = download_file(&info.url).await?;
    let mut rec = install_mrpack(&bytes, &info.instance_id, &info.name, None, base_dir).await?;
    rec.id = info.instance_id.clone();
    rec.name = info.name.clone();
    Ok(rec)
}

pub fn mods_dir(root: &Path) -> PathBuf {
    root.join("mods")
}

/* ── installed mod metadata (names + icons) ────────────────────────────── */

/// What the profile's mod list shows instead of a bare jar file name.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModMeta {
    pub title: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
}

/// One cache row: Modrinth is matched by jar hash, so the file stamp guards it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetaCacheEntry {
    size: u64,
    mtime: i64,
    #[serde(flatten)]
    meta: ModMeta,
}

const META_CACHE_FILE: &str = ".nexus-mod-meta.json";

fn file_sha1(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Some(hex::encode(hasher.finalize()))
}

fn file_stamp(path: &Path) -> (u64, i64) {
    let Ok(meta) = fs::metadata(path) else {
        return (0, 0);
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    (meta.len(), mtime)
}

/// Maps a jar to its Modrinth project, in bulk: `hash → project_id`.
async fn hash_to_projects(hashes: &[String]) -> HashMap<String, String> {
    let client = http_client();
    let mut out = HashMap::new();
    for chunk in hashes.chunks(100) {
        let body = serde_json::json!({ "hashes": chunk, "algorithm": "sha1" });
        let Ok(resp) = client.post(format!("{}/version_files", MODRINTH_API)).json(&body).send().await
        else {
            continue;
        };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(text) = resp.text().await else { continue };
        let Ok(map) = serde_json::from_str::<HashMap<String, ProjectVersion>>(&text) else {
            continue;
        };
        for (hash, version) in map {
            out.insert(hash, version.project_id);
        }
    }
    out
}

#[derive(Debug, Deserialize)]
struct ProjectInfo {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    icon_url: Option<String>,
}

async fn fetch_projects(ids: &[String]) -> HashMap<String, ModMeta> {
    let client = http_client();
    let mut out = HashMap::new();
    for chunk in ids.chunks(60) {
        let list = serde_json::to_string(chunk).unwrap_or_else(|_| "[]".to_string());
        let url = format!("{}/projects?ids={}", MODRINTH_API, urlencoding(&list));
        let Ok(resp) = client.get(url).send().await else { continue };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(text) = resp.text().await else { continue };
        let Ok(items) = serde_json::from_str::<Vec<ProjectInfo>>(&text) else {
            continue;
        };
        for item in items {
            out.insert(
                item.id.clone(),
                ModMeta {
                    title: item.title,
                    icon_url: item.icon_url,
                    slug: if item.slug.is_empty() { None } else { Some(item.slug) },
                    project_id: Some(item.id),
                },
            );
        }
    }
    out
}

/// Resolves title + icon for every jar in `mods/`. Cached per file stamp so the
/// profile list opens instantly after the first lookup; a failed lookup just
/// returns whatever is cached instead of breaking the list.
pub async fn resolve_mod_meta(root: &Path) -> HashMap<String, ModMeta> {
    let dir = mods_dir(root);
    let cache_path = root.join(META_CACHE_FILE);
    let mut cache: HashMap<String, MetaCacheEntry> = fs::read_to_string(&cache_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    let mut out: HashMap<String, ModMeta> = HashMap::new();
    /* (cache key, sha1) for jars Modrinth has not seen yet */
    let mut pending: Vec<(String, String)> = Vec::new();

    for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let raw_name = entry.file_name().to_string_lossy().to_string();
        if raw_name.starts_with('.') {
            continue;
        }
        let lower = raw_name.to_lowercase();
        if !lower.ends_with(".jar") && !lower.ends_with(".jar.disabled") {
            continue;
        }
        /* the content list drops the .disabled suffix, match that key */
        let key = raw_name.trim_end_matches(".disabled").to_string();
        let (size, mtime) = file_stamp(&path);
        if let Some(hit) = cache.get(&key) {
            if hit.size == size && hit.mtime == mtime {
                out.insert(key, hit.meta.clone());
                continue;
            }
        }
        if let Some(hash) = file_sha1(&path) {
            pending.push((key, hash));
        }
    }

    if !pending.is_empty() {
        let hashes: Vec<String> = pending.iter().map(|(_, h)| h.clone()).collect();
        let by_hash = hash_to_projects(&hashes).await;
        let mut ids: Vec<String> = by_hash.values().cloned().collect();
        ids.sort();
        ids.dedup();
        let projects = fetch_projects(&ids).await;

        for (key, hash) in pending {
            let Some(pid) = by_hash.get(&hash) else { continue };
            let Some(meta) = projects.get(pid) else { continue };
            let path = dir.join(&key);
            let (size, mtime) = if path.exists() {
                file_stamp(&path)
            } else {
                file_stamp(&dir.join(format!("{}.disabled", key)))
            };
            cache.insert(
                key.clone(),
                MetaCacheEntry {
                    size,
                    mtime,
                    meta: meta.clone(),
                },
            );
            out.insert(key, meta.clone());
        }
        if let Ok(json) = serde_json::to_string(&cache) {
            let _ = fs::write(&cache_path, json);
        }
    }

    out
}

fn urlencoding(input: &str) -> String {
    let mut res = String::new();
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                res.push(b as char);
            }
            b' ' => res.push('+'),
            _ => res.push_str(&format!("%{:02X}", b)),
        }
    }
    res
}
