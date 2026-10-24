# NEXUS LAUNCHER v0.1 Beta — Clean Dark

Minecraft launcher สำหรับ Linux/CachyOS เขียน Rust ล้วนทั้งระบบ (ไม่มี C/C++) โครงสร้าง
**สถาปัตยกรรม 3 ชั้น (Three-Layer Architecture)** แบบเดียวกับ Modrinth App (Theseus)
แต่หน้าตาเป็นดีไซน์ของเราเอง: ธีมมืดสะอาด (เหมือนแอปเกมร่วมสมัย) การ์ดมุมโค้งนุ่ม
พาดขอบละลายตัว (≈8 %) และใช้สีชมพูเป็นสีเน้นเดียวที่โผล่เมื่อ active/เลือก

---

## ✅ ฟีเจอร์ (v0.1 Beta)

| ฟีเจอร์ | สถานะ |
|---|---|
| หลายอินสแตนซ์ แยกเวอร์ชัน/มอดแพ็ก/โปรไฟล์ | หน้าอินสแตนซ์ — แยกโฟลเดอร์เกมจริงต่อโปรไฟล์ |
| Mod Loader: Vanilla / Fabric / Quilt | ใช้งานได้เต็ม |
| Mod Loader: Forge / NeoForge | ใช้ installer อย่างเป็นทางการ (ทดลอง) |
| มอด/แพ็กจาก Modrinth | ค้นหา ติดตั้ง พร้อม dependency + เช็กอัปเดตแพ็ก |
| มอด/แพ็กจาก CurseForge | ต้องใส่ API key ของผู้ใช้เอง (หน้าตั้งค่า) |
| นำเข้าแพ็กเก่า (.mrpack / .zip CurseForge / ไฟล์สำรอง Nexus) | หน้าอินสแตนซ์ → นำเข้า |
| Java อัตโนมัติ 8 / 17 / 21 | ตรวจเครื่องก่อน ไม่มีค่อยโหลดจาก Adoptium |
| RAM / JVM args ต่ออินสแตนซ์ | หน้าอินสแตนซ์ (เว้นว่าง = ใช้ค่าทั้งระบบ) |
| บัญชี Microsoft หลายบัญชี | device-code flow ผ่าน Azure client id ของผู้ใช้ |
| Import / Export อินสแตนซ์ | .zip เดียวจบ ย้ายเครื่อง/แชร์เพื่อนได้ |
| Worlds / Resource Packs / Shaders / Servers | เปิด-ปิด, ลบ, เปิดโฟลเดอร์, อ่าน servers.dat |
| Log / Crash report ในตัว | เก็บ stdout/stderr ของเกม + รายการ crash-reports |
| อัปเดตมอดแพ็ก | เช็กและอัปเดตเป็นรายแพ็ก |
| ไม่มีโฆษณา / ไม่มีของแถม / ไม่ขอรหัสผ่าน | ไม่มีเน็ตเวิร์กของเราเองเลย — ต่อตรง Mojang/Modrinth/CurseForge |

---

## 🏛️ สถาปัตยกรรม 3 ชั้น

```text
nexus-launcher/                  ← Cargo workspace + monorepo
├── apps/
│   ├── app-frontend/            ← ชั้น 1: UI (Vue 3 + Pinia + Tailwind)
│   │   ├── index.html
│   │   ├── vite.config.ts
│   │   └── src/
│   │       ├── App.vue          ← shell: title bar, sidebar nav, log ticker
│   │       ├── assets/fonts/    ← ฟอนต์ฝังในแอป (Prompt + IBM Plex Mono) ใช้ได้ตอนออฟไลน์
│   │       ├── views/           ← PlayView, ModsView, InstancesView, VersionsView, AccountsView, SettingsView
│   │       ├── stores/app.ts    ← state ทั้งหมดของ UI (Pinia)
│   │       └── api/ipc.ts       ← typed IPC bridge (ที่เดียวที่เรียก Tauri)
│   └── app/                     ← ชั้น 2: Tauri v2 shell (บาง ๆ)
│       ├── tauri.conf.json
│       ├── capabilities/        ← สิทธิ์ของ webview (จำเป็นใน Tauri v2)
│       └── src/
│           ├── lib.rs           ← plugin host + generate_handler
│           ├── commands.rs      ← IPC surface: แปลงคำสั่ง → nexus-core
│           └── main.rs
└── packages/
    └── nexus-core/              ← ชั้น 3: สมองกล (Rust ล้วน ไม่มี Tauri)
        └── src/
            ├── engine.rs        ← delta sync (SHA-1), natives, Java detection, JVM spawner + log capture
            ├── vanilla.rs       ← Mojang manifest, argument builder, loader merge (fabric/quilt/forge/neoforge)
            ├── forge.rs         ← ติดตั้ง Forge/NeoForge ผ่าน installer อย่างเป็นทางการ
            ├── modrinth.rs      ← ค้นหา/ติดตั้ง mod & modpack (.mrpack), dependency, เช็กอัปเดต
            ├── curseforge.rs    ← CurseForge: ค้นหา, ไฟล์, มอด, แพ็ก (ใช้ API key ของผู้ใช้)
            ├── auth.rs          ← Microsoft device-code login → Xbox Live → XSTS → Minecraft
            ├── java.rs          ← หา JDK ตาม major + ดาวน์โหลด JRE จาก Adoptium อัตโนมัติ
            ├── archive.rs       ← export/import อินสแตนซ์เป็น .zip (nexus / mrpack / CurseForge)
            ├── content.rs       ← worlds, resource packs, shaders, servers (อ่าน NBT), log + crash report
            └── state.rs         ← SQLite (rusqlite): config/instances/accounts + token store
```

