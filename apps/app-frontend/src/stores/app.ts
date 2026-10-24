import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { ipc, events } from "../api/ipc";
import { DEFAULT_CURSEFORGE_KEY, DEFAULT_MS_CLIENT_ID } from "../config";
import type {
  Account,
  AppConfig,
  CfHit,
  ClientCheck,
  ContentEntry,
  DeviceStart,
  InstanceRecord,
  JavaRuntime,
  LogFile,
  ModMeta,
  ModrinthHit,
  ModSource,
  ProgressPayload,
  TabId,
  VersionEntry,
} from "../types";

const emptyConfig = (): AppConfig => ({
  username: "SakuraPlayer",
  ram_mb: 4096,
  java_path: "",
  extra_jvm: "",
  selected_version: "1.21.1",
  instances: [],
  accounts: [],
  last_played: null,
  ms_client_id: "",
  curseforge_key: "",
  auto_java: true,
  active_account: null,
  close_on_launch: false,
});

export const useAppStore = defineStore("app", () => {
  const tab = ref<TabId>("instances");
  const config = ref<AppConfig>(emptyConfig());
  const manifest = ref<VersionEntry[]>([]);
  const installed = ref<string[]>([]);
  const logs = ref<string[]>([]);
  const progress = ref<ProgressPayload | null>(null);
  const busy = ref(false);
  const booted = ref(false);
  const toast = ref("");
  const java = ref<string | null>(null);
  const javaRuntimes = ref<JavaRuntime[]>([]);
  const dataDir = ref("");
  const exportsDir = ref("");
  const appVersion = ref("0.1.0");

  /* "0.1.0" → "v0.1 Beta": the launcher is still beta, so the label says so. */
  const versionLabel = computed(() => {
    const raw = appVersion.value || "0.1.0";
    const [major = "0", minor = "0", patch = "0"] = raw.split(".");
    const extra = patch !== "0" ? `.${patch}` : "";
    return `v${major}.${minor}${extra} Beta`;
  });

  const modHits = ref<ModrinthHit[]>([]);
  const cfHits = ref<CfHit[]>([]);
  const modQuery = ref("");
  const modType = ref<"modpack" | "mod">("modpack");
  const modSource = ref<ModSource>("modrinth");
  const modLimit = ref(20);
  const modTotal = ref(0);
  const modLoadingMore = ref(false);
  const packUrl = ref("");
  const versionFilter = ref<"release" | "snapshot" | "all">("release");
  const versionSearch = ref("");
  const newAccount = ref("");

  const device = ref<DeviceStart | null>(null);
  const deviceBusy = ref(false);
  const msCheck = ref<ClientCheck | null>(null);
  const msCheckBusy = ref(false);
  let deviceTimer: number | undefined;

  const contentKind = ref("worlds");
  const contentEntries = ref<ContentEntry[]>([]);
  /* jar file name → Modrinth title/icon, filled lazily for the mods tab */
  const modMeta = ref<Record<string, ModMeta>>({});
  const currentLog = ref("");
  const crashes = ref<LogFile[]>([]);
  const crashText = ref("");

  const newName = ref("");
  const newMc = ref("1.21.1");
  const newLoader = ref("vanilla");
  const newLoaderVersion = ref("");
  const loaderVersions = ref<string[]>([]);
  const loaderVersionsLoading = ref(false);

  let toastTimer: number | undefined;

  function say(message: string) {
    toast.value = message;
    window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => {
      if (toast.value === message) toast.value = "";
    }, 3600);
  }

  function log(line: string) {
    logs.value.push(line);
    if (logs.value.length > 400) logs.value.splice(0, logs.value.length - 400);
  }

  const selectedInstance = computed<InstanceRecord | undefined>(() =>
    config.value.instances.find((i) => i.id === config.value.selected_version),
  );

  const selectedName = computed(() => {
    const inst = selectedInstance.value;
    if (inst) return inst.name;
    return config.value.selected_version
      ? `Minecraft ${config.value.selected_version}`
      : "Minecraft";
  });

  const selectedVersion = computed(
    () => selectedInstance.value?.mc_version ?? config.value.selected_version ?? "1.21.1",
  );

  const selectedLoader = computed(() => {
    const inst = selectedInstance.value;
    if (!inst?.loader) return "VANILLA";
    return `${inst.loader.toUpperCase()}${inst.loader_version ? " " + inst.loader_version : ""}`;
  });

  const isSelectedInstalled = computed(() =>
    installed.value.includes(config.value.selected_version),
  );

  const filteredVersions = computed(() => {
    const q = versionSearch.value.trim().toLowerCase();
    return manifest.value
      .filter((v) => {
        const kindOk = versionFilter.value === "all" || v.type === versionFilter.value;
        const searchOk = !q || v.id.toLowerCase().includes(q);
        return kindOk && searchOk;
      })
      .slice(0, 120);
  });

  const recentVersions = computed(() =>
    manifest.value.filter((v) => v.type === "release").slice(0, 5),
  );

  const ramPercent = computed(() => Math.min(100, Math.round((config.value.ram_mb / 16384) * 100)));

  const effectiveRam = computed(() => selectedInstance.value?.ram_mb ?? config.value.ram_mb);

  const activeAccount = computed<Account | undefined>(() =>
    config.value.accounts.find((a) => a.id === config.value.active_account),
  );

  const signedIn = computed(() => activeAccount.value?.kind === "Microsoft");

  async function persist() {
    try {
      await ipc.saveConfig(config.value);
    } catch (err) {
      console.warn("save_config failed", err);
    }
  }

  async function boot() {
    try {
      const [unlistenLog, unlistenProgress] = await Promise.all([
        events.onLog((line) => log(line)),
        events.onProgress((p) => (progress.value = p)),
      ]);
      void unlistenLog;
      void unlistenProgress;

      const payload = await ipc.bootstrap();
      config.value = payload.config;
      /* เติมค่าที่ฝังมากับแอปให้ช่องที่ยังว่าง (ผู้ใช้ไม่ต้องกรอกเอง) */
      if (!config.value.curseforge_key && DEFAULT_CURSEFORGE_KEY) {
        config.value.curseforge_key = DEFAULT_CURSEFORGE_KEY;
        void persist();
      }
      manifest.value = payload.manifest;
      installed.value = payload.installed;
      java.value = payload.java;
      javaRuntimes.value = payload.java_runtimes;
      dataDir.value = payload.data_dir;
      exportsDir.value = payload.exports_dir;
      appVersion.value = payload.app_version;
      log(`[OK] nexus-core ${payload.app_version} พร้อมทำงาน`);
      if (payload.manifest.length === 0) {
        log("[ERR] ดึงรายการเวอร์ชันจาก Mojang ไม่สำเร็จ (ตรวจเน็ต)");
      } else {
        log(`[OK] พบ Minecraft ${payload.manifest.length} เวอร์ชันจาก Mojang`);
      }
      if (payload.java) log(`[OK] ตรวจพบ Java: ${payload.java}`);
      if (payload.config.instances.length === 0) log("[..] ยังไม่มีโปรไฟล์ — สร้างได้ที่หน้าอินสแตนซ์");
    } catch (err) {
      log(`[ERR] เชื่อมต่อ backend ไม่ได้: ${err}`);
      manifest.value = [
        { id: "1.21.1", type: "release", url: "", release_time: "2024-08-08" },
        { id: "1.20.1", type: "release", url: "", release_time: "2023-06-12" },
      ];
    } finally {
      booted.value = true;
    }
  }

  async function selectInstance(id: string) {
    if (config.value.selected_version === id) return;
    config.value.selected_version = id;
    await persist();
    const name = config.value.instances.find((i) => i.id === id)?.name ?? id;
    say(`สลับไปใช้ ${name} แล้ว`);
  }

  async function launch(shouldLaunch = true) {
    if (busy.value) return;
    busy.value = true;
    progress.value = { label: "กำลังตรวจสอบแซนด์บ็อกซ์...", frac: 0.02 };
    try {
      const pid = await ipc.launch(config.value.selected_version, shouldLaunch);
      say(shouldLaunch ? `เกมกำลังเปิด (PID ${pid ?? "?"})` : "ติดตั้งไฟล์เรียบร้อย");
      if (!shouldLaunch) installed.value = [...installed.value, config.value.selected_version];
    } catch (err) {
      log(`[ERR] ${err}`);
      say(`ผิดพลาด: ${err}`);
    } finally {
      busy.value = false;
      progress.value = null;
    }
  }

  /** เริ่มค้นหาใหม่จากหน้าแรก — ล้างผลเดิมทิ้ง */
  async function searchMods() {
    busy.value = true;
    modTotal.value = 0;
    try {
      if (modSource.value === "curseforge") {
        cfHits.value = await ipc.searchCurseforge(modQuery.value, modType.value, modLimit.value);
        say(`พบ ${cfHits.value.length} รายการบน CurseForge`);
      } else {
        modHits.value = [];
        const page = await ipc.searchModrinth(modQuery.value, modType.value, modLimit.value, 0);
        modHits.value = page.hits;
        modTotal.value = page.total_hits;
        say(`พบ ${page.total_hits.toLocaleString()} รายการบน Modrinth`);
      }
    } catch (err) {
      say(`ค้นหาไม่สำเร็จ: ${err}`);
    } finally {
      busy.value = false;
    }
  }

  /** ดึงผลชุดถัดไปต่อท้ายของเดิม (โหลดทีละ modLimit รายการจนหมด) */
  async function loadMoreMods() {
    if (modLoadingMore.value || busy.value) return;
    if (modSource.value !== "modrinth") return;
    if (modHits.value.length >= modTotal.value) return;
    modLoadingMore.value = true;
    try {
      const page = await ipc.searchModrinth(
        modQuery.value,
        modType.value,
        modLimit.value,
        modHits.value.length,
      );
      const seen = new Set(modHits.value.map((h) => h.project_id));
      modHits.value = [...modHits.value, ...page.hits.filter((h) => !seen.has(h.project_id))];
      modTotal.value = page.total_hits;
    } catch (err) {
      say(`โหลดเพิ่มไม่สำเร็จ: ${err}`);
    } finally {
      modLoadingMore.value = false;
    }
  }

  async function installHit(hit: ModrinthHit) {
    busy.value = true;
    progress.value = { label: `กำลังติดตั้ง ${hit.title}`, frac: 0.35 };
    try {
      config.value = await ipc.installProject(
        hit.slug || hit.project_id,
        hit.title,
        hit.project_type,
        "modrinth",
        config.value.selected_version,
      );
      say(`ติดตั้ง ${hit.title} สำเร็จ`);
    } catch (err) {
      say(`ติดตั้งไม่สำเร็จ: ${err}`);
    } finally {
      busy.value = false;
      progress.value = null;
    }
  }

  async function installCfHit(hit: CfHit) {
    busy.value = true;
    progress.value = { label: `กำลังติดตั้ง ${hit.name}`, frac: 0.35 };
    try {
      config.value = await ipc.installProject(
        String(hit.id),
        hit.name,
        modType.value,
        "curseforge",
        config.value.selected_version,
      );
      say(`ติดตั้ง ${hit.name} สำเร็จ`);
    } catch (err) {
      say(`ติดตั้งไม่สำเร็จ: ${err}`);
    } finally {
      busy.value = false;
      progress.value = null;
    }
  }

  async function importPack() {
    if (!packUrl.value.trim()) return;
    busy.value = true;
    try {
      config.value = await ipc.installPackUrl(packUrl.value.trim());
      packUrl.value = "";
      say("นำเข้าแพ็กสำเร็จ");
    } catch (err) {
      say(`นำเข้าไม่สำเร็จ: ${err}`);
    } finally {
      busy.value = false;
    }
  }

  async function checkUpdates() {
    try {
      say(await ipc.checkUpdates());
    } catch (err) {
      say(`เช็กอัปเดตไม่สำเร็จ: ${err}`);
    }
  }

  async function updatePack(id: string) {
    busy.value = true;
    try {
      config.value = await ipc.updatePack(id);
      say("อัปเดตแพ็กเรียบร้อย");
    } catch (err) {
      say(`${err}`);
    } finally {
      busy.value = false;
    }
  }

  async function refreshJava() {
    try {
      const [found, runtimes] = await Promise.all([ipc.detectJava(), ipc.listJava()]);
      javaRuntimes.value = runtimes;
      if (found) {
        java.value = found;
        config.value.java_path = found;
        await persist();
        say(`พบ Java: ${found}`);
      } else {
        say("ไม่พบ Java ในเครื่อง ระบุ path เองได้ในหน้าตั้งค่า");
      }
    } catch (err) {
      say(`ตรวจ Java ไม่สำเร็จ: ${err}`);
    }
  }

  async function installJava(major: number) {
    busy.value = true;
    progress.value = { label: `กำลังติดตั้ง Java ${major}`, frac: 0.2 };
    try {
      const path = await ipc.installJava(major);
      java.value = path;
      javaRuntimes.value = await ipc.listJava();
      say(`ติดตั้ง Java ${major} พร้อมใช้แล้ว`);
    } catch (err) {
      say(`ติดตั้ง Java ไม่สำเร็จ: ${err}`);
    } finally {
      busy.value = false;
      progress.value = null;
    }
  }

  async function openFolder() {
    try {
      await ipc.openDataFolder();
    } catch (err) {
      say(`เปิดโฟลเดอร์ไม่ได้: ${err}`);
    }
  }

  async function openUrl(url: string) {
    try {
      await ipc.openUrl(url);
    } catch (err) {
      say(`เปิดลิงก์ไม่ได้: ${err}`);
    }
  }

  async function openInstanceFolder(sub?: string, id?: string) {
    try {
      await ipc.openInstanceFolder(id ?? config.value.selected_version, sub);
    } catch (err) {
      say(`เปิดโฟลเดอร์ไม่ได้: ${err}`);
    }
  }

  /* ── accounts ───────────────────────────────────────────────────────── */

  function addAccount() {
    const name = newAccount.value.trim();
    if (!name) return;
    const acc: Account = {
      id: `acc-${Date.now()}`,
      username: name,
      kind: "Offline",
      skin_seed: name.length * 41,
      uuid: null,
    };
    config.value.accounts.push(acc);
    config.value.username = name;
    config.value.active_account = acc.id;
    newAccount.value = "";
    void persist();
    say(`เพิ่มโปรไฟล์ ${name} แล้ว`);
  }

  async function useAccount(acc: Account) {
    try {
      config.value = await ipc.setActiveAccount(acc.id);
      say(`สลับมาเล่นชื่อ ${acc.username}`);
    } catch (err) {
      say(`สลับบัญชีไม่สำเร็จ: ${err}`);
    }
  }

  async function removeAccount(acc: Account) {
    try {
      config.value =
        acc.kind === "Microsoft"
          ? await ipc.msLogout(acc.id)
          : (() => {
              config.value.accounts = config.value.accounts.filter((a) => a.id !== acc.id);
              if (config.value.active_account === acc.id) {
                config.value.active_account = config.value.accounts[0]?.id ?? null;
              }
              void persist();
              return config.value;
            })();
    } catch (err) {
      say(`ลบบัญชีไม่สำเร็จ: ${err}`);
    }
  }

  function stopDevicePoll() {
    window.clearTimeout(deviceTimer);
    deviceTimer = undefined;
    deviceBusy.value = false;
  }

  /** Client id ใช้ค่าที่ฝังมากับแอปเสมอ — ผู้ใช้ไม่ต้องกรอกเอง */
  function msClientId(): string {
    return config.value.ms_client_id.trim() || DEFAULT_MS_CLIENT_ID.trim();
  }

  async function startMsLogin() {
    const clientId = msClientId();
    if (!clientId) {
      say("แอปนี้ยังไม่ได้ฝัง Azure client id (src/config.ts)");
      return;
    }
    deviceBusy.value = true;
    try {
      const info = await ipc.msStart(clientId);
      device.value = info;
      const started = Date.now();
      const poll = async () => {
        if (!device.value) return;
        try {
          const updated = await ipc.msPoll(clientId, info.device_code);
          if (updated) {
            config.value = updated;
            device.value = null;
            stopDevicePoll();
            say("เข้าสู่ระบบ Microsoft สำเร็จ");
            return;
          }
        } catch (err) {
          device.value = null;
          stopDevicePoll();
          msCheck.value = { ok: false, message: `${err}` };
          say("ล็อกอินไม่สำเร็จ — ดูรายละเอียดในหน้านี้หรือหน้าตั้งค่า");
          return;
        }
        if (Date.now() - started > info.expires_in * 1000) {
          device.value = null;
          stopDevicePoll();
          say("โค้ดหมดอายุ");
          return;
        }
        deviceTimer = window.setTimeout(poll, Math.max(3, info.interval) * 1000);
      };
      deviceTimer = window.setTimeout(poll, Math.max(3, info.interval) * 1000);
    } catch (err) {
      deviceBusy.value = false;
      msCheck.value = { ok: false, message: `${err}` };
      say("ล็อกอินไม่สำเร็จ — ดูรายละเอียดในหน้านี้หรือหน้าตั้งค่า");
    }
  }

  function cancelMsLogin() {
    stopDevicePoll();
    device.value = null;
  }

  /** Validates the Azure client id before the user starts the device flow. */
  async function checkMsClient() {
    const id = msClientId();
    msCheckBusy.value = true;
    msCheck.value = null;
    await persist();
    try {
      msCheck.value = await ipc.msCheck(id);
    } catch (err) {
      msCheck.value = { ok: false, message: `${err}` };
    } finally {
      msCheckBusy.value = false;
    }
  }

  /* ── instances ──────────────────────────────────────────────────────── */

  async function createInstance() {
    if (!newMc.value) return;
    busy.value = true;
    try {
      config.value = await ipc.createInstance(
        newName.value,
        newMc.value,
        newLoader.value === "vanilla" ? null : newLoader.value,
        newLoaderVersion.value || null,
      );
      newName.value = "";
      newLoaderVersion.value = "";
      say("สร้างโปรไฟล์ใหม่แล้ว");
    } catch (err) {
      say(`สร้างไม่สำเร็จ: ${err}`);
    } finally {
      busy.value = false;
    }
  }

  async function saveInstance(record: InstanceRecord) {
    try {
      config.value = await ipc.updateInstance(record);
      say("บันทึกการตั้งค่าโปรไฟล์แล้ว");
    } catch (err) {
      say(`บันทึกไม่สำเร็จ: ${err}`);
    }
  }

  async function deleteInstance(id: string, purge = false) {
    try {
      config.value = await ipc.deleteInstance(id, purge);
      if (purge) {
        contentEntries.value = [];
        modMeta.value = {};
      }
      say(purge ? "ลบโปรไฟล์และไฟล์ทั้งหมดแล้ว" : "ลบโปรไฟล์ออกจากคลังแล้ว");
    } catch (err) {
      say(`ลบไม่สำเร็จ: ${err}`);
    }
  }

  async function loadLoaderVersions() {
    const loader = newLoader.value;
    const mc = newMc.value;
    if (loader === "vanilla") {
      loaderVersions.value = [];
      newLoaderVersion.value = "";
      loaderVersionsLoading.value = false;
      return;
    }
    loaderVersionsLoading.value = true;
    try {
      const list = await ipc.listLoaderVersions(loader, mc);
      /* ช้ากว่าที่ผู้ใช้เปลี่ยนเวอร์ชัน/loader — ทิ้งผลเก่าไป */
      if (loader !== newLoader.value || mc !== newMc.value) return;
      loaderVersions.value = list;
      if (newLoaderVersion.value && !list.includes(newLoaderVersion.value)) {
        newLoaderVersion.value = "";
      }
    } catch (err) {
      if (loader === newLoader.value && mc === newMc.value) {
        loaderVersions.value = [];
        say(`ดึงรายการ ${loader} ไม่สำเร็จ: ${err}`);
      }
    } finally {
      if (loader === newLoader.value && mc === newMc.value) loaderVersionsLoading.value = false;
    }
  }

  /* ── content + diagnostics ──────────────────────────────────────────── */

  async function loadContent(kind?: string) {
    if (kind) contentKind.value = kind;
    const instanceId = config.value.selected_version;
    try {
      contentEntries.value = await ipc.listContent(instanceId, contentKind.value);
    } catch {
      contentEntries.value = [];
    }
    if (contentKind.value !== "mods") {
      modMeta.value = {};
      return;
    }
    /* icons/names come from Modrinth by jar hash; a miss just leaves the name */
    try {
      const meta = await ipc.modContentMeta(instanceId);
      if (config.value.selected_version === instanceId) modMeta.value = meta;
    } catch {
      modMeta.value = {};
    }
  }

  async function toggleContent(entry: ContentEntry) {
    try {
      const enabled = await ipc.toggleContent(
        config.value.selected_version,
        contentKind.value,
        entry.name,
      );
      entry.enabled = enabled;
      say(enabled ? `เปิดใช้งาน ${entry.name}` : `ปิดใช้งาน ${entry.name}`);
    } catch (err) {
      say(`${err}`);
    }
  }

  async function deleteContent(entry: ContentEntry) {
    try {
      await ipc.deleteContent(config.value.selected_version, contentKind.value, entry.name);
      contentEntries.value = contentEntries.value.filter((e) => e.name !== entry.name);
      say(`ลบ ${entry.name} แล้ว`);
    } catch (err) {
      say(`${err}`);
    }
  }

  async function loadLog() {
    try {
      currentLog.value = await ipc.readLog(config.value.selected_version);
    } catch (err) {
      currentLog.value = `${err}`;
    }
  }

  async function loadCrashes() {
    try {
      crashes.value = await ipc.listCrashes(config.value.selected_version);
    } catch {
      crashes.value = [];
    }
  }

  async function openCrash(name: string) {
    try {
      crashText.value = await ipc.readCrash(config.value.selected_version, name);
    } catch (err) {
      crashText.value = `${err}`;
    }
  }

  /* ── import / export ────────────────────────────────────────────────── */

  async function exportInstance(id?: string) {
    const target = id ?? config.value.selected_version;
    busy.value = true;
    try {
      const path = await ipc.exportInstance(target);
      say(`สำรองไว้ที่ ${path}`);
    } catch (err) {
      say(`สำรองไม่สำเร็จ: ${err}`);
    } finally {
      busy.value = false;
    }
  }

  async function suggestedExport(id?: string) {
    try {
      return await ipc.suggestedExportPath(id ?? config.value.selected_version);
    } catch {
      return exportsDir.value;
    }
  }

  async function importInstance(path: string) {
    if (!path.trim()) return;
    busy.value = true;
    try {
      config.value = await ipc.importInstance(path.trim());
      say("นำเข้าโปรไฟล์สำเร็จ");
    } catch (err) {
      say(`นำเข้าไม่สำเร็จ: ${err}`);
    } finally {
      busy.value = false;
    }
  }

  function setRam(mb: number) {
    config.value.ram_mb = Math.max(1024, Math.min(32768, Math.round(mb)));
  }

  return {
    tab,
    config,
    manifest,
    installed,
    logs,
    progress,
    busy,
    booted,
    toast,
    java,
    javaRuntimes,
    dataDir,
    exportsDir,
    appVersion,
    modHits,
    cfHits,
    modQuery,
    modType,
    modSource,
    modLimit,
    modTotal,
    modLoadingMore,
    packUrl,
    versionFilter,
    versionSearch,
    newAccount,
    device,
    deviceBusy,
    msCheck,
    msCheckBusy,
    contentKind,
    contentEntries,
    modMeta,
    versionLabel,
    currentLog,
    crashes,
    crashText,
    newName,
    newMc,
    newLoader,
    newLoaderVersion,
    loaderVersions,
    loaderVersionsLoading,
    selectedInstance,
    selectedName,
    selectedVersion,
    selectedLoader,
    isSelectedInstalled,
    filteredVersions,
    recentVersions,
    ramPercent,
    effectiveRam,
    activeAccount,
    signedIn,
    say,
    log,
    persist,
    boot,
    selectInstance,
    launch,
    searchMods,
    loadMoreMods,
    installHit,
    installCfHit,
    importPack,
    checkUpdates,
    updatePack,
    refreshJava,
    installJava,
    openFolder,
    openUrl,
    openInstanceFolder,
    addAccount,
    useAccount,
    removeAccount,
    msClientId,
    startMsLogin,
    cancelMsLogin,
    checkMsClient,
    createInstance,
    saveInstance,
    deleteInstance,
    loadLoaderVersions,
    loadContent,
    toggleContent,
    deleteContent,
    loadLog,
    loadCrashes,
    openCrash,
    exportInstance,
    suggestedExport,
    importInstance,
    setRam,
  };
});
