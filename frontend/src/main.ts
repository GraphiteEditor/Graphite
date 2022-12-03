// This file is the browser's entry point for the JS bundle

// reflect-metadata allows for runtime reflection of types in JavaScript.
// It is needed for class-transformer to work and is imported as a side effect.
// The library replaces the Reflect API on the window to support more features.
import "reflect-metadata";
import { createApp } from "vue";

import { initWasm } from "@/wasm-communication/editor";

import App from "@/App.vue";

// This exported function is called in `index.html` after confirming that the browser supports all required JS standards
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(window as any).graphiteAppInit = async (): Promise<void> => {
	// Initialize the WASM module for the editor backend
	await initWasm();

	// Initialize the Vue application
	createApp(App)
		.directive("focus", {
			// When the bound element is inserted into the DOM
			mounted(el) {
				let focus = el;

				// Find actual relevant child
				while (focus.children.length) focus = focus.children[0];

				// Random timeout needed?
				setTimeout(() => {
					focus.focus(); // Focus the element
				}, 0);
			},
		})
		.mount("#app");
};
