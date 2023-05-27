const path = require("path");

const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

const mode = process.env.NODE_ENV === "production" ? "production" : "development";

module.exports = {
	mode,
	entry: {
		bundle: ["./src/main.ts"],
	},
	resolve: {
		alias: {
			"@": path.resolve(__dirname, "src/"),
		},
		extensions: [".ts", ".js"],
		mainFields: ["browser", "module", "main"],
	},
	output: {
		path: path.resolve(__dirname, "public/build"),
		publicPath: "/build/",
		filename: "[name].js"
	},
	module: {
		rules: [
			// Rule: SASS
			{
				test: /\.(scss|sass)$/,
				use: ["css-loader", "sass-loader"],
			},

			// Rule: CSS
			{
				test: /\.css$/,
				use: ["css-loader"],
			},

			// Rule: TypeScript
			{
				test: /\.ts$/,
				use: "ts-loader",
				exclude: /node_modules/,
			},
		],
	},
	devServer: {
		hot: true,
	},
	plugins: [
		new WasmPackPlugin({
			crateDirectory: path.resolve(__dirname, "wasm"),
			// Remove when this issue is resolved: https://github.com/wasm-tool/wasm-pack-plugin/issues/93
			outDir: path.resolve(__dirname, "wasm/pkg"),
			watchDirectories: ["../../../libraries/bezier-rs"].map((folder) => path.resolve(__dirname, folder)),
		}),
	],
	devtool: mode === "development" ? "source-map" : false,
	experiments: {
		asyncWebAssembly: true,
	},
	stats: {
		errorDetails: true,
	}
};
