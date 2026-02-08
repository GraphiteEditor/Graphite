import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import type { OpenDocument } from "@graphite/messages";
import {
	TriggerFetchAndOpenDocument,
	TriggerSaveDocument,
	TriggerExportImage,
	TriggerSaveFile,
	TriggerImport,
	TriggerOpen,
	UpdateActiveDocument,
	UpdateOpenDocumentsList,
	UpdateDataPanelState,
	UpdatePropertiesPanelState,
	UpdateLayersPanelState,
} from "@graphite/messages";
import { downloadFile, downloadFileBlob, upload } from "@graphite/utility-functions/files";
import { rasterizeSVG } from "@graphite/utility-functions/rasterization";

export function createPortfolioState(editor: Editor) {
	const { subscribe, update } = writable({
		unsaved: false,
		documents: [] as OpenDocument[],
		activeDocumentIndex: 0,
		dataPanelOpen: false,
		propertiesPanelOpen: true,
		layersPanelOpen: true,
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdateOpenDocumentsList, (data) => {
		update((state) => {
			state.documents = data.openDocuments;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateActiveDocument, (data) => {
		update((state) => {
			// Assume we receive a correct document id
			const activeId = state.documents.findIndex((doc) => doc.id === data.documentId);
			state.activeDocumentIndex = activeId;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(TriggerFetchAndOpenDocument, async (data) => {
		try {
			const url = new URL(`demo-artwork/${data.filename}`, document.location.href);
			const response = await fetch(url);
			editor.handle.openFile(data.filename, await response.bytes());
		} catch {
			// Needs to be delayed until the end of the current call stack so the existing demo artwork dialog can be closed first, otherwise this dialog won't show
			setTimeout(() => {
				editor.handle.errorDialog("Failed to open document", "The file could not be reached over the internet. You may be offline, or it may be missing.");
			}, 0);
		}
	});
	editor.subscriptions.subscribeJsMessage(TriggerOpen, async () => {
		const data = await upload(`image/*,.${editor.handle.fileExtension()}`, "data");
		editor.handle.openFile(data.filename, data.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerImport, async () => {
		// TODO: Use the same `accept` string as in the `TriggerOpen` handler once importing Graphite documents as nodes is supported
		const data = await upload("image/*", "data");
		editor.handle.importFile(data.filename, data.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerSaveDocument, (data) => {
		downloadFile(data.name, data.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerSaveFile, (data) => {
		downloadFile(data.name, data.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerExportImage, async (data) => {
		const { svg, name, mime, size } = data;

		// Fill the canvas with white if it'll be a JPEG (which does not support transparency and defaults to black)
		const backgroundColor = mime.endsWith("jpeg") ? "white" : undefined;

		// Rasterize the SVG to an image file
		try {
			const blob = await rasterizeSVG(svg, size.x, size.y, mime, backgroundColor);

			// Have the browser download the file to the user's disk
			downloadFileBlob(name, blob);
		} catch {
			// Fail silently if there's an error rasterizing the SVG, such as a zero-sized image
		}
	});
	editor.subscriptions.subscribeJsMessage(UpdateDataPanelState, async (data) => {
		update((state) => {
			state.dataPanelOpen = data.open;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdatePropertiesPanelState, async (data) => {
		update((state) => {
			state.propertiesPanelOpen = data.open;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateLayersPanelState, async (data) => {
		update((state) => {
			state.layersPanelOpen = data.open;
			return state;
		});
	});

	return {
		subscribe,
	};
}
export type PortfolioState = ReturnType<typeof createPortfolioState>;
