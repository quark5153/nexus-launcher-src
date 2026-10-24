<template>
  <div class="grid h-full grid-cols-[336px_minmax(0,1fr)] gap-5 overflow-hidden">
    <!-- ── left: sign-in + offline profile ─────────────────────────────── -->
    <section class="panel panel-spine flex min-h-0 flex-col overflow-y-auto p-5">
      <p class="hex-label"><i />บัญชี Microsoft</p>

      <template v-if="store.device">
        <p class="mt-3 text-[12px] leading-relaxed text-[var(--muted)]">
          เปิดลิงก์ด้านล่างแล้วกรอกรหัสนี้เพื่อยืนยันตัวตน
        </p>
        <p class="mt-3 select-all text-center font-mono text-[30px] font-semibold tracking-[0.18em] text-[var(--acc)]">
          {{ store.device.user_code }}
        </p>
        <button
          class="link mt-2 block w-full truncate text-center font-mono text-[10.5px]"
          @click="openBrowser()"
        >
          {{ store.device.verification_uri }} ↗ เปิดในเบราว์เซอร์
        </button>
        <div class="mt-4 flex items-center gap-2">
          <span class="status-led !static on animate-pulse" />
          <span class="text-[11px] text-[var(--dim)]">กำลังรอการยืนยันจาก Microsoft…</span>
        </div>
        <button class="btn mt-3 w-full" @click="store.cancelMsLogin()">ยกเลิก</button>
      </template>

      <template v-else>
        <p class="mt-2 text-[11px] leading-relaxed text-[var(--muted)]">
          ล็อกอินผ่าน Microsoft แบบ device code — ไม่ต้องกรอกรหัสผ่านในแอป และไม่เก็บรหัสผ่านไว้ที่ใด
          ต้องมี Azure application (client) id ที่หน้าตั้งค่าก่อน
        </p>
        <button class="btn btn-solid mt-4 w-full" :disabled="store.deviceBusy" @click="store.startMsLogin()">
          {{ store.deviceBusy ? "กำลังขอโค้ด…" : "เข้าสู่ระบบด้วย Microsoft" }}
        </button>
        <p v-if="!store.msClientId()" class="mt-2 text-[10px] leading-relaxed text-[#ffb3c7]">
          แอปนี้ยังไม่ได้ฝัง Azure client id — ล็อกอิน Microsoft ใช้ไม่ได้จนกว่าจะ build ใหม่
        </p>

        <div
          v-if="store.msCheck && !store.msCheck.ok"
          class="mt-3 rounded-lg bg-[rgba(255,120,150,0.08)] px-3 py-2"
        >
          <p class="font-mono text-[9px] uppercase tracking-[0.14em] text-[#ff9dbd]">
            ล็อกอินไม่ผ่าน {{ store.msCheck.code ? `· ${store.msCheck.code}` : "" }}
          </p>
          <p class="mt-1.5 whitespace-pre-line text-[10px] leading-relaxed text-[#ffc0cf]">
            {{ store.msCheck.message }}
          </p>

        </div>
      </template>

      <div class="mt-5 h-px w-full bg-[var(--line)]" />

      <p class="mt-4 hex-label"><i />โปรไฟล์ออฟไลน์</p>
      <div class="mt-3 flex items-center gap-4">
        <span class="monogram h-[58px] w-[52px] text-[24px]">{{ preview }}</span>
        <div class="min-w-0">
          <p class="truncate font-mono text-[11px] text-[var(--text)]">
            {{ store.newAccount.trim() || "—" }}
          </p>
          <p class="mt-1 font-mono text-[9px] uppercase tracking-[0.14em] text-[var(--dim)]">
            offline uuid · {{ uuidHint }}
          </p>
        </div>
      </div>

      <input
        v-model="store.newAccount"
        class="field mt-3"
        type="text"
        placeholder="NexusPlayer"
        maxlength="16"
        @keyup.enter="store.addAccount()"
      />
      <button class="btn mt-2 w-full" @click="store.addAccount()">เพิ่มโปรไฟล์</button>

      <p class="mt-auto pt-5 text-[10px] leading-relaxed text-[var(--dim)]">
        โปรไฟล์ออฟไลน์ใช้ได้กับเซิร์ฟเวอร์ crack เท่านั้น — ถ้าเล่นเซิร์ฟเวอร์จริงต้องใช้บัญชี Microsoft
      </p>
    </section>

    <!-- ── right: register ─────────────────────────────────────────────── -->
    <section class="panel flex min-h-0 flex-col overflow-hidden">
      <div class="flex shrink-0 items-center justify-between border-b border-[var(--line)] px-4 py-3">
        <span class="hex-label"><i />บัญชีทั้งหมด</span>
        <span class="font-mono text-[10px] tabular-nums text-[var(--dim)]">
          {{ pad(store.config.accounts.length) }}
        </span>
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto p-2">
        <div
          v-for="acc in store.config.accounts"
          :key="acc.id"
          class="account-row"
          :class="{ active: acc.id === store.config.active_account }"
        >
          <span class="monogram !h-9 !w-8 text-[14px]">
            {{ acc.username.charAt(0).toUpperCase() }}
          </span>
          <div class="min-w-0 flex-1">
            <p class="truncate text-[13px]">{{ acc.username }}</p>
            <p class="mt-0.5 truncate font-mono text-[9.5px] text-[var(--dim)]">
              {{ acc.kind }}{{ acc.uuid ? ` · ${acc.uuid}` : " · offline" }}
            </p>
          </div>
          <span
            v-if="acc.id === store.config.active_account"
            class="pill pill-on shrink-0 !py-1 font-mono text-[9px] uppercase tracking-[0.14em]"
          >
            ใช้งานอยู่
          </span>
          <button v-else class="btn btn-quiet shrink-0" @click="store.useAccount(acc)">ใช้บัญชีนี้</button>
          <button
            class="btn btn-quiet shrink-0 !px-2"
            :title="acc.kind === 'Microsoft' ? 'ออกจากระบบ' : 'ลบโปรไฟล์'"
            @click="store.removeAccount(acc)"
          >
            <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </div>

        <div v-if="store.config.accounts.length === 0" class="px-5 py-10">
          <p class="text-[14px] font-medium">ยังไม่มีบัญชี</p>
          <p class="mt-1.5 max-w-[320px] text-[12px] leading-relaxed text-[var(--dim)]">
            เพิ่มบัญชี Microsoft ทางซ้ายเพื่อเล่นเซิร์ฟเวอร์จริง หรือสร้างโปรไฟล์ออฟไลน์สำหรับทดสอบ
          </p>
        </div>
      </div>

      <div class="flex shrink-0 items-center justify-between gap-4 border-t border-[var(--line)] px-4 py-3">
        <p class="truncate text-[11px] text-[var(--muted)]">
          กำลังเล่นในชื่อ
          <span class="ml-1 font-medium text-[var(--text)]">{{ store.config.username }}</span>
        </p>
        <button class="btn" @click="store.tab = 'instances'">กลับไปหน้าอินสแตนซ์ →</button>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useAppStore } from "../stores/app";
import { ipc } from "../api/ipc";

const store = useAppStore();

function openBrowser() {
  const url = store.device?.verification_uri;
  if (!url) return;
  void ipc.openUrl(url).catch(() => store.say("เปิดเบราว์เซอร์ไม่สำเร็จ — คัดลอกลิงก์ไปเปิดเอง"));
}

const preview = computed(() => {
  const name = store.newAccount.trim() || store.config.username || "?";
  return name.charAt(0).toUpperCase();
});

const uuidHint = computed(() => {
  const name = store.newAccount.trim() || store.config.username || "";
  if (!name) return "—";
  const seed = name.length * 41;
  return `${seed.toString(16).padStart(4, "0")}…`;
});

function pad(n: number): string {
  return String(n).padStart(2, "0");
}
</script>

<style scoped>
.account-row {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border-radius: 12px;
  transition: background 120ms ease, box-shadow 120ms ease;
}
.account-row:hover {
  background: rgba(255, 255, 255, 0.04);
}
.account-row.active {
  background: var(--acc-soft);
  box-shadow:
    inset 0 0 0 1px rgba(200, 159, 255, 0.28),
    inset -3px 0 0 rgba(255, 157, 189, 0.85);
}
</style>
