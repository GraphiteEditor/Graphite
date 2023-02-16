// vite.config.js

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import wasm from "vite-plugin-wasm";
import toplevelawait from "vite-plugin-top-level-await"
import license from "rollup-plugin-license"
import * as path from "path";


// https://vitejs.dev/config/
export default defineConfig({
  plugins: [vue(), wasm(), toplevelawait()],
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
						output: path.resolve(__dirname, "dist/third-party-licenses.txt"),
					},
				}),
			],
		},
	},
})