| ชั้น | ภาษา/เทคโนโลยี | หน้าที่ | ห้ามทำ |
|---|---|---|---|
| 1 · `app-frontend` | Vue 3, Pinia, Tailwind | หน้าตา, state ของ UI, เรียก IPC | แตะไฟล์/เน็ต/โปรเซสเอง |
| 2 · `nexus-shell` | Tauri v2, Rust | หน้าต่าง, ปลั๊กอิน OS, IPC commands, events | มี business logic |
| 3 · `nexus-core` | Rust, tokio, reqwest, rusqlite | ดาวน์โหลด, ตรวจ SHA-1, SQLite, สั่ง JVM | รู้จัก Tauri/webview |

**จุดที่ต่างจาก Modrinth App:** core ของเรายุบเป็นโมดูลจบใน crate เดียว (Theseus แยกย่อยกว่านี้),
ไม่ใช้ SQLx (ใช้ rusqlite แบบ synchronous ง่ายกว่า), ล็อกอิน Microsoft ใช้ device-code flow กับ
Azure client id ของผู้ใช้เอง (ไม่ต้องมี client secret ไม่มี server กลาง), และ UI เป็นดีไซน์
"Clean Dark" ของเราเอง ไม่ใช่ Tailwind ชุดเดียวกับเขา

### IPC ที่มี

| Command | ทำอะไร |
|---|---|
| `bootstrap` | โหลด config + installed + manifest + java runtimes + data/exports dir |
| `save_config` | บันทึก config ลง SQLite |
| `create_instance` / `update_instance` / `delete_instance` | จัดการโปรไฟล์ (แรม/java/JVM แยกต่อโปรไฟล์) |
| `launch_instance` | เตรียมไฟล์ + เลือก Java + สั่ง JVM (`launch=false` = ดาวน์โหลดอย่างเดียว) |
| `list_java` / `install_java` / `detect_java` | ตรวจหาและติดตั้ง JRE 8/17/21 จาก Adoptium |
| `ms_check_client` | ตรวจ client id ก่อนล็อกอิน — แปลง AADSTS error เป็นคำแนะนำภาษาไทย |
| `ms_device_start` / `ms_device_poll` / `ms_logout` / `set_active_account` | ล็อกอิน Microsoft + สลับบัญชี |
| `search_modrinth` / `search_curseforge` / `install_project` | ค้นหา/ติดตั้งมอดและแพ็กจากสองแหล่ง |
| `install_custom_pack` | นำเข้าแพ็กจาก URL (.mrpack / .zip CurseForge) |
| `export_instance` / `import_instance` / `suggested_export_path` | สำรอง/ย้ายเครื่อง/แชร์ให้เพื่อน |
| `list_content` / `toggle_content` / `delete_content` | worlds, resource packs, shaders, servers |
| `read_log` / `list_crashes` / `read_crash` | ดู log และ crash report ในตัว |
| `check_updates` / `update_pack` | เช็ก/อัปเดตมอดแพ็กจาก Modrinth |
| `open_data_folder` / `open_instance_folder` / `list_loader_versions` | งานระบบ |

Events: `nexus-log` (ข้อความ log), `nexus-progress` (label + frac) → แถบ progress + ticker ด้านล่าง

---

