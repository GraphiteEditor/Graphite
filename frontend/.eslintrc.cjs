module.exports = {
	root: true,
	env: { browser: true, node: true },
	extends: [
		"eslint:recommended",
		"plugin:import/recommended",
		"plugin:@typescript-eslint/recommended",
		"plugin:import/typescript",
		"plugin:svelte/recommended",
		"plugin:svelte/prettier",
		"prettier",
	],
	plugins: ["import", "@typescript-eslint", "prettier"],
	settings: {
		"import/parsers": { "@typescript-eslint/parser": [".ts"] },
		"import/resolver": { typescript: true, node: true },
	},
	parser: "@typescript-eslint/parser",
	parserOptions: {
		ecmaVersion: "latest",
		project: "./tsconfig.json",
		extraFileExtensions: [".svelte", ".svelte.ts"],
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
	overrides: [
		{
			files: ["*.svelte", "*.svelte.ts"],
			parser: "svelte-eslint-parser",
			// Parse the `<script>` in `.svelte` as TypeScript by adding the following configuration.
			parserOptions: { parser: "@typescript-eslint/parser" },
		},
		{
			extends: ["plugin:@typescript-eslint/disable-type-checked"],
			files: ["./*.js", "./*.cjs"],
		},
	],
	rules: {
		// Standard ESLint config (for ordinary JS syntax linting)
		indent: "off",
		quotes: ["error", "double", { allowTemplateLiterals: true }],
		camelcase: ["error", { properties: "always" }],
		"linebreak-style": ["error", "unix"],
		"eol-last": ["error", "always"],
		"max-len": ["error", { code: 200, tabWidth: 4, ignorePattern: `d="([\\s\\S]*?)"` }],
		"prefer-destructuring": "off",
		"no-console": "warn",
		// eslint recommended for ts project
		// https://typescript-eslint.io/troubleshooting/faqs/eslint/#i-get-errors-from-the-no-undef-rule-about-global-variables-not-being-defined-even-though-there-are-no-typescript-errors
		"no-undef": "off",
		"no-debugger": "warn",
		"no-param-reassign": ["error", { props: false }],
		"no-bitwise": "off",
		"no-shadow": "off",
		"no-use-before-define": "off",
		"no-restricted-imports": ["error", { patterns: [".*", "!@graphite/*"] }],

		// TypeScript plugin config (for TS-specific linting)
		"@typescript-eslint/indent": "off",
		"@typescript-eslint/camelcase": "off",
		"@typescript-eslint/no-use-before-define": "off",
		"@typescript-eslint/no-unused-vars": [
			"error",
			{
				args: "all",
				argsIgnorePattern: "^_",
				caughtErrors: "all",
				caughtErrorsIgnorePattern: "^_",
				destructuredArrayIgnorePattern: "^_",
				varsIgnorePattern: "^_",
				ignoreRestSiblings: true,
			},
		],
		"@typescript-eslint/consistent-type-imports": "error",
		"@typescript-eslint/consistent-type-definitions": ["error", "type"],
		"@typescript-eslint/consistent-type-assertions": ["error", { assertionStyle: "as", objectLiteralTypeAssertions: "never" }],
		"@typescript-eslint/consistent-indexed-object-style": ["error", "record"],
		"@typescript-eslint/consistent-generic-constructors": ["error", "constructor"],
		"@typescript-eslint/no-restricted-types": ["error", { types: { null: "Use `undefined` instead." } }],

		// Prettier plugin config (for validating and fixing formatting)
		"prettier/prettier": "error",

		// Svelte plugin config (for validating Svelte-specific syntax)
		"svelte/no-at-html-tags": "off",
		"svelte/valid-compile": ["error", { ignoreWarnings: true }],

		// Import plugin config (for intelligently validating module import statements)
		"import/no-unresolved": "error",
		// `no-duplicates` disabled due to <https://github.com/import-js/eslint-plugin-import/issues/1479#issuecomment-1789527447>. Reenable if that issue gets fixed.
		"import/no-duplicates": "off",
		"import/prefer-default-export": "off",
		"import/no-relative-packages": "error",
		"import/order": [
			"error",
			{
				alphabetize: { order: "asc", caseInsensitive: true },
				pathGroups: [{ pattern: "**/*.svelte", group: "unknown", position: "after" }],
				"newlines-between": "always-and-inside-groups",
				warnOnUnassignedImports: true,
			},
		],
	},
};
