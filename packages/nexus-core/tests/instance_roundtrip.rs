//! Exercises the pieces that touch the filesystem without needing a GUI:
//! instance export → import, and the content manager's enable/disable.

use nexus_core::state::InstanceRecord;
use nexus_core::{archive, content};
use std::fs;
use std::path::PathBuf;

fn temp_base(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nexus-test-{}-{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn demo_record() -> InstanceRecord {
    InstanceRecord {
        id: "inst-demo".to_string(),
        name: "Demo".to_string(),
        provider: "custom".to_string(),
        mc_version: "1.20.1".to_string(),
        loader: Some("fabric".to_string()),
        loader_version: Some("0.15.0".to_string()),
        modpack_id: None,
        modpack_version: None,
        update_url: None,
        created_at: "2026-01-01 00:00".to_string(),
        last_played: None,
        ram_mb: Some(6144),
        java_path: None,
        extra_jvm: Some("-XX:+UseZGC".to_string()),
        notes: None,
        favorite: false,
    }
}

#[test]
fn content_manager_lists_and_toggles() {
    let base = temp_base("content");
    let root = base.join("custom").join("inst-demo");
    fs::create_dir_all(root.join("saves/World One")).unwrap();
    fs::write(root.join("saves/World One/level.dat"), b"nbt").unwrap();
    fs::create_dir_all(root.join("resourcepacks")).unwrap();
    fs::write(root.join("resourcepacks/faithful.zip"), b"zip").unwrap();

    let worlds = content::list_content(&root, "worlds");
    assert_eq!(worlds.len(), 1);
    assert_eq!(worlds[0].name, "World One");
    assert!(worlds[0].enabled);

    let packs = content::list_content(&root, "resourcepacks");
    assert_eq!(packs.len(), 1);
    assert!(packs[0].enabled);

    let now_enabled = content::toggle_content(&root, "resourcepacks", "faithful.zip").unwrap();
    assert!(!now_enabled, "toggling an enabled pack must disable it");
    assert!(root.join("resourcepacks/faithful.zip.disabled").exists());

    let after = content::list_content(&root, "resourcepacks");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].name, "faithful.zip");
    assert!(!after[0].enabled);

    let back = content::toggle_content(&root, "resourcepacks", "faithful.zip").unwrap();
    assert!(back);
    assert!(root.join("resourcepacks/faithful.zip").exists());

    // path traversal must be refused
    assert!(content::delete_content(&root, "resourcepacks", "../../etc/passwd").is_err());

    let _ = fs::remove_dir_all(&base);
}

#[tokio::test]
async fn export_then_import_keeps_player_data_only() {
    let base = temp_base("archive");
    let root = base.join("custom").join("inst-demo");
    fs::create_dir_all(root.join("mods")).unwrap();
    fs::write(root.join("mods/example.jar"), b"jar-bytes").unwrap();
    fs::write(root.join("options.txt"), "fov:70").unwrap();
    fs::create_dir_all(root.join("saves/world")).unwrap();
    fs::write(root.join("saves/world/level.dat"), b"level").unwrap();

    // shared, re-downloadable content must not be exported
    fs::create_dir_all(root.join("libraries/net")).unwrap();
    fs::write(root.join("libraries/net/big.jar"), b"big").unwrap();
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::write(root.join("assets/index.json"), b"{}").unwrap();
    fs::write(root.join("client.jar"), b"client").unwrap();

    let rec = demo_record();
    let dest = base.join("exports/demo.zip");
    let packed = archive::export_instance(&rec, &root, &dest).unwrap();
    assert_eq!(packed, 3, "mods/options/saves only, got {packed}");

    let import_base = base.join("imported");
    let imported = archive::import_instance(&dest, &import_base, "", "1.21.1")
        .await
        .unwrap();

    assert_eq!(imported.mc_version, "1.20.1");
    assert_eq!(imported.loader.as_deref(), Some("fabric"));
    assert_eq!(imported.ram_mb, Some(6144));
    assert_ne!(imported.id, rec.id, "imported instances get a fresh id");
    assert_eq!(imported.provider, "custom");

    let new_root = import_base.join(&imported.provider).join(&imported.id);
    assert_eq!(fs::read(new_root.join("mods/example.jar")).unwrap(), b"jar-bytes");
    assert_eq!(fs::read(new_root.join("options.txt")).unwrap(), b"fov:70");
    assert!(new_root.join("saves/world/level.dat").exists());
    // the sandbox pre-creates an empty libraries/, but no shared jar may travel
    assert!(!new_root.join("libraries/net/big.jar").exists());
    assert!(!new_root.join("client.jar").exists());
    assert!(!new_root.join("assets/index.json").exists());
    assert!(new_root.join(".nexus_installed").exists());

    let _ = fs::remove_dir_all(&base);
}

#[tokio::test]
async fn import_rejects_unknown_zip() {
    let base = temp_base("badzip");
    let zip_path = base.join("mystery.zip");
    let file = fs::File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("readme.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    use std::io::Write;
    zip.write_all(b"hi").unwrap();
    zip.finish().unwrap();

    let err = archive::import_instance(&zip_path, &base.join("target"), "", "1.21.1")
        .await
        .unwrap_err();
    assert!(err.contains("ไม่รู้จักรูปแบบไฟล์"), "unexpected error: {err}");

    let _ = fs::remove_dir_all(&base);
}