## 🎨 ดีไซน์ — Clean Dark

* พื้นหลังโทนมืดกลาง `#0e1014` / แผ่นคนล์ `#15181e` / การ์ดซ้อน `#1b1f27` — ไม่มีกรรมเรียด/ปริมพู
* **สีเน้นเดียว**: ชมพู `#ff8fb8` ใช้จุดโปร่ง — active nav, pill ที่เลือก, ปุ่มเล่น, สถานะสำคัญ
* **Title bar** กลางเฉียง: โลโก้ + เวอร์ชัน + สถานะ Java/ปุ่มเปิดโฟลเดอร์ แบบ pill เล็ก
* **Sidebar** ซ้าย: nav ไอคอน 5 หน้า (เล่นเกม / มอด & แพ็ก / เวอร์ชัน / โปรไฟล์ / ตั้งค่า) + การ์ดโปรไฟล์ล่าง
* **ปุ่มเล่น**: แถบกว้างเต็มคอลัมน์ สีชมพู rounded-xl (จุดชมพูใหญ่สุดในแอป) ใต้แถบ progress
* **ตาราง**: หน้า Versions เป็นตารางธรรมดา (เวอร์ชัน / ประเภท / วันวาง) หัว sticky
* **Log ticker**: บรรทัด mono ใต้หน้าจอ เลื่อนอัตโนมัติ
* ฟอนต์: **Prompt** (thai + latin, 3 น้ำหนัก) + IBM Plex Mono — **ฝังไฟล์ woff2 ไว้ในแอป** จึงเหมือนเดิมตอนออฟไลน์

---

## 🚀 build & ติดตั้งบน CachyOS

```bash
./build-package.sh            # 1) vite build  2) cargo build  3) makepkg
```

ได้ `dist-packages/nexus-launcher-0.1.0-1-x86_64.pkg.tar.zst` → ติดตั้งทั้งระบบ:

```bash
sudo pacman -U dist-packages/nexus-launcher-0.1.0-1-x86_64.pkg.tar.zst
```

เวอร์ชันเดินทีละ 0.01 (v0.1 Beta → v0.11 Beta → v0.12 …) — เขียนทุกไฟล์ให้พร้อมกันด้วย:

```bash
./tools/bump-version.py        # +0.01 แล้วอัปเดต tauri.conf.json / Cargo.toml / package.json / PKGBUILD
```

หรือติดตั้งเฉพาะผู้ใช้ (ไม่ต้อง root, ขึ้นเมนู KDE):

```bash
./install-user.sh
```

### dev mode

```bash
npm --prefix apps/app-frontend run dev      # เทอร์มินัล 1 (Vite :1420)
cargo run -p nexus-shell                    # เทอร์มินัล 2 → โหลด devUrl
```

> release build **ต้อง** ใส่ `--features custom-protocol` ไม่งั้นแอปจะไปเปิด `http://localhost:1420`
> (`build-package.sh` ใส่ให้แล้ว)

### ข้อมูลบนเครื่อง

* state: `~/.local/share/nexus/nexus_state.sqlite3` (config + instance + token ของบัญชี)
* เกม/ไฟล์ดาวน์โหลด: `~/.local/share/nexus/<provider>/<instance>/`
* JRE ที่ดาวน์โหลดเอง: `~/.local/share/nexus/runtimes/jre-<major>/`
* ไฟล์สำรองอินสแตนซ์: `~/.local/share/nexus/exports/`
* Java: หาจาก `JAVA_HOME`, `/usr/lib/jvm/*`, `/usr/bin/java` แล้วเลือก major ตาม `javaVersion` ของเวอร์ชันเกม
  (ไม่พบและเปิด auto_java → ดาวน์โหลด JRE จาก Adoptium ครั้งเดียวแล้วใช้ซ้ำ)

---

## 🔧 สิ่งที่แก้จากซอร์ส zip เดิม

