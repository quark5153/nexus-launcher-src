<template>
  <div class="sheet flex h-screen w-screen flex-col overflow-hidden">
    <!-- aurora backdrop -->
    <div class="aurora" />
    <div class="aurora-vignette" />

    <!-- ── header ──────────────────────────────────────────────────────── -->
    <header class="relative z-10 flex h-[64px] shrink-0 items-center justify-between gap-6 px-6">
      <div class="flex items-center gap-4">
        <span class="brandmark"><img :src="logoUrl" alt="Nexus" /></span>
        <div class="leading-none">
          <p class="text-[16px] font-semibold tracking-tight text-[var(--text)]">Nexus Launcher</p>
          <p class="mt-[6px] font-mono text-[9.5px] uppercase tracking-[0.18em] text-[var(--dim)]">
            {{ store.versionLabel }} · rust core
          </p>
        </div>
        <span class="chip ml-3 !py-2">{{ greeting }} <b class="font-medium text-[var(--text)]">{{ store.config.username }}</b></span>
      </div>

      <div class="flex items-center gap-2.5">
        <span class="java-chip" :class="{ on: Boolean(store.java || store.config.java_path) }" :title="javaLabel">
          <span class="jdot" />
          <span class="truncate">{{ javaLabel }}</span>
        </span>          <div class="lang-chip lang-switch" role="group" aria-label="ภาษา / Language">
            <button
              type="button"
              :class="{ on: store.lang === 'th' }"
              :aria-pressed="store.lang === 'th'"
              @click="store.setLang('th')"
            >TH</button>
            <button
              type="button"
              :class="{ on: store.lang === 'en' }"
              :aria-pressed="store.lang === 'en'"
              @click="store.setLang('en')"
            >EN</button>
          </div>
          <button class="masthead-action" @click="store.openFolder()">{{ store.t("header.dataFolder") }}</button>
        </div>
    </header>

    <!-- ── body ────────────────────────────────────────────────────────── -->
    <div class="relative z-10 flex min-h-0 flex-1 gap-5 px-5 pb-2">
      <!-- side dock: circular icons that magnify toward the pointer, label slides out on hover -->
      <aside class="side-nav shrink-0">
        <div class="dock-brand">
          <span class="dock-mark"><img :src="logoUrl" alt="Nexus" /></span>
        </div>

        <i class="dock-divider" />

        <nav ref="dockNav" class="dock-rail" @mousemove="onDockMove" @mouseleave="resetDock">
          <button
            v-for="(item, i) in nav"
            :key="item.id"
            class="dock-slot"
            :class="{ active: store.tab === item.id }"
            :style="{ transform: `translateY(${offsets[i] ?? 0}px)` }"
            :aria-label="store.t(item.labelKey)"
            :aria-current="store.tab === item.id ? 'page' : undefined"
            @click="store.tab = item.id"
          >
            <span class="dock-orb" :style="{ transform: `scale(${scales[i] ?? 1})` }">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <template v-if="item.id === 'mods'">
                  <path d="M12 3.2l2 5.4 5.4 2-5.4 2-2 5.4-2-5.4-5.4-2 5.4-2z" fill="currentColor" stroke="none" />
                </template>
                <template v-else-if="item.id === 'instances'">
                  <path d="M12 3.6l7.6 4.2v8.4L12 20.4l-7.6-4.2V7.8z" />
                  <path d="M4.6 8l7.4 4.1L19.4 8M12 20V12" />
                </template>
                <template v-else-if="item.id === 'accounts'">
                  <circle cx="12" cy="8.2" r="3.4" />
                  <path d="M5.5 19.5c.7-3 3.4-4.8 6.5-4.8s5.8 1.8 6.5 4.8" />
                </template>
                <template v-else>
                  <path d="M4 8h7M14.5 8H20M4 16h11M18.5 16H20" />
                  <circle cx="12.8" cy="8" r="1.9" fill="currentColor" stroke="none" />
                  <circle cx="16.7" cy="16" r="1.9" fill="currentColor" stroke="none" />
                </template>
              </svg>
            </span>
            <span class="dock-tip" aria-hidden="true">{{ store.t(item.labelKey) }}</span>
          </button>
        </nav>

        <div class="dock-status" :class="{ on: javaReady }"            :title="javaReady ? store.t('common.javaReady') : store.t('common.noJava')">
          <span class="status-led" :class="{ on: javaReady }" />
        </div>

        <div class="side-foot">
          <button
            class="dock-avatar"
            :title="`${store.config.username} · ${store.activeAccount?.kind === 'Microsoft' ? 'Microsoft' : store.t('common.offline')}`"
            @click="store.tab = 'accounts'"
          >
            <span class="monogram">{{ initials }}</span>
          </button>
        </div>
      </aside>

      <!-- view -->
      <main class="min-h-0 flex-1 overflow-hidden pt-0.5">
        <component :is="activeView" class="fade-in" :key="store.tab" />
      </main>
    </div>

    <!-- ── toast ───────────────────────────────────────────────────────── -->
    <transition name="toast">
      <div
        v-if="store.toast"
        class="pointer-events-none absolute bottom-12 left-1/2 z-50 -translate-x-1/2 rounded-full border border-[var(--line-2)] bg-[#171024]/95 px-5 py-2 text-[12px] text-[var(--text)] shadow-xl backdrop-blur"
      >
        {{ store.toast }}
      </div>
    </transition>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useAppStore } from "./stores/app";
