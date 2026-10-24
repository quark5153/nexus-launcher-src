// vanilla — extracted from the original monolithic lib.rs into the nexus-core crate.

use crate::engine::{
    Artifact, CustomClientHooks, DeltaSyncEngine, SandboxEnvironment, PATH_SEP,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(rename = "releaseTime", default)]
    pub release_time: String,
}

#[derive(Deserialize)]
struct ManifestDoc {
    versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone)]
pub struct LaunchSettings {
    pub java_path: String,
    pub ram_mb: u32,
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
    pub xuid: String,
    pub extra_jvm: String,
}

#[derive(Debug, Clone)]
pub struct PreparedVersion {
    pub vjson: Value,
    pub main_class: String,
    pub classpath: Vec<PathBuf>,
    pub assets_index_id: String,
    pub java_major: u32,
}

pub async fn fetch_manifest() -> Result<Vec<VersionEntry>, String> {
    let client = crate::engine::http_client();
    let resp = client
        .get(MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("Version list request failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Version list HTTP {}", resp.status()));
    }
    let doc: ManifestDoc = resp
        .json()
        .await
        .map_err(|e| format!("Version list parse error: {}", e))?;
    Ok(doc.versions)
}

fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn rules_allow(rules: Option<&Value>) -> bool {
    let arr = match rules.and_then(|r| r.as_array()) {
        Some(a) => a,
        None => return true,
    };
    if arr.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in arr {
        let mut matches = true;
        if let Some(os) = rule.get("os") {
            if let Some(n) = os.get("name").and_then(|v| v.as_str()) {
                if n != os_name() {
                    matches = false;
                }
            }
            if let Some(a) = os.get("arch").and_then(|v| v.as_str()) {
                if a == "x86" && cfg!(target_pointer_width = "64") {
                    matches = false;
                }
            }
        }
        if rule.get("features").is_some() {
            matches = false;
        }
        if matches {
            allowed = rule.get("action").and_then(|v| v.as_str()) == Some("allow");
        }
    }
    allowed
}

fn artifact_from(v: &Value, prefix: &str) -> Option<Artifact> {
    let path = v.get("path").and_then(|p| p.as_str())?;
    let url = v.get("url").and_then(|p| p.as_str())?;
    Some(Artifact {
        relative_path: format!("{}/{}", prefix, path),
        download_url: url.to_string(),
        expected_sha1: v
            .get("sha1")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        size_bytes: v.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
    })
}

pub fn offline_uuid(name: &str) -> String {
    let mut h = Sha1::new();
    h.update(format!("OfflinePlayer:{}", name).as_bytes());
    let d = h.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    let s = hex::encode(b);
    format!(
        "{}-{}-{}-{}-{}",
        &s[0..8],
        &s[8..12],
        &s[12..16],
        &s[16..20],
        &s[20..32]
    )
}

/// Java major the version json asks for (0 when it cannot be read). Needed
/// before `prepare` because the Forge/NeoForge installer runs with that JVM.
pub async fn required_java_major(sandbox: &SandboxEnvironment, entry: &VersionEntry) -> u32 {
    match load_version_json(sandbox, entry).await {
        Ok(v) => v
            .get("javaVersion")
            .and_then(|j| j.get("majorVersion"))
            .and_then(|m| m.as_u64())
            .unwrap_or(0) as u32,
        Err(_) => 0,
    }
}

async fn load_version_json(
    sandbox: &SandboxEnvironment,
    entry: &VersionEntry,
) -> Result<Value, String> {
    let cache = sandbox.root_dir.join("version.json");
    if let Ok(text) = fs::read_to_string(&cache) {
        if let Ok(v) = serde_json::from_str::<Value>(&text) {
            return Ok(v);
        }
    }
    let client = crate::engine::http_client();
    let resp = client
        .get(&entry.url)
        .send()
        .await
        .map_err(|e| format!("Version json request failed: {}", e))?;
    let text = resp
        .text()
        .await
        .map_err(|e| format!("Version json read error: {}", e))?;
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("Version json parse error: {}", e))?;
    let _ = fs::write(&cache, text);
    Ok(v)
}