| อาการ | สาเหตุ | ที่แก้ |
|---|---|---|
| Rust คอมไพล์ไม่ผ่าน 6 จุด | zip ทำ escape ในสตริงหาย (`'\''`, `'\t'`, `'\n'`, `'\r'`, `"\""` ×2) | คืน escape ใน `nexus-core` |
| `vue-tsc` ฟ้อง TS2322 | `v-for` ส่ง `string` เข้า union | ใช้ `as const` |
| เปิดแอปแล้วหน้าจอว่าง | ไม่มี feature `custom-protocol` → release ไปโหลด devUrl | เพิ่ม `[features]` + build flag |
| ค่าไม่บันทึก / ไม่มี DB | ไม่มี `capabilities/` ของ Tauri v2 → `listen()` ถูกปฏิเสธ | เพิ่ม `apps/app/capabilities/default.json` |
| ตัวกรองเวอร์ชันไม่ทำงาน | Rust ส่ง `type` แต่ UI อ่าน `kind` | ใช้ `v.type` ใน UI ใหม่ |
| เกม launch ไม่ติด | หยิบ Java 25 ทั้งที่ MC 1.21.1 ต้องใช้ 21 | `resolve_java_for_major()` เลือก JDK ให้ตรง |
| ล็อกอิน Microsoft ไม่ผ่านทุกครั้ง | ส่ง body เป็น JSON แต่ Entra รับเฉพาะ form-encoded → `AADSTS900144: body must contain client_id` | เปลี่ยน `devicecode` เป็น `.form([...])` + แปลงรหัส AADSTS เป็นคำแนะนำภาษาไทย |
| ล็อกอินผ่านถึง XSTS แล้วเจอ 403 `Invalid app registration` | Mojang รับเฉพาะ client id ที่อยู่ใน allow list ของตัวเอง (ตรวจด้วยมือทุกแอปใหม่) | ข้อความ error แปลงเป็นภาษาไทย + ลิงก์ฟอร์มขออนุมัติ `aka.ms/mce-reviewappid` |

## ข้อจำกัดที่รู้อยู่

* ทดสอบแล้ว: `cargo check` ทั้ง workspace, `vue-tsc`, unit test ของ export/import + content manager,
  build ผ่าน, หน้าต่างเปิด, `bootstrap` อ่าน/เขียน SQLite
* **Forge / NeoForge** ใช้ตัวติดตั้งอย่างเป็นทางการ (`--installClient`) — โค้ดพร้อมแต่ยังไม่ได้ทดสอบ
  ติดตั้งจริงกับทุกเวอร์ชัน เพราะต้องดาวน์โหลดหลายร้อย MB ต่อครั้ง
* **Microsoft login** ต้องมี Azure application (client) id ของผู้ใช้เอง (หน้าตั้งค่า) — ทดสอบกับ endpoint
  จริงแล้ว (ได้ device code / เจอ AADSTS ที่ถูกต้อง) แต่ยังไม่ได้ล็อกอินจบด้วยบัญชีจริง

### ตั้งค่า Azure ให้ล็อกอินได้ (สำคัญ)

App registration ต้องมี 3 อย่างนี้ ไม่อย่างนั้นจะได้ `AADSTS700016` / `AADSTS7000218`:

1. **Supported account types** = *Accounts in any organizational directory and personal Microsoft accounts*
   — บัญชี Minecraft ทุกบัญชีเป็นบัญชี Microsoft **ส่วนตัว** ถ้าแอปเป็น single-tenant จะหาไม่เจอใน tenant `consumers`
2. **Authentication → Allow public client flows** = *Yes* (device code flow ไม่มี client secret)
3. **API permissions** → Microsoft Graph → Delegated → `XboxLive.signin` (+ `offline_access` ตามมาเอง)

หน้าตั้งค่าในแอปมีปุ่ม **ตรวจสอบ client id** ที่ยิง `devicecode` จริงแล้วบอกว่า client id ใช้ได้หรือติดอะไร

4. **ต้องให้ Mojang อนุมัติ client id (allow list)** — ข้อนี้ Azure Portal เปิดให้เองไม่ได้
   `login_with_xbox` จะตอบ `403 Invalid app registration` จนกว่า Mojang จะเพิ่มแอปเข้า allow list
   (นโยบาย: ทุกแอปใหม่ที่เรียก Java game service API ต้องยื่นคำขอ) → กรอกฟอร์มที่
   <https://aka.ms/mce-reviewappid> ด้วย Application (client) ID แล้วรอผล
   ระหว่างรอ: เล่น Singleplayer/บัญชีออฟไลน์ได้ แต่เซิร์ฟเวอร์ออนไลน์ที่ต้องยืนยันตัวตนยังไม่ได้
* **CurseForge** ต้องมี API key ของผู้ใช้เอง — ค้นหา/ติดตั้งจะไม่ทำงานถ้าไม่ได้ใส่
* อัปเดตมอดเป็นรายแพ็ก (Modrinth) ยังไม่ใช่รายมอด และยังไม่มีการอัปเดตตัว launcher เอง
