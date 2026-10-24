<template>
  <div class="h-full overflow-y-auto pr-1">
    <div class="grid grid-cols-12 gap-5">
      <!-- ── memory ─────────────────────────────────────────────────────── -->
      <section class="panel panel-spine col-span-7 p-5">
        <div class="flex items-start justify-between gap-4">
          <p class="hex-label"><i />{{ store.t("settings.memory") }}</p>
          <div class="flex items-end gap-2">
            <p class="text-[30px] font-semibold leading-none text-transparent bg-clip-text bg-gradient-to-br from-[#ffd9e8] to-[#ff86b6]">
              {{ (store.config.ram_mb / 1024).toFixed(1) }}
            </p>
            <span class="pb-0.5 text-[13px] text-[var(--muted)]">GB</span>
          </div>
        </div>

        <input
          :value="store.config.ram_mb"
          class="mt-7"
          :style="{ '--ram-pct': (store.config.ram_mb / 16384) * 100 + '%' }"
          type="range"
          min="1024"
          max="16384"
          step="512"
          @input="onSlide"
        />

        <div class="mt-5 flex flex-wrap gap-1.5">
          <button
            v-for="preset in presets"
            :key="preset"
            class="pill"
            :class="{ 'pill-on': store.config.ram_mb === preset }"
            @click="setRam(preset)"
          >
            {{ preset / 1024 }} GB
          </button>
        </div>
        <p class="mt-3 text-[10.5px] leading-relaxed text-[var(--dim)]">
          {{ store.t("settings.ramNote") }}
        </p>
      </section>

      <!-- ── java ───────────────────────────────────────────────────────── -->
      <section class="panel panel-spine col-span-5 p-5">
        <p class="hex-label"><i />Java runtime</p>
        <label class="mt-3 flex items-center gap-2 text-[11px] text-[var(--muted)]">
          <input v-model="store.config.auto_java" type="checkbox" class="accent-[var(--acc)]" @change="store.persist()" />
          {{ store.t("settings.autoJre") }}
        </label>

        <label class="mt-4 block text-[10px] uppercase tracking-[0.14em] text-[var(--dim)]">{{ store.t("settings.javaPath") }}</label>
        <input
          v-model="store.config.java_path"
          class="field mt-1.5 font-mono text-[10px]"
          type="text"
          placeholder="/usr/lib/jvm/java-21-openjdk/bin/java"
          @change="store.persist()"
        />
        <button class="btn mt-2 w-full !py-1.5 text-[11px]" @click="store.refreshJava()">{{ store.t("settings.detectJava") }}</button>

        <div class="mt-3 flex flex-wrap gap-1.5">
          <button
            v-for="major in [8, 17, 21]"
            :key="major"
            class="pill !py-1 text-[10.5px]"
            :class="{ 'pill-on': store.javaRuntimes.some((r) => r.major === major) }"
            :disabled="store.busy"
            @click="store.installJava(major)"
          >
            Java {{ major }}
          </button>
        </div>

        <div class="mt-3 max-h-[92px] overflow-y-auto border-t border-[var(--line)] pt-2">
          <p
            v-for="rt in store.javaRuntimes"
            :key="rt.path"
            class="flex items-center gap-2 py-0.5 font-mono text-[9.5px] text-[var(--muted)]"
          >
            <span class="text-[var(--acc)]">{{ rt.major }}</span>
            <span class="truncate" :title="rt.path">{{ rt.path }}</span>
          </p>
          <p v-if="store.javaRuntimes.length === 0" class="text-[10px] text-[var(--dim)]">{{ store.t("settings.noJavaFound") }}</p>
        </div>
      </section>

      <!-- ── jvm args + data folder ─────────────────────────────────────── -->
      <section class="panel panel-spine col-span-12 p-5">
        <p class="hex-label"><i />{{ store.t("settings.extraJvm") }}</p>
        <textarea
          v-model="store.config.extra_jvm"
          class="field field-area mt-3 w-full"
          rows="3"
          placeholder="-XX:+UseZGC -Dfile.encoding=UTF-8"
          @change="store.persist()"
        />

        <div class="mt-4 flex items-center justify-between gap-4 border-t border-[var(--line)] pt-3">
          <div class="min-w-0">
            <p class="hex-label"><i />{{ store.t("settings.dataFolder") }}</p>
            <p class="mt-2 truncate font-mono text-[10px] text-[var(--muted)]">
              {{ store.dataDir || "~/.local/share/nexus" }}
            </p>
          </div>
          <div class="flex shrink-0 items-center gap-2">
            <button class="btn" @click="store.openFolder()">{{ store.t("settings.openFolder") }}</button>
            <button class="btn !px-2.5 !py-1 text-[9.5px]" @click="store.checkUpdates()">{{ store.t("settings.checkUpdates") }}</button>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useAppStore } from "../stores/app";

const store = useAppStore();
const presets = [2048, 4096, 8192, 12288];

function onSlide(event: Event) {
  store.setRam(Number((event.target as HTMLInputElement).value));
}

function setRam(mb: number) {
  store.setRam(mb);
  void store.persist();
}
</script>
