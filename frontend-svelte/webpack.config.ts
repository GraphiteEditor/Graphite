import * as path from "path";
import fs from "fs";
import { spawnSync } from "child_process";
import WasmPackPlugin from "@wasm-tool/wasm-pack-plugin";
import SvelteCheckPlugin from "svelte-check-plugin";
import SveltePreprocess from "svelte-preprocess";
import type * as webpack from "webpack";
import "webpack-dev-server";
const LicenseCheckerWebpackPlugin = require("license-checker-webpack-plugin");

const mode = process.env.NODE_ENV === "production" ? "production" : "development";

const config: webpack.Configuration = {
	mode,
	entry: {
		bundle: ["./src/main.ts"]
	},
	resolve: {
		alias: {
			// Note: Later in this config file, we'll automatically add paths from `tsconfig.compilerOptions.paths`
			svelte: path.resolve("node_modules", "svelte")
		},
		extensions: [".ts", ".js", ".svelte"],
		mainFields: ["svelte", "browser", "module", "main"]
	},
	output: {
		path: path.resolve(__dirname, "public/build"),
		publicPath: "/build/",
		filename: "[name].js",
		chunkFilename: "[name].[id].js"
	},
	module: {
		rules: [
			// Rule: Svelte
			{
				test: /\.svelte$/,
				use: {
					loader: "svelte-loader",
					options: {
						compilerOptions: {
							// Dev mode must be enabled for HMR to work!
							dev: mode === "development"
						},
						emitCss: mode === "production",
						hotReload: mode === "development",
						hotOptions: {
							// List of options and defaults: https://www.npmjs.com/package/svelte-loader-hot#usage
							noPreserveState: false,
							optimistic: true,
						},
						preprocess: SveltePreprocess({
							scss: true,
							sass: true,
						}),
						onwarn(warning: { code: string; }, handler: (warn: any) => any) {
							const suppress = [
								"css-unused-selector",
								"unused-export-let",
								"a11y-no-noninteractive-tabindex",
							];
							if (suppress.includes(warning.code)) return;

							handler(warning);
						},
					}
				},
			},

			// Required to prevent errors from Svelte on Webpack 5+
			// https://github.com/sveltejs/svelte-loader#usage
			{
				test: /node_modules\/svelte\/.*\.mjs$/,
				resolve: {
					fullySpecified: false
				}
			},

			// Rule: SASS
			{
				test: /\.(scss|sass)$/,
				use: [
					"css-loader",
					"sass-loader"
				]
			},

			// Rule: CSS
			{
				test: /\.css$/,
				use: [
					"css-loader",
				]
			},

			// Rule: TypeScript
			{
				test: /\.ts$/,
				use: "ts-loader",
				exclude: /node_modules/
			},

			// Rule: SVG
			{
				test: /\.svg$/,
				type: "asset/source",
			},
		]
	},
	devServer: {
		hot: true,
	},
	plugins: [
		// WASM Pack Plugin integrates compiled Rust code (.wasm) and generated wasm-bindgen code (.js) with the webpack bundle
		// Use this JS to import the bundled Rust entry points: const wasm = import("@/../wasm/pkg").then(panicProxy);
		// Then call WASM functions with: (await wasm).functionName()
		// https://github.com/wasm-tool/wasm-pack-plugin
		new WasmPackPlugin({
			crateDirectory: path.resolve(__dirname, "wasm"),
			// Remove when this issue is resolved: https://github.com/wasm-tool/wasm-pack-plugin/issues/93
			outDir: path.resolve(__dirname, "wasm/pkg"),
			watchDirectories: ["../editor", "../document-legacy", "../proc-macros", "../node-graph"].map((folder) => path.resolve(__dirname, folder)),
		}),

		// License Checker Webpack Plugin validates the license compatibility of all dependencies which are compiled into the webpack bundle
		// It also writes the third-party license notices to a file which is displayed in the application
		// https://github.com/microsoft/license-checker-webpack-plugin
		new LicenseCheckerWebpackPlugin({
			allow: "(Apache-2.0 OR BSD-2-Clause OR BSD-3-Clause OR MIT OR 0BSD)",
			emitError: true,
			outputFilename: "third-party-licenses.txt",
			outputWriter: formatThirdPartyLicenses,
			// Workaround for failure caused in WebPack 5: https://github.com/microsoft/license-checker-webpack-plugin/issues/25#issuecomment-833325799
			filter: /(^.*[/\\]node_modules[/\\]((?:@[^/\\]+[/\\])?(?:[^@/\\][^/\\]*)))/,
		}),

		// new SvelteCheckPlugin(),
	],
	devtool: mode === "development" ? "source-map" : false,
	// // https://cli.vuejs.org/guide/webpack.html
	// chainWebpack: (config) => {
	// 	// Change the loaders used by the Vue compilation process
	// 	config.module
	// 		// Replace Vue's existing base loader by first clearing it
	// 		// https://cli.vuejs.org/guide/webpack.html#replacing-loaders-of-a-rule
	// 		.rule("svg")
	// 		.uses.clear()
	// 		.end()
	// 		// Required (since upgrading vue-cli to v5) to stop the default import behavior, as documented in:
	// 		// https://webpack.js.org/configuration/module/#ruletype
	// 		.type("javascript/auto")
	// 		// Add vue-loader as a loader for Vue single-file components
	// 		// https://www.npmjs.com/package/vue-loader
	// 		.use("vue-loader")
	// 		.loader("vue-loader")
	// 		.end()
	// 		// Add vue-svg-loader as a loader for importing .svg files into Vue single-file components
	// 		// Located in ./vue-svg-loader.js
	// 		.use("./vue-svg-loader")
	// 		.loader("./vue-svg-loader")
	// 		.end();
	// },
	experiments: {
		asyncWebAssembly: true,
	},
};

