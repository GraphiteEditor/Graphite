// reflect-metadata allows for runtime reflection of types in JavaScript.
// It is needed for class-transformer to work and is imported as a side effect.
// The library replaces the Reflect API on the window to support more features.
import "reflect-metadata";
import { createApp } from "vue";
import "@/utilities/errors";
import App from "@/App.vue";
import { initWasm } from "./state/wasm-loader";

(async () => {
	await initWasm();

	// Initialize the Vue application
	createApp(App).mount("#app");
})();
