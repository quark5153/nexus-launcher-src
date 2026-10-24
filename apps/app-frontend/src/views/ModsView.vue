<template>
  <div class="grid h-full grid-cols-[232px_minmax(0,1fr)] gap-5 overflow-hidden">
    <!-- ── left: scope + import ─────────────────────────────────────────── -->
    <div class="flex min-h-0 flex-col gap-5">
      <section class="panel panel-spine p-4">
        <p class="hex-label"><i />{{ store.t("mods.scope") }}</p>
        <div class="mt-3 flex flex-col gap-1">
          <button
            v-for="t in types"
            :key="t.id"
            class="scope-btn"
            :class="{ on: store.modType === t.id }"
            @click="pickType(t.id)"
          >
            <span>{{ store.t(t.id === "modpack" ? "mods.modpack" : "mods.mod") }}</span>
            <i class="hex-tick" :class="{ hot: store.modType === t.id }" />
          </button>
        </div>

        <div class="mt-4 h-px w-full bg-[var(--line)]" />

        <p class="mt-3 hex-label"><i />{{ store.t("mods.source") }}</p>
        <div class="mt-2 flex gap-1.5">
          <button
            v-for="s in sources"
            :key="s.id"
            class="pill flex-1 !py-1.5 text-[11px]"
            :class="{ 'pill-on': store.modSource === s.id }"
            @click="pickSource(s.id)"
          >
            {{ s.label }}
          </button>
        </div>
        <p class="mt-2 text-[10px] leading-relaxed text-[var(--dim)]">
          {{ store.t(store.modSource === "curseforge" ? "mods.cfNote" : "mods.mrNote") }}
        </p>

        <div class="mt-4 h-px w-full bg-[var(--line)]" />

        <p class="mt-3 hex-label"><i />{{ store.t("mods.installTo") }}</p>
        <SelectField
          class="mt-2 text-[11px]"
          :model-value="store.config.selected_version"
          :options="profileOptions"
          @update:model-value="pickProfile"
        />
      </section>

      <section class="panel panel-spine mt-auto p-4">
        <label class="hex-label" for="pack-url"><i />{{ store.t("mods.packUrl") }}</label>
        <input
          id="pack-url"
          v-model="store.packUrl"
          class="field mt-3"
          type="text"
          placeholder="https://.../pack.mrpack"
          @keyup.enter="store.importPack()"
        />
        <button class="btn btn-solid mt-3 w-full" :disabled="store.busy" @click="store.importPack()">
          {{ store.t("mods.importPack") }}
        </button>
      </section>
    </div>

    <!-- ── right: results (layer 1) + detail (layer 2) ─────────────────── -->
    <section class="panel relative flex min-h-0 flex-col overflow-hidden">
      <div class="flex shrink-0 items-center gap-4 border-b border-[var(--line)] px-4 py-3">
        <input
          v-model="store.modQuery"
          class="field flex-1"
          type="text"
          :placeholder="store.t('mods.searchHint')"
          @keyup.enter="store.searchMods()"
        />
        <label class="flex shrink-0 items-center gap-2">
          <span class="font-mono text-[10px] uppercase tracking-[0.12em] text-[var(--dim)]">{{ store.t("mods.show") }}</span>
          <SelectField
            class="w-[74px] text-center font-mono text-[11px]"
            :model-value="String(store.modLimit)"
            :options="limitOptions"
            :disabled="store.busy"
            @update:model-value="setLimit"
          />
          <span class="font-mono text-[10.5px] text-[var(--dim)]">mod</span>
        </label>
        <span class="shrink-0 font-mono text-[11px] tabular-nums text-[var(--dim)]">
          <template v-if="store.modSource === 'curseforge'">{{ store.t("common.results", { n: store.cfHits.length }) }}</template>
          <template v-else-if="store.modTotal > 0">
            {{ store.modHits.length }} / {{ store.t("common.results", { n: store.modTotal.toLocaleString() }) }}
          </template>
          <template v-else>{{ store.t("common.results", { n: store.modHits.length }) }}</template>
        </span>
        <button class="btn btn-solid shrink-0" :disabled="store.busy" @click="store.searchMods()">
          {{ store.busy ? store.t("common.searching") : store.t("common.search") }}
        </button>
      </div>

      <div ref="scroller" class="min-h-0 flex-1 overflow-y-auto p-4">
        <template v-if="store.modSource === 'modrinth' && store.modHits.length > 0">
          <div class="grid auto-rows-min grid-cols-[repeat(auto-fill,minmax(192px,1fr))] gap-3.5">
            <article
              v-for="(hit, i) in store.modHits"
              :key="hit.project_id"
              v-spotlight
              class="game-card"
              :style="coverStyle(i)"
              @click="openModrinth(hit)"
            >
              <div class="cover h-[86px] shrink-0 !rounded-none">
                <i class="card-spine !opacity-100" />
                <div class="cover-mesh" />
                <div class="cover-shine" />
                <span class="mod-icon">
                  <img
                    v-if="hit.icon_url"
                    :src="hit.icon_url"
                    alt=""
                    loading="lazy"
                    decoding="async"
                    class="h-full w-full object-cover"
                  />
                  <template v-else>{{ hit.title.charAt(0) }}</template>
                </span>
                <span class="badge !top-2 !right-2 !left-auto">
                  {{ hit.project_type === "modpack" ? "pack" : "mod" }}
                </span>
              </div>

              <span class="flex min-h-0 flex-1 flex-col p-3">
                <span class="truncate text-[13px] font-medium">{{ hit.title }}</span>
                <span class="mt-0.5 line-clamp-2 text-[10.5px] leading-snug text-[var(--muted)]">
                  {{ hit.description }}
                </span>
                <span class="mt-1 flex items-center gap-1.5 text-[10px] text-[var(--dim)]">
                  <span class="font-mono text-[var(--acc)]">{{ formatCount(hit.downloads) }}</span>
                  <span>ดาวน์โหลด</span>
                  <span v-if="hit.categories[0]">· {{ hit.categories[0] }}</span>
                </span>
                <button
                  class="btn btn-solid mt-auto w-full !py-1.5 text-[11.5px]"
                  :disabled="store.busy"
                  @click.stop="store.installHit(hit)"
                >
                  {{ store.t("common.install") }}
                </button>
              </span>
            </article>
          </div>

          <!-- โหลดหน้าถัดไปจนครบทุกรายการ -->
          <div v-if="store.modHits.length < store.modTotal" class="mt-4 flex flex-col items-center gap-1.5">
            <button class="btn" :disabled="store.modLoadingMore" @click="store.loadMoreMods()">
              {{ store.modLoadingMore ? store.t("common.loading") : store.t("mods.loadMore", { n: store.modLimit }) }}
            </button>
            <span class="font-mono text-[10px] tabular-nums text-[var(--dim)]">
              {{ store.modHits.length }} / {{ store.modTotal.toLocaleString() }}
            </span>
          </div>
        </template>

        <div
          v-else-if="store.modSource === 'curseforge' && store.cfHits.length > 0"
          class="grid auto-rows-min grid-cols-[repeat(auto-fill,minmax(192px,1fr))] gap-3.5"
        >
          <article
            v-for="(hit, i) in store.cfHits"
            :key="hit.id"
            v-spotlight
            class="game-card"
            :style="coverStyle(i + 2)"
            @click="openCurseforge(hit)"
          >
            <div class="cover h-[86px] shrink-0 !rounded-none">
              <i class="card-spine !opacity-100" />
              <div class="cover-mesh" />
              <div class="cover-shine" />
              <span class="mod-icon">
                <img
                  v-if="hit.icon_url"
                  :src="hit.icon_url"
                  alt=""
                  loading="lazy"
                  decoding="async"
                  class="h-full w-full object-cover"
                />
                <template v-else>{{ hit.name.charAt(0) }}</template>
              </span>
              <span class="badge !top-2 !right-2 !left-auto">
                {{ store.modType === "modpack" ? "pack" : "mod" }}
              </span>
            </div>

            <span class="flex min-h-0 flex-1 flex-col p-3">
              <span class="truncate text-[13px] font-medium">{{ hit.name }}</span>
              <span class="mt-0.5 line-clamp-2 text-[10.5px] leading-snug text-[var(--muted)]">
                {{ hit.summary }}
              </span>
              <span class="mt-1 flex items-center gap-1.5 text-[10px] text-[var(--dim)]">
                <span class="font-mono text-[var(--acc)]">{{ formatCount(hit.download_count) }}</span>
                <span>ดาวน์โหลด</span>
              </span>
              <button
                class="btn btn-solid mt-auto w-full !py-1.5 text-[11.5px]"
                :disabled="store.busy"
                @click.stop="store.installCfHit(hit)"
              >                  {{ store.t("common.install") }}
                </button>
              </span>
            </article>
        </div>

        <div v-else class="grid h-full place-items-center px-6">
          <div class="max-w-[340px] text-center">
            <span class="hex-wrap mx-auto block h-[64px] w-[58px]">
              <span class="hex-core h-full w-full !bg-transparent">
                <svg class="h-6 w-6 text-[var(--acc)]" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                  <path d="M12 3.2l2 5.4 5.4 2-5.4 2-2 5.4-2-5.4-5.4-2 5.4-2z" />
                </svg>
              </span>
            </span>
            <p class="mt-5 text-[16px] font-semibold">{{ store.t("mods.emptyTitle") }}</p>
            <p class="mt-1.5 text-[12.5px] leading-relaxed text-[var(--dim)]">
              {{ store.t("mods.emptyDesc") }}
            </p>
          </div>
        </div>
      </div>

      <!-- ── layer 2: detail of the clicked mod / modpack, replaces the list ── -->
      <transition name="detail">
        <div v-if="detail" class="detail-layer">
          <div class="flex shrink-0 items-center gap-2 px-4 py-3">
            <button class="btn btn-quiet !px-2.5 !py-1.5 text-[11px]" @click="closeDetail()">
              {{ store.t("mods.back") }}
            </button>
            <span class="ml-auto font-mono text-[9.5px] uppercase tracking-[0.14em] text-[var(--dim)]">
              {{ detailType === "modpack" ? "modpack" : "mod" }} · {{ detailSource }}
            </span>
          </div>

          <div class="min-h-0 flex-1 overflow-y-auto px-5 pb-5">
            <div class="detail-hero">
              <span class="detail-icon">
                <img
                  v-if="detailIcon"
                  :src="detailIcon"
                  alt=""
                  decoding="async"
                  class="h-full w-full object-cover"
                />
                <template v-else>{{ detailTitle.charAt(0) }}</template>
              </span>
              <div class="min-w-0 flex-1">
                <h3 class="truncate text-[20px] font-semibold leading-tight">{{ detailTitle }}</h3>
                <p class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-[10px] text-[var(--dim)]">
                  <span class="text-[var(--acc-2)]">{{ formatCount(detailDownloads) }} {{ store.t("common.downloads") }}</span>
                  <span v-if="detailFollows">· {{ formatCount(detailFollows) }} {{ store.t("mods.follows") }}</span>
                  <span v-if="detailSlug">· {{ detailSlug }}</span>
                </p>
                <div class="mt-2 flex flex-wrap gap-1.5">
                  <span v-for="c in detailCategories" :key="c" class="pill !py-0.5 text-[9.5px]">{{ c }}</span>
                </div>
              </div>
            </div>

            <p class="mt-4 whitespace-pre-line text-[12.5px] leading-relaxed text-[var(--muted)]">
              {{ detailDescription }}
            </p>

            <div class="mt-5 rounded-[12px] border border-[var(--line)] bg-black/25 p-3.5">
              <p class="hex-label"><i />{{ store.t("mods.destination") }}</p>
              <p class="mt-2 text-[11.5px] text-[var(--muted)]">
                {{ store.t("mods.installingInto") }}
                <b class="font-medium text-[var(--text)]">{{ profileLabel }}</b>
              </p>
              <p class="mt-1 font-mono text-[9.5px] text-[var(--dim)]">
                {{ store.t(detailType === "modpack" ? "mods.packNewProfile" : "mods.modIntoMods") }}
              </p>
            </div>

            <div class="mt-4 flex flex-wrap gap-2">
              <button class="btn btn-solid flex-1" :disabled="store.busy" @click="installDetail()">
                {{ store.busy ? store.t("common.installing") : store.t(detailType === "modpack" ? "mods.installPack" : "mods.installMod") }}
              </button>
              <button class="btn" @click="store.openUrl(projectUrl)">{{ store.t("mods.openProject") }}</button>
            </div>
          </div>
        </div>
      </transition>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useAppStore } from "../stores/app";
