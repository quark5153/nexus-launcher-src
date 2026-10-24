<template>
  <div class="grid h-full grid-cols-[276px_minmax(0,1fr)] gap-5 overflow-hidden">
    <!-- ── left: new profile, profile library, import/export ───────────── -->
    <div class="flex min-h-0 flex-col gap-4 pr-1">
      <section class="panel panel-spine shrink-0 p-4">
        <p class="hex-label"><i />{{ store.t("instances.new") }}</p>

        <label class="mt-3 block text-[10px] uppercase tracking-[0.14em] text-[var(--dim)]">{{ store.t("instances.name") }}</label>
        <input v-model="store.newName" class="field mt-1.5" type="text" :placeholder="store.t('instances.nameHint')" maxlength="40" />

        <div class="mt-3 grid grid-cols-2 gap-2">
          <div>
            <label class="block text-[10px] uppercase tracking-[0.14em] text-[var(--dim)]">{{ store.t("instances.gameVersion") }}</label>
            <SelectField
              class="mt-1.5 font-mono text-[11px]"
              :model-value="store.newMc"
              :options="mcOptions"
              @update:model-value="setMc"
            />
          </div>
          <div>
            <label class="block text-[10px] uppercase tracking-[0.14em] text-[var(--dim)]">{{ store.t("instances.loader") }}</label>
            <SelectField
              class="mt-1.5 text-[11px]"
              :model-value="store.newLoader"
              :options="loaderOptions"
              @update:model-value="setLoader"
            />
          </div>
        </div>

        <div v-if="store.newLoader !== 'vanilla'" class="mt-3">
          <div class="flex items-baseline justify-between gap-2">
            <label class="block text-[10px] uppercase tracking-[0.14em] text-[var(--dim)]">
              {{ store.t("instances.loaderVersion", { loader: store.newLoader }) }}
            </label>
            <span class="font-mono text-[9px] tabular-nums text-[var(--dim)]">
              {{ store.loaderVersionsLoading ? store.t("common.loading") : store.t("instances.builds", { n: store.loaderVersions.length }) }}
            </span>
          </div>
          <SelectField
            class="mt-1.5 font-mono text-[11px]"
            :model-value="store.newLoaderVersion"
            :options="loaderVersionOptions"
            @update:model-value="(value: string) => (store.newLoaderVersion = value)"
          />
          <p v-if="!store.loaderVersionsLoading && store.loaderVersions.length === 0" class="mt-1 text-[9.5px] leading-relaxed text-[var(--dim)]">
            {{ store.t("instances.noBuilds", { loader: store.newLoader, mc: store.newMc }) }}
          </p>
        </div>

        <button class="btn btn-solid mt-3 w-full" :disabled="store.busy" @click="store.createInstance()">
          {{ store.t("instances.new") }}
        </button>
      </section>

      <!-- switching profiles (including every installed modpack) -->
      <section class="panel panel-spine flex min-h-0 flex-1 flex-col p-4">
        <div class="flex items-center justify-between">
          <p class="hex-label"><i />{{ store.t("instances.list") }}</p>
          <span class="font-mono text-[9px] tabular-nums text-[var(--dim)]">
            {{ store.config.instances.length }}
          </span>
        </div>

        <div class="mt-2.5 min-h-0 flex-1 overflow-y-auto pr-1">
          <p v-if="store.config.instances.length === 0" class="py-4 text-center text-[11px] text-[var(--dim)]">
            {{ store.t("instances.empty") }}
          </p>
          <button
            v-for="item in store.config.instances"
            :key="item.id"
            class="lib-row"
            :class="{ on: item.id === inst?.id }"
            @click="store.selectInstance(item.id)"
          >
            <span class="lib-mark" :class="{ on: item.id === inst?.id }" />
            <span class="min-w-0 flex-1 text-left">
              <span class="block truncate text-[12.5px]" :class="{ 'text-[var(--text)]': item.id === inst?.id }">
                {{ item.name }}
              </span>
              <span class="block truncate font-mono text-[9px] text-[var(--dim)]">
                MC {{ item.mc_version }} ·
                {{ item.loader ? `${item.loader}${item.loader_version ? " " + item.loader_version : ""}` : store.t("common.vanilla") }}
              </span>
            </span>
            <span v-if="item.provider === 'modpack'" class="lib-tag">pack</span>
          </button>
        </div>
      </section>

      <section class="panel panel-spine shrink-0 p-4">
        <p class="hex-label"><i />{{ store.t("instances.importExport") }}</p>
        <p class="mt-2 text-[10.5px] leading-relaxed text-[var(--dim)]">
          {{ store.t("instances.importHint") }}
        </p>
        <input v-model="importPath" class="field mt-2.5 font-mono text-[10px]" type="text" placeholder="/path/to/pack.zip" />
        <button class="btn mt-2 w-full" :disabled="store.busy || !importPath.trim()" @click="doImport()">
          {{ store.t("instances.importFile") }}
        </button>
        <div class="mt-3 h-px w-full bg-[var(--line)]" />
        <button class="btn mt-3 w-full" :disabled="!inst" @click="store.exportInstance()">
          {{ store.t("instances.backup") }}
        </button>
        <p class="mt-2 truncate font-mono text-[9px] text-[var(--dim)]" :title="store.exportsDir">
          {{ store.exportsDir }}
        </p>
      </section>
    </div>

    <!-- ── right: selected profile ─────────────────────────────────────── -->
    <section v-if="inst" class="flex min-h-0 flex-col gap-4 overflow-hidden">
      <header class="panel panel-spine flex shrink-0 items-center gap-4 px-5 py-3.5">
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <h2 class="truncate text-[18px] font-semibold">{{ inst.name }}</h2>
            <span class="pill !py-0.5 font-mono text-[9px] uppercase tracking-[0.14em]">
              MC {{ inst.mc_version }}
            </span>
            <span class="pill !py-0.5 font-mono text-[9px] uppercase tracking-[0.14em]">
              {{ inst.loader ? inst.loader + " " + (inst.loader_version || "") : store.t("common.vanilla") }}
            </span>
          </div>
          <p class="mt-1 truncate font-mono text-[9.5px] text-[var(--dim)]">
            {{ inst.id }} · {{ store.t("instances.lastPlayed") }} {{ inst.last_played || store.t("instances.never") }}
          </p>
        </div>
        <div class="flex shrink-0 gap-2">
          <button
            v-if="inst.provider === 'modpack'"
            class="btn !py-1.5 text-[11px]"
            :disabled="store.busy"
            @click="store.updatePack(inst.id)"
          >
            {{ store.t("instances.updatePack") }}
          </button>
          <button class="btn !py-1.5 text-[11px]" @click="store.openInstanceFolder()">{{ store.t("instances.openFolder") }}</button>
          <button class="btn btn-solid !py-1.5 text-[11px]" :disabled="store.busy" @click="store.launch(true)">
            {{ store.t("instances.play") }}
          </button>
        </div>
      </header>

      <div class="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(0,1.05fr)] gap-4 overflow-hidden">
        <!-- left: settings + content -->
        <div class="flex min-h-0 flex-col gap-4 overflow-y-auto pr-1">
          <section class="panel panel-spine shrink-0 p-4">
            <div class="flex items-center justify-between">
              <p class="hex-label"><i />{{ store.t("instances.settings") }}</p>
              <span class="font-mono text-[9px] uppercase tracking-[0.14em] text-[var(--dim)]">
                {{ draft.ramMb ? `${(draft.ramMb / 1024).toFixed(1)} GB` : store.t("common.auto") }}
              </span>
            </div>

            <div class="mt-3 flex items-center gap-3">
              <label class="flex items-center gap-2 text-[11px] text-[var(--muted)]">
                <input v-model="draft.useGlobalRam" type="checkbox" class="accent-[var(--acc)]" />
                {{ store.t("instances.useGlobalRam") }}
              </label>
              <input
                v-model.number="draft.ramMb"
                :disabled="draft.useGlobalRam"
                class="field flex-1 !py-1 font-mono text-[11px]"
                type="number"
                min="1024"
                max="32768"
                step="512"
              />
            </div>

            <label class="mt-3 block text-[10px] uppercase tracking-[0.14em] text-[var(--dim)]">
              {{ store.t("instances.javaPath") }}
            </label>
            <input v-model="draft.javaPath" class="field mt-1.5 font-mono text-[10px]" type="text" placeholder="auto" />

            <label class="mt-3 block text-[10px] uppercase tracking-[0.14em] text-[var(--dim)]">
              {{ store.t("instances.extraJvm") }}
            </label>
            <textarea v-model="draft.extraJvm" class="field field-area mt-1.5 w-full" rows="2" placeholder="-XX:+UseZGC" />

            <div class="mt-3 flex gap-2">
              <button class="btn btn-solid flex-1 !py-1.5 text-[11.5px]" @click="saveDraft()">{{ store.t("common.save") }}</button>
              <button class="btn !py-1.5 text-[11.5px]" @click="removeProfile(false)">{{ store.t("instances.removeFromLibrary") }}</button>
            </div>

            <!-- removing the profile record is not the same as freeing the disk -->
            <div class="mt-2">
              <button
                v-if="!confirmPurge"
                class="btn danger-btn w-full !py-1.5 text-[11.5px]"
                :disabled="store.busy"
                @click="confirmPurge = true"
              >
                {{ store.t(inst.provider === "modpack" ? "instances.deletePack" : "instances.deleteProfile") }}
              </button>
              <div v-else class="flex gap-2">
                <button
                  class="btn danger-btn flex-1 !py-1.5 text-[11.5px]"
                  :disabled="store.busy"
                  @click="removeProfile(true)"
                >
                  {{ store.t("instances.confirmDelete") }}
                </button>
                <button class="btn !py-1.5 text-[11.5px]" @click="confirmPurge = false">{{ store.t("common.cancel") }}</button>
              </div>
              <p class="mt-1.5 text-[9.5px] leading-relaxed text-[var(--dim)]">
                {{ store.t("instances.deleteHint") }}
              </p>
            </div>
          </section>

          <section class="panel panel-spine flex min-h-[220px] flex-col p-4">
            <div class="flex flex-wrap items-center gap-1.5">
              <button
                v-for="k in kinds"
                :key="k.id"
                class="pill !py-1 text-[10.5px]"
                :class="{ 'pill-on': store.contentKind === k.id }"
                @click="store.loadContent(k.id)"
              >
                {{ store.t(`instances.kind.${k.id}`) }}
              </button>
              <span class="ml-auto font-mono text-[10px] text-[var(--dim)]">{{ store.contentEntries.length }}</span>
            </div>

            <div class="mt-3 min-h-0 flex-1 overflow-y-auto">
              <div v-if="store.contentEntries.length === 0" class="py-6 text-center text-[11px] text-[var(--dim)]">
                {{ store.t("instances.kindEmpty") }}
              </div>
              <div
                v-for="entry in store.contentEntries"
                :key="entry.name"
                class="content-row"
                :class="{ off: !entry.enabled }"
              >
                <span class="content-icon">
                  <img
                    v-if="iconFor(entry)"
                    :src="iconFor(entry)"
                    alt=""
                    loading="lazy"
                    decoding="async"
                    class="h-full w-full object-cover"
                  />
                  <!-- shader packs carry no preview image of their own -->
                  <svg
                    v-else-if="entry.kind === 'shaderpacks'"
                    class="h-3.5 w-3.5 opacity-80"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                    aria-hidden="true"
                  >
                    <path d="M11 3.2l1.7 4.9 4.9 1.7-4.9 1.7L11 16.4l-1.7-4.9-4.9-1.7 4.9-1.7z" />
                    <path d="M18.2 15.4l.8 2.3 2.3.8-2.3.8-.8 2.3-.8-2.3-2.3-.8 2.3-.8z" opacity=".55" />
                  </svg>
                  <template v-else>{{ initialFor(entry) }}</template>
                </span>
                <div class="min-w-0 flex-1">
                  <p class="truncate text-[12px]" :class="{ 'line-through opacity-60': !entry.enabled }">
                    {{ titleFor(entry) }}
                  </p>
                  <p class="truncate font-mono text-[9px] text-[var(--dim)]">
                    <template v-if="metaFor(entry)">{{ entry.name }} · </template>
                    {{ entry.detail || `${fmtSize(entry.size)} · ${entry.modified}` }}
                  </p>
                </div>
                <button
                  v-if="store.contentKind !== 'servers'"
                  class="btn btn-quiet !px-2 !py-1 text-[9.5px]"
                  @click="store.toggleContent(entry)"
                >
                  {{ store.t(entry.enabled ? "common.disable" : "common.enable") }}
                </button>
                <button
                  v-if="store.contentKind !== 'servers'"
                  class="btn btn-quiet !px-2 !py-1 text-[9.5px]"
                  @click="store.deleteContent(entry)"
                >
                  {{ store.t("common.delete") }}
                </button>
              </div>
            </div>

            <button class="btn mt-3 w-full !py-1.5 text-[11px]" @click="store.openInstanceFolder(folderFor(store.contentKind))">
              {{ store.t("instances.openKindFolder", { kind: store.contentKind }) }}
            </button>
          </section>
        </div>

        <!-- right: diagnostics -->
        <section class="panel flex min-h-0 flex-col overflow-hidden">
          <div class="flex shrink-0 items-center gap-2 border-b border-[var(--line)] px-4 py-2.5">
            <span class="hex-label"><i />{{ store.t("instances.latestLog") }}</span>
            <button class="btn btn-quiet ml-auto !px-2 !py-1 text-[10px]" @click="refreshLog()">{{ store.t("common.refresh") }}</button>
            <button class="btn btn-quiet !px-2 !py-1 text-[10px]" @click="store.openInstanceFolder('logs')">{{ store.t("instances.logFolder") }}</button>
          </div>

          <pre class="log-box min-h-0 flex-1 overflow-auto p-4">{{ store.currentLog || store.t("instances.noLog") }}</pre>

          <div class="shrink-0 border-t border-[var(--line)] px-4 py-2.5">
            <div class="flex items-center gap-2">
              <span class="hex-label"><i />{{ store.t("instances.crashes") }}</span>
              <span class="ml-auto font-mono text-[10px] text-[var(--dim)]">{{ store.crashes.length }}</span>
            </div>
            <div class="mt-2 flex max-h-[92px] flex-wrap gap-1.5 overflow-y-auto">
              <button
                v-for="crash in store.crashes"
                :key="crash.name"
                class="pill !py-1 font-mono text-[9.5px]"
                @click="store.openCrash(crash.name)"
              >
                {{ crash.name }}
              </button>
              <span v-if="store.crashes.length === 0" class="text-[10.5px] text-[var(--dim)]">{{ store.t("instances.noCrashes") }}</span>
            </div>
            <pre v-if="store.crashText" class="log-box mt-2 max-h-[160px] overflow-auto p-2.5">{{ store.crashText }}</pre>
          </div>
        </section>
      </div>
    </section>

    <section v-else class="panel grid place-items-center">
      <div class="max-w-[360px] text-center">
        <span class="hex-wrap mx-auto block h-[64px] w-[58px]">
          <span class="hex-core h-full w-full !bg-transparent">
            <svg class="h-6 w-6 text-[var(--acc)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
              <path d="M12 3.6l7.6 4.2v8.4L12 20.4l-7.6-4.2V7.8z" />
              <path d="M4.6 8l7.4 4.1L19.4 8M12 20V12" />
            </svg>
          </span>
        </span>
        <p class="mt-4 text-[16px] font-semibold">{{ store.t("instances.empty") }}</p>
        <p class="mt-1.5 text-[12px] leading-relaxed text-[var(--dim)]">
          {{ store.t("instances.emptyDesc") }}
        </p>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useAppStore } from "../stores/app";
