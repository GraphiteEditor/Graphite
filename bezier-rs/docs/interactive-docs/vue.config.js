const path = require("path");

const { defineConfig } = require("@vue/cli-service");

const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

module.exports = defineConfig({
	transpileDependencies: true,
	// https://cli.vuejs.org/guide/webpack.html
	chainWebpack: (config) => {
		// WASM Pack Plugin integrates compiled Rust code (.wasm) and generated wasm-bindgen code (.js) with the webpack bundle
		// Use this JS to import the bundled Rust entry points: const wasm = import("@/../wasm/pkg").then(panicProxy);
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
						// Remove when this issue is resolved: https://github.com/wasm-tool/wasm-pack-plugin/issues/93
						outDir: path.resolve(__dirname, "wasm/pkg"),
						watchDirectories: ["../../lib"].map((folder) => path.resolve(__dirname, folder)),
					})
			)
			.end();
	},
	configureWebpack: {
		experiments: {
			asyncWebAssembly: true,
		},
	},
});
