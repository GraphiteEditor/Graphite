// vite.config.js

import * as path from "path";

import { svelte } from "@sveltejs/vite-plugin-svelte";
import license from "rollup-plugin-license";
import { defineConfig } from "vite";
import toplevelawait from "vite-plugin-top-level-await";
import wasm from "vite-plugin-wasm";

// https://vitejs.dev/config/
export default defineConfig({
	plugins: [svelte({prebundleSvelteLibraries: false}), toplevelawait(), wasm()],
	resolve: {
		extensions: [".mjs", ".js", ".ts", ".jsx", ".tsx", ".json", ".vue"],
		alias: {
			"@": path.resolve(__dirname, "./src"),
		},
	},
	optimizeDeps: {
		disabled: true
	},
	build: {
		rollupOptions: {
			plugins: [
				license({
					thirdParty: {
						output: path.resolve(__dirname, "dist/third-party-licenses.txt"),
					},
				}),
			],
		},
	},
	server: {
		port: 8080
	}
});
