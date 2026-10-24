// engine — extracted from the original monolithic lib.rs into the nexus-core crate.

use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[cfg(windows)]
pub const PATH_SEP: &str = ";";
#[cfg(not(windows))]
pub const PATH_SEP: &str = ":";

#[cfg(windows)]
pub const DEFAULT_JAVA: &str = "C:/Program Files/Java/jdk-17/bin/javaw.exe";
#[cfg(not(windows))]
pub const DEFAULT_JAVA: &str = "/usr/bin/java";

/// Returns the default base directory for Nexus data on the current OS.
pub fn default_base_dir() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from("C:/Nexus_Data")
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".local/share/nexus")
        } else {
            PathBuf::from("/tmp/nexus_data")
        }
    }
}

/// Returns the platform-specific IPC pipe or Unix domain socket path for a provider.
pub fn default_ipc_pipe(provider: &str) -> String {
    #[cfg(windows)]
    {
        format!(r"\.\pipe\{}_ipc", provider)
    }
    #[cfg(not(windows))]
    {
        let uid = std::env::var("UID").unwrap_or_else(|_| "1000".to_string());
        format!("/tmp/nexus_{}_{}.sock", provider, uid)
    }
}

/// Makes a string safe to use as a directory name.
pub fn slug(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Shared HTTP client for every network request the launcher makes.
pub fn http_client() -> Client {
    Client::builder()
        .user_agent("NexusLauncher/2.0.0 (PureRust-TauriV2)")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .build()
        .unwrap_or_else(|_| Client::new())
}

/// Searches the machine for an installed Java runtime.
pub fn detect_java() -> Option<String> {
    let exe = if cfg!(windows) { "javaw.exe" } else { "java" };
    let mut found: Vec<PathBuf> = Vec::new();

    if let Ok(home) = std::env::var("JAVA_HOME") {
        let p = PathBuf::from(home).join("bin").join(exe);
        if p.exists() {
            found.push(p);
        }
    }

    let roots: Vec<&str> = if cfg!(windows) {
        vec![
            "C:/Program Files/Java",
            "C:/Program Files/Eclipse Adoptium",
            "C:/Program Files/Microsoft",
            "C:/Program Files/Zulu",
            "C:/Program Files/BellSoft",
            "C:/Program Files/Amazon Corretto",
        ]
    } else {
        vec!["/usr/lib/jvm"]
    };
    for root in roots {
        if let Ok(rd) = fs::read_dir(root) {
            let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
            dirs.sort();
            dirs.reverse();
            for d in dirs {
                let p = d.join("bin").join(exe);
                if p.exists() {
                    found.push(p);
                }
            }
        }
    }

    let fallback = if cfg!(windows) {
        "C:/ProgramData/Oracle/Java/javapath/javaw.exe"
    } else {
        "/usr/bin/java"
    };
    let fb = PathBuf::from(fallback);
    if fb.exists() {
        found.push(fb);
    }

    found.into_iter().next().map(|p| p.to_string_lossy().to_string())
}

/// Extracts a plausible Java major version out of a JVM path like
/// `/usr/lib/jvm/java-21-openjdk` or `C:/Program Files/Java/jdk-17`.
pub fn java_major_of_path(path: &str) -> Option<u32> {
    let bytes: Vec<char> = path.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if let Ok(n) = bytes[start..i].iter().collect::<String>().parse::<u32>() {
                if (8..=30).contains(&n) {
                    return Some(n);
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Picks the best installed JVM for the Java major version a Minecraft version needs.
/// Falls back to the configured/detected runtime when no exact match exists.
pub fn resolve_java_for_major(configured: &str, required_major: u32) -> String {
    if required_major == 0 {
        return resolve_java(configured);
    }
    if java_major_of_path(configured) == Some(required_major) {
        return configured.to_string();
    }
    #[cfg(unix)]
    {
        let mut dirs: Vec<PathBuf> = fs::read_dir("/usr/lib/jvm")
            .map(|rd| rd.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        dirs.sort();
        dirs.reverse();
        for d in dirs {
            if java_major_of_path(&d.to_string_lossy()) != Some(required_major) {
                continue;
            }
            let exe = d.join("bin").join("java");
            if exe.exists() {
                return exe.to_string_lossy().to_string();
            }
        }
    }
    resolve_java(configured)
}

pub fn resolve_java(configured: &str) -> String {
    if !configured.is_empty() && Path::new(configured).exists() {
        return configured.to_string();
    }
    if let Some(found) = detect_java() {
        return found;
    }
    if cfg!(windows) {
        "javaw".to_string()
    } else {
        "java".to_string()
    }
}

/// Pure Rust command-line tokenizer (replaces C++ `tokenize` in `nexus_engine.cpp`).
pub fn tokenize_args(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut token_started = false;

    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && !in_single_quote {
            if let Some(next_c) = chars.next() {
                current.push(next_c);
                token_started = true;
                continue;
            }
        }
        if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            token_started = true;
            continue;
        }
        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            token_started = true;
            continue;
        }
        if (c == ' ' || c == '\t' || c == '\n' || c == '\r')
            && !in_single_quote
            && !in_double_quote
        {
            if token_started {
                tokens.push(current.clone());
                current.clear();
                token_started = false;
            }
            continue;
        }
        current.push(c);
        token_started = true;
    }
    if token_started {
        tokens.push(current);
    }
    tokens
}

/// Pure Rust IPC Bridge Worker (replaces C++ `ipc_socket_worker_linux` & `ipc_pipe_worker_win`).
fn start_pure_rust_ipc_bridge(pipe_or_sock: String, payload: String) {
    std::thread::spawn(move || {
        #[cfg(unix)]
        {
            use std::os::unix::net::UnixListener;
            let _ = fs::remove_file(&pipe_or_sock);
            if let Ok(listener) = UnixListener::bind(&pipe_or_sock) {
                if let Ok((mut stream, _)) = listener.accept() {
                    if !payload.is_empty() {
                        let _ = stream.write_all(payload.as_bytes());
                        let _ = stream.flush();
                    }
                    let mut buf = [0u8; 4096];
                    while let Ok(n) = stream.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        let ack = br#"{"status":"ack","bridge":"nexus_pure_rust_unix_socket"}"#;
                        let _ = stream.write_all(ack);
                        let _ = stream.flush();
                    }
                }
                let _ = fs::remove_file(&pipe_or_sock);
            }
        }
        #[cfg(windows)]
        {
            // Pure Rust fallback listener notification
            let _ = (pipe_or_sock, payload);
        }
    });
}

/// Opens a folder (or file) with the desktop's default handler.
pub fn open_path(path: &Path) {
    let _ = fs::create_dir_all(path);
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(path).spawn();
    }
}

