import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default {
	preprocess: [vitePreprocess()],
	compilerOptions: /** @type {import("svelte/compiler").ModuleCompileOptions} */ ({
		warningFilter: (warning) => !warning.code.startsWith("a11y_") && !["css_unused_selector"].includes(warning.code),
	}),
};
