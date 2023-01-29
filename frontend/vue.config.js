/* eslint-disable @typescript-eslint/no-var-requires, no-console */
const { spawnSync } = require("child_process");
const path = require("path");

const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const LicenseCheckerWebpackPlugin = require("license-checker-webpack-plugin");

module.exports = {
	lintOnSave: "warning",
	// https://cli.vuejs.org/guide/webpack.html
	chainWebpack: (config) => {
		// WASM Pack Plugin integrates compiled Rust code (.wasm) and generated wasm-bindgen code (.js) with the webpack bundle
		// Use this JS to import the bundled Rust entry points: const wasm = import("@/../wasm/pkg").then(panicProxy);
		// Then call WASM functions with: (await wasm).functionName()
		// https://github.com/wasm-tool/wasm-pack-plugin
		config
			// https://cli.vuejs.org/guide/webpack.html#modifying-options-of-a-plugin
			.plugin("wasm-pack")
			.use(WasmPackPlugin)
			.init(
				(Plugin) =>
					new Plugin({
						crateDirectory: path.resolve(__dirname, "wasm"),
						// Remove when this issue is resolved: https://github.com/wasm-tool/wasm-pack-plugin/issues/93
						outDir: path.resolve(__dirname, "wasm/pkg"),
						watchDirectories: ["../editor", "../document-legacy", "../proc-macros", "../node-graph"].map((folder) => path.resolve(__dirname, folder)),
					})
			)
			.end();

		// License Checker Webpack Plugin validates the license compatibility of all dependencies which are compiled into the webpack bundle
		// It also writes the third-party license notices to a file which is displayed in the application
		// https://github.com/microsoft/license-checker-webpack-plugin
		config
			.plugin("license-checker")
			.use(LicenseCheckerWebpackPlugin)
			.init(
				(Plugin) =>
					new Plugin({
						allow: "(Apache-2.0 OR BSD-2-Clause OR BSD-3-Clause OR MIT OR 0BSD)",
						emitError: true,
						outputFilename: "third-party-licenses.txt",
						outputWriter: formatThirdPartyLicenses,
						// Workaround for failure caused in WebPack 5: https://github.com/microsoft/license-checker-webpack-plugin/issues/25#issuecomment-833325799
						filter: /(^.*[/\\]node_modules[/\\]((?:@[^/\\]+[/\\])?(?:[^@/\\][^/\\]*)))/,
					})
			);

		// Change the loaders used by the Vue compilation process
		config.module
			// Replace Vue's existing base loader by first clearing it
			// https://cli.vuejs.org/guide/webpack.html#replacing-loaders-of-a-rule
			.rule("svg")
			.uses.clear()
			.end()
			// Required (since upgrading vue-cli to v5) to stop the default import behavior, as documented in:
			// https://webpack.js.org/configuration/module/#ruletype
			.type("javascript/auto")
			// Add vue-loader as a loader for Vue single-file components
			// https://www.npmjs.com/package/vue-loader
			.use("vue-loader")
			.loader("vue-loader")
			.end()
			// Add vue-svg-loader as a loader for importing .svg files into Vue single-file components
			// Located in ./vue-svg-loader.js
			.use("./vue-svg-loader")
			.loader("./vue-svg-loader")
			.end();
	},
	configureWebpack: {
		experiments: {
			asyncWebAssembly: true,
		},
	},
};

function formatThirdPartyLicenses(jsLicenses) {
	let rustLicenses;
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
	let licenses = (rustLicenses || []).map((rustLicense) => ({
		licenseName: htmlDecode(rustLicense.licenseName),
		licenseText: trimBlankLines(htmlDecode(rustLicense.licenseText)),
		packages: rustLicense.packages.map((packageInfo) => ({
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

		const packages = { name, version, author, repository: repo };
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

function generateRustLicenses() {
	console.info("Generating license information for Rust code");
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

	// Make sure the output starts as expected, we don't want to eval an error message.
	if (!stdout.trim().startsWith("GENERATED_BY_CARGO_ABOUT:")) {
		console.error("Unexpected output from cargo-about", stdout);
		return undefined;
	}

	// Security-wise, eval() isn't any worse than require(), but it doesn't need a temporary file.
	// eslint-disable-next-line no-eval
	return eval(stdout);
}

function htmlDecode(input) {
	if (!input) return input;

	const htmlEntities = {
		nbsp: " ",
		copy: "©",
		reg: "®",
		lt: "<",
		gt: ">",
		amp: "&",
		apos: "'",
		// eslint-disable-next-line quotes
		quot: '"',
	};

	return input.replace(/&([^;]+);/g, (entity, entityCode) => {
		let match;

		if (entityCode in htmlEntities) {
			return htmlEntities[entityCode];
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

function trimBlankLines(input) {
	let result = input.replace(/\r/g, "");

	while (result.charAt(0) === "\r" || result.charAt(0) === "\n") {
		result = result.slice(1);
	}
	while (result.slice(-1) === "\r" || result.slice(-1) === "\n") {
		result = result.slice(0, -1);
	}

	return result;
}
