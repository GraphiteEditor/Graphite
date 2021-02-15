const path = require('path');
const HtmlWebpackPlugin = require("html-webpack-plugin");
const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

module.exports = {
	entry: "./main.js",
	output: {
		path: path.resolve(__dirname, "dist"),
		filename: "main.js",
	},
	plugins: [
		new HtmlWebpackPlugin({ title: 'Graphite' }),
		new WasmPackPlugin({
			crateDirectory: path.resolve(__dirname, "..", "packages", "wasm-bindings"),
			outDir: path.resolve(__dirname, "pkg"),
		}),
	],
	mode: "development",
	devtool: 'source-map',
	experiments: {
		syncWebAssembly: true,
	},
};
