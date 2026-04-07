import type { Directive } from "vue";

type SpotlightElement = HTMLElement & {
  __spotlightMove__?: (event: MouseEvent) => void;
};

export const spotlightDirective: Directive<SpotlightElement> = {
  mounted(el) {
    const onMove = (event: MouseEvent) => {
      const rect = el.getBoundingClientRect();
      el.style.setProperty("--mouse-x", `${event.clientX - rect.left}px`);
      el.style.setProperty("--mouse-y", `${event.clientY - rect.top}px`);
    };

    el.__spotlightMove__ = onMove;
    el.addEventListener("mousemove", onMove);
  },
  unmounted(el) {
    if (el.__spotlightMove__) {
      el.removeEventListener("mousemove", el.__spotlightMove__);
    }
  },
};
