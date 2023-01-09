// This file is the browser's entry point for the JS bundle

// reflect-metadata allows for runtime reflection of types in JavaScript.
// It is needed for class-transformer to work and is imported as a side effect.
// The library replaces the Reflect API on the window to support more features.
import "reflect-metadata";

// import { initWasm } from "@/wasm-communication/editor";

import App from "./App.svelte";
// TODO: Svelte: re-enable the below browser check before app launch
// // This exported function is called in `index.html` after confirming that the browser supports all required JS standards
// // eslint-disable-next-line @typescript-eslint/no-explicit-any
// (window as any).graphiteAppInit = async (): Promise<void> => {

	// Initialize the WASM module for the editor backend
	// await initWasm();

	// TODO: Svelte: clean up the replacement of `#app` with `document.body` and remove that `#app` div and CSS
	// Initialize the SvelteVue application
	export default new App({ target: document.body });

// };
