import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { downloadFile, downloadFileBlob, upload } from "/src/utility-functions/files";
import { rasterizeSVG } from "/src/utility-functions/rasterization";
import type { EditorWrapper, OpenDocument } from "/wrapper/pkg/graphite_wasm_wrapper";

export type PortfolioStore = ReturnType<typeof createPortfolioStore>;

type PortfolioStoreState = {
	unsaved: boolean;
	documents: OpenDocument[];
	activeDocumentIndex: number;
	dataPanelOpen: boolean;
	propertiesPanelOpen: boolean;
	layersPanelOpen: boolean;
};
const initialState: PortfolioStoreState = {
	unsaved: false,
	documents: [],
	activeDocumentIndex: 0,
	dataPanelOpen: false,
	propertiesPanelOpen: true,
	layersPanelOpen: true,
};

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<PortfolioStoreState> = import.meta.hot?.data?.store || writable<PortfolioStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createPortfolioStore(subscriptions: SubscriptionsRouter, editor: EditorWrapper) {
	destroyPortfolioStore();

	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("UpdateOpenDocumentsList", (data) => {
		update((state) => {
			state.documents = data.openDocuments;
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
			editor.openFile(data.filename, await response.bytes());
		} catch {
			// Needs to be delayed until the end of the current call stack so the existing demo artwork dialog can be closed first, otherwise this dialog won't show
			setTimeout(() => {
				editor.errorDialog("Failed to open document", "The file could not be reached over the internet. You may be offline, or it may be missing.");
			}, 0);
		}
	});

	subscriptions.subscribeFrontendMessage("TriggerOpen", async () => {
		const data = await upload(`image/*,.${editor.fileExtension()}`, "data");
		editor.openFile(data.filename, data.content);
	});

	subscriptions.subscribeFrontendMessage("TriggerImport", async () => {
		// TODO: Use the same `accept` string as in the `TriggerOpen` handler once importing Graphite documents as nodes is supported
		const data = await upload("image/*", "data");
		editor.importFile(data.filename, data.content);
	});

	subscriptions.subscribeFrontendMessage("TriggerSaveDocument", (data) => {
		downloadFile(data.name, data.content);
	});

	subscriptions.subscribeFrontendMessage("TriggerSaveFile", (data) => {
		downloadFile(data.name, data.content);
	});

	subscriptions.subscribeFrontendMessage("TriggerExportImage", async (data) => {
		const { svg, name, mime, size } = data;

		// Fill the canvas with white if it'll be a JPEG (which does not support transparency and defaults to black)
		const backgroundColor = mime.endsWith("jpeg") ? "white" : undefined;

		// Rasterize the SVG to an image file
		try {
			const blob = await rasterizeSVG(svg, size[0], size[1], mime, backgroundColor);

			// Have the browser download the file to the user's disk
			downloadFileBlob(name, blob);
		} catch {
			// Fail silently if there's an error rasterizing the SVG, such as a zero-sized image
		}
	});

	subscriptions.subscribeFrontendMessage("UpdateDataPanelState", async (data) => {
		update((state) => {
			state.dataPanelOpen = data.open;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdatePropertiesPanelState", async (data) => {
		update((state) => {
			state.propertiesPanelOpen = data.open;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateLayersPanelState", async (data) => {
		update((state) => {
			state.layersPanelOpen = data.open;
			return state;
		});
	});

	return { subscribe };
}

export function destroyPortfolioStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("UpdateOpenDocumentsList");
	subscriptions.unsubscribeFrontendMessage("UpdateActiveDocument");
	subscriptions.unsubscribeFrontendMessage("TriggerFetchAndOpenDocument");
	subscriptions.unsubscribeFrontendMessage("TriggerOpen");
	subscriptions.unsubscribeFrontendMessage("TriggerImport");
	subscriptions.unsubscribeFrontendMessage("TriggerSaveDocument");
	subscriptions.unsubscribeFrontendMessage("TriggerSaveFile");
	subscriptions.unsubscribeFrontendMessage("TriggerExportImage");
	subscriptions.unsubscribeFrontendMessage("UpdateDataPanelState");
	subscriptions.unsubscribeFrontendMessage("UpdatePropertiesPanelState");
	subscriptions.unsubscribeFrontendMessage("UpdateLayersPanelState");
}
