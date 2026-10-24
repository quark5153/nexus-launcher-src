<template>
  <div ref="root" class="sel">
    <button
      ref="trigger"
      type="button"
      class="sel-trigger"
      :disabled="disabled"
      :aria-expanded="open"
      aria-haspopup="listbox"
      @click="toggle"
      @keydown.down.prevent="openWith(1)"
      @keydown.up.prevent="openWith(-1)"
      @keydown.enter.prevent="toggle"
    >
      <span class="sel-label" :class="{ placeholder: !selected }">
        {{ selected?.label ?? placeholder }}
      </span>
      <svg class="sel-caret" viewBox="0 0 12 8" fill="none" aria-hidden="true">
        <path d="M1 1.6 6 6.4l5-4.8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </button>

    <Teleport to="body">
      <Transition name="sel-pop">
        <div
          v-if="open"
          ref="popup"
          class="sel-pop"
          :style="popupStyle"
          role="listbox"
          tabindex="-1"
          @keydown.down.prevent="move(1)"
          @keydown.up.prevent="move(-1)"
          @keydown.enter.prevent="commit"
          @keydown.esc.prevent="close"
          @keydown.tab="close"
        >
          <button
            v-for="(opt, i) in options"
            :key="opt.value"
            type="button"
            class="sel-opt"
            :class="{ on: opt.value === modelValue, hot: i === active }"
            role="option"
            :aria-selected="opt.value === modelValue"
            @mouseenter="active = i"
            @click="pick(opt)"
          >
            <span class="sel-opt-label">{{ opt.label }}</span>
            <svg v-if="opt.value === modelValue" class="sel-tick" viewBox="0 0 12 12" fill="none" aria-hidden="true">
              <path d="M2.4 6.3 5 8.9l4.6-5.2" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </button>
          <p v-if="!options.length" class="sel-empty">ไม่มีตัวเลือก</p>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";

export interface SelectOption {
  value: string;
  label: string;
}

const props = withDefaults(
  defineProps<{
    modelValue: string;
    options: SelectOption[];
    placeholder?: string;
    disabled?: boolean;
  }>(),
  { placeholder: "—", disabled: false },
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  change: [value: string];
}>();

const root = ref<HTMLElement | null>(null);
const trigger = ref<HTMLElement | null>(null);
const popup = ref<HTMLElement | null>(null);

const open = ref(false);
const active = ref(0);
const box = ref({ top: 0, left: 0, width: 0 });

const selected = computed(() => props.options.find((o) => o.value === props.modelValue));

const popupStyle = computed(() => ({
  top: `${box.value.top}px`,
  left: `${box.value.left}px`,
  width: `${box.value.width}px`,
}));

/** Anchor the popup under the trigger, flipping above when there is no room. */
function place() {
  const el = trigger.value;
  if (!el) return;
  const r = el.getBoundingClientRect();
  const height = Math.min(props.options.length * 34 + 12, 300);
  const width = Math.max(r.width, 192);
  const flip = window.innerHeight - r.bottom < height + 12 && r.top > height + 12;
  box.value = {
    top: Math.max(8, flip ? r.top - height - 8 : r.bottom + 8),
    left: Math.max(8, Math.min(r.left, window.innerWidth - width - 8)),
    width,
  };
}

function scrollActive() {
  const el = popup.value?.children[active.value] as HTMLElement | undefined;
  el?.scrollIntoView({ block: "nearest" });
}

function onDocPointer(event: PointerEvent) {
  const target = event.target as Node | null;
  if (!target) return;
  if (root.value?.contains(target) || popup.value?.contains(target)) return;
  close();
}

function openList() {
  if (props.disabled || open.value) return;
  const idx = props.options.findIndex((o) => o.value === props.modelValue);
  active.value = idx >= 0 ? idx : 0;
  place();
  open.value = true;
  window.addEventListener("scroll", place, true);
  window.addEventListener("resize", place);
  document.addEventListener("pointerdown", onDocPointer, true);
  void nextTick(() => {
    popup.value?.focus({ preventScroll: true });
    scrollActive();
  });
}

