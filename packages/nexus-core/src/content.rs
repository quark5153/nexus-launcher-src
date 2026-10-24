// content — per-instance content manager (worlds, resource packs, shaders,
// servers) plus the log / crash-report reader used by the diagnostics panel.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentEntry {
    pub name: String,
    pub size: u64,
    pub enabled: bool,
    pub modified: String,
    pub kind: String,
    pub detail: Option<String>,
    /// Preview image as a `data:` URI (world icon.png, pack.png in a zip, or
    /// the server icon stored in servers.dat). Mods get theirs from Modrinth.
    #[serde(default)]
    pub icon: Option<String>,
}

const MAX_ICON_BYTES: usize = 900_000;

/// Minimal base64 so local images can be handed to the webview as data URIs.
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = *chunk.get(1).unwrap_or(&0) as usize;
        let b2 = *chunk.get(2).unwrap_or(&0) as usize;
        out.push(TABLE[b0 >> 2] as char);
        out.push(TABLE[((b0 & 0b11) << 4) | (b1 >> 4)] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((b1 & 0b1111) << 2) | (b2 >> 6)] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[b2 & 0b111111] as char
        } else {
            '='
        });
    }
    out
}

fn png_data_uri(bytes: &[u8]) -> Option<String> {
    const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.len() < 8 || bytes.len() > MAX_ICON_BYTES || bytes[..8] != PNG_MAGIC {
        return None;
    }
    Some(format!("data:image/png;base64,{}", base64_encode(bytes)))
}

fn icon_from_file(path: &Path) -> Option<String> {
    png_data_uri(&fs::read(path).ok()?)
}

/// Reads `pack.png` from a resource pack archive; shader packs rarely have one.
fn icon_from_zip(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();
    /* root pack.png first, then any nested copy */
    names.sort_by_key(|n| (n.matches('/').count(), n.len()));
    let name = names
        .into_iter()
        .find(|n| n.to_lowercase().ends_with("pack.png"))?;
    let mut entry = archive.by_name(&name).ok()?;
    if entry.size() as usize > MAX_ICON_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    png_data_uri(&bytes)
}

/// Folder packs keep their preview next to `pack.mcmeta`.
fn icon_for_entry(path: &Path, kind: &str) -> Option<String> {
    if path.is_dir() {
        if path.join("icon.png").exists() {
            return icon_from_file(&path.join("icon.png"));
        }
        if path.join("pack.png").exists() {
            return icon_from_file(&path.join("pack.png"));
        }
        return None;
    }
    match kind {
        "resourcepacks" | "shaderpacks" => icon_from_zip(path),
        _ => None,
    }
}

fn modified_of(path: &Path) -> String {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%Y-%m-%d %H:%M").to_string()
        })
        .unwrap_or_default()
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(rd) = fs::read_dir(path) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = fs::metadata(&p) {
                total += meta.len();
            }
        }
    }
    total
}

fn folder_for(root: &Path, kind: &str) -> Option<PathBuf> {
    match kind {
        "mods" => Some(root.join("mods")),
        "worlds" => Some(root.join("saves")),
        "resourcepacks" => Some(root.join("resourcepacks")),
        "shaderpacks" => Some(root.join("shaderpacks")),
        _ => None,
    }
}

