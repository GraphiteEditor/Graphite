module.exports = {
	root: true,
	env: {
		node: true,
	},
	extends: ["plugin:vue/vue3-essential", "@vue/airbnb", "@vue/typescript/recommended", "plugin:prettier-vue/recommended", "prettier"],
	parserOptions: {
		ecmaVersion: 2020,
	},
	settings: {
		"import/resolver": {
			// `node` must be listed first
			node: {},
			webpack: { config: require.resolve("@vue/cli-service/webpack.config.js") },
		},
		"prettier-vue": {
			SFCBlocks: {
				template: true,
				style: true,
			},
		},
	},
	rules: {
		indent: ["error", "tab"],
		quotes: ["error", "double"],
		"linebreak-style": ["error", "unix"],
		"eol-last": ["error", "always"],
		"no-console": process.env.NODE_ENV === "production" ? "warn" : "off",
		"no-debugger": process.env.NODE_ENV === "production" ? "warn" : "off",
		"max-len": ["error", { code: 200, tabWidth: 4 }],
		"@typescript-eslint/camelcase": "off",
		camelcase: ["error", { ignoreImports: true, ignoreDestructuring: true }],
		"prettier-vue/prettier": [
			"error",
			{
				tabWidth: 4,
				tabs: true,
				printWidth: 200,
			},
		],
	},
};
