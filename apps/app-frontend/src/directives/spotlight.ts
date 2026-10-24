import type { Directive } from "vue";

/**
 * v-spotlight — pointer glow for cards.
 *
 * One shared `pointermove` listener for the whole app (not one per card) and
 * one rAF-throttled style write per frame, targeting only the card under the
 * cursor. Writing the variables on every card for every pointer event made
 * long Modrinth result grids stutter, because each write invalidated a
 * fixed-attachment background across dozens of elements.
 */
interface SpotlightEl extends HTMLElement {
  __glow?: boolean;
}

const GLOW = "[data-glow]";

let users = 0;
let frame = 0;
let pending: PointerEvent | null = null;

function paint() {
  frame = 0;
  const e = pending;
  pending = null;
  if (!e) return;
  const target = (e.target as Element | null)?.closest?.(GLOW) as HTMLElement | null;
  if (!target) return;
  target.style.setProperty("--x", e.clientX.toFixed(1));
  target.style.setProperty("--y", e.clientY.toFixed(1));
  target.style.setProperty("--xp", (e.clientX / window.innerWidth).toFixed(2));
  target.style.setProperty("--yp", (e.clientY / window.innerHeight).toFixed(2));
}

function onPointerMove(e: PointerEvent) {
  pending = e;
  if (!frame) frame = requestAnimationFrame(paint);
}

function acquire() {
  if (users++ === 0) window.addEventListener("pointermove", onPointerMove, { passive: true });
}

function release() {
  users = Math.max(0, users - 1);
  if (users > 0) return;
  window.removeEventListener("pointermove", onPointerMove);
  if (frame) cancelAnimationFrame(frame);
  frame = 0;
  pending = null;
}

export const spotlight: Directive<SpotlightEl> = {
  mounted(el) {
    el.dataset.glow = "";
    acquire();
  },
  beforeUnmount() {
    release();
  },
};