function close() {
  if (!open.value) return;
  open.value = false;
  window.removeEventListener("scroll", place, true);
  window.removeEventListener("resize", place);
  document.removeEventListener("pointerdown", onDocPointer, true);
}

function toggle() {
  open.value ? close() : openList();
}

function openWith(dir: number) {
  if (!open.value) openList();
  else move(dir);
}

function move(dir: number) {
  if (!props.options.length) return;
  active.value = (active.value + dir + props.options.length) % props.options.length;
  scrollActive();
}

function pick(option: SelectOption) {
  if (option.value !== props.modelValue) {
    emit("update:modelValue", option.value);
    emit("change", option.value);
  }
  close();
}

function commit() {
  const option = props.options[active.value];
  if (option) pick(option);
}

watch(
  () => props.disabled,
  (value) => {
    if (value) close();
  },
);

onBeforeUnmount(close);
</script>

<style scoped>
.sel {
  position: relative;
  width: 100%;
}

.sel-trigger {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  padding: 8px 12px 8px 14px;
  font: inherit;
  color: var(--text);
  text-align: left;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid var(--line);
  border-radius: 11px;
  cursor: pointer;
  transition:
    border-color 140ms ease,
    background 140ms ease,
    box-shadow 140ms ease;
}
.sel-trigger:hover:not(:disabled):not([aria-expanded="true"]) {
  background: rgba(255, 255, 255, 0.06);
  border-color: var(--line-2);
}
.sel-trigger[aria-expanded="true"],
.sel-trigger:focus-visible {
  outline: none;
  border-color: var(--acc-line);
  box-shadow: 0 0 0 3px rgba(255, 143, 190, 0.14);
}
.sel-trigger:disabled {
  opacity: 0.4;
  cursor: default;
}
.sel-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sel-label.placeholder {
  color: var(--dim);
}
.sel-caret {
  flex: 0 0 auto;
  width: 11px;
  height: 7px;
  color: var(--acc);
  transition: transform 190ms cubic-bezier(0.22, 1, 0.36, 1);
}
.sel-trigger[aria-expanded="true"] .sel-caret {
  transform: rotate(180deg);
}

/* popup is teleported to <body> so panels/scroll containers never clip it */
.sel-pop {
  position: fixed;
  z-index: 200;
  max-height: 300px;
  overflow-y: auto;
  padding: 5px;
  border-radius: 13px;
  border: 1px solid var(--line-2);
  background: linear-gradient(180deg, rgba(30, 20, 40, 0.99), rgba(17, 10, 22, 0.99));
  box-shadow:
    0 22px 50px rgba(2, 1, 4, 0.72),
    inset 0 1px 0 rgba(255, 255, 255, 0.06);
  backdrop-filter: blur(18px);
  outline: none;
  overscroll-behavior: contain;
}
.sel-pop::-webkit-scrollbar {
  width: 7px;
}

.sel-opt {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  padding: 7px 9px;
  border-radius: 9px;
  font-size: 12px;
  text-align: left;
  color: var(--muted);
  transition:
    background 110ms ease,
    color 110ms ease;
}
.sel-opt.hot {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text);
}
.sel-opt.on {
  color: var(--acc-2);
  background: var(--acc-soft);
  font-weight: 500;
}
.sel-opt-label {
  flex: 1;
  min-width: 0;
  line-height: 1.35;
  overflow-wrap: anywhere;
}
.sel-tick {
  flex: 0 0 auto;
  width: 12px;
  height: 12px;
}
.sel-empty {
  padding: 8px 10px;
  font-size: 11px;
  color: var(--dim);
}

.sel-pop-enter-active,
.sel-pop-leave-active {
  transition:
    opacity 130ms ease,
    transform 160ms cubic-bezier(0.22, 1, 0.36, 1);
}
.sel-pop-enter-from,
.sel-pop-leave-to {
  opacity: 0;
  transform: translateY(-5px) scale(0.985);
}
</style>
