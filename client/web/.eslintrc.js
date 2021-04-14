module.exports = {
	root: true,
	env: {
		node: true,
	},
	extends: [
		"plugin:vue/vue3-essential",
		"@vue/airbnb",
		"@vue/typescript/recommended",
	],
	parserOptions: {
		ecmaVersion: 2020,
	},
	settings: {
		"import/resolver": {
			// `node` must be the top property
			node: {},
			webpack: {
				config: require.resolve("@vue/cli-service/webpack.config.js"),
			},
		},
	},
	rules: {
		"no-console": process.env.NODE_ENV === "production" ? "warn" : "off",
		"no-debugger": process.env.NODE_ENV === "production" ? "warn" : "off",
		"no-tabs": 0,
		"max-len": 0,
		"linebreak-style": ["error", "unix"],
		indent: ["error", "tab"],
		quotes: ["error", "double"],
		"@typescript-eslint/camelcase": "off",
		camelcase: ["error", { ignoreImports: true, ignoreDestructuring: true }],
		"import/extensions": ["error", "ignorePackages", {
			js: "never", jsx: "never", ts: "never", tsx: "never",
		}],
	},
};
