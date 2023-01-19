module.exports = {
	root: true,
	env: {
		browser: true,
		es2020: true,
	},
	parserOptions: {
		ecmaVersion: 2020,
		sourceType: "module",
	},
	plugins: [
		"svelte3",
		"@typescript-eslint",
	],
	extends: [
		"eslint:recommended",
		"plugin:@typescript-eslint/recommended",
		// General Prettier defaults
		"prettier",
	],
	settings: {
		"svelte3/typescript": () => require("typescript"),
	},
	ignorePatterns: [
		// Ignore generated directories
		"node_modules/",
		"dist/",
		"pkg/",
		"wasm/pkg/",

		// Don't ignore JS and TS dotfiles in this folder
		// "!.*.js",
		// "!.*.ts",
	],
	rules: {
		// Standard ESLint config
		indent: "off",
		quotes: ["error", "double", { allowTemplateLiterals: true }],
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
		"no-restricted-imports": ["error", { patterns: [".*", "!@/*"] }],

		// TypeScript plugin config
		"@typescript-eslint/indent": "off",
		"@typescript-eslint/camelcase": "off",
		"@typescript-eslint/no-use-before-define": "off",
		"@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_", ignoreRestSiblings: true }],
		"@typescript-eslint/explicit-function-return-type": "error",
		"@typescript-eslint/consistent-type-imports": "error",
		"@typescript-eslint/consistent-type-definitions": ["error", "type"],
		"@typescript-eslint/consistent-type-assertions": ["error", { assertionStyle: "as", objectLiteralTypeAssertions: "never" }],
		"@typescript-eslint/consistent-indexed-object-style": ["error", "record"],
		"@typescript-eslint/consistent-generic-constructors": ["error", "constructor"],
		"@typescript-eslint/ban-types": ["error", { types: { null: "Use `undefined` instead." } }],

		// Import plugin config (used to intelligently validate module import statements)
		"import/prefer-default-export": "off",
		// "import/no-relative-packages": "error",
		// "import/order": [
		// 	"error",
		// 	{
		// 		alphabetize: {
		// 			order: "asc",
		// 			caseInsensitive: true,
		// 		},
		// 		warnOnUnassignedImports: true,
		// 		"newlines-between": "always-and-inside-groups",
		// 		pathGroups: [
		// 			{
		// 				pattern: "**/*.vue",
		// 				group: "unknown",
		// 				position: "after",
		// 			},
		// 		],
		// 	},
		// ],
	},
	overrides: [
		{
			files: ["**.svelte"],
			processor: "svelte3/svelte3"
		},
		{
			files: ["*.js"],
			rules: {
				"@typescript-eslint/explicit-function-return-type": ["off"],
			},
		},
	],
};
