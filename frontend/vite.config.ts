// vite.config.js

import * as path from "path";

import vue from "@vitejs/plugin-vue";
import license from "rollup-plugin-license";
import { defineConfig } from "vite";
import toplevelawait from "vite-plugin-top-level-await";
import wasm from "vite-plugin-wasm";
import svgLoader from "vite-svg-loader";

// https://vitejs.dev/config/
export default defineConfig({
	plugins: [vue(), toplevelawait(), wasm(), svgLoader()],
	resolve: {
		extensions: [".mjs", ".js", ".ts", ".jsx", ".tsx", ".json", ".vue"],
		alias: {
			"@": path.resolve(__dirname, "./src"),
		},
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
		port: 8080,
	},
});
