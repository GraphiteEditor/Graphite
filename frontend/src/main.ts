// Allows for runtime reflection of types in javascript.
// It is needed for class-transformer to work and is imported as a side effect.
// The library replaces the Reflect Api on the window to support more features.
import "reflect-metadata";
import { createApp } from "vue";
import { fullscreenModeChanged } from "@/utilities/fullscreen";
import { onKeyUp, onKeyDown, onPointerMove, onPointerDown, onPointerUp, onMouseScroll, onWindowResize, onBeforeUnload } from "@/utilities/input";
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

	window.addEventListener("beforeunload", onBeforeUnload);

	document.addEventListener("contextmenu", (e) => e.preventDefault());
	document.addEventListener("fullscreenchange", () => fullscreenModeChanged());

	window.addEventListener("keyup", onKeyUp);
	window.addEventListener("keydown", onKeyDown);

	window.addEventListener("pointerdown", onPointerDown);
	window.addEventListener("pointermove", onPointerMove);
	window.addEventListener("pointerup", onPointerUp);

	window.addEventListener("wheel", onMouseScroll, { passive: false });
})();