import type { TabId } from "./types";
import ModsView from "./views/ModsView.vue";
import InstancesView from "./views/InstancesView.vue";
import AccountsView from "./views/AccountsView.vue";
import SettingsView from "./views/SettingsView.vue";
import logoUrl from "./assets/logo.png";

const store = useAppStore();

const nav: { id: TabId; labelKey: string }[] = [
  { id: "instances", labelKey: "nav.instances" },
  { id: "mods", labelKey: "nav.mods" },
  { id: "accounts", labelKey: "nav.accounts" },
  { id: "settings", labelKey: "nav.settings" },
];

const views = {
  mods: ModsView,
  instances: InstancesView,
  accounts: AccountsView,
  settings: SettingsView,
} as const;

const hour = new Date().getHours();
const greeting = computed(() =>
  store.t(
    hour < 12 ? "greeting.morning" : hour < 17 ? "greeting.afternoon" : hour < 21 ? "greeting.evening" : "greeting.night",
  ),
);

const activeView = computed(() => views[store.tab]);
const initials = computed(() => (store.config.username || "?").charAt(0).toUpperCase());
const javaLabel = computed(() => store.java || store.config.java_path || store.t("common.noJava"));
const javaReady = computed(() => Boolean(store.java || store.config.java_path));

/* ── magnifying dock ──────────────────────────────────────────────────── */
const BASE = 44; // resting orb diameter — must match .dock-orb
const REACH = 130; // px of pointer influence around each orb
const PEAK = 1.5; // scale of the orb directly under the pointer

const dockNav = ref<HTMLElement | null>(null);
const scales = ref<number[]>(nav.map(() => 1));

/* Slot centres only move when the window does, so measure them once and
   coalesce pointer events into a single rAF pass instead of re-rendering
   the dock on every mousemove. */
let dockFrame = 0;
let dockY = 0;
let centres: number[] = [];

function measureDock() {
  const rail = dockNav.value;
  if (!rail) return;
  centres = Array.from(rail.children).map((el) => {
    const slot = el as HTMLElement;
    return slot.offsetTop + slot.offsetHeight / 2;
  });
}

/** Scale every orb by how close the pointer is to its centre (dock magnify). */
function applyDock() {
  dockFrame = 0;
  const rail = dockNav.value;
  if (!rail) return;
  if (centres.length !== rail.children.length) measureDock();
  const y = dockY - rail.getBoundingClientRect().top;
  const next = centres.map((centre) => {
    const t = Math.max(0, 1 - Math.abs(y - centre) / REACH);
    return 1 + (PEAK - 1) * t * t;
  });
  if (next.every((s, i) => Math.abs(s - (scales.value[i] ?? 1)) < 0.004)) return;
  scales.value = next;
}

function onDockMove(e: MouseEvent) {
  dockY = e.clientY;
  if (!dockFrame) dockFrame = requestAnimationFrame(applyDock);
}

function resetDock() {
  if (dockFrame) cancelAnimationFrame(dockFrame);
  dockFrame = 0;
  centres = [];
  scales.value = nav.map(() => 1);
}

/** Neighbours slide apart so a swollen orb never sits on top of them. */
const offsets = computed(() =>
  scales.value.map((_, i) => {
    let shift = 0;
    scales.value.forEach((s, j) => {
      const half = ((s - 1) * BASE) / 2;
      if (j < i) shift += half;
      else if (j > i) shift -= half;
    });
    return shift;
  }),
);

/* 1–6 switch tabs, unless the user is typing in a field */
function onKeydown(e: KeyboardEvent) {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  const el = e.target as HTMLElement | null;
  const tag = el?.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el?.isContentEditable) return;
  const idx = Number(e.key) - 1;
  if (Number.isInteger(idx) && idx >= 0 && idx < nav.length) {
    e.preventDefault();
    store.tab = nav[idx].id;
  }
}

function onResize() {
  resetDock();
}

onMounted(() => {
  void store.boot();
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("resize", onResize);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("resize", onResize);
});
</script>
