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
			// `node` must be listed first!
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
	ignorePatterns: ["node_modules/", "dist/", "pkg/", "wasm/pkg/*", "!.*.js", "!.*.ts", "!.*.json"],
	rules: {
		indent: ["error", "tab", { SwitchCase: 1 }],
		quotes: ["error", "double"],
		"linebreak-style": ["error", "unix"],
		"eol-last": ["error", "always"],
		"no-console": process.env.NODE_ENV === "production" ? "warn" : "off",
		"no-debugger": process.env.NODE_ENV === "production" ? "warn" : "off",
		"max-len": ["error", { code: 200, tabWidth: 4 }],
		"no-param-reassign": [2, { "props": false }],
		"@typescript-eslint/camelcase": "off",
		"@typescript-eslint/no-use-before-define": "off",
		"@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
		camelcase: ["error", { allow: ["^(?:[a-z]+_)*[a-z]+$"] }],
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
