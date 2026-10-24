// java — auto provisioning of the JRE a Minecraft version needs.
//
// The version json tells us which Java major is required (8 for 1.8–1.16,
// 17 for 1.17–1.20, 21 for 1.20.5+). Matching runtimes are found on the
// machine first; when nothing fits and auto mode is on, an Adoptium JRE is
// downloaded once into `<data>/runtimes` and reused by every instance.

use crate::engine::{http_client, java_major_of_path, resolve_java};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaRuntime {
    pub path: String,
    pub major: u32,
    pub source: String,
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn os_key() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

fn arch_key() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    }
}

fn runtimes_dir(base: &Path) -> PathBuf {
    base.join("runtimes")
}

/// Every JVM the launcher can see: JAVA_HOME, /usr/lib/jvm, managed runtimes.
pub fn scan() -> Vec<JavaRuntime> {
    let mut out: Vec<JavaRuntime> = Vec::new();
    let mut push = |path: PathBuf, source: &str| {
        if !path.exists() {
            return;
        }
        let text = path.to_string_lossy().to_string();
        if out.iter().any(|r| r.path == text) {
            return;
        }
        if let Some(major) = java_major_of_path(&text) {
            out.push(JavaRuntime {
                path: text,
                major,
                source: source.to_string(),
            });
        }
    };

    if let Ok(home) = std::env::var("JAVA_HOME") {
        push(PathBuf::from(home).join("bin").join(exe_name()), "JAVA_HOME");
    }

    if !cfg!(windows) {
        if let Ok(rd) = fs::read_dir("/usr/lib/jvm") {
            for e in rd.flatten() {
                push(e.path().join("bin").join("java"), "system");
            }
        }
    }

    let managed = runtimes_dir(&crate::engine::default_base_dir());
    if let Ok(rd) = fs::read_dir(&managed) {
        for e in rd.flatten() {
            push(e.path().join("bin").join(exe_name()), "managed");
        }
    }

    if !cfg!(windows) {
        push(PathBuf::from("/usr/bin/java"), "system");
    }

    out.sort_by(|a, b| a.major.cmp(&b.major));
    out
}

pub fn find_major(major: u32) -> Option<String> {
    scan()
        .into_iter()
        .find(|r| r.major == major)
        .map(|r| r.path)
}

/// Resolves (and if allowed, downloads) a runtime for the required major.
pub async fn ensure<F>(
    configured: &str,
    required: u32,
    auto: bool,
    base: &Path,
    log: F,
) -> Result<String, String>
where
    F: Fn(String) + Send + Sync,
{
    if required == 0 {
        return Ok(resolve_java(configured));
    }
    if !configured.is_empty() && java_major_of_path(configured) == Some(required) {
        return Ok(configured.to_string());
    }
    if let Some(found) = find_major(required) {
        return Ok(found);
    }
    if !auto {
        log(format!(
            "[..] ไม่พบ Java {} ในเครื่อง — ใช้ runtime ที่ตั้งไว้แทน",
            required
        ));
        return Ok(resolve_java(configured));
    }

    log(format!(
        "[..] ไม่พบ Java {} — กำลังดาวน์โหลด JRE จาก Adoptium",
        required
    ));
    match download(required, base, &log).await {
        Ok(path) => {
            log(format!("[OK] ติดตั้ง Java {} ให้แล้ว: {}", required, path));
            Ok(path)
        }
        Err(err) => {
            log(format!(
                "[ERR] ดาวน์โหลด Java {} ไม่สำเร็จ: {} — จะลองใช้ runtime เดิม",
                required, err
            ));
            Ok(resolve_java(configured))
        }
    }
}

#[derive(Debug, Deserialize)]
struct AdoptiumRelease {
    binary: AdoptiumBinary,
}

#[derive(Debug, Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Debug, Deserialize)]
struct AdoptiumPackage {
    link: String,
    name: String,
}

pub async fn download<F>(major: u32, base: &Path, log: &F) -> Result<String, String>
where
    F: Fn(String) + Send + Sync,
{
    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{}/hotspot?architecture={}&image_type=jre&os={}&vendor=eclipse",
        major,
        arch_key(),
        os_key()
    );
    let resp = http_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Adoptium request failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Adoptium HTTP {}", resp.status()));
    }
    let releases: Vec<AdoptiumRelease> = resp
        .json()
        .await
        .map_err(|e| format!("Adoptium parse failed: {}", e))?;
    let pkg = releases
        .into_iter()
        .next()
        .ok_or_else(|| format!("Adoptium ไม่มี JRE {} สำหรับระบบนี้", major))?
        .binary
        .package;

    let dir = runtimes_dir(base).join(format!("jre-{}", major));
    if dir.join("bin").join(exe_name()).exists() {
        return Ok(dir.join("bin").join(exe_name()).to_string_lossy().to_string());
    }
    let _ = fs::create_dir_all(&dir);

    let archive_path = dir.join(&pkg.name);
    log(format!("[..] ดาวน์โหลด {}", pkg.name));
    let bytes = http_client()
        .get(&pkg.link)
        .send()
        .await
        .map_err(|e| format!("download failed: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("download read failed: {}", e))?;
    fs::write(&archive_path, &bytes).map_err(|e| format!("save archive failed: {}", e))?;

    if pkg.name.ends_with(".zip") {
        let file = fs::File::open(&archive_path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("zip open: {}", e))?;
        zip.extract(&dir).map_err(|e| format!("zip extract: {}", e))?;
    } else {
        let status = std::process::Command::new("tar")
            .arg("-xzf")
            .arg(&archive_path)
            .arg("-C")
            .arg(&dir)
            .arg("--strip-components=1")
            .status()
            .map_err(|e| format!("tar not available: {}", e))?;
        if !status.success() {
            return Err("แตกไฟล์ tar.gz ไม่สำเร็จ".to_string());
        }
    }
    let _ = fs::remove_file(&archive_path);

    // Windows/macOS archives keep a top-level folder: flatten if needed.
    let mut bin = dir.join("bin").join(exe_name());
    if !bin.exists() {
        if let Ok(rd) = fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let candidate = entry.path().join("bin").join(exe_name());
                if candidate.exists() {
                    bin = candidate;
                    break;
                }
            }
        }
    }
    if !bin.exists() {
        return Err("แตกไฟล์เสร็จแต่ไม่พบ bin/java".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&bin) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&bin, perms);
        }
    }
    Ok(bin.to_string_lossy().to_string())
}
