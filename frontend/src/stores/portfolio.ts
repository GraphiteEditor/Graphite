import { SvelteMap } from "svelte/reactivity";
import { get, writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { acceptStringFromFilters, browseFileHandles, downloadFile, fileHandlesSupported, saveFileHandle, upload, writeFileHandle } from "/src/utility-functions/files";
import type { UploadResult } from "/src/utility-functions/files";
import { createLayout } from "/src/utility-functions/layout-state.svelte";
import { fileHandleGet, fileHandleStore } from "/src/utility-functions/persistence";
import { deepEqual, patchLayout } from "/src/utility-functions/widgets";
import type { EditorWrapper, DocumentInfo, FileFilter, LayerPanelEntry, LayerStructureEntry, Layout, WorkspacePanelLayout } from "/wrapper/pkg/graphite_wasm_wrapper";

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
export const collaborationPanelLayout = persistedLayout("collaborationPanelLayout");
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
			editor.ingestPicked(data.filename, "", await response.bytes(), "OpenUnique");
		} catch {
			// Needs to be delayed until the end of the current call stack so the existing demo artwork dialog can be closed first, otherwise this dialog won't show
			setTimeout(() => {
				editor.errorDialog("Failed to open document", "The file could not be reached over the internet. You may be offline, or it may be missing.");
			}, 0);
		}
	});

	subscriptions.subscribeFrontendMessage("TriggerBrowse", async ({ options, action }) => {
		if (action === "Open" && fileHandlesSupported()) {
			const files = await browseFilesWithHandles(editor, options.filters, options.multiple);
			if (files) {
				files.forEach((file) => editor.ingestPicked(file.filename, file.type, file.content, action));
				return;
			}
		}

		const accept = acceptStringFromFilters(options.filters);
		const files = options.multiple ? await upload(accept, "data", true) : [await upload(accept, "data")];
		files.forEach((file) => editor.ingestPicked(file.filename, file.type, file.content, action));
	});

	// Picked when Save is pressed, since the picker needs a recent user interaction and the export can take longer than that
	const saveTargets = new Map<bigint, Promise<SaveTarget | undefined>>();

	subscriptions.subscribeFrontendMessage("TriggerPrepareSaveDocument", ({ save_id: saveId, name, path, filters }) => {
		if (!fileHandlesSupported()) return;

		const target = pickSaveTarget(name, path, filters);
		// Handled when the save arrives
		target.catch(() => undefined);
		saveTargets.set(saveId, target);
	});

	subscriptions.subscribeFrontendMessage("TriggerSaveDocument", async (data) => {
		const { document_id: documentId, save_id: saveId, name, path, filters, content, hash } = data;

		if (!fileHandlesSupported()) {
			downloadFile(name, content);
			editor.savedDocument(documentId, path, hash);
			return;
		}

		const pendingTarget = saveTargets.get(saveId) ?? pickSaveTarget(name, path, filters);
		saveTargets.delete(saveId);

		let target: SaveTarget | undefined;
		try {
			target = await pendingTarget;
		} catch {
			// The picker couldn't open, so download instead
			downloadFile(name, content);
			editor.savedDocument(documentId, undefined, hash);
			return;
		}
		if (!target) return;

		const { handle, existing, granted } = target;
		if (!granted) {
			editor.errorDialog("Failed to save document", `Permission to edit "${handle.name}" was not granted, so the document was not saved. Use File > Save As… to save it to another file.`);
			return;
		}

		try {
			await writeFileHandle(handle, content);
		} catch {
			editor.errorDialog("Failed to save document", `"${handle.name}" could not be written, so the document was not saved. Use File > Save As… to save it to another file.`);
			return;
		}

		// If the handle can't be stored, the next save asks where to save
		const savedPath = existing ? path : await fileHandleStore(handle).catch(() => undefined);
		editor.savedDocument(documentId, savedPath, hash);
	});

	subscriptions.subscribeFrontendMessage("TriggerSaveFile", (data) => {
		downloadFile(data.name, data.content);
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

	subscriptions.subscribeLayoutUpdate("CollaborationPanel", (data) => {
		patchLayout(collaborationPanelLayout, data);
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
	subscriptions.unsubscribeFrontendMessage("TriggerPrepareSaveDocument");
	subscriptions.unsubscribeFrontendMessage("TriggerSaveDocument");
	subscriptions.unsubscribeFrontendMessage("TriggerSaveFile");
	subscriptions.unsubscribeFrontendMessage("UpdateWorkspacePanelLayout");
	subscriptions.unsubscribeLayoutUpdate("WelcomeScreenButtons");
	subscriptions.unsubscribeLayoutUpdate("Inspector");
	subscriptions.unsubscribeLayoutUpdate("DataPanel");
	subscriptions.unsubscribeLayoutUpdate("DataPanelContext");
	subscriptions.unsubscribeLayoutUpdate("DataPanelPath");
	subscriptions.unsubscribeLayoutUpdate("CollaborationPanel");
	subscriptions.unsubscribeLayoutUpdate("LayersPanelControlLeftBar");
	subscriptions.unsubscribeLayoutUpdate("LayersPanelBottomLeftBar");
	subscriptions.unsubscribeLayoutUpdate("LayersPanelBottomRightBar");
	subscriptions.unsubscribeFrontendMessage("UpdateDocumentLayerStructure");
	subscriptions.unsubscribeFrontendMessage("UpdateDocumentLayerDetails");
}

// Names each picked file after the path of its stored file handle, which the startup cleanup drops unless it was opened as a document, or returns undefined if the picker can't open
async function browseFilesWithHandles(editor: EditorWrapper, filters: FileFilter[], multiple: boolean): Promise<UploadResult<Uint8Array>[] | undefined> {
	let handles: FileSystemFileHandle[];
	try {
		handles = await browseFileHandles(filters, multiple);
	} catch {
		// Fails in cross-origin iframes, where the file input still works
		return undefined;
	}

	// Failures are reported, since the file input fallback would need another user interaction
	const files: UploadResult<Uint8Array>[] = [];
	const unreadable: string[] = [];
	for (const handle of handles) {
		try {
			const file = await handle.getFile();
			const content = new Uint8Array(await file.arrayBuffer());
			const filename = await fileHandleStore(handle).catch(() => file.name);
			files.push({ filename, type: file.type, content });
		} catch {
			unreadable.push(`"${handle.name}"`);
		}
	}
	if (unreadable.length > 0) editor.errorDialog("Failed to open file", `${unreadable.join(", ")} could not be read.`);

	return files;
}

type SaveTarget = { handle: FileSystemFileHandle; existing: boolean; granted: boolean };

// Resolves to undefined if the user cancels the picker
async function pickSaveTarget(name: string, path: string | undefined, filters: FileFilter[]): Promise<SaveTarget | undefined> {
	const existing = path ? await fileHandleGet(path) : undefined;
	const handle = existing ?? (await saveFileHandle(name, filters));
	if (!handle) return undefined;

	// Throws without a recent user interaction
	const permission = await handle.requestPermission({ mode: "readwrite" }).catch(() => "denied");
	return { handle, existing: existing !== undefined, granted: permission === "granted" };
}
