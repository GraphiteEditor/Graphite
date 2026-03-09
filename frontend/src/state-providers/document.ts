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

	function destroy() {
		editor.subscriptions.unsubscribeFrontendMessage("UpdateGraphFadeArtwork");
		editor.subscriptions.unsubscribeFrontendMessage("UpdateGraphViewOverlay");
		editor.subscriptions.unsubscribeLayoutUpdate("ToolOptions");
		editor.subscriptions.unsubscribeLayoutUpdate("DocumentBar");
		editor.subscriptions.unsubscribeLayoutUpdate("ToolShelf");
		editor.subscriptions.unsubscribeLayoutUpdate("WorkingColors");
		editor.subscriptions.unsubscribeLayoutUpdate("NodeGraphControlBar");
	}

	return {
		subscribe,
		destroy,
	};
}
export type DocumentState = ReturnType<typeof createDocumentState>;

// This store is bound to the component tree via setContext() and can't be hot-replaced, so we force a full page reload
import.meta.hot?.accept(() => location.reload());
