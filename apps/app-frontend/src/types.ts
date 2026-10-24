export interface Account {
  id: string;
  username: string;
  kind: string;
  skin_seed: number;
  uuid?: string | null;
}

export interface InstanceRecord {
  id: string;
  name: string;
  provider: string;
  mc_version: string;
  loader?: string | null;
  loader_version?: string | null;
  modpack_id?: string | null;
  modpack_version?: string | null;
  update_url?: string | null;
  created_at: string;
  last_played?: string | null;
  ram_mb?: number | null;
  java_path?: string | null;
  extra_jvm?: string | null;
  notes?: string | null;
  favorite?: boolean;
}

export interface AppConfig {
  username: string;
  ram_mb: number;
  java_path: string;
  extra_jvm: string;
  selected_version: string;
  instances: InstanceRecord[];
  accounts: Account[];
  last_played?: string | null;
  ms_client_id: string;
  curseforge_key: string;
  auto_java: boolean;
  active_account?: string | null;
  close_on_launch: boolean;
}

/** Rust serialises `VersionEntry.kind` as `type`. */
export interface VersionEntry {
  id: string;
  type: string;
  url: string;
  release_time: string;
}

export interface ModrinthHit {
  project_id: string;
  project_type: string;
  slug: string;
  title: string;
  description: string;
  categories: string[];
  client_side: string;
  server_side: string;
  icon_url?: string | null;
  downloads: number;
  follows: number;
}

export interface ModrinthPage {
  hits: ModrinthHit[];
  total_hits: number;
  offset: number;
}

export interface CfHit {
  id: number;
  name: string;
  slug: string;
  summary: string;
  download_count: number;
  icon_url?: string | null;
  class_id?: number | null;
  latest_file_name?: string | null;
}

export interface JavaRuntime {
  path: string;
  major: number;
  source: string;
}

export interface ContentEntry {
  name: string;
  size: number;
  enabled: boolean;
  modified: string;
  kind: string;
  detail?: string | null;
  /** `data:image/png;base64,…` preview from the local files. */
  icon?: string | null;
}

/** Modrinth info resolved from an installed jar's hash. */
export interface ModMeta {
  title: string;
  icon_url?: string | null;
  slug?: string | null;
  project_id?: string | null;
}

export interface LogFile {
  name: string;
  size: number;
  modified: string;
}

export interface DeviceStart {
  device_code: string;
  user_code: string;
  verification_uri: string;
  interval: number;
  expires_in: number;
  message: string;
}

export interface ClientCheck {
  ok: boolean;
  code?: string | null;
  message: string;
}

export interface BootstrapPayload {
  config: AppConfig;
  installed: string[];
  manifest: VersionEntry[];
  java: string | null;
  java_runtimes: JavaRuntime[];
  data_dir: string;
  exports_dir: string;
  app_version: string;
}

export interface ProgressPayload {
  label: string;
  frac: number;
}

export type TabId = "mods" | "instances" | "accounts" | "settings" | "github";

export interface NavItem {
  id: TabId;
  glyph: string;
  label: string;
  hint: string;
}

export type ModSource = "modrinth" | "curseforge";