import type { CfHit, ModrinthHit } from "../types";
import SelectField from "../components/SelectField.vue";

const store = useAppStore();
const scroller = ref<HTMLElement | null>(null);

type Detail =
  | { kind: "modrinth"; hit: ModrinthHit }
  | { kind: "curseforge"; hit: CfHit };

const detail = ref<Detail | null>(null);

const profileOptions = computed(() =>
  store.config.instances.length
    ? store.config.instances.map((i) => ({ value: i.id, label: `${i.name} · MC ${i.mc_version}` }))
    : [{ value: store.config.selected_version, label: store.t("mods.noProfile") }],
);

const limitOptions = [20, 40, 60].map((n) => ({ value: String(n), label: String(n) }));

function pickProfile(id: string) {
  store.config.selected_version = id;
  void store.persist();
}

function setLimit(value: string) {
  store.modLimit = Number(value);
  void store.searchMods();
}

/* ผลลัพธ์โหลดทันทีที่เข้าแท็บนี้ ไม่ต้องกดปุ่มค้นหาก่อน */
onMounted(() => {
  if (store.modHits.length === 0 && store.cfHits.length === 0) void store.searchMods();
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
});

/* เปลี่ยนประเภท = เริ่มค้นใหม่ เพื่อให้เห็นเฉพาะมอดแพ็ก หรือเฉพาะมอดเดี่ยว */
function pickType(id: "modpack" | "mod") {
  if (store.modType === id) return;
  store.modType = id;
  detail.value = null;
  resetScroll();
  void store.searchMods();
}