import type { ContentEntry, InstanceRecord } from "../types";
import SelectField from "../components/SelectField.vue";

const store = useAppStore();
const importPath = ref("");
const confirmPurge = ref(false);

const loaders = [
  { id: "vanilla", label: "Vanilla" },
  { id: "fabric", label: "Fabric" },
  { id: "quilt", label: "Quilt" },
  { id: "forge", label: "Forge" },
  { id: "neoforge", label: "NeoForge" },
];

/* labels come from i18n: instances.kind.<id> */
const kinds = ["mods", "worlds", "resourcepacks", "shaderpacks", "servers"].map((id) => ({ id }));

const inst = computed(() => store.selectedInstance);

const releases = computed(() => {
  const list = store.manifest.filter((v) => v.type === "release");
  return list.length ? list.slice(0, 80) : store.manifest.slice(0, 80);
});

const mcOptions = computed(() => releases.value.map((v) => ({ value: v.id, label: v.id })));
const loaderOptions = loaders.map((l) => ({ value: l.id, label: l.label }));
const loaderVersionOptions = computed(() => [
  { value: "", label: store.t("instances.autoBuild") },
  ...store.loaderVersions.map((v) => ({ value: v, label: v })),
]);

/* ดึงบิลด์ของ loader ที่เลือกไว้ล่วงหน้า เพื่อให้เลือกเองได้ทันทีโดยไม่ต้องรอ */
onMounted(() => {
  if (store.newLoader !== "vanilla" && store.loaderVersions.length === 0) {
    void store.loadLoaderVersions();
  }
});

