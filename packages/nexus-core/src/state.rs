// state — persistent launcher state (SQLite, single JSON blob + secrets table).

use crate::engine::{default_base_dir, DEFAULT_JAVA};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub username: String,
    pub kind: String,
    pub skin_seed: u32,
    /// Real Mojang UUID for Microsoft accounts; `None` means offline-mode account.
    #[serde(default)]
    pub uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceRecord {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub mc_version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub modpack_id: Option<String>,
    pub modpack_version: Option<String>,
    pub update_url: Option<String>,
    pub created_at: String,
    pub last_played: Option<String>,
    /// Per-instance overrides. `None` falls back to the global setting.
    #[serde(default)]
    pub ram_mb: Option<u32>,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub extra_jvm: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub username: String,
    pub ram_mb: u32,
    pub java_path: String,
    pub extra_jvm: String,
    pub selected_version: String,
    pub instances: Vec<InstanceRecord>,
    pub accounts: Vec<Account>,
    pub last_played: Option<String>,
    /// Azure application (client) id used for the Microsoft device-code flow.
    pub ms_client_id: String,
    /// User supplied CurseForge API key (stored locally, never sent anywhere else).
    pub curseforge_key: String,
    /// Download a matching JRE from Adoptium when no installed Java fits the version.
    pub auto_java: bool,
    /// `Account.id` currently selected for launching.
    pub active_account: Option<String>,
    /// Close the launcher window when the game starts.
    pub close_on_launch: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            username: "SakuraPlayer".to_string(),
            ram_mb: 4096,
            java_path: DEFAULT_JAVA.to_string(),
            extra_jvm: String::new(),
            selected_version: "1.21.1".to_string(),
            instances: Vec::new(),
            accounts: vec![Account {
                id: "acc-default".to_string(),
                username: "SakuraPlayer".to_string(),
                kind: "Offline".to_string(),
                skin_seed: 77,
                uuid: None,
            }],
            last_played: None,
            ms_client_id: String::new(),
            curseforge_key: String::new(),
            auto_java: true,
            active_account: Some("acc-default".to_string()),
            close_on_launch: false,
        }
    }
}

/// Long lived credentials for one Microsoft account. Kept out of `AppConfig`
/// so the JSON blob handed to the UI never contains a refresh token.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenSet {
    pub refresh_token: String,
    pub uuid: String,
    pub name: String,
    pub xuid: String,
}

pub fn sqlite_path() -> PathBuf {
    default_base_dir().join("nexus_state.sqlite3")
}

fn open_db() -> Option<Connection> {
    let db_path = sqlite_path();
    if let Some(parent) = db_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    Connection::open(&db_path).ok()
}

pub fn load() -> AppConfig {
    if let Some(conn) = open_db() {
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS app_state (id INTEGER PRIMARY KEY, json_data TEXT NOT NULL)",
            [],
        );
        if let Ok(raw) = conn.query_row("SELECT json_data FROM app_state WHERE id = 1", [], |row| {
            row.get::<_, String>(0)
        }) {
            match serde_json::from_str::<AppConfig>(&raw) {
                Ok(cfg) => return cfg,
                Err(err) => eprintln!("[nexus] config migrate failed: {err}"),
            }
        }
    }
    let default_cfg = AppConfig::default();
    save(&default_cfg);
    default_cfg
}

pub fn save(cfg: &AppConfig) {
    let Some(conn) = open_db() else { return };
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS app_state (id INTEGER PRIMARY KEY, json_data TEXT NOT NULL)",
        [],
    );
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO app_state (id, json_data) VALUES (1, ?1)",
            params![json],
        );
    }
}

pub fn save_tokens(account_id: &str, tokens: &TokenSet) {
    let Some(conn) = open_db() else { return };
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS account_tokens (account_id TEXT PRIMARY KEY, json TEXT NOT NULL)",
        [],
    );
    if let Ok(json) = serde_json::to_string(tokens) {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO account_tokens (account_id, json) VALUES (?1, ?2)",
            params![account_id, json],
        );
    }
}

pub fn load_tokens(account_id: &str) -> Option<TokenSet> {
    let conn = open_db()?;
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS account_tokens (account_id TEXT PRIMARY KEY, json TEXT NOT NULL)",
        [],
    );
    let raw: String = conn
        .query_row(
            "SELECT json FROM account_tokens WHERE account_id = ?1",
            params![account_id],
            |row| row.get(0),
        )
        .ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn clear_tokens(account_id: &str) {
    if let Some(conn) = open_db() {
        let _ = conn.execute(
            "DELETE FROM account_tokens WHERE account_id = ?1",
            params![account_id],
        );
    }
}

pub fn scan_installed() -> Vec<String> {
    let mut found = Vec::new();
    for provider in &["native", "modpack", "custom", "instances"] {
        let dir = default_base_dir().join(provider);
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                if e.path().join(".nexus_installed").exists() {
                    found.push(e.file_name().to_string_lossy().to_string());
                }
            }
        }
    }
    found.sort();
    found
}

/// Directory that holds an instance's own `.minecraft` layout.
pub fn instance_dir(rec: &InstanceRecord) -> PathBuf {
    default_base_dir().join(&rec.provider).join(&rec.id)
}

pub fn find_instance(cfg: &AppConfig, id: &str) -> Option<InstanceRecord> {
    cfg.instances.iter().find(|i| i.id == id).cloned()
}

pub fn upsert_instance(cfg: &mut AppConfig, rec: InstanceRecord) {
    match cfg.instances.iter().position(|i| i.id == rec.id) {
        Some(pos) => cfg.instances[pos] = rec,
        None => cfg.instances.push(rec),
    }
}

/// Turns the selected account into the credentials the JVM needs.
#[derive(Debug, Clone)]
pub struct LaunchUser {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
}

impl LaunchUser {
    pub fn offline(name: &str) -> Self {
        Self {
            username: name.to_string(),
            uuid: crate::vanilla::offline_uuid(name),
            access_token: "0".to_string(),
            user_type: "legacy".to_string(),
        }
    }
}

pub fn active_account(cfg: &AppConfig) -> Option<Account> {
    cfg.active_account
        .as_ref()
        .and_then(|id| cfg.accounts.iter().find(|a| &a.id == id))
        .or_else(|| cfg.accounts.first())
        .cloned()
}