fn extract_natives(natives_dir: &Path, jars: &[(PathBuf, Vec<String>)]) -> Result<(), String> {
    for (jar, excludes) in jars {
        let file = fs::File::open(jar).map_err(|e| format!("Open {:?}: {}", jar, e))?;
        let mut zip =
            zip::ZipArchive::new(file).map_err(|e| format!("Zip {:?}: {}", jar, e))?;
        for i in 0..zip.len() {
            let mut entry = zip
                .by_index(i)
                .map_err(|e| format!("Zip entry error: {}", e))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();
            if excludes.iter().any(|ex| name.starts_with(ex.as_str())) {
                continue;
            }
            let lower = name.to_lowercase();
            let is_native = lower.ends_with(".dll")
                || lower.ends_with(".so")
                || lower.ends_with(".dylib")
                || lower.ends_with(".jnilib");
            if !is_native {
                continue;
            }
            let fname = match Path::new(&name).file_name() {
                Some(f) => f.to_os_string(),
                None => continue,
            };
            let mut out = fs::File::create(natives_dir.join(fname))
                .map_err(|e| format!("Create native file: {}", e))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| format!("Extract native file: {}", e))?;
        }
    }
    Ok(())
}

async fn http_text(url: &str) -> Result<String, String> {
    let resp = crate::engine::http_client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request failed ({}): {}", url, e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} from {}", resp.status(), url));
    }
    resp.text()
        .await
        .map_err(|e| format!("read error ({}): {}", url, e))
}

async fn load_loader_json(
    sandbox: &SandboxEnvironment,
    kind: &str,
    mc: &str,
    wanted: Option<&str>,
    java: &str,
) -> Result<Value, String> {
    let cache = sandbox.root_dir.join("loader.json");
    if let Ok(text) = fs::read_to_string(&cache) {
        if let Ok(v) = serde_json::from_str::<Value>(&text) {
            return Ok(v);
        }
    }
    if kind == "forge" || kind == "neoforge" {
        let profile = crate::forge::ensure_installed(
            &sandbox.root_dir,
            java,
            kind,
            mc,
            wanted,
            |line| eprintln!("{}", line),
        )
        .await?;
        return Ok(profile);
    }
    let base = match kind {
        "fabric" => "https://meta.fabricmc.net/v2",
        "quilt" => "https://meta.quiltmc.org/v3",
        other => {
            return Err(format!(
                "loader '{}' ไม่รองรับ (vanilla / fabric / quilt / forge / neoforge)",
                other
            ))
        }
    };
    let version = match wanted {
        Some(v) => v.to_string(),
        None => {
            let text = http_text(&format!("{}/versions/loader/{}", base, mc)).await?;
            let list: Value =
                serde_json::from_str(&text).map_err(|e| format!("loader list parse: {}", e))?;
            let arr = list.as_array().cloned().unwrap_or_default();
            let pick = arr
                .iter()
                .find(|e| {
                    e.get("loader")
                        .and_then(|l| l.get("stable"))
                        .and_then(|s| s.as_bool())
                        .unwrap_or(false)
                })
                .or_else(|| arr.first())
                .and_then(|e| e.get("loader"))
                .and_then(|l| l.get("version"))
                .and_then(|v| v.as_str())
                .map(String::from);
            pick.ok_or_else(|| format!("no {} loader available for Minecraft {}", kind, mc))?
        }
    };
    let text = http_text(&format!(
        "{}/versions/loader/{}/{}/profile/json",
        base, mc, version
    ))
    .await?;
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("loader profile parse: {}", e))?;
    let _ = fs::write(&cache, text);
    Ok(v)
}

fn merge_loader(base: &mut Value, loader: &Value) {
    let mut libs: Vec<Value> = loader
        .get("libraries")
        .and_then(|l| l.as_array())
        .cloned()
        .unwrap_or_default();
    if let Some(bl) = base.get("libraries").and_then(|l| l.as_array()) {
        libs.extend(bl.iter().cloned());
    }
    base["libraries"] = Value::Array(libs);
    if let Some(mc) = loader.get("mainClass").cloned() {
        base["mainClass"] = mc;
    }
    for key in ["jvm", "game"] {
        let extra: Vec<Value> = loader
            .get("arguments")
            .and_then(|a| a.get(key))
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        if extra.is_empty() {
            continue;
        }
        if let Some(arr) = base
            .get_mut("arguments")
            .and_then(|a| a.get_mut(key))
            .and_then(|a| a.as_array_mut())
        {
            arr.extend(extra);
        }
    }
}