// Load path aliases from the tsconfig.json file
const tsconfigPath = path.resolve(__dirname, "tsconfig.json");
const tsconfig = fs.existsSync(tsconfigPath) ? require(tsconfigPath) : {};

if ("compilerOptions" in tsconfig && "paths" in tsconfig.compilerOptions) {
	const aliases = tsconfig.compilerOptions.paths;

	for (const alias in aliases) {
		const paths = aliases[alias].map((p: string) => path.resolve(__dirname, p));

		// Our tsconfig uses glob path formats, whereas webpack just wants directories
		// We'll need to transform the glob format into a format acceptable to webpack

		const wpAlias = alias.replace(/(\\|\/)\*$/, "");
		const wpPaths = paths.map((p: string) => p.replace(/(\\|\/)\*$/, ""));

		if (config.resolve && config.resolve.alias) {
			if (!(wpAlias in config.resolve.alias) && wpPaths.length) {
				(config.resolve.alias as any)[wpAlias] = wpPaths.length > 1 ? wpPaths : wpPaths[0];
			}
		}
	}
}

module.exports = config;

type LicenseInfo = {
	licenseName: string;
	licenseText: string;
	packages: PackageInfo[]
}

type PackageInfo = {
	name: string;
	version: string;
	author: string;
	repository: string;
}

type Dependency = {
	licenseName: string;
	licenseText?: string;
} & PackageInfo

