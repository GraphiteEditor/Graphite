import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "path";
import { sveltePreprocess } from "svelte-preprocess/dist/autoProcess";

const projectRootDir = resolve(__dirname);

// https://vitejs.dev/config/
export default defineConfig({
	plugins: [
		svelte({
			preprocess: [sveltePreprocess()],
			onwarn(warning, defaultHandler) {
				const suppressed = ["vite-plugin-svelte-css-no-scopable-elements"];
				if (suppressed.includes(warning.code)) return;

				defaultHandler(warning);
			}
		}),
	],
	resolve: {
		alias: [
			{ find: /@graphite-frontend\/(.*\.svg)/, replacement: resolve(projectRootDir, "$1?raw") },
			{ find: "@graphite-frontend", replacement: projectRootDir },
			{ find: "@graphite/../assets", replacement: resolve(projectRootDir, "assets") },
			{ find: "@graphite/../public", replacement: resolve(projectRootDir, "public") },
			{ find: "@graphite", replacement: resolve(projectRootDir, "src") },
		]
	},
	server: {
		port: 8080,
	},
});