fn maven_artifact(lib: &Value) -> Option<Artifact> {
    let name = lib.get("name")?.as_str()?;
    let url = lib.get("url")?.as_str()?;
    // Maven coords may carry `group:artifact:version[:classifier][:ext]` and an
    // explicit `@extension` suffix (Forge profiles use both forms).
    let (coords, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e.to_string()),
        None => (name, "jar".to_string()),
    };
    let parts: Vec<&str> = coords.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0];
    let artifact = parts[1];
    let version = parts[2];
    let (classifier, ext) = match parts.len() {
        3 => (None, ext),
        4 => (Some(parts[3]), ext),
        _ => (Some(parts[3]), parts[4].to_string()),
    };
    let file = match classifier {
        Some(c) => format!("{}-{}-{}.{}", artifact, version, c, ext),
        None => format!("{}-{}.{}", artifact, version, ext),
    };
    let path = format!("{}/{}/{}/{}", group.replace('.', "/"), artifact, version, file);
    let base = if url.ends_with('/') {
        url.to_string()
    } else {
        format!("{}/", url)
    };
    Some(Artifact {
        relative_path: format!("libraries/{}", path),
        download_url: format!("{}{}", base, path),
        expected_sha1: lib
            .get("sha1")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string(),
        size_bytes: lib.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
    })
}

pub async fn prepare<FLog, FProg>(
    sync: &DeltaSyncEngine,
    sandbox: &SandboxEnvironment,
    entry: &VersionEntry,
    loader: Option<&(String, Option<String>)>,
    java_path: &str,
    on_log: FLog,
    on_prog: FProg,
) -> Result<PreparedVersion, String>
where
    FLog: Fn(String) + Send + Sync + 'static,
    FProg: Fn(String, f32) + Send + Sync + 'static,
{
    on_log(format!("[..] กำลังเตรียมข้อมูลเวอร์ชัน {}", entry.id));
    on_prog("กำลังโหลดข้อมูลเวอร์ชัน".to_string(), 0.05);

    let mut vjson = load_version_json(sandbox, entry).await?;
    if let Some((kind, wanted)) = loader {
        let lj = load_loader_json(sandbox, kind, &entry.id, wanted.as_deref(), java_path).await?;
        merge_loader(&mut vjson, &lj);
        on_log(format!("[..] ผสาน Mod Loader: {}", kind));
    }

    let mut artifacts: Vec<Artifact> = Vec::new();
    let mut classpath: Vec<PathBuf> = Vec::new();
    let mut natives: Vec<(PathBuf, Vec<String>)> = Vec::new();

    if let Some(libs) = vjson.get("libraries").and_then(|v| v.as_array()) {
        for lib in libs {
            if !rules_allow(lib.get("rules")) {
                continue;
            }
            let dl = lib.get("downloads");
            let mut handled = false;
            if let Some(art) = dl.and_then(|d| d.get("artifact")) {
                if let Some(a) = artifact_from(art, "libraries") {
                    classpath.push(sandbox.root_dir.join(&a.relative_path));
                    artifacts.push(a);
                    handled = true;
                }
            }
            if !handled && lib.get("natives").is_none() {
                if let Some(a) = maven_artifact(lib) {
                    classpath.push(sandbox.root_dir.join(&a.relative_path));
                    artifacts.push(a);
                }
            }
            if let Some(classifier) = lib
                .get("natives")
                .and_then(|n| n.get(os_name()))
                .and_then(|v| v.as_str())
            {
                let key = classifier.replace("${arch}", "64");
                if let Some(a) = dl
                    .and_then(|d| d.get("classifiers"))
                    .and_then(|c| c.get(&key))
                    .and_then(|c| artifact_from(c, "libraries"))
                {
                    let excludes: Vec<String> = lib
                        .get("extract")
                        .and_then(|e| e.get("exclude"))
                        .and_then(|e| e.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|s| s.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    natives.push((sandbox.root_dir.join(&a.relative_path), excludes));
                    artifacts.push(a);
                }
            }
        }
    }

    let client_dl = vjson
        .get("downloads")
        .and_then(|d| d.get("client"))
        .ok_or_else(|| "Version has no client download".to_string())?;
    artifacts.push(Artifact {
        relative_path: "client.jar".to_string(),
        download_url: client_dl
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or_else(|| "Client url missing".to_string())?
            .to_string(),
        expected_sha1: client_dl
            .get("sha1")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string(),
        size_bytes: client_dl.get("size").and_then(|u| u.as_u64()).unwrap_or(0),
    });
    classpath.push(sandbox.root_dir.join("client.jar"));

    let ai = vjson
        .get("assetIndex")
        .ok_or_else(|| "Version has no assetIndex".to_string())?;
    let assets_index_id = ai
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("legacy")
        .to_string();
    let index_art = Artifact {
        relative_path: format!("assets/indexes/{}.json", assets_index_id),
        download_url: ai
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Asset index url missing".to_string())?
            .to_string(),
        expected_sha1: ai
            .get("sha1")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        size_bytes: ai.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
    };
    sync.sync_sandbox(&sandbox.root_dir, &[index_art.clone()])
        .await?;

    let index_text = fs::read_to_string(sandbox.root_dir.join(&index_art.relative_path))
        .map_err(|e| format!("Read asset index: {}", e))?;
    let index_json: Value = serde_json::from_str(&index_text)
        .map_err(|e| format!("Parse asset index: {}", e))?;

    if let Some(objs) = index_json.get("objects").and_then(|o| o.as_object()) {
        for (_name, obj) in objs {
            let hash = obj.get("hash").and_then(|h| h.as_str()).unwrap_or("");
            if hash.len() < 2 {
                continue;
            }
            let size = obj.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
            let prefix = &hash[..2];
            artifacts.push(Artifact {
                relative_path: format!("assets/objects/{}/{}", prefix, hash),
                download_url: format!(
                    "https://resources.download.minecraft.net/{}/{}",
                    prefix, hash
                ),
                expected_sha1: hash.to_string(),
                size_bytes: size,
            });
        }
    }

    let mut seen_paths: HashSet<String> = HashSet::new();
    artifacts.retain(|a| seen_paths.insert(a.relative_path.clone()));
    on_log(format!(
        "[..] ตรวจสอบไฟล์ทั้งหมด {} รายการด้วย SHA-1 Delta Sync",
        artifacts.len()
    ));

    let prog_arc = Arc::new(on_prog);
    let prog_cb_clone = prog_arc.clone();
    let cb: Arc<dyn Fn(usize, usize) + Send + Sync> = Arc::new(move |done, total| {
        prog_cb_clone(
            format!("กำลังซิงค์ไฟล์ {}/{}", done, total),
            done as f32 / total.max(1) as f32,
        );
    });
    sync.sync_sandbox_progress(&sandbox.root_dir, &artifacts, cb)
        .await?;

    prog_arc("กำลังแตกไฟล์ Natives...".to_string(), 1.0);
    extract_natives(&sandbox.natives_dir, &natives)?;

    let main_class = vjson
        .get("mainClass")
        .and_then(|m| m.as_str())
        .unwrap_or("net.minecraft.client.main.Main")
        .to_string();
    let java_major = vjson
        .get("javaVersion")
        .and_then(|j| j.get("majorVersion"))
        .and_then(|m| m.as_u64())
        .unwrap_or(8) as u32;

    on_log(format!("[OK] เตรียมความพร้อม {} เสร็จสมบูรณ์", entry.id));
    Ok(PreparedVersion {
        vjson,
        main_class,
        classpath,
        assets_index_id,
        java_major,
    })
}

fn substitute(input: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = input.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("${{{}}}", k), v);
    }
    while let Some(start) = out.find("${") {
        match out[start..].find('}') {
            Some(end) => out.replace_range(start..start + end + 1, ""),
            None => break,
        }
    }
    out
}

