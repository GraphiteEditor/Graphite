import { tick } from "svelte";
import { writable } from "svelte/store";

import type { Layout } from "@graphite/../wasm/pkg/graphite_wasm";
import type { Editor } from "@graphite/editor";
import { patchLayout } from "@graphite/utility-functions/widgets";

export function createDocumentState(editor: Editor) {
	const state = writable<{
		toolOptionsLayout: Layout;
		documentBarLayout: Layout;
		toolShelfLayout: Layout;
		workingColorsLayout: Layout;
		nodeGraphControlBarLayout: Layout;
		graphViewOverlayOpen: boolean;
		fadeArtwork: number;
	}>({
		toolOptionsLayout: [],
		documentBarLayout: [],
		toolShelfLayout: [],
		workingColorsLayout: [],
		nodeGraphControlBarLayout: [],
		graphViewOverlayOpen: false,
		fadeArtwork: 100,
	});
	const { subscribe, update } = state;

	// Update layouts
	editor.subscriptions.subscribeFrontendMessage("UpdateGraphFadeArtwork", (data) => {
		update((state) => {
			state.fadeArtwork = data.percentage;
			return state;
		});
	});
	editor.subscriptions.subscribeLayoutUpdate("ToolOptions", async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.toolOptionsLayout, data);
			return state;
		});
	});
	editor.subscriptions.subscribeLayoutUpdate("DocumentBar", async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.documentBarLayout, data);
			return state;
		});
	});
	editor.subscriptions.subscribeLayoutUpdate("ToolShelf", async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.toolShelfLayout, data);
			return state;
		});
	});
	editor.subscriptions.subscribeLayoutUpdate("WorkingColors", async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.workingColorsLayout, data);
			return state;
		});
	});
	editor.subscriptions.subscribeLayoutUpdate("NodeGraphControlBar", (data) => {
		update((state) => {
			patchLayout(state.nodeGraphControlBarLayout, data);
			return state;
		});
	});

	// Show or hide the graph view overlay
	editor.subscriptions.subscribeFrontendMessage("UpdateGraphViewOverlay", (data) => {
		update((state) => {
			state.graphViewOverlayOpen = data.open;
			return state;
		});
	});

	return {
		subscribe,
	};
}
export type DocumentState = ReturnType<typeof createDocumentState>;
