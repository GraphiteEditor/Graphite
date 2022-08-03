// reflect-metadata allows for runtime reflection of types in JavaScript.
// It is needed for class-transformer to work and is imported as a side effect.
// The library replaces the Reflect API on the window to support more features.
import "reflect-metadata";
import { createApp } from "vue";

import { stripIndents } from "@/utility-functions/strip-indents";
import { initWasm } from "@/wasm-communication/editor";

import App from "@/App.vue";

(async (): Promise<void> => {
	if (!("BigUint64Array" in window)) {
		const body = document.body;
		const message = stripIndents`
			<style>
			h2, p, a { text-align: center; color: white; }
			#app { display: none; }
			</style>
			<h2>This browser is too old</h2>
			<p>Please upgrade to a modern web browser such as the latest Firefox, Chrome, Edge, or Safari version 15 or newer.</p>
			<p>(The <a href="https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/BigInt64Array#browser_compatibility" target="_blank"><code>BigInt64Array</code></a>
			JavaScript API must be supported by the browser for Graphite to function.)</p>
			`;
		body.innerHTML = message + body.innerHTML;
		return;
	}

	// Initialize the WASM module for the editor backend
	await initWasm();

	// Initialize the Vue application
	createApp(App).mount("#app");
})();