function pickSource(id: "modrinth" | "curseforge") {
  if (store.modSource === id) return;
  store.modSource = id;
  detail.value = null;
  resetScroll();
  void store.searchMods();
}

function resetScroll() {
  if (scroller.value) scroller.value.scrollTop = 0;
}

function openModrinth(hit: ModrinthHit) {
  detail.value = { kind: "modrinth", hit };
}

function openCurseforge(hit: CfHit) {
  detail.value = { kind: "curseforge", hit };
}

function closeDetail() {
  detail.value = null;
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && detail.value) {
    e.preventDefault();
    closeDetail();
  }
}

const detailHit = computed(() => detail.value?.hit);
const detailSource = computed(() => (detail.value?.kind === "curseforge" ? "CurseForge" : "Modrinth"));

const detailTitle = computed(() => {
  const d = detail.value;
  if (!d) return "";
  return d.kind === "modrinth" ? d.hit.title : d.hit.name;
});

const detailDescription = computed(() => {
  const d = detail.value;
  if (!d) return "";
  return d.kind === "modrinth" ? d.hit.description : d.hit.summary;
});

const detailIcon = computed(() => (detailHit.value?.icon_url ? detailHit.value.icon_url : ""));

const detailDownloads = computed(() => {
  const d = detail.value;
  if (!d) return 0;
  return d.kind === "modrinth" ? d.hit.downloads : d.hit.download_count;
});

