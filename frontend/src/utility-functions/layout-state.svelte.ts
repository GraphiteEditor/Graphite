import type { Layout } from "/wrapper/pkg/graphite_wasm_wrapper";

// A layout held in a deeply reactive proxy, so `patchLayout` changing it in place re-runs only the effects that read the changed widgets
export function createLayout(): Layout {
	const layout = $state<Layout>([]);
	return layout;
}
