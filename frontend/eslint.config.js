import js from "@eslint/js";
import { defineConfig, globalIgnores } from "eslint/config";
import * as pluginImport from "eslint-plugin-import";
import pluginPrettier from "eslint-plugin-prettier";
import pluginSvelte from "eslint-plugin-svelte";
import globals from "globals";
import ts from "typescript-eslint";

const importOrder = {
	alphabetize: { order: "asc", caseInsensitive: true },
	"newlines-between": "never",
	warnOnUnassignedImports: true,
	pathGroups: [
		{ pattern: "/**", group: "internal" },
		{ pattern: "/../**", group: "internal" },
	],
	pathGroupsExcludedImportTypes: [],
};

export default defineConfig([
	js.configs.recommended,
	ts.configs.recommended,
	pluginImport.flatConfigs.recommended,
	pluginImport.flatConfigs.typescript,
	pluginSvelte.configs["flat/recommended"],
	pluginSvelte.configs["flat/prettier"],
	globalIgnores([
		// Ignore generated directories
		"**/node_modules/",
		"**/dist/",
		"**/pkg/",
		"wasm/pkg/",
		// Don't ignore JS and TS dotfiles in this folder
		"!**/.*.js",
		"!**/.*.ts",
	]),
	{
		plugins: {
			prettier: pluginPrettier,
		},
		settings: {
			"import/parsers": { "@typescript-eslint/parser": [".ts"] },
			"import/resolver": { typescript: true, node: true },
		},
		languageOptions: {
			globals: {
				...globals.browser,
				...globals.node,
			},
		},
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
			"no-debugger": "warn",
			"no-param-reassign": ["error", { props: false }],
			"no-bitwise": "off",
			"no-shadow": "off",
			"no-use-before-define": "off",
			"no-restricted-imports": [
				"error",
				{
					patterns: [
						{ group: ["./**", "../**"], message: "\nImports must be absolute (e.g. '/src/<subpath>') not relative (e.g. './<subpath>')." },
						{ group: ["src/**", "assets/**", "wrapper/**"], message: "\nLocal imports must start with '/' (e.g. '/src/<subpath>' not 'src/<subpath>')." },
					],
				},
			],

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
			"@typescript-eslint/no-non-null-assertion": "error",
			"@typescript-eslint/consistent-type-imports": "error",
			"@typescript-eslint/consistent-type-definitions": ["error", "type"],
			"@typescript-eslint/consistent-type-assertions": ["error", { assertionStyle: "never" }],
			"@typescript-eslint/consistent-indexed-object-style": ["error", "record"],
			"@typescript-eslint/consistent-generic-constructors": ["error", "constructor"],
			"@typescript-eslint/no-restricted-types": ["error", { types: { null: "Use `undefined` instead." } }],

			// Prettier plugin config (for validating and fixing formatting)
			"prettier/prettier": "error",

			// Svelte plugin config (for validating Svelte-specific syntax)
			"svelte/no-at-html-tags": "off",
			"svelte/no-useless-mustaches": "off",
			"svelte/valid-compile": ["error", { ignoreWarnings: true }],
			"svelte/require-each-key": "off", // TODO: Remove this rule and fix the places where it's violated

			// Import plugin config (for intelligently validating module import statements)
			"import/consistent-type-specifier-style": ["error", "prefer-top-level"],
			"import/no-unresolved": "error",
			// `no-duplicates` disabled due to <https://github.com/import-js/eslint-plugin-import/issues/1479#issuecomment-1789527447>. Reenable if that issue gets fixed.
			"import/no-duplicates": "off",
			"import/prefer-default-export": "off",
			"import/no-relative-packages": "error",
			"import/no-named-as-default-member": "off",
			"import/order": ["error", importOrder],
		},
	},
	{
		files: ["**/icons.ts"],
		rules: { "import/order": ["error", { ...importOrder, "newlines-between": "ignore" }] },
	},
	{
		files: ["**/*.svelte"],
		languageOptions: {
			// Parse the `<script>` in `.svelte` as TypeScript by adding the following configuration.
			parserOptions: {
				projectService: true,
				parser: ts.parser,
				extraFileExtensions: [".svelte"],
			},
		},
	},
	{
		files: ["**/*.js"],
		extends: [ts.configs.disableTypeChecked],
	},
]);