const detailFollows = computed(() => {
  const d = detail.value;
  return d && d.kind === "modrinth" ? d.hit.follows : 0;
});

const detailSlug = computed(() => (detailHit.value?.slug ? detailHit.value.slug : ""));

const detailCategories = computed(() => {
  const d = detail.value;
  if (!d) return [] as string[];
  if (d.kind === "modrinth") return d.hit.categories.slice(0, 6);
  return [
    store.modType === "modpack" ? "modpack" : "mod",
    d.hit.latest_file_name ? d.hit.latest_file_name : "",
  ].filter(Boolean);
});

const detailType = computed(() => {
  const d = detail.value;
  if (!d) return "mod" as const;
  if (d.kind === "modrinth") return d.hit.project_type === "modpack" ? "modpack" : "mod";
  return store.modType;
});

const projectUrl = computed(() => {
  const d = detail.value;
  if (!d) return "";
  if (d.kind === "modrinth") {
    return `https://modrinth.com/${detailType.value === "modpack" ? "modpack" : "mod"}/${d.hit.slug || d.hit.project_id}`;
  }
  const seg = detailType.value === "modpack" ? "modpacks" : "mc-mods";
  return `https://www.curseforge.com/minecraft/${seg}/${d.hit.slug || d.hit.id}`;
});

