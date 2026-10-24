# NEXUS LAUNCHER v0.11 Beta — Clean Dark

**English** · [ไทย](README.th.md)

A cross-platform Minecraft launcher (Windows / Linux), written entirely in Rust (no C/C++) with a
**three-layer architecture** modelled on Modrinth App (Theseus) but with its own look: a clean dark
theme, softly rounded cards, a fading hairline (≈8 %) and a single pink accent that only appears on
active/selected elements.

---

## ✅ Features (v0.11 Beta)

| Feature | Status |
|---|---|
| Multiple instances — separate version/modpack/profile | Instances page — one real game folder per profile |
| Mod loaders: Vanilla / Fabric / Quilt | Fully working |
| Mod loaders: Forge / NeoForge | Official installer (experimental) |
| Mods/packs from Modrinth | Search, install with dependencies, pack update checks |
| Mods/packs from CurseForge | Requires the user's own API key (Settings page) |
| Import existing packs (.mrpack / CurseForge .zip / Nexus backup) | Instances page → Import |
| Automatic Java 8 / 17 / 21 | Checks the machine first, downloads from Adoptium only if missing |
| Per-instance RAM / JVM args | Instances page (empty = global value) |
| Multiple Microsoft accounts | Device-code flow using the Azure client id baked into the app |
| Instance import / export | One .zip — move machines or share with a friend |
| Worlds / Resource Packs / Shaders / Servers | Enable, delete, open folder, read servers.dat |
| Built-in log / crash reports | Captures the game's stdout/stderr plus the crash-report list |
| Modpack updates | Check and update per pack |
| No ads, no extras, never asks for a password | No backend of our own — direct to Mojang/Modrinth/CurseForge |

---

## 🏛️ Three-layer architecture

```text
nexus-launcher/                  ← Cargo workspace + monorepo
├── apps/
│   ├── app-frontend/            ← layer 1: UI (Vue 3 + Pinia + Tailwind)
│   │   ├── index.html
│   │   ├── vite.config.ts
│   │   └── src/
│   │       ├── App.vue          ← shell: title bar, dock navigation, log ticker
│   │       ├── assets/fonts/    ← fonts bundled in the app (Prompt + IBM Plex Mono), works offline
│   │       ├── views/           ← InstancesView, ModsView, AccountsView, SettingsView
│   │       ├── stores/app.ts    ← all UI state (Pinia)
│   │       ├── i18n.ts          ← Thai / English catalog
│   │       └── api/ipc.ts       ← typed IPC bridge (the only place that calls Tauri)
│   └── app/                     ← layer 2: Tauri v2 shell (thin)
│       ├── tauri.conf.json
│       ├── capabilities/        ← webview permissions (required in Tauri v2)
│       └── src/
│           ├── lib.rs           ← plugin host + generate_handler
│           ├── commands.rs      ← IPC surface: translates commands → nexus-core
│           └── main.rs
└── packages/
    └── nexus-core/              ← layer 3: the brain (pure Rust, no Tauri)
        └── src/
            ├── engine.rs        ← delta sync (SHA-1), natives, Java detection, JVM spawner + log capture
            ├── vanilla.rs       ← Mojang manifest, argument builder, loader merge (fabric/quilt/forge/neoforge)
            ├── forge.rs         ← installs Forge/NeoForge with the official installer
            ├── modrinth.rs      ← search/install mods & modpacks (.mrpack), dependencies, update checks
            ├── curseforge.rs    ← CurseForge: search, files, mods, packs (uses the user's API key)
            ├── auth.rs          ← Microsoft device-code login → Xbox Live → XSTS → Minecraft
            ├── java.rs          ← find a JDK by major + download a JRE from Adoptium automatically
            ├── archive.rs       ← export/import an instance as .zip (nexus / mrpack / CurseForge)
            ├── content.rs       ← worlds, resource packs, shaders, servers (NBT), log + crash report
            └── state.rs         ← SQLite (rusqlite): config/instances/accounts + token store
```

| Layer | Language / tech | Responsibility | Must not |
|---|---|---|---|
| 1 · `app-frontend` | Vue 3, Pinia, Tailwind | Rendering, UI state, IPC calls | Touch files/network/processes itself |
| 2 · `nexus-shell` | Tauri v2, Rust | Window, OS plugins, IPC commands, events | Hold business logic |
| 3 · `nexus-core` | Rust, tokio, reqwest, rusqlite | Downloads, SHA-1 checks, SQLite, JVM spawning | Know about Tauri/webview |