/// Opens an http(s) URL in the user's normal browser (never inside the webview).
pub fn open_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("รองรับเฉพาะลิงก์ http/https".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
            .map_err(|e| format!("เปิดบราวเซอร์ไม่สำเร็จ: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("เปิดบราวเซอร์ไม่สำเร็จ: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("เปิดบราวเซอร์ไม่สำเร็จ: {}", e))?;
    }
    Ok(())
}

/// Pure Rust JVM Process Spawner — 100% replacement for C++ `nexus_spawn_jvm`.
/// When `log_path` is given the game's stdout/stderr are streamed into it so
/// the launcher can show live output and crash context without a console.
pub fn spawn_game_process(
    java_path: &str,
    full_arguments: &str,
    working_dir: &Path,
    ipc_pipe_name: Option<&str>,
    ipc_handshake_payload: Option<&str>,
) -> Result<u32, String> {
    spawn_game_process_logged(
        java_path,
        full_arguments,
        working_dir,
        ipc_pipe_name,
        ipc_handshake_payload,
        None,
    )
}

pub fn spawn_game_process_logged(
    java_path: &str,
    full_arguments: &str,
    working_dir: &Path,
    ipc_pipe_name: Option<&str>,
    ipc_handshake_payload: Option<&str>,
    log_path: Option<&Path>,
) -> Result<u32, String> {
    if let Some(pipe) = ipc_pipe_name {
        if !pipe.is_empty() {
            start_pure_rust_ipc_bridge(
                pipe.to_string(),
                ipc_handshake_payload.unwrap_or("{}").to_string(),
            );
        }
    }

    let parsed_args = tokenize_args(full_arguments);

    let mut cmd = Command::new(java_path);
    cmd.args(&parsed_args)
        .current_dir(working_dir)
        .stdin(Stdio::null());

    match log_path {
        Some(path) => {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let out = fs::File::create(path)
                .map_err(|e| format!("สร้างไฟล์ log ไม่สำเร็จ {:?}: {}", path, e))?;
            let err = out
                .try_clone()
                .map_err(|e| format!("clone log handle failed: {}", e))?;
            cmd.stdout(Stdio::from(out)).stderr(Stdio::from(err));
        }
        None => {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const ABOVE_NORMAL_PRIORITY_CLASS: u32 = 0x00008000;
        cmd.creation_flags(CREATE_NO_WINDOW | ABOVE_NORMAL_PRIORITY_CLASS);
    }

    let child = cmd.spawn().map_err(|e| {
        format!(
            "Pure Rust JVM Spawner failed to execute '{}': {}",
            java_path, e
        )
    })?;

    let pid = child.id();
    println!("[Nexus Pure Rust] Spawned JVM PID {}", pid);
    Ok(pid)
}

#[derive(Debug, Clone)]
pub struct SandboxEnvironment {
    pub root_dir: PathBuf,
    pub mods_dir: PathBuf,
    pub natives_dir: PathBuf,
    pub libraries_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub config_dir: PathBuf,
    pub saves_dir: PathBuf,
}

impl SandboxEnvironment {
    pub fn initialize(
        base_dir: &Path,
        provider_id: &str,
        instance_id: &str,
    ) -> Result<Self, String> {
        let root_dir = base_dir.join(provider_id).join(instance_id);
        let mods_dir = root_dir.join("mods");
        let natives_dir = root_dir.join("natives");
        let libraries_dir = root_dir.join("libraries");
        let assets_dir = root_dir.join("assets");
        let config_dir = root_dir.join("config");
        let saves_dir = root_dir.join("saves");

        for dir in [
            &root_dir,
            &mods_dir,
            &natives_dir,
            &libraries_dir,
            &assets_dir,
            &config_dir,
            &saves_dir,
        ] {
            fs::create_dir_all(dir)
                .map_err(|e| format!("Failed to create sandbox dir {:?}: {}", dir, e))?;
        }

        Ok(Self {
            root_dir,
            mods_dir,
            natives_dir,
            libraries_dir,
            assets_dir,
            config_dir,
            saves_dir,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub relative_path: String,
    pub download_url: String,
    pub expected_sha1: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomClientHooks {
    pub java_agents: Vec<String>,
    pub tweak_classes: Vec<String>,
    pub jvm_system_props: Vec<(String, String)>,
    pub main_class_override: Option<String>,
    pub ipc_pipe_name: Option<String>,
    pub ipc_handshake_json: Option<String>,
}

fn sha1_matches(bytes: &[u8], expected: &str) -> bool {
    if expected.is_empty() || expected == "skip" {
        return true;
    }
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize()).eq_ignore_ascii_case(expected)
}

#[derive(Clone)]
pub struct DeltaSyncEngine {
    client: Client,
}

impl Default for DeltaSyncEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DeltaSyncEngine {
    pub fn new() -> Self {
        Self {
            client: http_client(),
        }
    }

    pub fn verify_local_hash(path: &Path, expected_sha1: &str) -> bool {
        if !path.exists() {
            return false;
        }
        if expected_sha1.is_empty() || expected_sha1 == "skip" {
            return true;
        }
        let mut file = match fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return false,
        };
        let mut hasher = Sha1::new();
        let mut buffer = [0u8; 8192];
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => hasher.update(&buffer[..n]),
                Err(_) => return false,
            }
        }
        let actual = hex::encode(hasher.finalize());
        actual.eq_ignore_ascii_case(expected_sha1)
    }

    fn is_current(path: &Path, art: &Artifact) -> bool {
        match fs::metadata(path) {
            Ok(meta) => {
                if art.size_bytes == 0 {
                    Self::verify_local_hash(path, &art.expected_sha1)
                } else {
                    meta.len() == art.size_bytes
                }
            }
            Err(_) => false,
        }
    }

    async fn fetch_one(client: &Client, art: &Artifact, target: &Path) -> Result<(), String> {
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed creating dir {:?}: {}", parent, e))?;
        }

        let mut last_err = String::new();
        for _attempt in 0..3 {
            match client.get(&art.download_url).send().await {
                Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                    Ok(bytes) => {
                        if !sha1_matches(&bytes, &art.expected_sha1) {
                            last_err = format!("SHA-1 mismatch for {}", art.relative_path);
                            continue;
                        }
                        match tokio::fs::write(target, &bytes).await {
                            Ok(_) => return Ok(()),
                            Err(e) => last_err = format!("Write error {:?}: {}", target, e),
                        }
                    }
                    Err(e) => last_err = format!("Read error {}: {}", art.download_url, e),
                },
                Ok(resp) => {
                    last_err = format!("HTTP {} for {}", resp.status(), art.download_url)
                }
                Err(e) => last_err = format!("Download error {}: {}", art.download_url, e),
            }
        }
        Err(last_err)
    }

    pub async fn sync_sandbox(
        &self,
        sandbox_root: &Path,
        artifacts: &[Artifact],
    ) -> Result<(), String> {
        self.sync_sandbox_progress(sandbox_root, artifacts, Arc::new(|_, _| {}))
            .await
    }

    pub async fn sync_sandbox_progress(
        &self,
        sandbox_root: &Path,
        artifacts: &[Artifact],
        on_progress: Arc<dyn Fn(usize, usize) + Send + Sync>,
    ) -> Result<(), String> {
        let mut pending = Vec::new();
        for art in artifacts {
            let target = sandbox_root.join(&art.relative_path);
            if !Self::is_current(&target, art) {
                pending.push((art.clone(), target));
            }
        }

        let total = pending.len();
        if total == 0 {
            return Ok(());
        }

        let done = Arc::new(AtomicUsize::new(0));
        let client = self.client.clone();
        let results: Vec<Result<(), String>> = stream::iter(pending)
            .map(|(art, target_path)| {
                let client = client.clone();
                let done = done.clone();
                let cb = on_progress.clone();
                async move {
                    let r = Self::fetch_one(&client, &art, &target_path).await;
                    let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                    cb(n, total);
                    r
                }
            })
            .buffer_unordered(16)
            .collect()
            .await;

        for res in results {
            res?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct InstanceTarget {
    pub provider: String,
    pub instance_id: String,
    pub loader: Option<(String, Option<String>)>,
}

impl InstanceTarget {
    pub fn vanilla(version_id: &str) -> Self {
        Self {
            provider: "native".to_string(),
            instance_id: slug(version_id),
            loader: None,
        }
    }

    pub fn for_instance(rec: &crate::state::InstanceRecord) -> Self {
        Self {
            provider: rec.provider.clone(),
            instance_id: rec.id.clone(),
            loader: rec.loader.as_ref().map(|l| (l.clone(), rec.loader_version.clone())),
        }
    }
}
