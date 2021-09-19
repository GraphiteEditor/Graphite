import { createApp } from "vue";

import { fullscreenModeChanged } from "@/utilities/fullscreen";
import { onKeyUp, onKeyDown, onMouseMove, onMouseDown, onMouseUp, onMouseScroll, onWindowResize } from "@/utilities/input";
import "@/utilities/errors";
import App from "@/App.vue";
import wasm, { initWasm } from "./utilities/wasm-loader";

(async () => {
	await initWasm();

	// Initialize the Vue application
	createApp(App).mount("#app");

	// Load the initial document list
	wasm().get_open_documents_list();

	// Bind global browser events
	window.addEventListener("resize", onWindowResize);
	onWindowResize();

	document.addEventListener("contextmenu", (e) => e.preventDefault());
	document.addEventListener("fullscreenchange", () => fullscreenModeChanged());

	window.addEventListener("keyup", onKeyUp);
	window.addEventListener("keydown", onKeyDown);

	window.addEventListener("mousemove", onMouseMove);
	window.addEventListener("mousedown", onMouseDown);
	window.addEventListener("mouseup", onMouseUp);

	window.addEventListener("wheel", onMouseScroll, { passive: false });
})();
