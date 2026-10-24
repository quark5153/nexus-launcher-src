<template>
  <div class="grid h-full grid-cols-[minmax(0,1.15fr)_minmax(0,1fr)] gap-5 overflow-y-auto pr-1">
    <!-- ── about the project ───────────────────────────────────────────── -->
    <section class="panel panel-spine flex min-h-0 flex-col p-5">
      <div class="flex items-start gap-4">
        <span class="repo-mark">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M9.3 20.2c-3.6 1.1-3.6-1.6-4.8-2M14.6 21v-3.1c0-.9.1-1.3-.4-1.8 2.3-.3 4.6-1.1 4.6-5a3.9 3.9 0 0 0-1.1-2.7 3.6 3.6 0 0 0-.1-2.7s-.9-.3-2.9 1.1a10 10 0 0 0-5.2 0C7.5 5.4 6.6 5.7 6.6 5.7a3.6 3.6 0 0 0-.1 2.7A3.9 3.9 0 0 0 5.4 11c0 3.9 2.3 4.7 4.6 5-.4.4-.5.9-.5 1.4V21" />
          </svg>
        </span>
        <div class="min-w-0 flex-1">
          <div class="flex flex-wrap items-center gap-2">
            <h2 class="truncate text-[19px] font-semibold">{{ owner }}/{{ repoName }}</h2>
            <span class="pill !py-0.5 font-mono text-[9px] uppercase tracking-[0.14em]">{{ store.t("github.private") }}</span>
            <span class="pill !py-0.5 font-mono text-[9px]">{{ store.versionLabel }}</span>
          </div>
          <p class="mt-2 text-[12px] leading-relaxed text-[var(--muted)]">
            {{ store.t("github.aboutDesc") }}
          </p>
        </div>
      </div>

      <div class="mt-5 grid grid-cols-2 gap-2.5">
        <a
          v-for="link in links"
          :key="link.href"
          class="repo-link"
          :href="link.href"
          @click.prevent="store.openUrl(link.href)"
        >
          <span class="repo-link-icon" v-html="link.glyph" />
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[12px] text-[var(--text)]">{{ store.t(link.key) }}</span>
            <span class="block truncate font-mono text-[9px] text-[var(--dim)]">{{ link.hint }}</span>
          </span>
          <span class="repo-link-go">↗</span>
        </a>
      </div>

      <div class="mt-5 rounded-[12px] border border-[var(--line)] bg-black/25 p-4">
        <p class="hex-label"><i />{{ store.t("github.build") }}</p>
        <p class="mt-2 text-[11.5px] leading-relaxed text-[var(--muted)]">
          {{ store.t("github.buildDesc") }}
        </p>
        <pre class="build-box mt-2.5">./build-package.sh
./install-user.sh</pre>
      </div>

      <div class="mt-auto flex flex-wrap gap-2 pt-5">
        <button class="btn btn-solid" @click="store.openUrl(sourceUrl)">{{ store.t("github.open") }}</button>
        <button class="btn" @click="copyLink()">{{ store.t("github.copy") }}</button>
      </div>
    </section>

    <!-- ── repository facts ────────────────────────────────────────────── -->
    <section class="flex min-h-0 flex-col gap-4">
      <section class="panel panel-spine p-5">
        <p class="hex-label"><i />{{ store.t("github.about") }}</p>
        <dl class="mt-3 space-y-2.5">
          <div v-for="row in facts" :key="row.key" class="flex items-baseline gap-3">
            <dt class="w-[96px] shrink-0 font-mono text-[9.5px] uppercase tracking-[0.12em] text-[var(--dim)]">
              {{ store.t(row.key) }}
            </dt>
            <dd class="min-w-0 flex-1 truncate text-[12px] text-[var(--text)]">{{ row.value }}</dd>
          </div>
        </dl>
      </section>

      <section class="panel panel-spine flex min-h-0 flex-1 flex-col p-5">
        <p class="hex-label"><i />{{ store.t("github.stack") }}</p>
        <div class="mt-3 flex flex-wrap gap-1.5">
          <span v-for="tech in stack" :key="tech" class="pill !py-1 text-[10.5px]">{{ tech }}</span>
        </div>
        <p class="mt-4 font-mono text-[9.5px] leading-relaxed text-[var(--dim)]">
          apps/app-frontend · apps/app · packages/nexus-core
        </p>
      </section>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useAppStore } from "../stores/app";

const store = useAppStore();

const owner = "quark5153";
const repoName = "nexus-launcher-src";
const sourceUrl = `https://github.com/${owner}/${repoName}`;
const docsUrl = `https://github.com/${owner}/nexus-launcher`;

const links = [
  {
    key: "github.repo",
    hint: `${owner}/${repoName}`,
    href: sourceUrl,
    glyph: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M9 18l-6-6 6-6M15 6l6 6-6 6"/></svg>',
  },
  {
    key: "github.releases",
    hint: `${repoName}/releases`,
    href: `${sourceUrl}/releases`,
    glyph: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M12 3v12M7.5 10.5L12 15l4.5-4.5M4 20h16"/></svg>',
  },
  {
    key: "github.issues",
    hint: `${repoName}/issues`,
    href: `${sourceUrl}/issues`,
    glyph: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="8.5"/><path d="M12 7.5v5M12 16.2v.2"/></svg>',
  },
  {
    key: "github.docs",
    hint: `${owner}/nexus-launcher`,
    href: docsUrl,
    glyph: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M5 4.5h9l5 5V20H5z"/><path d="M13.5 4.7V10H19"/></svg>',
  },
];

const facts = computed(() => [
  { key: "github.version", value: store.versionLabel },
  { key: "github.license", value: "MIT" },
  { key: "github.visibility", value: store.t("github.private") },
  { key: "github.repo", value: `${owner}/${repoName}` },
]);

const stack = ["Vue 3", "Vite", "Tauri v2", "Rust", "nexus-core", "SQLite", "rusqlite"];

function copyLink() {
  void navigator.clipboard
    .writeText(sourceUrl)
    .then(() => store.say(store.t("github.copied")))
    .catch(() => store.say(sourceUrl));
}

</script>

<style scoped>
.repo-mark {
  display: grid;
  place-items: center;
  flex: 0 0 auto;
  width: 52px;
  height: 52px;
  border-radius: 14px;
  border: 1px solid var(--line-2);
  background: rgba(10, 6, 14, 0.7);
  color: var(--acc);
}
.repo-mark svg {
  width: 26px;
  height: 26px;
}

.repo-link {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 11px;
  border: 1px solid var(--line);
  background: rgba(255, 255, 255, 0.02);
  transition: background 130ms ease, border-color 130ms ease, transform 130ms ease;
}
.repo-link:hover {
  background: var(--acc-soft);
  border-color: var(--acc-line);
  transform: translateY(-1px);
}
.repo-link-icon {
  display: grid;
  place-items: center;
  width: 16px;
  height: 16px;
  flex: 0 0 auto;
  color: var(--acc-2);
}
.repo-link-icon :deep(svg) {
  width: 15px;
  height: 15px;
}
.repo-link-go {
  color: var(--dim);
  font-size: 11px;
}

.build-box {
  font-family: "IBM Plex Mono", ui-monospace, monospace;
  font-size: 10px;
  line-height: 1.7;
  color: var(--acc-2);
  background: rgba(0, 0, 0, 0.35);
  border-radius: 9px;
  padding: 8px 10px;
  white-space: pre-wrap;
}
</style>
