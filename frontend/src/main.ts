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
