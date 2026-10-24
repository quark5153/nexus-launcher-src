//! nexus-core — the headless engine behind the launcher.
//!
//! No Tauri, no windowing, no UI: only files, network, SQLite and processes,
//! so it can be reused by the desktop shell, a CLI, or tests.

pub mod archive;
pub mod auth;
pub mod content;
pub mod curseforge;
pub mod engine;
pub mod forge;
pub mod java;
pub mod modrinth;
pub mod state;
pub mod vanilla;

pub use engine::{
    default_base_dir, Artifact, CustomClientHooks, DeltaSyncEngine, SandboxEnvironment,
};
pub use state::AppConfig;
pub use vanilla::VersionEntry;
