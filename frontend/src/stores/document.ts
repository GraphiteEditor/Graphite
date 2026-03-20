import { tick } from "svelte";
import { writable } from "svelte/store";
import type { Writable } from "svelte/store";

import type { Layout } from "@graphite/../wasm/pkg/graphite_wasm";
import type { Editor } from "@graphite/editor";
import { patchLayout } from "@graphite/utility-functions/widgets";

export type DocumentStore = ReturnType<typeof createDocumentStore>;

type DocumentStoreState = {
	toolOptionsLayout: Layout;
	documentBarLayout: Layout;
	toolShelfLayout: Layout;
	workingColorsLayout: Layout;
	nodeGraphControlBarLayout: Layout;
	graphViewOverlayOpen: boolean;
	fadeArtwork: number;
};
const initialState: DocumentStoreState = {
	toolOptionsLayout: [],
	documentBarLayout: [],
	toolShelfLayout: [],
	workingColorsLayout: [],
	nodeGraphControlBarLayout: [],
	graphViewOverlayOpen: false,
	fadeArtwork: 100,
};

let editorRef: Editor | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<DocumentStoreState> = import.meta.hot?.data?.store || writable<DocumentStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createDocumentStore(editor: Editor) {
	editorRef = editor;

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
	editor.subscriptions.subscribeLayoutUpdate("NodeGraphControlBar", async (data) => {
		await tick();

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

	return { subscribe };
}

export function destroyDocumentStore() {
	const editor = editorRef;
	if (!editor) return;

	editor.subscriptions.unsubscribeFrontendMessage("UpdateGraphFadeArtwork");
	editor.subscriptions.unsubscribeFrontendMessage("UpdateGraphViewOverlay");
	editor.subscriptions.unsubscribeLayoutUpdate("ToolOptions");
	editor.subscriptions.unsubscribeLayoutUpdate("DocumentBar");
	editor.subscriptions.unsubscribeLayoutUpdate("ToolShelf");
	editor.subscriptions.unsubscribeLayoutUpdate("WorkingColors");
	editor.subscriptions.unsubscribeLayoutUpdate("NodeGraphControlBar");
}
