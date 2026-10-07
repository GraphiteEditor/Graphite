// The website follows the frontend's lint rules, checked by the ESLint the frontend installs
// eslint-disable-next-line no-restricted-imports, import/no-relative-packages -- The frontend's config is in another package, which isn't installed here
import frontendConfig from "../frontend/eslint.config.js";

export default [
	...frontendConfig,
	{
		ignores: [
			// Ignore generated directories
			"public/",
			"static/wasm/",
			// Ignore vendored code
			"static/*.js",
		],
	},
];
