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
		// JS defaults
		"airbnb-base",
		// General Prettier defaults
		"prettier",
	],
	settings: {
		// https://github.com/import-js/eslint-plugin-import#resolvers
		"import/resolver": {
			// `node` must be listed first!
			node: {},
		},
	},
	ignorePatterns: [
		// Ignore generated directories
		"node_modules/",
		"public/",

		// Don't ignore JS and TS dotfiles in this folder
		"!.*.js",
		"!.*.ts",
	],
	plugins: ["prettier"],
	rules: {
		// Standard ESLint config
		indent: "off",
		quotes: ["error", "double"],
		camelcase: ["error", { properties: "always" }],
		"linebreak-style": ["error", "unix"],
		"eol-last": ["error", "always"],
		"max-len": ["error", { code: 200, tabWidth: 4 }],
		"prefer-destructuring": "off",
		"no-console": "warn",
		"no-debugger": "warn",
		"no-param-reassign": ["error", { props: false }],
		"no-bitwise": "off",
		"no-shadow": "off",
		"no-use-before-define": "off",
		"no-restricted-imports": ["error", { patterns: [".*", "!@graphite/*"] }],

		// Import plugin config (used to intelligently validate module import statements)
		"import/prefer-default-export": "off",
		"import/no-relative-packages": "error",
		"import/order": [
			"error",
			{
				alphabetize: {
					order: "asc",
					caseInsensitive: true,
				},
				warnOnUnassignedImports: true,
				"newlines-between": "always-and-inside-groups",
				pathGroups: [],
			},
		],

		// Prettier plugin config (used to enforce HTML, CSS, and JS formatting styles as an ESLint plugin, where fixes are reported to ESLint to be applied when linting)
		"prettier/prettier": [
			"error",
			{
				tabWidth: 4,
				tabs: true,
				printWidth: 200,
			},
		],
	},
	overrides: [
		{
			files: ["*.js"],
			rules: {
				"@typescript-eslint/explicit-function-return-type": ["off"],
			},
		},
	],
};
