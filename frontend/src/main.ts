import { createApp } from "vue";

import { fullscreenModeChanged } from "@/utilities/fullscreen";
import { handleKeyUp, handleKeyDown, handleMouseDown } from "@/utilities/input";
import "@/utilities/errors";

import App from "@/App.vue";

// Bind global browser events
document.addEventListener("contextmenu", (e) => e.preventDefault());
document.addEventListener("fullscreenchange", () => fullscreenModeChanged());
window.addEventListener("keyup", (e: KeyboardEvent) => handleKeyUp(e));
window.addEventListener("keydown", (e: KeyboardEvent) => handleKeyDown(e));
window.addEventListener("mousedown", (e: MouseEvent) => handleMouseDown(e));

// Initialize the Vue application
createApp(App).mount("#app");
