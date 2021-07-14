import { createApp } from "vue";
import { handleKeyUp, handleKeyDown } from "@/utilities/input";
import App from "./App.vue";

// Bind global browser events
document.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keyup", (e: KeyboardEvent) => handleKeyUp(e));
window.addEventListener("keydown", (e: KeyboardEvent) => handleKeyDown(e));

// Initialize the Vue application
createApp(App).mount("#app");
