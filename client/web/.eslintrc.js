module.exports = {
	root: true,
	env: {
		node: true,
	},
	extends: [
		"plugin:vue/vue3-essential",
		"@vue/airbnb",
		"@vue/typescript/recommended",
	],
	parserOptions: {
		ecmaVersion: 2020,
	},
	rules: {
		"no-console": process.env.NODE_ENV === "production" ? "warn" : "off",
		"no-debugger": process.env.NODE_ENV === "production" ? "warn" : "off",
		"no-tabs": 0,
		"max-len": 0,
		"linebreak-style": ["error", "unix"],
		indent: ["error", "tab"],
		quotes: ["error", "double"],
		camelcase: ["error", { ignoreImports: true, ignoreDestructuring: true }],
	},
};
