const webpackConfigPath = require.resolve("@vue/cli-service/webpack.config.js");

module.exports = {
	root: true,
	env: {
		browser: true,
		node: true,
		es2020: true,
	},
	parserOptions: {
		ecmaVersion: 2020,
	},
	extends: [
		// Vue-specific defaults
		"plugin:vue/vue3-essential",
		// Vue-compatible JS defaults
		"@vue/airbnb",
		// Vue-compatible TS defaults
		"@vue/typescript/recommended",
		// Vue-compatible Prettier defaults
		"plugin:prettier-vue/recommended",
		// General Prettier defaults
		"prettier",
	],
	settings: {
		// https://github.com/import-js/eslint-plugin-import#resolvers
		"import/resolver": {
			// `node` must be listed first!
			node: {},
			webpack: { config: webpackConfigPath },
		},

		// https://github.com/meteorlxy/eslint-plugin-prettier-vue
		"prettier-vue": {
			// Use Prettier to format the HTML, CSS, and JS blocks of .vue single-file components
			SFCBlocks: {
				template: true,
				style: true,
				script: true,
			},
		},
	},
	ignorePatterns: [
		// Ignore generated directories
		"node_modules/",
		"dist/",
		"pkg/",
		"wasm/pkg/",

		// Don't ignore JS and TS dotfiles in this folder
		"!.*.js",
		"!.*.ts",
	],
	rules: {
		// Standard ESLint config
		indent: ["error", "tab", { SwitchCase: 1 }],
		quotes: ["error", "double"],
		camelcase: ["error", { properties: "always" }],
		"linebreak-style": ["error", "unix"],
		"eol-last": ["error", "always"],
		"max-len": ["error", { code: 200, tabWidth: 4 }],
		"no-console": process.env.NODE_ENV === "production" ? "warn" : "off",
		"no-debugger": process.env.NODE_ENV === "production" ? "warn" : "off",
		"no-param-reassign": ["error", { props: false }],
		"no-bitwise": "off",
		"no-use-before-define": "off",
		"no-shadow": "off",

		// TypeScript plugin config
		"@typescript-eslint/camelcase": "off",
		"@typescript-eslint/no-use-before-define": "off",
		"@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
		"@typescript-eslint/no-shadow": "error",

		// Import plugin config (used to intelligently validate module import statements)
		"import/prefer-default-export": "off",

		// Prettier plugin config (used to enforce HTML, CSS, and JS formatting styles as an ESLint plugin, where fixes are reported to ESLint to be applied when linting)
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
