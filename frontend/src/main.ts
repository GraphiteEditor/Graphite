import { createApp } from "vue";

import { fullscreenModeChanged } from "@/utilities/fullscreen";
import { onKeyUp, onKeyDown, onMouseMove, onMouseDown, onMouseUp, onMouseScroll, onWindowResize } from "@/utilities/input";
import "@/utilities/errors";

import App from "@/App.vue";

// Bind global browser events
window.addEventListener("resize", onWindowResize);
window.addEventListener("DOMContentLoaded", onWindowResize);

document.addEventListener("contextmenu", (e) => e.preventDefault());
document.addEventListener("fullscreenchange", () => fullscreenModeChanged());

window.addEventListener("keyup", onKeyUp);
window.addEventListener("keydown", onKeyDown);

window.addEventListener("mousemove", onMouseMove);
window.addEventListener("mousedown", onMouseDown);
window.addEventListener("mouseup", onMouseUp);

window.addEventListener("wheel", onMouseScroll, { passive: false });

// Initialize the Vue application
createApp(App).mount("#app");