/// Lists installed worlds / packs / shaders, or the saved server list.
pub fn list_content(root: &Path, kind: &str) -> Vec<ContentEntry> {
    if kind == "servers" {
        return list_servers(root);
    }
    let Some(dir) = folder_for(root, kind) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            let raw_name = entry.file_name().to_string_lossy().to_string();
            if raw_name.starts_with('.') || raw_name.ends_with(".tmp") {
                continue;
            }
            let enabled = !raw_name.ends_with(".disabled");
            let name = raw_name.trim_end_matches(".disabled").to_string();
            let size = if path.is_dir() {
                dir_size(&path)
            } else {
                fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
            };
            out.push(ContentEntry {
                name,
                size,
                enabled,
                modified: modified_of(&path),
                kind: kind.to_string(),
                detail: None,
                icon: icon_for_entry(&path, kind),
            });
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

/// Renames `name` to `name.disabled` (and back) so the game ignores it.
pub fn toggle_content(root: &Path, kind: &str, name: &str) -> Result<bool, String> {
    let Some(dir) = folder_for(root, kind) else {
        return Err(format!("ไม่รู้จักชนิดเนื้อหา '{}'", kind));
    };
    let safe = safe_join(&dir, name)?;
    let disabled = dir.join(format!("{}.disabled", name));
    if safe.exists() {
        fs::rename(&safe, &disabled).map_err(|e| format!("ปิดใช้งานไม่สำเร็จ: {}", e))?;
        Ok(false)
    } else if disabled.exists() {
        fs::rename(&disabled, &safe).map_err(|e| format!("เปิดใช้งานไม่สำเร็จ: {}", e))?;
        Ok(true)
    } else {
        Err(format!("ไม่พบ '{}'", name))
    }
}

pub fn delete_content(root: &Path, kind: &str, name: &str) -> Result<(), String> {
    let Some(dir) = folder_for(root, kind) else {
        return Err(format!("ไม่รู้จักชนิดเนื้อหา '{}'", kind));
    };
    let target = if safe_join(&dir, name).is_ok_and(|p| p.exists()) {
        safe_join(&dir, name)?
    } else {
        safe_join(&dir, &format!("{}.disabled", name))?
    };
    if target.is_dir() {
        fs::remove_dir_all(&target).map_err(|e| format!("ลบโฟลเดอร์ไม่สำเร็จ: {}", e))
    } else {
        fs::remove_file(&target).map_err(|e| format!("ลบไฟล์ไม่สำเร็จ: {}", e))
    }
}

/// Blocks `../` escapes from names coming out of the UI.
fn safe_join(base: &Path, name: &str) -> Result<PathBuf, String> {
    let candidate = Path::new(name);
    if candidate
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return Err("ชื่อไฟล์ไม่ปลอดภัย".to_string());
    }
    Ok(base.join(candidate))
}

/* ── servers.dat (NBT) ─────────────────────────────────────────────────── */

struct Nbt<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Nbt<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn u8(&mut self) -> Option<u8> {
        let v = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }
    fn i16(&mut self) -> Option<i16> {
        let b = self.data.get(self.pos..self.pos + 2)?;
        self.pos += 2;
        Some(i16::from_be_bytes([b[0], b[1]]))
    }
    fn i32(&mut self) -> Option<i32> {
        let b = self.data.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        self.pos += n;
        (self.pos <= self.data.len()).then_some(())
    }
    fn string(&mut self) -> Option<String> {
        let len = self.i16()? as usize;
        let b = self.data.get(self.pos..self.pos + len)?;
        self.pos += len;
        Some(String::from_utf8_lossy(b).to_string())
    }
    fn payload(&mut self, tag: u8) -> Option<NbtValue> {
        Some(match tag {
            1 => NbtValue::Num(self.u8()? as i64),
            2 => NbtValue::Num(self.i16()? as i64),
            3 => NbtValue::Num(self.i32()? as i64),
            4 => {
                let b = self.data.get(self.pos..self.pos + 8)?;
                self.pos += 8;
                NbtValue::Num(i64::from_be_bytes([
                    b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
                ]))
            }
            5 => {
                self.skip(4)?;
                NbtValue::Num(0)
            }
            6 => {
                self.skip(8)?;
                NbtValue::Num(0)
            }
            7 => {
                let n = self.i32()?.max(0) as usize;
                self.skip(n)?;
                NbtValue::Empty
            }
            8 => NbtValue::Str(self.string()?),
            9 => {
                let elem = self.u8()?;
                let n = self.i32()?.max(0) as usize;
                let mut items = Vec::new();
                for _ in 0..n {
                    items.push(self.payload(elem)?);
                }
                NbtValue::List(items)
            }
            10 => {
                let mut map = Vec::new();
                loop {
                    let t = self.u8()?;
                    if t == 0 {
                        break;
                    }
                    let name = self.string()?;
                    let value = self.payload(t)?;
                    map.push((name, value));
                }
                NbtValue::Compound(map)
            }
            11 => {
                let n = self.i32()?.max(0) as usize;
                self.skip(n * 4)?;
                NbtValue::Empty
            }
            12 => {
                let n = self.i32()?.max(0) as usize;
                self.skip(n * 8)?;
                NbtValue::Empty
            }
            _ => return None,
        })
    }
}

