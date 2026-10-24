import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppConfig,
  BootstrapPayload,
  CfHit,
  ClientCheck,
  ContentEntry,
  DeviceStart,
  InstanceRecord,
  JavaRuntime,
  LogFile,
  ModMeta,
  ModrinthPage,
  ModSource,
  ProgressPayload,
} from "../types";

/** Every IPC call the frontend is allowed to make, in one typed place. */
export const ipc = {
  bootstrap: () => invoke<BootstrapPayload>("bootstrap"),
  saveConfig: (cfg: AppConfig) => invoke<void>("save_config", { cfg }),

  /* java */
  detectJava: () => invoke<string | null>("detect_java"),
  listJava: () => invoke<JavaRuntime[]>("list_java"),
  installJava: (major: number) => invoke<string>("install_java", { major }),

  /* folders */
  openDataFolder: () => invoke<void>("open_data_folder"),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openInstanceFolder: (instanceId: string, sub?: string) =>
    invoke<void>("open_instance_folder", { instanceId, sub: sub ?? null }),

  /* instances */
  createInstance: (
    name: string,
    mcVersion: string,
    loader?: string | null,
    loaderVersion?: string | null,
  ) =>
    invoke<AppConfig>("create_instance", {
      name,
      mcVersion,
      loader: loader ?? null,
      loaderVersion: loaderVersion ?? null,
    }),
  updateInstance: (record: InstanceRecord) => invoke<AppConfig>("update_instance", { record }),
  deleteInstance: (instanceId: string, purge: boolean) =>
    invoke<AppConfig>("delete_instance", { instanceId, purge }),
  listLoaderVersions: (loader: string, mcVersion: string) =>
    invoke<string[]>("list_loader_versions", { loader, mcVersion }),

  /* launch + install */
  launch: (versionOrInstanceId: string, launch: boolean) =>
    invoke<number | null>("launch_instance", { versionOrInstanceId, launch }),
  searchModrinth: (query: string, projectType: string, limit: number, offset = 0) =>
    invoke<ModrinthPage>("search_modrinth", { query, projectType, limit, offset }),
  searchCurseforge: (query: string, projectType: string, limit: number) =>
    invoke<CfHit[]>("search_curseforge", { query, projectType, limit }),
  installProject: (
    projectId: string,
    title: string,
    projectType: string,
    source: ModSource,
    instanceId: string,
  ) =>
    invoke<AppConfig>("install_project", {
      projectId,
      title,
      projectType,
      source,
      instanceId,
    }),
  installPackUrl: (url: string) => invoke<AppConfig>("install_custom_pack", { url }),

  /* import / export */
  suggestedExportPath: (instanceId: string) =>
    invoke<string>("suggested_export_path", { instanceId }),
  exportInstance: (instanceId: string, dest?: string | null) =>
    invoke<string>("export_instance", { instanceId, dest: dest ?? null }),
  importInstance: (path: string) => invoke<AppConfig>("import_instance", { path }),

  /* content + diagnostics */
  listContent: (instanceId: string, kind: string) =>
    invoke<ContentEntry[]>("list_content", { instanceId, kind }),
  modContentMeta: (instanceId: string) =>
    invoke<Record<string, ModMeta>>("mod_content_meta", { instanceId }),
  toggleContent: (instanceId: string, kind: string, name: string) =>
    invoke<boolean>("toggle_content", { instanceId, kind, name }),
  deleteContent: (instanceId: string, kind: string, name: string) =>
    invoke<void>("delete_content", { instanceId, kind, name }),
  readLog: (instanceId: string) => invoke<string>("read_log", { instanceId }),
  listCrashes: (instanceId: string) => invoke<LogFile[]>("list_crashes", { instanceId }),
  readCrash: (instanceId: string, name: string) =>
    invoke<string>("read_crash", { instanceId, name }),

  /* updates */
  checkUpdates: () => invoke<string>("check_updates"),
  updatePack: (instanceId: string) => invoke<AppConfig>("update_pack", { instanceId }),

  /* accounts */
  msStart: (clientId: string) => invoke<DeviceStart>("ms_device_start", { clientId }),
  msCheck: (clientId: string) => invoke<ClientCheck>("ms_check_client", { clientId }),
  msPoll: (clientId: string, deviceCode: string) =>
    invoke<AppConfig | null>("ms_device_poll", { clientId, deviceCode }),
  msLogout: (accountId: string) => invoke<AppConfig>("ms_logout", { accountId }),
  setActiveAccount: (accountId: string) =>
    invoke<AppConfig>("set_active_account", { accountId }),
};

export const events = {
  onLog: (cb: (line: string) => void): Promise<UnlistenFn> =>
    listen<string>("nexus-log", (e) => cb(e.payload)),
  onProgress: (cb: (p: ProgressPayload) => void): Promise<UnlistenFn> =>
    listen<ProgressPayload>("nexus-progress", (e) => cb(e.payload)),
};