**How it differs from Modrinth App:** our core collapses into modules inside a single crate (Theseus is
split further), we do not use SQLx (plain synchronous rusqlite is simpler), Microsoft sign-in uses the
device-code flow with an Azure client id baked into the app (public client, no client secret, no
intermediate server), and the UI is our own "Clean Dark" design rather than the same Tailwind set.

### IPC commands

| Command | What it does |
|---|---|
| `bootstrap` | Loads config + installed + manifest + java runtimes + data/exports dir |
| `save_config` | Writes the config to SQLite |
| `create_instance` / `update_instance` / `delete_instance` | Manage profiles (per-profile RAM/java/JVM) |
| `launch_instance` | Prepares files + picks Java + spawns the JVM (`launch=false` = download only) |
| `list_java` / `install_java` / `detect_java` | Detect and install JRE 8/17/21 from Adoptium |
| `ms_check_client` | Pre-flights the client id and turns AADSTS errors into advice |
| `ms_device_start` / `ms_device_poll` / `ms_logout` / `set_active_account` | Microsoft sign-in + account switching |
| `search_modrinth` / `search_curseforge` / `install_project` | Search/install mods and packs from both sources |
| `install_custom_pack` | Import a pack from a URL (.mrpack / CurseForge .zip) |
| `export_instance` / `import_instance` / `suggested_export_path` | Back up / move machines / share |
| `list_content` / `toggle_content` / `delete_content` | Worlds, resource packs, shaders, servers |
| `read_log` / `list_crashes` / `read_crash` | In-app log and crash reports |
| `check_updates` / `update_pack` | Check/update Modrinth modpacks |
| `open_data_folder` / `open_instance_folder` / `list_loader_versions` | System helpers |

Events: `nexus-log` (log lines), `nexus-progress` (label + frac) → the progress bar and footer ticker.

---

## 🎨 Design — Clean Dark

* Neutral dark background `#0e1014` / panel `#15181e` / raised card `#1b1f27` — no gradients or shadows on layout
* **One accent**: pink `#ff8fb8`, used sparingly — active nav, selected pill, launch action, key status
* **Title bar**: logo + version on the left, Java status and the data-folder button as small pills on the right
* **Language**: a `TH | EN` switch sits in the title bar and the choice is remembered
* **Sidebar dock**: four icon destinations (Instances / Mods & Packs / Accounts / Settings) that magnify toward the pointer
* **Log ticker**: a mono line under the screen that scrolls on its own
* Fonts: **Prompt** (Thai + Latin, 3 weights) + IBM Plex Mono — the woff2 files are **bundled in the app**, so it looks identical offline

---

## 🚀 Build & install (Arch / CachyOS)

```bash
./build-package.sh            # 1) vite build  2) cargo build  3) makepkg
```

This produces `dist-packages/nexus-launcher-<version>-1-x86_64.pkg.tar.zst`; install it system-wide:

```bash
sudo pacman -U dist-packages/nexus-launcher-<version>-1-x86_64.pkg.tar.zst
```

The version moves in steps of 0.01 (v0.1 Beta → v0.11 Beta → v0.12 …) and every manifest is written
in one go:

```bash
./tools/bump-version.py        # +0.01 and updates tauri.conf.json / Cargo.toml / package.json / PKGBUILD
```

Or install for the current user only (no root, appears in the KDE menu):

```bash
./install-user.sh
```

### Dev mode

```bash
npm --prefix apps/app-frontend run dev      # terminal 1 (Vite :1420)
cargo run -p nexus-shell                    # terminal 2 → loads devUrl
```

> A release build **must** use `--features custom-protocol`, otherwise the app opens
> `http://localhost:1420` (`build-package.sh` already passes it).

### Data on disk

* State: `~/.local/share/nexus/nexus_state.sqlite3` (config + instances + account tokens)
* Game files/downloads: `~/.local/share/nexus/<provider>/<instance>/`
* Self-downloaded JREs: `~/.local/share/nexus/runtimes/jre-<major>/`
* Instance backups: `~/.local/share/nexus/exports/`
* Java: looks in `JAVA_HOME`, `/usr/lib/jvm/*`, `/usr/bin/java` and picks the major the game version
  asks for (if missing and `auto_java` is on, a JRE is downloaded from Adoptium once and reused)