function setMc(value: string) {
  if (store.newMc === value) return;
  store.newMc = value;
  store.newLoaderVersion = "";
  void store.loadLoaderVersions();
}

function setLoader(value: string) {
  if (store.newLoader === value) return;
  store.newLoader = value;
  store.newLoaderVersion = "";
  void store.loadLoaderVersions();
}

const draft = reactive({
  useGlobalRam: true,
  ramMb: store.config.ram_mb,
  javaPath: "",
  extraJvm: "",
});

/* installed mod names/icons resolved from Modrinth by jar hash */
function metaFor(entry: { name: string }) {
  return store.modMeta[entry.name];
}

/* mods → Modrinth icon, worlds/packs/servers → local preview image */
function iconFor(entry: ContentEntry) {
  return metaFor(entry)?.icon_url || entry.icon || "";
}

function titleFor(entry: { name: string }) {
  return metaFor(entry)?.title || entry.name.replace(/\.jar(\.disabled)?$/i, "");
}

function initialFor(entry: { name: string }) {
  const text = titleFor(entry).trim();
  return text ? text.charAt(0).toUpperCase() : "?";
}

function resetDraft(record: InstanceRecord | undefined) {
  draft.useGlobalRam = !record?.ram_mb;
  draft.ramMb = record?.ram_mb ?? store.config.ram_mb;
  draft.javaPath = record?.java_path ?? "";
  draft.extraJvm = record?.extra_jvm ?? "";
}