function formatThirdPartyLicenses(jsLicenses: {dependencies: Dependency[]}): string {
	let rustLicenses: LicenseInfo[] | undefined;
	if (process.env.NODE_ENV === "production" && process.env.SKIP_CARGO_ABOUT === undefined) {
		try {
			rustLicenses = generateRustLicenses();
		} catch (err) {
			// Nothing to show. Error messages were printed above.
		}

		if (rustLicenses === undefined) {
			// This is probably caused by cargo about not being installed
			console.error(
				`
				Could not run \`cargo about\`, which is required to generate license information.
				To install cargo-about on your system, you can run \`cargo install cargo-about\`.
				License information is required on production builds. Aborting.
				`
					.trim()
					.split("\n")
					.map((line) => line.trim())
					.join("\n")
			);
			process.exit(1);
		}
	}

	// Remove the HTML character encoding caused by Handlebars
	let licenses = (rustLicenses || []).map((rustLicense): LicenseInfo => ({
		licenseName: htmlDecode(rustLicense.licenseName),
		licenseText: trimBlankLines(htmlDecode(rustLicense.licenseText)),
		packages: rustLicense.packages.map((packageInfo): PackageInfo => ({
			name: htmlDecode(packageInfo.name),
			version: htmlDecode(packageInfo.version),
			author: htmlDecode(packageInfo.author).replace(/\[(.*), \]/, "$1"),
			repository: htmlDecode(packageInfo.repository),
		})),
	}));

	// De-duplicate any licenses with the same text by merging their lists of packages
	licenses.forEach((license, licenseIndex) => {
		licenses.slice(0, licenseIndex).forEach((comparisonLicense) => {
			if (license.licenseText === comparisonLicense.licenseText) {
				license.packages.push(...comparisonLicense.packages);
				comparisonLicense.packages = [];
				// After emptying the packages, the redundant license with no packages will be removed in the next step's `filter()`
			}
		});
	});

	// Delete the internal Graphite crates, which are not third-party and belong elsewhere
	licenses = licenses.filter((license) => {
		license.packages = license.packages.filter((packageInfo) => !(packageInfo.repository && packageInfo.repository.includes("github.com/GraphiteEditor/Graphite")));
		return license.packages.length > 0;
	});

	// Augment the imported Rust license list with the provided JS license list
	jsLicenses.dependencies.forEach((jsLicense) => {
		const { name, version, author, repository, licenseName } = jsLicense;
		const licenseText = trimBlankLines(jsLicense.licenseText ?? "");

		// Remove the `git+` or `git://` prefix and `.git` suffix
		const repo = repository ? repository.replace(/^.*(github.com\/.*?\/.*?)(?:.git)/, "https://$1") : repository;

		const matchedLicense = licenses.find((license) => trimBlankLines(license.licenseText) === licenseText);

		const packages: PackageInfo = { name, version, author, repository: repo };
		if (matchedLicense) matchedLicense.packages.push(packages);
		else licenses.push({ licenseName, licenseText, packages: [packages] });
	});

	// Sort the licenses, and the packages using each license, alphabetically
	licenses.sort((a, b) => a.licenseName.localeCompare(b.licenseName));
	licenses.sort((a, b) => a.licenseText.localeCompare(b.licenseText));
	licenses.forEach((license) => {
		license.packages.sort((a, b) => a.name.localeCompare(b.name));
	});

	// Generate the formatted text file
	let formattedLicenseNotice = "GRAPHITE THIRD-PARTY SOFTWARE LICENSE NOTICES\n\n";
	if (!rustLicenses) formattedLicenseNotice += "WARNING: Licenses for Rust packages are excluded in debug mode to improve performance — do not release without their inclusion!\n\n";

	licenses.forEach((license) => {
		let packagesWithSameLicense = "";
		license.packages.forEach((packageInfo) => {
			const { name, version, author, repository } = packageInfo;
			packagesWithSameLicense += `${name} ${version}${author ? ` - ${author}` : ""}${repository ? ` - ${repository}` : ""}\n`;
		});
		packagesWithSameLicense = packagesWithSameLicense.trim();
		const packagesLineLength = Math.max(...packagesWithSameLicense.split("\n").map((line) => line.length));

		formattedLicenseNotice += `--------------------------------------------------------------------------------

The following packages are licensed under the terms of the ${license.licenseName} license as printed beneath:
${"_".repeat(packagesLineLength)}
${packagesWithSameLicense}
${"‾".repeat(packagesLineLength)}
${license.licenseText}

`;
	});

	return formattedLicenseNotice;
}

function generateRustLicenses(): LicenseInfo[] | undefined {
	console.info("Generating license information for Rust code");
	// This `about.hbs` file is written so it generates a valid JavaScript array expression which we evaluate below
	const { stdout, stderr, status } = spawnSync("cargo", ["about", "generate", "about.hbs"], {
		cwd: path.join(__dirname, ".."),
		encoding: "utf8",
		timeout: 60000, // One minute
		shell: true,
		windowsHide: true, // Hide the terminal on Windows
	});

	if (status !== 0) {
		if (status !== 101) {
			// Cargo returns 101 when the subcommand wasn't found
			console.error("cargo-about failed", status, stderr);
		}
		return undefined;
	}

	// Make sure the output starts with this expected label, we don't want to eval an error message.
	if (!stdout.trim().startsWith("GENERATED_BY_CARGO_ABOUT:")) {
		console.error("Unexpected output from cargo-about", stdout);
		return undefined;
	}

	// Security-wise, eval() isn't any worse than require(), but it doesn't need a temporary file.
	// eslint-disable-next-line no-eval
	return eval(stdout) as LicenseInfo[];
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
		// TODO: Svelte: check if this can be removed
		// eslint-disable-next-line quotes
		quot: '"',
	};

	return input.replace(/&([^;]+);/g, (entity, entityCode: string) => {
		let match;

		const maybeEntity = Object.entries(htmlEntities).find((entry) => entry[1] === entityCode);
		if (maybeEntity) {
			return maybeEntity[1];
		}
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
