import js from "@eslint/js";
import { defineConfig, globalIgnores } from "eslint/config";
import * as pluginImport from "eslint-plugin-import";
import pluginPrettier from "eslint-plugin-prettier";
import globals from "globals";
import ts from "typescript-eslint";

export default defineConfig([
	js.configs.recommended,
	ts.configs.recommended,
	pluginImport.flatConfigs.recommended,
	pluginImport.flatConfigs.typescript,
	globalIgnores([
		// Ignore generated directories
		"node_modules/",
		"public/",
		// Ignore vendored code
		"static/*.js",
		// Don't ignore JS and TS dotfiles in this folder
		"!.*.js",
		"!.*.ts",
	]),
	{
		plugins: {
			prettier: pluginPrettier,
		},
		settings: {
			"import/parsers": { "@typescript-eslint/parser": [".ts", ".js"] },
			"import/resolver": { typescript: true, node: true },
		},
		languageOptions: {
			parserOptions: {
				project: "./tsconfig.json",
			},
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
			curly: ["error", "multi-line"],
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
			"no-restricted-imports": ["error", { patterns: [".*"] }],

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

			// Import plugin config (for intelligently validating module import statements)
			"import/no-unresolved": "error",
			"import/prefer-default-export": "off",
			"import/no-relative-packages": "error",
			"import/no-named-as-default-member": "off",
			"import/order": [
				"error",
				{
					alphabetize: { order: "asc", caseInsensitive: true },
					warnOnUnassignedImports: true,
					"newlines-between": "always-and-inside-groups",
				},
			],

			// Prettier plugin config (for validating and fixing formatting)
			"prettier/prettier": [
				"error",
				{
					tabWidth: 4,
					tabs: true,
					printWidth: 200,
				},
			],
		},
	},
]);