watch(inst, (value) => {
  resetDraft(value);
  confirmPurge.value = false;
  if (value) {
    /* a pack is about its mods — open that tab first */
    void store.loadContent(value.provider === "modpack" ? "mods" : "worlds");
    void refreshLog();
  }
}, { immediate: true });

watch(
  () => store.selectedInstance?.id,
  () => {
    if (store.selectedInstance) {
      void store.loadCrashes();
    }
  },
);

function saveDraft() {
  const current = inst.value;
  if (!current) return;
  void store.saveInstance({
    ...current,
    ram_mb: draft.useGlobalRam ? null : Math.max(1024, Math.min(32768, Number(draft.ramMb) || 4096)),
    java_path: draft.javaPath.trim() ? draft.javaPath.trim() : null,
    extra_jvm: draft.extraJvm.trim() ? draft.extraJvm : null,
  });
}

function removeProfile(purge: boolean) {
  const current = inst.value;
  if (!current) return;
  void store.deleteInstance(current.id, purge);
}

function doImport() {
  void store.importInstance(importPath.value).then(() => {
    importPath.value = "";
  });
}

async function refreshLog() {
  await store.loadLog();
  await store.loadCrashes();
}

function folderFor(kind: string): string {
  return kind === "worlds" ? "saves" : kind;
}

