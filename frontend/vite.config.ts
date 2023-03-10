// vite.config.js

import * as path from "path";


import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import license from "rollup-plugin-license";
import toplevelawait from "vite-plugin-top-level-await";
import wasm from "vite-plugin-wasm";

// https://vitejs.dev/config/
export default defineConfig({
	plugins: [svelte(), toplevelawait(), wasm()],
	resolve: {
		alias: {
			"@": path.resolve(__dirname, "./src"),
		},
	},
	build: {
		rollupOptions: {
			plugins: [
				license({
					thirdParty: {
						output: path.resolve(__dirname, "public/third-party-licenses.txt"),
					},
				}),
			],
		},
	},
	server: {
		port: 8080
	}
});
