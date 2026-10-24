// archive — instance export / import.
//
// Export writes a self-contained `.zip` (settings + mods/config/saves, never
// the shared libraries and assets). Import understands three shapes:
//   • a Nexus export (`nexus-instance.json`)
//   • a Modrinth pack (`modrinth.index.json`)
//   • a CurseForge pack (`manifest.json`)

use crate::engine::{slug, SandboxEnvironment};
use crate::state::InstanceRecord;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MARKER: &str = "nexus-instance.json";

#[derive(Serialize, Deserialize)]
struct ExportDoc {
    format: u32,
    record: InstanceRecord,
}

/// Folders that are re-downloaded on first launch and must not be zipped.
const SKIP_TOP: &[&str] = &[
    "libraries",
    "assets",
    "natives",
    "downloads",
    "logs",
    "crash-reports",
    "client.jar",
    "version.json",
    "loader.json",
    ".nexus_installed",
    "launcher_profiles.json",
    "launcher_accounts.json",
    "launcher_log.txt",
    "usercache.json",
    "usernamecache.json",
];

const MAX_FILE: u64 = 512 * 1024 * 1024;

fn should_skip(root: &Path, path: &Path) -> bool {
    let rel = match path.strip_prefix(root) {
        Ok(r) => r,
        Err(_) => return true,
    };
    let mut comps = rel.components();
    let Some(first) = comps.next() else {
        return false;
    };
    let first = first.as_os_str().to_string_lossy().to_string();
    if comps.next().is_none() && SKIP_TOP.contains(&first.as_str()) {
        return true;
    }
    first == "libraries" || first == "assets" || first == "natives"
}

fn add_dir(
    zip: &mut zip::ZipWriter<fs::File>,
    root: &Path,
    dir: &Path,
    opts: zip::write::SimpleFileOptions,
    added: &mut u32,
) -> Result<(), String> {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(()),
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if should_skip(root, &path) {
            continue;
        }
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            add_dir(zip, root, &path, opts, added)?;
        } else if meta.len() <= MAX_FILE {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let name = rel.to_string_lossy().replace('\\', "/");
            zip.start_file(name, opts)
                .map_err(|e| format!("zip entry failed: {}", e))?;
            let mut file = fs::File::open(&path).map_err(|e| format!("open {:?}: {}", path, e))?;
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = file.read(&mut buf).map_err(|e| format!("read {:?}: {}", path, e))?;
                if n == 0 {
                    break;
                }
                zip.write_all(&buf[..n])
                    .map_err(|e| format!("write zip: {}", e))?;
            }
            *added += 1;
        }
    }
    Ok(())
}

/// Writes `dest` and returns the number of files packed.
pub fn export_instance(rec: &InstanceRecord, root: &Path, dest: &Path) -> Result<u32, String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("สร้างโฟลเดอร์ปลายทาง: {}", e))?;
    }
    let file = fs::File::create(dest).map_err(|e| format!("สร้างไฟล์ zip: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let doc = ExportDoc {
        format: 1,
        record: rec.clone(),
    };
    let json = serde_json::to_string_pretty(&doc).map_err(|e| format!("serialize: {}", e))?;
    zip.start_file(MARKER, opts)
        .map_err(|e| format!("zip marker: {}", e))?;
    zip.write_all(json.as_bytes())
        .map_err(|e| format!("write marker: {}", e))?;

    let mut added = 0;
    add_dir(&mut zip, root, root, opts, &mut added)?;
    zip.finish().map_err(|e| format!("finalize zip: {}", e))?;
    Ok(added)
}

fn default_exports_dir() -> PathBuf {
    crate::engine::default_base_dir().join("exports")
}

pub fn suggested_export_path(rec: &InstanceRecord) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M");
    default_exports_dir().join(format!("{}-{}.zip", slug(&rec.name), stamp))
}

fn zip_names(bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("ไฟล์ไม่ใช่ zip ที่อ่านได้: {}", e))?;
    Ok((0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .collect())
}

/// Installs a pack that arrived as raw bytes (download or upload).
pub async fn import_from_bytes(
    bytes: &[u8],
    slug_hint: &str,
    base_dir: &Path,
    cf_key: &str,
) -> Result<InstanceRecord, String> {
    let names = zip_names(bytes)?;
    let unique = format!("{}-{}", slug(slug_hint), chrono::Local::now().timestamp());

    if names.iter().any(|n| n == "modrinth.index.json") {
        return crate::modrinth::install_mrpack(bytes, &unique, slug_hint, None, base_dir).await;
    }
    if names.iter().any(|n| n == "manifest.json") {
        return crate::curseforge::install_pack(bytes, &unique, slug_hint, cf_key, base_dir).await;
    }
    Err("ไม่รู้จักรูปแบบแพ็ก — รองรับ .mrpack และ .zip แบบ CurseForge".to_string())
}

/// Imports any supported archive and returns the new instance record.
pub async fn import_instance(
    zip_path: &Path,
    base_dir: &Path,
    cf_key: &str,
    fallback_mc: &str,
) -> Result<InstanceRecord, String> {
    let bytes = fs::read(zip_path).map_err(|e| format!("เปิดไฟล์ zip ไม่ได้: {}", e))?;
    let stem = zip_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "imported".to_string());
    let names = zip_names(&bytes)?;

    if names.iter().any(|n| n == "modrinth.index.json")
        || names.iter().any(|n| n == "manifest.json")
    {
        return import_from_bytes(&bytes, &stem, base_dir, cf_key).await;
    }

    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes))
        .map_err(|e| format!("ไฟล์ไม่ใช่ zip ที่อ่านได้: {}", e))?;

    let Some(marker) = names.iter().find(|n| n.as_str() == MARKER).cloned() else {
        return Err(
            "ไม่รู้จักรูปแบบไฟล์นี้ — รองรับ .zip ที่มี nexus-instance.json, modrinth.index.json หรือ manifest.json (CurseForge)"
                .to_string(),
        );
    };

    let mut doc_text = String::new();
    zip.by_name(&marker)
        .map_err(|e| format!("อ่าน {}: {}", marker, e))?
        .read_to_string(&mut doc_text)
        .map_err(|e| format!("parse {}: {}", marker, e))?;
    let doc: ExportDoc =
        serde_json::from_str(&doc_text).map_err(|e| format!("{} เสียหาย: {}", marker, e))?;

    let mut rec = doc.record;
    if rec.mc_version.is_empty() {
        rec.mc_version = fallback_mc.to_string();
    }
    rec.id = format!(
        "imp-{}-{}",
        slug(&rec.name),
        chrono::Local::now().format("%H%M%S")
    );
    rec.provider = "custom".to_string();
    rec.last_played = None;
    rec.created_at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

    let sandbox = SandboxEnvironment::initialize(base_dir, &rec.provider, &rec.id)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("zip entry: {}", e))?;
        let name = entry.name().to_string();
        if name == MARKER || entry.is_dir() {
            continue;
        }
        let Some(safe_name) = Path::new(&name)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
            .then_some(name.clone())
        else {
            continue;
        };
        let dest = sandbox.root_dir.join(&safe_name);
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut out =
            fs::File::create(&dest).map_err(|e| format!("เขียน {:?}: {}", dest, e))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("copy entry: {}", e))?;
    }
    let _ = fs::write(sandbox.root_dir.join(".nexus_installed"), b"ok");
    Ok(rec)
}
