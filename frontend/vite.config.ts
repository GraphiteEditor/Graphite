import { execSync } from "child_process";
import { createHash } from "crypto";
import { copyFileSync, cpSync, existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "fs";
import path from "path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import type { PluginOption } from "vite";

const projectRootDir = path.resolve(__dirname);

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
	return {
		plugins: [svelteGlobalStyles(), svelte(), staticAssets(), mode !== "native" && thirdPartyLicenses(), mode !== "native" && serviceWorker()],
		resolve: {
			alias: [{ find: /\/..\/branding\/(.*\.svg)/, replacement: path.resolve(projectRootDir, "../branding", "$1?raw") }],
		},
		server: {
			port: 8080,
			host: "0.0.0.0",
		},
	};
});

// Wraps the content of every `<style lang="scss">...</style>` block in Svelte components with `:global { ... }` to work around Svelte's unwanted scoped styles
function svelteGlobalStyles(): PluginOption {
	return {
		name: "svelte-global-styles",
		enforce: "pre",
		transform(code, id) {
			if (!id.endsWith(".svelte")) return;

			return code.replace(/<style(?=\s|>)([^>]*)>(.*?)<\/style>/gs, (_, attrs, content) => `<style${attrs}>\n:global {\n${content}\n}\n</style>`);
		},
	};
}

function staticAssets(): PluginOption {
	const STATIC_ASSET_DIRS: { source: string; urlPrefix: string }[] = [
		{ source: "../demo-artwork", urlPrefix: "/demo-artwork" },
		{ source: "../branding/favicons", urlPrefix: "" },
	];

	// MIME types for all file extensions found in the static asset directories
	const MIME_TYPES: Record<string, string> = {
		".graphite": "application/json",
		".ico": "image/x-icon",
		".png": "image/png",
		".svg": "image/svg+xml",
		".webmanifest": "application/manifest+json",
		".xml": "application/xml",
	};

	return {
		name: "static-assets",
		// Dev: serve files from the listed directories via middleware
		configureServer(server) {
			server.middlewares.use((req, res, next) => {
				if (!req.url) return next();
				const urlPath = decodeURIComponent(req.url.split("?")[0]);

				const match = STATIC_ASSET_DIRS.find(({ source, urlPrefix }) => {
					if (urlPrefix && !urlPath.startsWith(urlPrefix + "/") && urlPath !== urlPrefix) return false;
					if (!urlPrefix && urlPath.startsWith("/@")) return false;

					const relativePath = urlPrefix ? urlPath.slice(urlPrefix.length) : urlPath;
					const filePath = path.resolve(projectRootDir, source, "." + relativePath);

					const sourceDir = path.resolve(projectRootDir, source) + path.sep;
					if (!filePath.startsWith(sourceDir)) return false;

					return existsSync(filePath) && !statSync(filePath).isDirectory();
				});
				if (!match) return next();
				const { source, urlPrefix } = match;

				const relativePath = urlPrefix ? urlPath.slice(urlPrefix.length) : urlPath;
				const filePath = path.resolve(projectRootDir, source, "." + relativePath);
				const extension = path.extname(filePath).toLowerCase();
				const contentType = MIME_TYPES[extension] || "application/octet-stream";

				res.setHeader("Content-Type", contentType);
				res.end(readFileSync(filePath));
			});
		},
		// Build: copy the listed directories into the output
		writeBundle(options) {
			STATIC_ASSET_DIRS.forEach(({ source, urlPrefix }) => {
				const sourceDir = path.resolve(projectRootDir, source);
				const destinationDir = path.join(options.dir || "dist", urlPrefix);

				if (existsSync(sourceDir)) cpSync(sourceDir, destinationDir, { recursive: true });
			});
		},
	};
}

function thirdPartyLicenses(): PluginOption {
	return {
		name: "third-party-licenses",
		buildStart() {
			try {
				execSync("cargo run -p third-party-licenses", { stdio: "inherit" });
			} catch (e) {
				throw new Error("Failed to generate third-party licenses", { cause: e });
			}
		},
		writeBundle(options) {
			copyFileSync(path.resolve(projectRootDir, "third-party-licenses.txt"), path.join(options.dir || "dist", "third-party-licenses.txt"));
		},
	};
}

function serviceWorker(): PluginOption {
	// Files that should never be precached
	const EXCLUDED_FILES = new Set(["service-worker.js"]);
	const DEFERRED_PREFIXES = ["demo-artwork/", "third-party-licenses.txt"];

	function collectFiles(directory: string, prefix: string): string[] {
		const results: string[] = [];
		if (!existsSync(directory)) return results;

		readdirSync(directory, { withFileTypes: true }).forEach((entry) => {
			const relativePath = prefix ? `${prefix}/${entry.name}` : entry.name;
			if (entry.isDirectory()) {
				results.push(...collectFiles(path.join(directory, entry.name), relativePath));
			} else {
				results.push(relativePath);
			}
		});
		return results;
	}

	function contentHash(filePath: string): string {
		const contents = readFileSync(filePath);
		return createHash("sha256").update(contents).digest("hex").slice(0, 12);
	}

	// Vite appends content hashes to filenames in its build output, like "index-BV2NauF8.js"
	function hasContentHash(fileName: string): boolean {
		return /\w+-[A-Za-z0-9_-]{6,}\.\w+$/.test(fileName);
	}

	return {
		name: "service-worker",
		async writeBundle(options) {
			const outputDir = options.dir || "dist";
			const allFiles = collectFiles(outputDir, "");

			const precacheManifest: { url: string; revision: string | undefined }[] = [];
			const deferredManifest: { url: string; revision: string | undefined }[] = [];

			allFiles.forEach((relativePath) => {
				const fileName = path.basename(relativePath);
				const filePath = path.join(outputDir, relativePath);
				const url = `/${relativePath.replace(/\\/g, "/")}`;

				// Skip excluded files
				if (EXCLUDED_FILES.has(fileName)) return;

				// Deferred files are cached in the background after initial load
				if (DEFERRED_PREFIXES.some((prefix) => relativePath.startsWith(prefix))) {
					deferredManifest.push({ url, revision: contentHash(filePath) });
					return;
				}

				// Hashed filenames don't need a revision (the hash is in the URL)
				if (hasContentHash(fileName)) {
					precacheManifest.push({ url, revision: undefined });
				} else {
					precacheManifest.push({ url, revision: contentHash(filePath) });
				}
			});

			// Compute a content hash from both manifests combined
			const allManifestJson = JSON.stringify({ precache: precacheManifest, deferred: deferredManifest });
			const serviceWorkerContentHash = createHash("sha256").update(allManifestJson).digest("hex").slice(0, 12);

			// Read the service worker source and replace placeholder tokens with actual values
			const serviceWorkerSourcePath = path.resolve(projectRootDir, "src/service-worker.js");
			const serviceWorkerSource = readFileSync(serviceWorkerSourcePath, "utf-8");
			const serviceWorkerFinal = serviceWorkerSource
				.replace("self.__PRECACHE_MANIFEST", JSON.stringify(precacheManifest))
				.replace("self.__DEFERRED_CACHE_MANIFEST", JSON.stringify(deferredManifest))
				.replace("self.__SERVICE_WORKER_CONTENT_HASH", JSON.stringify(serviceWorkerContentHash));

			writeFileSync(path.join(outputDir, "service-worker.js"), serviceWorkerFinal);
		},
	};
}
