import { createApp } from "vue";
import { fullscreenModeChanged } from "@/utilities/fullscreen";
import { handleKeyUp, handleKeyDown } from "@/utilities/input";
import App from "@/App.vue";

// Bind global browser events
document.addEventListener("contextmenu", (e) => e.preventDefault());
document.addEventListener("fullscreenchange", () => fullscreenModeChanged());
window.addEventListener("keyup", (e: KeyboardEvent) => handleKeyUp(e));
window.addEventListener("keydown", (e: KeyboardEvent) => handleKeyDown(e));

// Initialize the Vue application
createApp(App).mount("#app");
