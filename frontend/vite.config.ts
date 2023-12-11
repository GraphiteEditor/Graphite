/* eslint-disable no-console */

import { spawnSync } from "child_process";

import path from "path";

import { svelte } from "@sveltejs/vite-plugin-svelte";
import rollupPluginLicense, { type Dependency } from "rollup-plugin-license";
import { sveltePreprocess } from "svelte-preprocess/dist/autoProcess";
import { defineConfig } from "vite";
import { default as viteMultipleAssets } from "vite-multiple-assets";

const projectRootDir = path.resolve(__dirname);

// Keep this list in sync with those in `/about.toml` and `/deny.toml`.
const ALLOWED_LICENSES = [
	"Apache-2.0 WITH LLVM-exception",
	"Apache-2.0",
	"BSD-2-Clause",
	"BSD-3-Clause",
	"BSL-1.0",
	"CC0-1.0",
	"ISC",
	"MIT-0",
	"MIT",
	"MPL-2.0",
	"OpenSSL",
	"Unicode-DFS-2016",
	"Zlib",
];

// https://vitejs.dev/config/
export default defineConfig({
	plugins: [
		svelte({
			preprocess: [sveltePreprocess()],
			onwarn(warning, defaultHandler) {
				const suppressed = ["css-unused-selector", "vite-plugin-svelte-css-no-scopable-elements"];
				if (suppressed.includes(warning.code)) return;

				defaultHandler?.(warning);
			},
		}),
		viteMultipleAssets(["../demo-artwork"]),
	],
	resolve: {
		alias: [
			{ find: /@graphite-frontend\/(.*\.svg)/, replacement: path.resolve(projectRootDir, "$1?raw") },
			{ find: "@graphite-frontend", replacement: projectRootDir },
			{ find: "@graphite/../assets", replacement: path.resolve(projectRootDir, "assets") },
			{ find: "@graphite/../public", replacement: path.resolve(projectRootDir, "public") },
			{ find: "@graphite", replacement: path.resolve(projectRootDir, "src") },
		],
	},
	server: {
		port: 8080,
		host: "0.0.0.0",
	},
	build: {
		rollupOptions: {
			plugins: [
				rollupPluginLicense({
					thirdParty: {
						allow: {
							test: `(${ALLOWED_LICENSES.join(" OR ")})`,
							failOnUnlicensed: true,
							failOnViolation: true,
						},
						output: {
							file: path.resolve(__dirname, "./dist/third-party-licenses.txt"),
							template: formatThirdPartyLicenses,
						},
					},
				}),
			],
			output: {
				// Inject `.min` into the filename of minified CSS files to tell Cloudflare not to minify it again.
				// Cloudflare's minifier breaks the CSS due to a bug where it removes whitespace around calc() plus operators.
				assetFileNames: (info) => `assets/[name]-[hash]${info.name?.endsWith(".css") ? ".min" : ""}[extname]`,
			},
		},
	},
});

type LicenseInfo = {
	licenseName: string;
	licenseText: string;
	packages: PackageInfo[];
};

type PackageInfo = {
	name: string;
	version: string;
	author: string;
	repository: string;
};

function formatThirdPartyLicenses(jsLicenses: Dependency[]): string {
	// Generate the Rust license information.
	let licenses = generateRustLicenses() || [];

	// Ensure we have license information to work with before proceeding.
	if (licenses.length === 0) {
		// This is probably caused by `cargo about` not being installed.
		console.error("Could not run `cargo about`, which is required to generate license information.");
		console.error("To install cargo-about on your system, you can run `cargo install cargo-about`.");
		console.error("License information is required in production builds. Aborting.");

		process.exit(1);
	}
	if (jsLicenses.length === 0) {
		console.error("No JavaScript package licenses were found by `rollup-plugin-license`. Please investigate.");
		console.error("License information is required in production builds. Aborting.");

		process.exit(1);
	}

	// Augment the imported Rust license list with the provided JS license list.
	jsLicenses.forEach((jsLicense) => {
		const name = jsLicense.name || "";
		const version = jsLicense.version || "";
		const author = jsLicense.author?.text() || "";
		const licenseText = trimBlankLines(jsLicense.licenseText ?? "");
		const licenseName = jsLicense.license || "";
		let repository = jsLicense.repository || "";
		if (repository && typeof repository === "object") repository = repository.url;

		// Remove the `git+` or `git://` prefix and `.git` suffix.
		const repo = repository ? repository.replace(/^.*(github.com\/.*?\/.*?)(?:.git)/, "https://$1") : repository;

		const matchedLicense = licenses.find((license) => trimBlankLines(license.licenseText || "") === licenseText);

		const packages: PackageInfo = { name, version, author, repository: repo };
		if (matchedLicense) matchedLicense.packages.push(packages);
		else licenses.push({ licenseName, licenseText, packages: [packages] });
	});

	// De-duplicate any licenses with the same text by merging their lists of packages.
	licenses.forEach((license, licenseIndex) => {
		licenses.slice(0, licenseIndex).forEach((comparisonLicense) => {
			if (license.licenseText === comparisonLicense.licenseText) {
				license.packages.push(...comparisonLicense.packages);
				comparisonLicense.packages = [];
				// After emptying the packages, the redundant license with no packages will be removed in the next step's `filter()`.
			}
		});
	});

	// Filter out the internal Graphite crates, which are not third-party.
	licenses = licenses.filter((license) => {
		license.packages = license.packages.filter(
			(packageInfo) =>
				!(packageInfo.repository && packageInfo.repository.toLowerCase().includes("github.com/GraphiteEditor/Graphite".toLowerCase())) &&
				!(packageInfo.author && packageInfo.author.toLowerCase().includes("contact@graphite.rs")),
		);
		return license.packages.length > 0;
	});

	// Sort the licenses, and the packages using each license, alphabetically.
	licenses.sort((a, b) => a.licenseName.localeCompare(b.licenseName));
	licenses.sort((a, b) => a.licenseText.localeCompare(b.licenseText));
	licenses.forEach((license) => {
		license.packages.sort((a, b) => a.name.localeCompare(b.name));
	});

	// Append a block for each license shared by multiple packages with identical license text.
	let formattedLicenseNotice = "GRAPHITE THIRD-PARTY SOFTWARE LICENSE NOTICES";
	licenses.forEach((license) => {
		let packagesWithSameLicense = "";
		license.packages.forEach((packageInfo) => {
			const { name, version, author, repository } = packageInfo;
			packagesWithSameLicense += `${name} ${version}${author ? ` - ${author}` : ""}${repository ? ` - ${repository}` : ""}\n`;
		});
		packagesWithSameLicense = packagesWithSameLicense.trim();
		const packagesLineLength = Math.max(...packagesWithSameLicense.split("\n").map((line) => line.length));

		formattedLicenseNotice += "\n\n--------------------------------------------------------------------------------\n\n";
		formattedLicenseNotice += `The following packages are licensed under the terms of the ${license.licenseName} license as printed beneath:\n`;
		formattedLicenseNotice += `${"_".repeat(packagesLineLength)}\n`;
		formattedLicenseNotice += `${packagesWithSameLicense}\n`;
		formattedLicenseNotice += `${"‾".repeat(packagesLineLength)}\n`;
		formattedLicenseNotice += `${license.licenseText}\n`;
	});
	return formattedLicenseNotice;
}

