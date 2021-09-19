import { createApp } from "vue";

import "@/utilities/errors";
import App from "@/App.vue";
import { initWasm } from "./utilities/wasm-loader";

(async () => {
	await initWasm();

	// Initialize the Vue application
	createApp(App).mount("#app");
})();
