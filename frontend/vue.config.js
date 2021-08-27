/* eslint-disable @typescript-eslint/no-var-requires */
const path = require("path");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

module.exports = {
	lintOnSave: "warning",
	// https://cli.vuejs.org/guide/webpack.html
	chainWebpack: (config) => {
		// WASM Pack Plugin integrates compiled Rust code (.wasm) and generated wasm-bindgen code (.js) with the webpack bundle
		// Use this JS to import the bundled Rust entry points: const wasm = import("@/../wasm/pkg");
		// Then call WASM functions with: (await wasm).function_name()
		// https://github.com/wasm-tool/wasm-pack-plugin
		config
			// https://cli.vuejs.org/guide/webpack.html#modifying-options-of-a-plugin
			.plugin("wasm-pack")
			.use(WasmPackPlugin)
			.init(
				(Plugin) =>
					new Plugin({
						crateDirectory: path.resolve(__dirname, "wasm"),
						// Remove when this issue is resolved https://github.com/wasm-tool/wasm-pack-plugin/issues/93
						outDir: path.resolve(__dirname, "wasm/pkg"),
						watchDirectories: [
							path.resolve(__dirname, "../editor"),
							path.resolve(__dirname, "../graphene"),
							path.resolve(__dirname, "../charcoal"),
							path.resolve(__dirname, "../proc-macros"),
						],
					})
			)
			.end();

		// Vue SVG Loader enables importing .svg files into .vue single-file components and using them directly in the HTML
		// https://vue-svg-loader.js.org/
		config.module
			// Replace Vue's existing base loader by first clearing it (https://cli.vuejs.org/guide/webpack.html#replacing-loaders-of-a-rule)
			.rule("svg")
			.uses.clear()
			.end()
			// Add vue-loader as a loader
			.use("vue-loader")
			.loader("vue-loader")
			.end()
			// Add vue-svg-loader as a loader
			.use("vue-svg-loader")
			.loader("vue-svg-loader")
			.end();
	},
};
