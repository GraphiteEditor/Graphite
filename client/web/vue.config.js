/* eslint-disable @typescript-eslint/no-var-requires */
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");
const path = require("path");

module.exports = {
	lintOnSave: false,
	chainWebpack: (config) => {
		// Rust wasm bindgen https://github.com/rustwasm/wasm-bindgen
		config
			.plugin("wasm-pack")
			.use(WasmPackPlugin)
			.init(
				(Plugin) => new Plugin({
					crateDirectory: path.resolve(__dirname, "wasm"),
				}),
			)
			.end();

		const svgRule = config.module.rule("svg");
		svgRule.uses.clear();
		svgRule
			.use("vue-loader")
			.loader("vue-loader")
			.end()
			.use("vue-svg-loader")
			.loader("vue-svg-loader");
	},
};
