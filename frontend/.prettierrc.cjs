module.exports = {
	"singleQuote": false,
	"useTabs": true,
	"tabWidth": 4,
	"printWidth": 200,
	"plugins": [
		import("prettier-plugin-svelte")
	],
	"overrides": [
		{
			"files": [
				"*.svelte"
			],
			"options": {
				"parser": "svelte"
			}
		}
	]
}
