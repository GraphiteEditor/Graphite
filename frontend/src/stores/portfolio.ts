import { SvelteMap } from "svelte/reactivity";
import { get, writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { acceptStringFromFilters, downloadFile, upload } from "/src/utility-functions/files";
import { createLayout } from "/src/utility-functions/layout-state.svelte";
import { rasterizeSVGCanvas } from "/src/utility-functions/rasterization";
import { deepEqual, patchLayout } from "/src/utility-functions/widgets";
import type { EditorWrapper, DocumentInfo, LayerPanelEntry, LayerStructureEntry, Layout, WorkspacePanelLayout } from "/wrapper/pkg/graphite_wasm_wrapper";

export type PortfolioStore = ReturnType<typeof createPortfolioStore>;

type PortfolioStoreState = {
	unsaved: boolean;
	documents: DocumentInfo[];
	activeDocumentIndex: number;
	panelLayout: WorkspacePanelLayout;
	layerCache: SvelteMap<string, LayerPanelEntry>;
	layerStructure: LayerStructureEntry[];
};
const initialState: PortfolioStoreState = {
	unsaved: false,
	documents: [],
	activeDocumentIndex: 0,
	panelLayout: {},
	layerCache: new SvelteMap<string, LayerPanelEntry>(),
	layerStructure: [],
};

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

// The layers (by ID string) the Layers panel has rows rendered for, whose details are the only ones it keeps
let layersWithDetails = new Set<string>();

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<PortfolioStoreState> = import.meta.hot?.data?.store || writable<PortfolioStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export const welcomeScreenButtonsLayout = persistedLayout("welcomeScreenButtonsLayout");
export const inspectorLayout = persistedLayout("inspectorLayout");
export const dataPanelLayout = persistedLayout("dataPanelLayout");
export const dataPanelContextLayout = persistedLayout("dataPanelContextLayout");
export const dataPanelPathLayout = persistedLayout("dataPanelPathLayout");
export const layersPanelControlBarLeftLayout = persistedLayout("layersPanelControlBarLeftLayout");
export const layersPanelBottomBarLeftLayout = persistedLayout("layersPanelBottomBarLeftLayout");
export const layersPanelBottomBarRightLayout = persistedLayout("layersPanelBottomBarRightLayout");

// Each panel layout is its own reactive layout, patched in place so an update only re-renders the widgets it changes.
// Putting them at module scope (not inside the component) lets them survive a Svelte remount during a
// panel-tree restructure, since the backend's diff-based updates aren't re-sent on subscribe.
function persistedLayout(name: string): Layout {
	const layout: Layout = import.meta.hot?.data?.[name] || createLayout();
	if (import.meta.hot) import.meta.hot.data[name] = layout;
	return layout;
}

export function createPortfolioStore(subscriptions: SubscriptionsRouter, editor: EditorWrapper) {
	destroyPortfolioStore();

	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("UpdateOpenDocumentsList", (data) => {
		update((state) => {
			state.documents = data.openDocuments;
			if (state.documents.length === 0) state.activeDocumentIndex = 0;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateActiveDocument", (data) => {
		update((state) => {
			// Assume we receive a correct document id
			const activeId = state.documents.findIndex((doc) => doc.id === data.documentId);
			state.activeDocumentIndex = activeId;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("TriggerFetchAndOpenDocument", async (data) => {
		try {
			const url = new URL(`demo-artwork/${data.filename}`, document.location.href);
			const response = await fetch(url);
			editor.ingestPicked(data.filename, "", await response.bytes(), "Open");
		} catch {
			// Needs to be delayed until the end of the current call stack so the existing demo artwork dialog can be closed first, otherwise this dialog won't show
			setTimeout(() => {
				editor.errorDialog("Failed to open document", "The file could not be reached over the internet. You may be offline, or it may be missing.");
			}, 0);
		}
	});

	subscriptions.subscribeFrontendMessage("TriggerBrowse", async ({ options, action }) => {
		const accept = acceptStringFromFilters(options.filters);
		const files = options.multiple ? await upload(accept, "data", true) : [await upload(accept, "data")];
		files.forEach((file) => editor.ingestPicked(file.filename, file.type, file.content, action));
	});

	subscriptions.subscribeFrontendMessage("TriggerSaveDocument", (data) => {
		downloadFile(data.name, data.content);
	});

	subscriptions.subscribeFrontendMessage("TriggerSaveFile", (data) => {
		downloadFile(data.name, data.content);
	});

	subscriptions.subscribeFrontendMessage("TriggerExportImage", async (data) => {
		const { svg, name, fileType, size } = data;

		// Rasterize the SVG and hand its pixels back to the editor for encoding
		try {
			const canvas = await rasterizeSVGCanvas(svg, size[0], size[1]);
			const pixels = canvas.getContext("2d")?.getImageData(0, 0, canvas.width, canvas.height);
			if (!pixels) return;

			editor.saveRasterizedExport(name, fileType, pixels.width, pixels.height, new Uint8Array(pixels.data.buffer));
		} catch {
			// Fail silently if there's an error rasterizing the SVG, such as a zero-sized image
		}
	});

	subscriptions.subscribeFrontendMessage("UpdateWorkspacePanelLayout", (data) => {
		update((state) => {
			state.panelLayout = data.panelLayout;
			return state;
		});
	});

	subscriptions.subscribeLayoutUpdate("WelcomeScreenButtons", (data) => {
		patchLayout(welcomeScreenButtonsLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("Inspector", (data) => {
		patchLayout(inspectorLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("DataPanel", (data) => {
		patchLayout(dataPanelLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("DataPanelContext", (data) => {
		patchLayout(dataPanelContextLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("DataPanelPath", (data) => {
		patchLayout(dataPanelPathLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("LayersPanelControlLeftBar", (data) => {
		patchLayout(layersPanelControlBarLeftLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("LayersPanelBottomLeftBar", (data) => {
		patchLayout(layersPanelBottomBarLeftLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("LayersPanelBottomRightBar", (data) => {
		patchLayout(layersPanelBottomBarRightLayout, data);
	});

	subscriptions.subscribeFrontendMessage("UpdateDocumentLayerStructure", (data) => {
		// The backend resends the structure whenever the graph changes, so an unchanged one is skipped instead of rebuilding the Layers panel
		if (deepEqual(get(store).layerStructure, data.layerStructure)) return;

		update((state) => {
			state.layerStructure = data.layerStructure;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateDocumentLayerDetails", (data) => {
		// Details sent before their layer's row left view would otherwise be kept without the backend updating them
		if (!layersWithDetails.has(String(data.data.id))) return;

		update((state) => {
			state.layerCache.set(String(data.data.id), data.data);
			return state;
		});
	});

	return { subscribe };
}

// Keeps only the given layers' details (by ID string), the only ones the backend keeps current, or none if those kept were for another document
export function retainLayerDetails(layerIds: Set<string>, otherDocument: boolean) {
	layersWithDetails = layerIds;
	update((state) => {
		Array.from(state.layerCache.keys()).forEach((id) => {
			if (otherDocument || !layerIds.has(id)) state.layerCache.delete(id);
		});
		return state;
	});
}

export function destroyPortfolioStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("UpdateOpenDocumentsList");
	subscriptions.unsubscribeFrontendMessage("UpdateActiveDocument");
	subscriptions.unsubscribeFrontendMessage("TriggerFetchAndOpenDocument");
	subscriptions.unsubscribeFrontendMessage("TriggerBrowse");
	subscriptions.unsubscribeFrontendMessage("TriggerSaveDocument");
	subscriptions.unsubscribeFrontendMessage("TriggerSaveFile");
	subscriptions.unsubscribeFrontendMessage("TriggerExportImage");
	subscriptions.unsubscribeFrontendMessage("UpdateWorkspacePanelLayout");
	subscriptions.unsubscribeLayoutUpdate("WelcomeScreenButtons");
	subscriptions.unsubscribeLayoutUpdate("Inspector");
	subscriptions.unsubscribeLayoutUpdate("DataPanel");
	subscriptions.unsubscribeLayoutUpdate("DataPanelContext");
	subscriptions.unsubscribeLayoutUpdate("DataPanelPath");
	subscriptions.unsubscribeLayoutUpdate("LayersPanelControlLeftBar");
	subscriptions.unsubscribeLayoutUpdate("LayersPanelBottomLeftBar");
	subscriptions.unsubscribeLayoutUpdate("LayersPanelBottomRightBar");
	subscriptions.unsubscribeFrontendMessage("UpdateDocumentLayerStructure");
	subscriptions.unsubscribeFrontendMessage("UpdateDocumentLayerDetails");
}
