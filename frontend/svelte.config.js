import { sveltePreprocess } from "svelte-preprocess";

export default {
	preprocess: sveltePreprocess(),
	compilerOptions: /** @type {import("svelte/compiler").ModuleCompileOptions} */ ({
		warningFilter: (warning) => !warning.code.startsWith("a11y_") && !["css_unused_selector"].includes(warning.code),
	}),
};
