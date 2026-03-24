import { execSync } from "child_process";
import { readFileSync } from "fs";
import path from "path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import type { PluginOption } from "vite";
import { DynamicPublicDirectory as viteMultipleAssets } from "vite-multiple-assets";

const projectRootDir = path.resolve(__dirname);

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
	return {
		plugins: plugins(mode),
		resolve: {
			alias: [{ find: /\/..\/branding\/(.*\.svg)/, replacement: path.resolve(projectRootDir, "../branding", "$1?raw") }],
		},
		server: {
			port: 8080,
			host: "0.0.0.0",
		},
	};
});

function plugins(mode: string): PluginOption[] {
	const plugins = [
		svelte(),
		viteMultipleAssets(
			// Additional static asset directories
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