function generateRustLicenses(): LicenseInfo[] | undefined {
	// Log the starting status to the build output.
	console.info("\n\nGenerating license information for Rust code\n");

	try {
		// Call `cargo about` in the terminal to generate the license information for Rust crates.
		// The `about.hbs` file is written so it generates a valid JavaScript array expression which we evaluate below.
		const { stdout, stderr, status } = spawnSync("cargo", ["about", "generate", "about.hbs"], {
			cwd: path.join(__dirname, ".."),
			encoding: "utf8",
			timeout: 60000, // One minute
			shell: true,
			windowsHide: true, // Hide the terminal on Windows
		});

		// If the command failed, print the error message and exit early.
		if (status !== 0) {
			// Cargo returns 101 when the subcommand (`about`) wasn't found, so we skip printing the below error message in that case.
			if (status !== 101) {
				console.error("cargo-about failed", status, stderr);
			}
			return undefined;
		}

		// Make sure the output starts with this expected label, which lets us know the file generated with expected output.
		// We don't want to eval an error message or something else, so we fail early if that happens.
		if (!stdout.trim().startsWith("GENERATED_BY_CARGO_ABOUT:")) {
			console.error("Unexpected output from cargo-about", stdout);
			return undefined;
		}

		// Convert the array JS syntax string into an actual JS array in memory.
		// Security-wise, eval() isn't any worse than require(), but it's able to work without a temporary file.
		// We call eval indirectly to avoid a warning as explained here: <https://esbuild.github.io/content-types/#direct-eval>.
		const indirectEval = eval;
		const licensesArray = indirectEval(stdout) as LicenseInfo[];

		// Remove the HTML character encoding caused by Handlebars.
		const rustLicenses = (licensesArray || []).map(
			(rustLicense): LicenseInfo => ({
				licenseName: htmlDecode(rustLicense.licenseName),
				licenseText: trimBlankLines(htmlDecode(rustLicense.licenseText)),
				packages: rustLicense.packages.map(
					(packageInfo): PackageInfo => ({
						name: htmlDecode(packageInfo.name),
						version: htmlDecode(packageInfo.version),
						author: htmlDecode(packageInfo.author)
							.replace(/\[(.*), \]/, "$1")
							.replace("[]", ""),
						repository: htmlDecode(packageInfo.repository),
					}),
				),
			}),
		);

		return rustLicenses;
	} catch (_) {
		return undefined;
	}
}

function htmlDecode(input: string): string {
	if (!input) return input;

	const htmlEntities = {
		nbsp: " ",
		copy: "©",
		reg: "®",
		lt: "<",
		gt: ">",
		amp: "&",
		apos: "'",
		quot: `"`,
	};

	return input.replace(/&([^;]+);/g, (entity: string, entityCode: string) => {
		const maybeEntity = Object.keys(htmlEntities).find((key) => key === entityCode);
		if (maybeEntity) {
			return maybeEntity[1];
		}

		let match;
		// eslint-disable-next-line no-cond-assign
		if ((match = entityCode.match(/^#x([\da-fA-F]+)$/))) {
			return String.fromCharCode(parseInt(match[1], 16));
		}
		// eslint-disable-next-line no-cond-assign
		if ((match = entityCode.match(/^#(\d+)$/))) {
			return String.fromCharCode(~~match[1]);
		}
		return entity;
	});
}

function trimBlankLines(input: string): string {
	let result = input.replace(/\r/g, "");

	while (result.charAt(0) === "\r" || result.charAt(0) === "\n") {
		result = result.slice(1);
	}
	while (result.slice(-1) === "\r" || result.slice(-1) === "\n") {
		result = result.slice(0, -1);
	}

	return result;
}