function fmtSize(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${bytes} B`;
}
</script>

<style scoped>
.lib-row {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 7px 9px;
  border-radius: 10px;
  color: var(--muted);
  transition: background 120ms ease, color 120ms ease;
}
.lib-row:hover {
  background: rgba(255, 255, 255, 0.05);
  color: var(--text);
}
.lib-row.on {
  background: var(--acc-soft);
  box-shadow: inset 0 0 0 1px rgba(200, 159, 255, 0.28);
}
.lib-mark {
  flex: 0 0 auto;
  width: 6px;
  height: 6px;
  border-radius: 2px;
  transform: rotate(45deg);
  background: rgba(255, 255, 255, 0.16);
}
.lib-mark.on {
  background: var(--grad);
  box-shadow: 0 0 9px rgba(255, 157, 189, 0.7);
}
.lib-tag {
  flex: 0 0 auto;
  padding: 2px 6px;
  border-radius: 999px;
  border: 1px solid var(--line-2);
  font-family: "IBM Plex Mono", ui-monospace, monospace;
  font-size: 8.5px;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--acc-2);
}

.content-icon {
  display: grid;
  place-items: center;
  flex: 0 0 auto;
  width: 26px;
  height: 26px;
  overflow: hidden;
  border-radius: 7px;
  border: 1px solid var(--line-2);
  background: rgba(10, 6, 14, 0.72);
  font-size: 11px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.85);
}

.danger-btn {
  border-color: rgba(255, 120, 140, 0.42);
  color: #ffb4c0;
  background: rgba(255, 90, 120, 0.08);
}
.danger-btn:hover:not(:disabled) {
  background: rgba(255, 90, 120, 0.18);
  border-color: rgba(255, 120, 140, 0.7);
}

.content-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 8px;
  border-radius: 9px;
}
.content-row:hover {
  background: rgba(255, 255, 255, 0.04);
}
.content-row.off {
  opacity: 0.75;
}
.log-box {
  font-family: "IBM Plex Mono", ui-monospace, monospace;
  font-size: 10px;
  line-height: 1.5;
  color: var(--muted);
  background: rgba(0, 0, 0, 0.35);
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
