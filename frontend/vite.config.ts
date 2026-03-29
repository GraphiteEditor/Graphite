import { execSync } from "child_process";
import { copyFileSync, cpSync, existsSync, readFileSync, statSync } from "fs";
import path from "path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import type { PluginOption } from "vite";

const projectRootDir = path.resolve(__dirname);

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
	return {
		plugins: [svelte(), staticAssets(), mode !== "native" && thirdPartyLicenses()],
		resolve: {
			alias: [{ find: /\/..\/branding\/(.*\.svg)/, replacement: path.resolve(projectRootDir, "../branding", "$1?raw") }],
		},
		server: {
			port: 8080,
			host: "0.0.0.0",
		},
	};
});

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

					const sourceDir = path.resolve(projectRootDir, source);
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
			execSync("cargo run -p third-party-licenses", { stdio: "inherit" });
		},
		writeBundle(options) {
			copyFileSync(path.resolve(projectRootDir, "third-party-licenses.txt"), path.join(options.dir || "dist", "third-party-licenses.txt"));
		},
	};
}
