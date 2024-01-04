module.exports = {
	root: true,
	env: { browser: true, node: true },
	extends: ["eslint:recommended", "plugin:import/recommended", "plugin:@typescript-eslint/recommended", "plugin:import/typescript", "prettier"],
	plugins: ["import", "@typescript-eslint", "prettier"],
	settings: {
		"import/parsers": { "@typescript-eslint/parser": [".ts"] },
		"import/resolver": { typescript: true, node: true },
	},
	parser: "@typescript-eslint/parser",
	parserOptions: {
		ecmaVersion: "latest",
		project: "./tsconfig.json",
	},
	overrides: [
		{
			extends: ["plugin:@typescript-eslint/disable-type-checked"],
			files: [".eslintrc.cjs"],
		},
	],
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
		// Standard ESLint config (for ordinary JS syntax linting)
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

		// TypeScript plugin config (for TS-specific linting)
		"@typescript-eslint/indent": "off",
		"@typescript-eslint/camelcase": "off",
		"@typescript-eslint/no-use-before-define": "off",
		"@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_", ignoreRestSiblings: true }],
		"@typescript-eslint/consistent-type-imports": "error",
		"@typescript-eslint/consistent-type-definitions": ["error", "type"],
		"@typescript-eslint/consistent-type-assertions": ["error", { assertionStyle: "as", objectLiteralTypeAssertions: "never" }],
		"@typescript-eslint/consistent-indexed-object-style": ["error", "record"],
		"@typescript-eslint/consistent-generic-constructors": ["error", "constructor"],
		"@typescript-eslint/ban-types": ["error", { types: { null: "Use `undefined` instead." } }],

		// Prettier plugin config (for validating and fixing formatting)
		"prettier/prettier": "error",

		// Import plugin config (for intelligently validating module import statements)
		"import/no-unresolved": "error",
		"import/prefer-default-export": "off",
		"import/no-relative-packages": "error",
		"import/order": [
			"error",
			{
				alphabetize: { order: "asc", caseInsensitive: true },
				"newlines-between": "always-and-inside-groups",
				warnOnUnassignedImports: true,
			},
		],
	},
};