#[allow(dead_code)] // only the string/compound/list shapes are read today
enum NbtValue {
    Empty,
    Num(i64),
    Str(String),
    List(Vec<NbtValue>),
    Compound(Vec<(String, NbtValue)>),
}

impl NbtValue {
    fn field(&self, key: &str) -> Option<&NbtValue> {
        match self {
            NbtValue::Compound(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

fn list_servers(root: &Path) -> Vec<ContentEntry> {
    let path = root.join("servers.dat");
    let Ok(mut file) = fs::File::open(&path) else {
        return Vec::new();
    };
    let mut data = Vec::new();
    if file.read_to_end(&mut data).is_err() {
        return Vec::new();
    }
    let mut reader = Nbt::new(&data);
    let Some(tag) = reader.u8() else {
        return Vec::new();
    };
    if tag != 10 {
        return Vec::new();
    }
    let _root_name = reader.string();
    let Some(root_value) = reader.payload(10) else {
        return Vec::new();
    };
    let Some(NbtValue::List(servers)) = root_value.field("servers") else {
        return Vec::new();
    };
    servers
        .iter()
        .filter_map(|entry| {
            let name = match entry.field("name") {
                Some(NbtValue::Str(s)) => s.clone(),
                _ => return None,
            };
            let ip = match entry.field("ip") {
                Some(NbtValue::Str(s)) => s.clone(),
                _ => String::new(),
            };
            /* the client stores the 64×64 icon as base64 PNG bytes */
            let icon = match entry.field("icon") {
                Some(NbtValue::Str(s)) if s.starts_with("iVBOR") => {
                    Some(format!("data:image/png;base64,{}", s))
                }
                _ => None,
            };
            Some(ContentEntry {
                name,
                size: 0,
                enabled: true,
                modified: modified_of(&path),
                kind: "servers".to_string(),
                detail: Some(ip),
                icon,
            })
        })
        .collect()
}

/* ── logs & crash reports ──────────────────────────────────────────────── */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFile {
    pub name: String,
    pub size: u64,
    pub modified: String,
}

pub fn log_dir(root: &Path) -> PathBuf {
    root.join("logs")
}

/// Newest launcher log, falling back to the game's own `logs/latest.log`.
pub fn latest_log_path(root: &Path) -> Option<PathBuf> {
    let dir = log_dir(root);
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(rd) = fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !(name.starts_with("nexus-launch") || name == "latest.log") {
                continue;
            }
            let modified = fs::metadata(&p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().is_none_or(|(t, _)| modified > *t) {
                best = Some((modified, p));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Reads the tail of the newest log (at most `max_bytes`).
pub fn read_log(root: &Path, max_bytes: usize) -> String {
    let Some(path) = latest_log_path(root) else {
        return String::new();
    };
    let Ok(data) = fs::read(&path) else {
        return String::new();
    };
    let start = data.len().saturating_sub(max_bytes);
    String::from_utf8_lossy(&data[start..]).to_string()
}

pub fn list_crashes(root: &Path) -> Vec<LogFile> {
    let dir = root.join("crash-reports");
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            out.push(LogFile {
                name: entry.file_name().to_string_lossy().to_string(),
                size: fs::metadata(&p).map(|m| m.len()).unwrap_or(0),
                modified: modified_of(&p),
            });
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

pub fn read_crash(root: &Path, name: &str) -> Result<String, String> {
    let dir = root.join("crash-reports");
    let path = safe_join(&dir, name)?;
    let data = fs::read(&path).map_err(|e| format!("อ่าน crash report ไม่สำเร็จ: {}", e))?;
    let start = data.len().saturating_sub(120_000);
    Ok(String::from_utf8_lossy(&data[start..]).to_string())
}