fn expand_args(node: Option<&Value>, vars: &HashMap<&str, String>) -> Vec<String> {
    let mut out = Vec::new();
    let arr = match node.and_then(|n| n.as_array()) {
        Some(a) => a,
        None => return out,
    };
    for item in arr {
        match item {
            Value::String(s) => out.push(substitute(s, vars)),
            Value::Object(_) => {
                if rules_allow(item.get("rules")) {
                    match item.get("value") {
                        Some(Value::String(s)) => out.push(substitute(s, vars)),
                        Some(Value::Array(a)) => {
                            for v in a {
                                if let Some(s) = v.as_str() {
                                    out.push(substitute(s, vars));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn quote(arg: &str) -> String {
    let needs = arg.is_empty() || arg.chars().any(|c| c == ' ' || c == '\t' || c == '"');
    if needs {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

pub fn build_arguments(
    sandbox: &SandboxEnvironment,
    prepared: &PreparedVersion,
    settings: &LaunchSettings,
    hooks: Option<&CustomClientHooks>,
) -> String {
    let root = sandbox.root_dir.to_string_lossy().to_string();
    let natives = sandbox.natives_dir.to_string_lossy().to_string();
    let assets = sandbox.assets_dir.to_string_lossy().to_string();
    let classpath = prepared
        .classpath
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(PATH_SEP);

    let version_id = prepared
        .vjson
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let version_type = prepared
        .vjson
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("release")
        .to_string();

    let uuid = if settings.uuid.is_empty() {
        offline_uuid(&settings.username)
    } else {
        settings.uuid.clone()
    };
    let token = if settings.access_token.is_empty() {
        "0".to_string()
    } else {
        settings.access_token.clone()
    };
    let library_directory = sandbox.libraries_dir.to_string_lossy().to_string();

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", settings.username.clone());
    vars.insert("version_name", version_id);
    vars.insert("game_directory", root);
    vars.insert("assets_root", assets.clone());
    vars.insert("game_assets", assets);
    vars.insert("assets_index_name", prepared.assets_index_id.clone());
    vars.insert("auth_uuid", uuid);
    vars.insert("auth_access_token", token.clone());
    vars.insert("auth_session", format!("token:{}", token));
    vars.insert("clientid", "nexus".to_string());
    vars.insert("auth_xuid", settings.xuid.clone());
    vars.insert(
        "user_type",
        if settings.user_type.is_empty() {
            "legacy".to_string()
        } else {
            settings.user_type.clone()
        },
    );
    vars.insert("user_properties", "{}".to_string());
    vars.insert("version_type", version_type);
    vars.insert("natives_directory", natives.clone());
    vars.insert("library_directory", library_directory);
    vars.insert("classpath_separator", PATH_SEP.to_string());
    vars.insert("launcher_name", "NexusPureRust".to_string());
    vars.insert(
        "launcher_version",
        env!("CARGO_PKG_VERSION").to_string(),
    );
    vars.insert("classpath", classpath.clone());

    let ram = settings.ram_mb.max(1024);
    let mut jvm: Vec<String> = vec![
        format!("-Xms{}M", (ram / 4).max(512)),
        format!("-Xmx{}M", ram),
        "-XX:+UseG1GC".to_string(),
        "-XX:+ParallelRefProcEnabled".to_string(),
        "-XX:MaxGCPauseMillis=200".to_string(),
        "-XX:+UnlockExperimentalVMOptions".to_string(),
        "-XX:+DisableExplicitGC".to_string(),
    ];
    for token in settings.extra_jvm.split_whitespace() {
        jvm.push(token.to_string());
    }

    if let Some(h) = hooks {
        for (k, v) in &h.jvm_system_props {
            jvm.push(format!("-D{}={}", k, v));
        }
        for agent in &h.java_agents {
            jvm.push(format!(
                "-javaagent:{}",
                sandbox.root_dir.join(agent).to_string_lossy()
            ));
        }
    }

    let mut game: Vec<String>;
    if let Some(args) = prepared.vjson.get("arguments") {
        jvm.extend(expand_args(args.get("jvm"), &vars));
        game = expand_args(args.get("game"), &vars);
    } else {
        game = prepared
            .vjson
            .get("minecraftArguments")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .split_whitespace()
            .map(|t| substitute(t, &vars))
            .collect();
    }
    // Forge/NeoForge list the jars their transformer must skip by file name;
    // our client jar lives at the sandbox root as `client.jar`, so make sure it
    // is ignored as well instead of being rewritten at launch.
    for arg in jvm.iter_mut() {
        if let Some(list) = arg.strip_prefix("-DignoreList=") {
            if !list.split(',').any(|part| part == "client.jar") {
                *arg = format!("-DignoreList={},client.jar", list);
            }
        }
    }
    if !jvm.iter().any(|a| a.starts_with("-Djava.library.path")) {
        jvm.push(format!("-Djava.library.path={}", natives));
    }
    if !jvm.iter().any(|a| a == "-cp" || a == "-classpath") {
        jvm.push("-cp".to_string());
        jvm.push(classpath);
    }

    let main_class = hooks
        .and_then(|h| h.main_class_override.clone())
        .unwrap_or_else(|| prepared.main_class.clone());

    if let Some(h) = hooks {
        for tweak in &h.tweak_classes {
            game.push("--tweakClass".to_string());
            game.push(tweak.clone());
        }
    }

    let mut parts: Vec<String> = Vec::new();
    parts.extend(jvm);
    parts.push(main_class);
    parts.extend(game);
    parts
        .iter()
        .map(|a| quote(a))
        .collect::<Vec<_>>()
        .join(" ")
}
