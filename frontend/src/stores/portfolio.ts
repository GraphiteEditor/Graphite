import { tick } from "svelte";
import { SvelteMap } from "svelte/reactivity";
import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { downloadFile, downloadFileBlob, upload } from "/src/utility-functions/files";
import { rasterizeSVG } from "/src/utility-functions/rasterization";
import { patchLayout } from "/src/utility-functions/widgets";
import { createZipFromFiles } from "/wrapper/pkg/graphite_wasm_wrapper";
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

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<PortfolioStoreState> = import.meta.hot?.data?.store || writable<PortfolioStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export const welcomeScreenButtonsLayout = makeLayoutStore("welcomeScreenButtonsLayout");
export const propertiesPanelLayout = makeLayoutStore("propertiesPanelLayout");
export const dataPanelLayout = makeLayoutStore("dataPanelLayout");
export const layersPanelControlBarLeftLayout = makeLayoutStore("layersPanelControlBarLeftLayout");
export const layersPanelControlBarRightLayout = makeLayoutStore("layersPanelControlBarRightLayout");
export const layersPanelBottomBarLayout = makeLayoutStore("layersPanelBottomBarLayout");

// Each panel layout has its own dedicated store so a layout update only re-renders that panel's consumers.
// Putting them at module scope (not inside the component) lets them survive a Svelte remount during a
// panel-tree restructure, since the backend's diff-based updates aren't re-sent on subscribe.
function makeLayoutStore(name: string): Writable<Layout> {
	const persisted = import.meta.hot?.data?.[name];
	const layoutStore: Writable<Layout> = persisted || writable<Layout>([]);
	if (import.meta.hot) import.meta.hot.data[name] = layoutStore;
	return layoutStore;
}

function patchLayoutStore(layoutStore: Writable<Layout>, data: Parameters<typeof patchLayout>[1]) {
	layoutStore.update((layout) => {
		patchLayout(layout, data);
		return layout;
	});
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
			editor.openFile(data.filename, await response.bytes());
		} catch {
			// Needs to be delayed until the end of the current call stack so the existing demo artwork dialog can be closed first, otherwise this dialog won't show
			setTimeout(() => {
				editor.errorDialog("Failed to open document", "The file could not be reached over the internet. You may be offline, or it may be missing.");
			}, 0);
		}
	});

	subscriptions.subscribeFrontendMessage("TriggerOpen", async () => {
		const files = await upload(`image/*,.${editor.fileExtension()}`, "data", true);
		files.forEach((file) => editor.openFile(file.filename, file.content));
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

	// TODO: This handler orchestrates rasterization + zipping in JS because PNG/JPG frames arrive as SVG strings
	// TODO: that need the frontend's canvas-based `rasterizeSVG()` to encode. Once SVG rasterization moves to
	// TODO: always occur in Rust, the executor can build the .zip itself and emit a single `TriggerSaveFile`,
	// TODO: matching how PNG/JPG/SVG/.graphite single-file exports work today.
	subscriptions.subscribeFrontendMessage("TriggerExportAnimation", async (data) => {
		const { name, extension, mime, size, frames } = data;
		const isRaster = extension === "png" || extension === "jpg";
		const backgroundColor = mime.endsWith("jpeg") ? "white" : undefined;
		const padWidth = Math.max(4, String(frames.length).length);

		// Materialize each frame to bytes, rasterizing SVG via canvas when the destination format is raster.
		// Any per-frame failure aborts the export rather than silently dropping frames, so the user never gets
		// a zip with mismatched indices vs. the requested playback range.
		const entries: [string, Uint8Array][] = [];
		try {
			for (let i = 0; i < frames.length; i++) {
				const frame = frames[i];
				const filename = `${name}_${String(i + 1).padStart(padWidth, "0")}.${extension}`;

				let bytes: Uint8Array;
				if ("Bytes" in frame) {
					bytes = frame.Bytes;
				} else if (isRaster) {
					const blob = await rasterizeSVG(frame.Svg, size[0], size[1], mime, backgroundColor);
					bytes = new Uint8Array(await blob.arrayBuffer());
				} else {
					bytes = new TextEncoder().encode(frame.Svg);
				}
				entries.push([filename, bytes]);
			}
		} catch (error) {
			editor.errorDialog("Animation export failed", error instanceof Error ? error.message : String(error));
			return;
		}

		if (entries.length === 0) return;

		// Build the .zip in Rust (uncompressed store mode); web APIs can only deliver a single download, so the user gets one .zip
		const zipBytes = createZipFromFiles(entries);
		downloadFileBlob(`${name}.zip`, new Blob([new Uint8Array(zipBytes)], { type: "application/zip" }));
	});

	subscriptions.subscribeFrontendMessage("UpdateWorkspacePanelLayout", (data) => {
		update((state) => {
			state.panelLayout = data.panelLayout;
			return state;
		});
	});

	// Each panel layout uses its own store so updates only re-render that panel's consumers
	subscriptions.subscribeLayoutUpdate("WelcomeScreenButtons", async (data) => {
		await tick();
		patchLayoutStore(welcomeScreenButtonsLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("PropertiesPanel", async (data) => {
		await tick();
		patchLayoutStore(propertiesPanelLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("DataPanel", async (data) => {
		await tick();
		patchLayoutStore(dataPanelLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("LayersPanelControlLeftBar", async (data) => {
		await tick();
		patchLayoutStore(layersPanelControlBarLeftLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("LayersPanelControlRightBar", async (data) => {
		await tick();
		patchLayoutStore(layersPanelControlBarRightLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("LayersPanelBottomBar", async (data) => {
		await tick();
		patchLayoutStore(layersPanelBottomBarLayout, data);
	});

	subscriptions.subscribeFrontendMessage("UpdateDocumentLayerStructure", (data) => {
		update((state) => {
			state.layerStructure = data.layerStructure;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateDocumentLayerDetails", (data) => {
		update((state) => {
			state.layerCache.set(String(data.data.id), data.data);
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
	subscriptions.unsubscribeFrontendMessage("TriggerExportAnimation");
	subscriptions.unsubscribeFrontendMessage("UpdateWorkspacePanelLayout");
	subscriptions.unsubscribeLayoutUpdate("WelcomeScreenButtons");
	subscriptions.unsubscribeLayoutUpdate("PropertiesPanel");
	subscriptions.unsubscribeLayoutUpdate("DataPanel");
	subscriptions.unsubscribeLayoutUpdate("LayersPanelControlLeftBar");
	subscriptions.unsubscribeLayoutUpdate("LayersPanelControlRightBar");
	subscriptions.unsubscribeLayoutUpdate("LayersPanelBottomBar");
	subscriptions.unsubscribeFrontendMessage("UpdateDocumentLayerStructure");
	subscriptions.unsubscribeFrontendMessage("UpdateDocumentLayerDetails");
}
