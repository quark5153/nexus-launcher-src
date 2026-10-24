//! Layer 2 — the Tauri shell. Thin IPC surface only: every command here
//! translates a frontend call into a `nexus-core` operation and streams
//! progress back as events. No game logic lives in this file.

use nexus_core::state::{AppConfig, InstanceRecord};
use nexus_core::{archive, auth, content, curseforge, engine, forge, java, modrinth, state, vanilla};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

#[derive(serde::Serialize)]
pub struct BootstrapPayload {
    pub config: AppConfig,
    pub installed: Vec<String>,
    pub manifest: Vec<vanilla::VersionEntry>,
    pub java: Option<String>,
    pub java_runtimes: Vec<java::JavaRuntime>,
    pub data_dir: String,
    pub exports_dir: String,
    pub app_version: String,
}

#[derive(serde::Serialize, Clone)]
pub struct ProgressPayload {
    pub label: String,
    pub frac: f32,
}

fn log_to(app: &AppHandle, line: impl Into<String>) {
    let _ = app.emit("nexus-log", line.into());
}

#[tauri::command]
pub async fn bootstrap() -> Result<BootstrapPayload, String> {
    let mut cfg = state::load();
    if !std::path::Path::new(&cfg.java_path).exists() {
        if let Some(found) = engine::detect_java() {
            cfg.java_path = found;
        }
    }
    // Always write once on boot: this also migrates an older config blob by
    // filling in the fields added in newer releases.
    state::save(&cfg);
    let installed = state::scan_installed();
    let manifest = vanilla::fetch_manifest().await.unwrap_or_default();
    let runtimes = java::scan();
    Ok(BootstrapPayload {
        config: cfg,
        installed,
        manifest,
        java: engine::detect_java(),
        java_runtimes: runtimes,
        data_dir: engine::default_base_dir().to_string_lossy().to_string(),
        exports_dir: engine::default_base_dir()
            .join("exports")
            .to_string_lossy()
            .to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[tauri::command]
pub async fn save_config(cfg: AppConfig) -> Result<(), String> {
    state::save(&cfg);
    Ok(())
}

#[tauri::command]
pub async fn detect_java() -> Result<Option<String>, String> {
    Ok(engine::detect_java())
}

#[tauri::command]
pub async fn list_java() -> Result<Vec<java::JavaRuntime>, String> {
    Ok(java::scan())
}

#[tauri::command]
pub async fn install_java(app: AppHandle, major: u32) -> Result<String, String> {
    let handle = app.clone();
    let base = engine::default_base_dir();
    let path = java::download(major, &base, &move |line| log_to(&handle, line)).await?;
    log_to(&app, format!("[OK] Java {} พร้อมใช้งาน: {}", major, path));
    Ok(path)
}

#[tauri::command]
pub async fn open_data_folder() -> Result<(), String> {
    engine::open_path(&engine::default_base_dir());
    Ok(())
}

#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    engine::open_url(&url)
}

#[tauri::command]
pub async fn open_instance_folder(instance_id: String, sub: Option<String>) -> Result<(), String> {
    let cfg = state::load();
    let mut path = match state::find_instance(&cfg, &instance_id) {
        Some(rec) => state::instance_dir(&rec),
        None => engine::default_base_dir().join("native").join(engine::slug(&instance_id)),
    };
    if let Some(sub) = sub.filter(|s| !s.is_empty()) {
        if !sub.contains("..") {
            path = path.join(sub);
        }
    }
    engine::open_path(&path);
    Ok(())
}

/* ── instances ─────────────────────────────────────────────────────────── */

fn new_instance_id(name: &str) -> String {
    format!(
        "inst-{}-{}",
        engine::slug(name),
        chrono::Local::now().format("%H%M%S")
    )
}

#[tauri::command]
pub async fn create_instance(
    name: String,
    mc_version: String,
    loader: Option<String>,
    loader_version: Option<String>,
) -> Result<AppConfig, String> {
    let mut cfg = state::load();
    let trimmed = name.trim();
    let label = if trimmed.is_empty() {
        format!("Minecraft {}", mc_version)
    } else {
        trimmed.to_string()
    };
    let loader = loader.filter(|l| !l.is_empty() && l != "vanilla");
    let rec = InstanceRecord {
        id: new_instance_id(&label),
        name: label,
        provider: "custom".to_string(),
        mc_version: mc_version.clone(),
        loader: loader.clone(),
        loader_version: loader_version.filter(|v| !v.is_empty()),
        modpack_id: None,
        modpack_version: None,
        update_url: None,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        last_played: None,
        ram_mb: None,
        java_path: None,
        extra_jvm: None,
        notes: None,
        favorite: false,
    };
    let sandbox = engine::SandboxEnvironment::initialize(
        &engine::default_base_dir(),
        &rec.provider,
        &rec.id,
    )?;
    let _ = std::fs::create_dir_all(sandbox.mods_dir.join(".keep"));
    state::upsert_instance(&mut cfg, rec.clone());
    cfg.selected_version = rec.id;
    state::save(&cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn update_instance(record: InstanceRecord) -> Result<AppConfig, String> {
    let mut cfg = state::load();
    state::upsert_instance(&mut cfg, record);
    state::save(&cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn delete_instance(instance_id: String, purge: bool) -> Result<AppConfig, String> {
    let mut cfg = state::load();
    if purge {
        if let Some(rec) = state::find_instance(&cfg, &instance_id) {
            let dir = state::instance_dir(&rec);
            if dir.exists() && dir.components().count() > 3 {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }
    cfg.instances.retain(|i| i.id != instance_id);
    if cfg.selected_version == instance_id {
        cfg.selected_version = cfg
            .instances
            .first()
            .map(|i| i.id.clone())
            .unwrap_or_else(|| "1.21.1".to_string());
    }
    state::save(&cfg);
    Ok(cfg)
}

/* ── accounts ──────────────────────────────────────────────────────────── */

#[tauri::command]
pub async fn ms_device_start(client_id: String) -> Result<auth::DeviceStart, String> {
    auth::device_start(&client_id).await
}

#[tauri::command]
pub async fn ms_check_client(client_id: String) -> auth::ClientCheck {
    auth::check_client(&client_id).await
}

#[tauri::command]
pub async fn ms_device_poll(
    client_id: String,
    device_code: String,
) -> Result<Option<AppConfig>, String> {
    let Some(account) = auth::device_poll(&client_id, &device_code).await? else {
        return Ok(None);
    };
    let mut cfg = state::load();
    if let Some(pos) = cfg.accounts.iter().position(|a| a.id == account.id) {
        cfg.accounts[pos] = account.clone();
    } else {
        cfg.accounts.push(account.clone());
    }
    cfg.username = account.username.clone();
    cfg.active_account = Some(account.id.clone());
    state::save(&cfg);
    Ok(Some(cfg))
}

#[tauri::command]
pub async fn ms_logout(account_id: String) -> Result<AppConfig, String> {
    state::clear_tokens(&account_id);
    let mut cfg = state::load();
    cfg.accounts.retain(|a| a.id != account_id);
    if cfg.active_account.as_deref() == Some(account_id.as_str()) {
        cfg.active_account = cfg.accounts.first().map(|a| a.id.clone());
        if let Some(first) = cfg.accounts.first() {
            cfg.username = first.username.clone();
        }
    }
    state::save(&cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn set_active_account(account_id: String) -> Result<AppConfig, String> {
    let mut cfg = state::load();
    if let Some(acc) = cfg.accounts.iter().find(|a| a.id == account_id) {
        cfg.username = acc.username.clone();
        cfg.active_account = Some(acc.id.clone());
        state::save(&cfg);
    }
    Ok(cfg)
}

async fn resolve_launch_user(app: &AppHandle, cfg: &AppConfig) -> state::LaunchUser {
    let Some(account) = state::active_account(cfg) else {
        return state::LaunchUser::offline(&cfg.username);
    };
    if account.kind == "Microsoft" {
        match auth::launch_auth(&cfg.ms_client_id, &account.id).await {
            Ok(auth) => {
                log_to(
                    app,
                    format!("[OK] ล็อกอิน Microsoft: {} ({})", auth.name, auth.uuid),
                );
                return state::LaunchUser {
                    username: auth.name,
                    uuid: auth.uuid,
                    access_token: auth.access_token,
                    user_type: "msa".to_string(),
                };
            }
            Err(err) => {
                log_to(
                    app,
                    format!("[ERR] ล็อกอิน Microsoft ไม่สำเร็จ: {} — ใช้โหมดออฟไลน์ชั่วคราว", err),
                );
            }
        }
    }
    state::LaunchUser::offline(&account.username)
}

/* ── launch / install ──────────────────────────────────────────────────── */

#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    version_or_instance_id: String,
    launch: bool,
) -> Result<Option<u32>, String> {
    let mut cfg = state::load();
    let manifest = vanilla::fetch_manifest().await?;

    let maybe_inst = state::find_instance(&cfg, &version_or_instance_id);
    let (mc_id, target, inst_settings) = match maybe_inst {
        Some(inst) => (
            inst.mc_version.clone(),
            engine::InstanceTarget::for_instance(&inst),
            Some(inst),
        ),
        None => (
            version_or_instance_id.clone(),
            engine::InstanceTarget::vanilla(&version_or_instance_id),
            None,
        ),
    };

    let entry = manifest
        .iter()
        .find(|e| e.id == mc_id)
        .cloned()
        .ok_or_else(|| format!("ไม่พบข้อมูลเวอร์ชัน {}", mc_id))?;

    let base_dir = engine::default_base_dir();
    let sandbox = engine::SandboxEnvironment::initialize(
        &base_dir,
        &target.provider,
        &target.instance_id,
    )?;
    let sync = engine::DeltaSyncEngine::new();

    // Per-instance overrides fall back to the global settings.
    let inst_ram = inst_settings
        .as_ref()
        .and_then(|i| i.ram_mb)
        .unwrap_or(cfg.ram_mb);
    let inst_java = inst_settings
        .as_ref()
        .and_then(|i| i.java_path.clone())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| cfg.java_path.clone());
    let inst_jvm = inst_settings
        .as_ref()
        .and_then(|i| i.extra_jvm.clone())
        .unwrap_or_else(|| cfg.extra_jvm.clone());

    // Pick the JDK before preparing: the version decides the major, and the
    // Forge/NeoForge installer runs with this exact JVM during prepare.
    let java_handle = app.clone();
    let required_major = vanilla::required_java_major(&sandbox, &entry).await;
    let java_path = java::ensure(
        &inst_java,
        required_major,
        cfg.auto_java,
        &base_dir,
        move |line| {
            let _ = java_handle.emit("nexus-log", line);
        },
    )
    .await?;

    let app_log = app.clone();
    let app_prog = app.clone();

    let prepared = vanilla::prepare(
        &sync,
        &sandbox,
        &entry,
        target.loader.as_ref(),
        &java_path,
        move |line| {
            let _ = app_log.emit("nexus-log", line);
        },
        move |label, frac| {
            let _ = app_prog.emit("nexus-progress", ProgressPayload { label, frac });
        },
    )
    .await?;

    let _ = std::fs::write(sandbox.root_dir.join(".nexus_installed"), b"ok");

    if !launch {
        log_to(
            &app,
            format!("[OK] ติดตั้ง {} เสร็จเรียบร้อย", entry.id),
        );
        return Ok(None);
    }

    let user = resolve_launch_user(&app, &cfg).await;

    let settings = vanilla::LaunchSettings {
        java_path: java_path.clone(),
        ram_mb: inst_ram,
        username: user.username.clone(),
        uuid: user.uuid.clone(),
        access_token: user.access_token.clone(),
        user_type: user.user_type.clone(),
        xuid: String::new(),
        extra_jvm: inst_jvm,
    };

    let args = vanilla::build_arguments(&sandbox, &prepared, &settings, None);
    let log_path = content::log_dir(&sandbox.root_dir).join(format!(
        "nexus-launch-{}.log",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));

    log_to(
        &app,
        format!(
            "[..] เริ่ม JVM: {} (Java {}) ในชื่อ {}",
            java_path, prepared.java_major, user.username
        ),
    );
    let pid = engine::spawn_game_process_logged(
        &java_path,
        &args,
        &sandbox.root_dir,
        None,
        None,
        Some(&log_path),
    )?;
    log_to(
        &app,
        format!("[OK] เกมเปิดทำงานแล้ว (PID {}) — ดู log ได้ที่หน้าอินสแตนซ์", pid),
    );

    if let Some(inst) = inst_settings {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        let mut updated = inst.clone();
        updated.last_played = Some(now.clone());
        state::upsert_instance(&mut cfg, updated);
        cfg.last_played = Some(now);
    } else {
        cfg.last_played = Some(chrono::Local::now().format("%Y-%m-%d %H:%M").to_string());
    }
    state::save(&cfg);

    Ok(Some(pid))
}

/// How many results a browse/search request should return (frontend default: 20).
fn result_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(20).clamp(1, 100)
}

#[tauri::command]
pub async fn search_modrinth(
    query: String,
    project_type: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<modrinth::SearchPage, String> {
    modrinth::search(
        &query,
        &project_type,
        result_limit(limit),
        offset.unwrap_or(0),
    )
    .await
}

#[tauri::command]
pub async fn search_curseforge(
    query: String,
    project_type: String,
    limit: Option<usize>,
) -> Result<Vec<curseforge::CfHit>, String> {
    let cfg = state::load();
    curseforge::search(&query, &project_type, &cfg.curseforge_key, result_limit(limit)).await
}

/// Installs a mod or modpack from either provider into the selected profile.
#[tauri::command]
pub async fn install_project(
    app: AppHandle,
    project_id: String,
    title: String,
    project_type: String,
    source: String,
    instance_id: String,
) -> Result<AppConfig, String> {
    let mut cfg = state::load();
    let base_dir = engine::default_base_dir();
    let target = state::find_instance(&cfg, &instance_id);

    if project_type == "modpack" {
        log_to(&app, format!("[..] กำลังติดตั้งมอดแพ็ก {}", title));
        let rec = if source == "curseforge" {
            let id: u64 = project_id
                .parse()
                .map_err(|_| "รหัสโปรเจกต์ CurseForge ไม่ถูกต้อง".to_string())?;
            let file = curseforge::latest_file(id, &cfg.curseforge_key).await?;
            let url = file.download_url.clone().unwrap_or_else(|| {
                format!(
                    "https://edge.forgecdn.net/files/{}/{}/{}",
                    file.id / 1000,
                    file.id % 1000,
                    file.file_name
                )
            });
            let bytes = engine::http_client()
                .get(&url)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .bytes()
                .await
                .map_err(|e| e.to_string())?
                .to_vec();
            curseforge::install_pack(&bytes, &project_id, &title, &cfg.curseforge_key, &base_dir)
                .await?
        } else {
            let versions = modrinth::fetch_versions(&project_id).await?;
            let latest = modrinth::pick_for(&versions, "", None)
                .or_else(|| versions.first().cloned())
                .ok_or_else(|| "ไม่พบเวอร์ชันของโปรเจกต์นี้".to_string())?;
            let file = latest
                .files
                .iter()
                .find(|f| f.primary)
                .or_else(|| latest.files.first())
                .ok_or_else(|| "ไม่พบไฟล์ดาวน์โหลด".to_string())?;
            let bytes = engine::http_client()
                .get(&file.url)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .bytes()
                .await
                .map_err(|e| e.to_string())?
                .to_vec();
            if file.filename.ends_with(".mrpack") {
                modrinth::install_mrpack(&bytes, &project_id, &title, None, &base_dir).await?
            } else {
                // CurseForge-style zip or plain archive: reuse the importer.
                let tmp = std::env::temp_dir().join(format!("nexus-{}.zip", engine::slug(&title)));
                std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
                archive::import_instance(&tmp, &base_dir, &cfg.curseforge_key, "1.21.1").await?
            }
        };
        state::upsert_instance(&mut cfg, rec.clone());
        cfg.selected_version = rec.id.clone();
        state::save(&cfg);
        log_to(&app, format!("[OK] ติดตั้งมอดแพ็ก '{}' สำเร็จ", rec.name));
        return Ok(cfg);
    }

    let Some(inst) = target else {
        return Err("กรุณาเลือกโปรไฟล์ปลายทางก่อนติดตั้งมอด".to_string());
    };
    let root = state::instance_dir(&inst);
    if source == "curseforge" {
        let id: u64 = project_id
            .parse()
            .map_err(|_| "รหัสโปรเจกต์ CurseForge ไม่ถูกต้อง".to_string())?;
        let file = curseforge::install_mod(
            id,
            &root,
            &inst.mc_version,
            inst.loader.as_deref(),
            &cfg.curseforge_key,
        )
        .await?;
        log_to(&app, format!("[OK] ติดตั้ง {} ลง {}", file, inst.name));
    } else {
        let handle = app.clone();
        let file = modrinth::install_mod_with_deps(&project_id, &inst, &root, move |line| {
            let _ = handle.emit("nexus-log", line);
        })
        .await?;
        log_to(&app, format!("[OK] ติดตั้ง {} ลง {}", file, inst.name));
    }
    Ok(cfg)
}

#[tauri::command]
pub async fn install_custom_pack(app: AppHandle, url: String) -> Result<AppConfig, String> {
    let mut cfg = state::load();
    let base_dir = engine::default_base_dir();
    log_to(&app, format!("[..] กำลังดึงข้อมูลแพ็กจาก {}", url));

    let bytes = engine::http_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;

    let pack_slug = url
        .split('/')
        .next_back()
        .unwrap_or("custom-pack")
        .trim_end_matches(".mrpack")
        .trim_end_matches(".zip")
        .to_string();

    let rec = if bytes.len() > 4 && &bytes[..2] == b"PK" {
        archive::import_from_bytes(&bytes, &pack_slug, &base_dir, &cfg.curseforge_key).await?
    } else {
        return Err("ไฟล์ที่ดาวน์โหลดไม่ใช่แพ็ก (zip/mrpack)".to_string());
    };

    state::upsert_instance(&mut cfg, rec.clone());
    cfg.selected_version = rec.id.clone();
    state::save(&cfg);
    log_to(&app, format!("[OK] นำเข้าแพ็ก '{}' สำเร็จ", rec.name));
    Ok(cfg)
}

/* ── import / export ───────────────────────────────────────────────────── */

#[tauri::command]
pub async fn suggested_export_path(instance_id: String) -> Result<String, String> {
    let cfg = state::load();
    let rec = state::find_instance(&cfg, &instance_id)
        .ok_or_else(|| "ไม่พบอินสแตนซ์นี้".to_string())?;
    Ok(archive::suggested_export_path(&rec).to_string_lossy().to_string())
}

#[tauri::command]
pub async fn export_instance(
    app: AppHandle,
    instance_id: String,
    dest: Option<String>,
) -> Result<String, String> {
    let cfg = state::load();
    let rec = state::find_instance(&cfg, &instance_id)
        .ok_or_else(|| "ไม่พบอินสแตนซ์นี้".to_string())?;
    let dest = dest
        .filter(|d| !d.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| archive::suggested_export_path(&rec));
    let root = state::instance_dir(&rec);
    log_to(&app, format!("[..] กำลังแพ็กอินสแตนซ์ {}", rec.name));
    let count = archive::export_instance(&rec, &root, &dest)?;
    log_to(
        &app,
        format!("[OK] สำรอง {} ไฟล์ไปที่ {}", count, dest.display()),
    );
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn import_instance(app: AppHandle, path: String) -> Result<AppConfig, String> {
    let mut cfg = state::load();
    let base_dir = engine::default_base_dir();
    let zip_path = PathBuf::from(&path);
    if !zip_path.exists() {
        return Err(format!("ไม่พบไฟล์ {}", path));
    }
    log_to(&app, format!("[..] กำลังนำเข้า {}", path));
    let rec =
        archive::import_instance(&zip_path, &base_dir, &cfg.curseforge_key, "1.21.1").await?;
    log_to(&app, format!("[OK] นำเข้า '{}' สำเร็จ", rec.name));
    state::upsert_instance(&mut cfg, rec.clone());
    cfg.selected_version = rec.id;
    state::save(&cfg);
    Ok(cfg)
}

/* ── content, logs, crash reports ──────────────────────────────────────── */

fn instance_root(instance_id: &str) -> Result<(InstanceRecord, PathBuf), String> {
    let cfg = state::load();
    let rec = state::find_instance(&cfg, instance_id)
        .ok_or_else(|| "ไม่พบอินสแตนซ์นี้".to_string())?;
    let root = state::instance_dir(&rec);
    std::fs::create_dir_all(&root).ok();
    Ok((rec, root))
}

#[tauri::command]
pub async fn list_content(
    instance_id: String,
    kind: String,
) -> Result<Vec<content::ContentEntry>, String> {
    let (_, root) = instance_root(&instance_id)?;
    Ok(content::list_content(&root, &kind))
}

#[tauri::command]
pub async fn mod_content_meta(
    instance_id: String,
) -> Result<std::collections::HashMap<String, modrinth::ModMeta>, String> {
    let (_, root) = instance_root(&instance_id)?;
    Ok(modrinth::resolve_mod_meta(&root).await)
}

#[tauri::command]
pub async fn toggle_content(
    instance_id: String,
    kind: String,
    name: String,
) -> Result<bool, String> {
    let (_, root) = instance_root(&instance_id)?;
    content::toggle_content(&root, &kind, &name)
}

#[tauri::command]
pub async fn delete_content(
    instance_id: String,
    kind: String,
    name: String,
) -> Result<(), String> {
    let (_, root) = instance_root(&instance_id)?;
    content::delete_content(&root, &kind, &name)
}

#[tauri::command]
pub async fn read_log(instance_id: String) -> Result<String, String> {
    let (_, root) = instance_root(&instance_id)?;
    Ok(content::read_log(&root, 200_000))
}

#[tauri::command]
pub async fn list_crashes(instance_id: String) -> Result<Vec<content::LogFile>, String> {
    let (_, root) = instance_root(&instance_id)?;
    Ok(content::list_crashes(&root))
}

#[tauri::command]
pub async fn read_crash(instance_id: String, name: String) -> Result<String, String> {
    let (_, root) = instance_root(&instance_id)?;
    content::read_crash(&root, &name)
}

/* ── updates ───────────────────────────────────────────────────────────── */

#[tauri::command]
pub async fn check_updates(app: AppHandle) -> Result<String, String> {
    let cfg = state::load();
    let packs: Vec<InstanceRecord> = cfg
        .instances
        .iter()
        .filter(|i| i.provider == "modpack")
        .cloned()
        .collect();
    if packs.is_empty() {
        return Ok("ยังไม่มีมอดแพ็กที่ติดตั้งไว้ให้ตรวจ".to_string());
    }
    let updates = modrinth::check_updates(&packs).await;
    if updates.is_empty() {
        log_to(&app, "[OK] ทุกมอดแพ็กเป็นเวอร์ชันล่าสุดแล้ว".to_string());
        return Ok("ทุกมอดแพ็กเป็นเวอร์ชันล่าสุดแล้ว".to_string());
    }
    let mut report = String::new();
    for u in &updates {
        report.push_str(&format!(
            "{}: {} → {}\n",
            u.name, u.current, u.latest
        ));
    }
    log_to(&app, format!("[..] พบอัปเดต {} แพ็ก", updates.len()));
    Ok(report.trim_end().to_string())
}

#[tauri::command]
pub async fn update_pack(app: AppHandle, instance_id: String) -> Result<AppConfig, String> {
    let mut cfg = state::load();
    let inst = state::find_instance(&cfg, &instance_id)
        .ok_or_else(|| "ไม่พบอินสแตนซ์นี้".to_string())?;
    let updates = modrinth::check_updates(std::slice::from_ref(&inst)).await;
    let Some(info) = updates.into_iter().next() else {
        return Err("แพ็กนี้เป็นเวอร์ชันล่าสุดแล้ว".to_string());
    };
    let handle = app.clone();
    let updated = modrinth::apply_update(&info, &engine::default_base_dir(), move |line| {
        let _ = handle.emit("nexus-log", line);
    })
    .await?;
    state::upsert_instance(&mut cfg, updated);
    state::save(&cfg);
    Ok(cfg)
}

#[tauri::command]
pub async fn list_loader_versions(
    loader: String,
    mc_version: String,
) -> Result<Vec<String>, String> {
    forge::list_versions(&loader, &mc_version).await
}
