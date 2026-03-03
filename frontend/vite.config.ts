import { execSync } from "child_process";
import { readFileSync } from "fs";
import path from "path";

import { svelte } from "@sveltejs/vite-plugin-svelte";
import { sveltePreprocess } from "svelte-preprocess";
import { defineConfig, type PluginOption } from "vite";
import { DynamicPublicDirectory as viteMultipleAssets } from "vite-multiple-assets";

const projectRootDir = path.resolve(__dirname);

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
	return {
		plugins: plugins(mode),
		resolve: {
			alias: [
				{ find: /@branding\/(.*\.svg)/, replacement: path.resolve(projectRootDir, "../branding", "$1?raw") },
				{ find: "@graphite/../assets", replacement: path.resolve(projectRootDir, "assets") },
				{ find: "@graphite/../public", replacement: path.resolve(projectRootDir, "public") },
				{ find: "@graphite", replacement: path.resolve(projectRootDir, "src") },
			],
		},
		server: {
			port: 8080,
			host: "0.0.0.0",
		},
	};
});

function plugins(mode: string): PluginOption[] {
	const plugins = [
		svelte({
			preprocess: [sveltePreprocess()],
			onwarn(warning, defaultHandler) {
				const suppressed = [
					"css-unused-selector", // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
					"vite-plugin-svelte-css-no-scopable-elements", // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
					"a11y-no-static-element-interactions", // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
					"a11y-no-noninteractive-element-interactions", // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
					"a11y-click-events-have-key-events", // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
					"a11y_consider_explicit_label", // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
					"a11y_click_events_have_key_events", // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
					"a11y_no_noninteractive_element_interactions", // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
					"a11y_no_static_element_interactions", // NOTICE: Keep this list in sync with the list in `.vscode/settings.json`
				];
				if (suppressed.includes(warning.code)) return;

				defaultHandler?.(warning);
			},
		}),
		viteMultipleAssets(
			// Additional static asset directories besides `public/`
			[
				{ input: "../demo-artwork/**", output: "demo-artwork" },
				{ input: "../branding/favicons/**", output: "" },
			],
			// Options where we set custom MIME types
			{ mimeTypes: { ".graphite": "application/json" } },
		),
	];

	if (mode !== "native") {
		plugins.push({
			name: "third-party-licenses",
			buildStart() {
				try {
					execSync("cargo run -p third-party-licenses", {
						stdio: "inherit",
					});
				} catch (_e) {
					this.error("Failed to generate third-party licenses");
				}
			},
			generateBundle() {
				const source = readFileSync(path.resolve(projectRootDir, "third-party-licenses.txt"), "utf-8");
				this.emitFile({
					type: "asset",
					fileName: "third-party-licenses.txt",
					source,
				});
			},
		});
	}

	return plugins;
}
