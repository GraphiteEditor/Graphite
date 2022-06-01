module.exports = {
	root: true,
	env: {
		browser: true,
		node: true,
		es2020: true,
	},
	extends: [
		"plugin:vue/vue3-essential",
		// Vue-compatible JS defaults
		"@vue/airbnb",
		// Vue-compatible Prettier defaults
		"plugin:prettier-vue/recommended",
		// General Prettier defaults
		"prettier",
		// "eslint:recommended",
	],
	settings: {
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
	parserOptions: {
		// parser: "@babel/eslint-parser",
		ecmaVersion: 2020,
	},
	ignorePatterns: [
		// Ignore generated directories
		"node_modules/",
		"dist/",
		"pkg/",
		"wasm/pkg/",
	],
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
		// TODO: Renable the below rule
		// "no-restricted-imports": ["error", { patterns: [".*", "!@/*"] }],

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
				pathGroups: [
					{
						pattern: "**/*.vue",
						group: "unknown",
						position: "after",
					},
				],
			},
		],

		// Prettier plugin config (used to enforce HTML, CSS, and JS formatting styles as an ESLint plugin, where fixes are reported to ESLint to be applied when linting)
		"prettier-vue/prettier": [
			"error",
			{
				tabWidth: 4,
				tabs: true,
				printWidth: 200,
			},
		],

		// Vue plugin config (used to validate Vue single-file components)
		"vue/multi-word-component-names": "off",

		// Vue Accessibility plugin config (included by airbnb defaults but undesirable for a web app project)
		"vuejs-accessibility/form-control-has-label": "off",
		"vuejs-accessibility/label-has-for": "off",
		"vuejs-accessibility/click-events-have-key-events": "off",
	},
};
