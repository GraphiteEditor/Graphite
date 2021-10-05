import { createApp } from "vue";

import { fullscreenModeChanged } from "@/utilities/fullscreen";
import { onKeyUp, onKeyDown, onMouseMove, onMouseDown, onMouseUp, onMouseScroll, onWindowResize, onScroll } from "@/utilities/input";
import "@/utilities/errors";
import App from "@/App.vue";
import { panicProxy } from "@/utilities/panic-proxy";

const wasm = import("@/../wasm/pkg").then(panicProxy);
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(window as any).wasmMemory = undefined;

(async () => {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	(window as any).wasmMemory = (await wasm).wasm_memory;

	// Initialize the Vue application
	createApp(App).mount("#app");

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
	window.addEventListener("scroll", onScroll, true);
})();