const profileLabel = computed(() => {
  const inst = store.selectedInstance;
  return inst ? `${inst.name} · MC ${inst.mc_version}` : `MC ${store.config.selected_version}`;
});

function installDetail() {
  const d = detail.value;
  if (!d) return;
  if (d.kind === "modrinth") void store.installHit(d.hit);
  else void store.installCfHit(d.hit);
  closeDetail();
}

/* labels come from i18n: mods.modpack / mods.mod */
const types = [{ id: "modpack" as const }, { id: "mod" as const }];

const sources = [
  { id: "modrinth" as const, label: "Modrinth" },
  { id: "curseforge" as const, label: "CurseForge" },
];

const tints = [
  "linear-gradient(150deg, #3a2140 0%, #201029 60%, #100818 100%)",
  "linear-gradient(150deg, #402a4e 0%, #231229 60%, #130a1a 100%)",
  "linear-gradient(150deg, #2e1d43 0%, #180d24 60%, #0b0611 100%)",
  "linear-gradient(150deg, #332038 0%, #1b0f20 60%, #0d0712 100%)",
  "linear-gradient(150deg, #43254d 0%, #24122d 60%, #12091b 100%)",
  "linear-gradient(150deg, #341c44 0%, #1c0e26 60%, #0d0714 100%)",
];

function coverStyle(i: number): Record<string, string> {
  return { "--cover-tint": tints[i % tints.length] };
}

function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}K`;
  return String(n);
}
</script>

<style scoped>
.scope-btn {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 10px;
  text-align: left;
  font-size: 12px;
  color: var(--muted);
  background: transparent;
  transition: background 120ms ease, color 120ms ease;
}
.scope-btn:hover {
  background: rgba(255, 255, 255, 0.05);
  color: var(--text);
}
.scope-btn.on {
  background: var(--acc-soft);
  color: var(--text);
  box-shadow: inset 0 0 0 1px rgba(200, 159, 255, 0.3);
}
.hex-tick {
  clip-path: var(--hex);
  width: 13px;
  height: 15px;
  background: rgba(255, 255, 255, 0.14);
  flex: 0 0 auto;
}
.hex-tick.hot {
  background: var(--grad);
  box-shadow: 0 0 10px rgba(255, 157, 189, 0.6);
}

/* Solid fill instead of backdrop-filter: 60 blurred layers on one screen was
   the heaviest paint in the grid. */
.mod-icon {
  position: absolute;
  bottom: 8px;
  left: 12px;
  z-index: 10;
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  overflow: hidden;
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.2);
  background: rgba(10, 6, 14, 0.72);
  color: rgba(255, 255, 255, 0.9);
  font-size: 14px;
  font-weight: 600;
}

/* layer 2 — stacks over the result grid, the grid itself is never unmounted */
.detail-layer {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  background:
    linear-gradient(180deg, rgba(22, 13, 28, 0.96), rgba(12, 7, 17, 0.98));
  backdrop-filter: blur(2px);
  box-shadow: 0 -18px 60px rgba(0, 0, 0, 0.55);
}

.detail-hero {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  padding: 4px 0 0;
}

.detail-icon {
  display: grid;
  place-items: center;
  width: 76px;
  height: 76px;
  flex: 0 0 auto;
  overflow: hidden;
  border-radius: 16px;
  border: 1px solid var(--line-2);
  background: rgba(10, 6, 14, 0.7);
  color: rgba(255, 255, 255, 0.9);
  font-size: 26px;
  font-weight: 600;
  box-shadow: 0 12px 30px rgba(0, 0, 0, 0.45);
}

.detail-enter-active,
.detail-leave-active {
  transition: opacity 180ms cubic-bezier(0.22, 1, 0.36, 1), transform 180ms cubic-bezier(0.22, 1, 0.36, 1);
}
.detail-enter-from,
.detail-leave-to {
  opacity: 0;
  transform: translateY(10px) scale(0.995);
}
</style>
