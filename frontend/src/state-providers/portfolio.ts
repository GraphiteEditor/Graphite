import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import type { OpenDocument } from "@graphite/messages";
import {
	TriggerFetchAndOpenDocument,
	TriggerSaveDocument,
	TriggerExportImage,
	TriggerSaveFile,
	TriggerImport,
	TriggerOpenDocument,
	UpdateActiveDocument,
	UpdateOpenDocumentsList,
	UpdateDataPanelState,
	UpdatePropertiesPanelState,
	UpdateLayersPanelState,
} from "@graphite/messages";
import { downloadFile, downloadFileBlob, upload } from "@graphite/utility-functions/files";
import { extractPixelData, rasterizeSVG } from "@graphite/utility-functions/rasterization";

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
	editor.subscriptions.subscribeJsMessage(UpdateOpenDocumentsList, (updateOpenDocumentList) => {
		update((state) => {
			state.documents = updateOpenDocumentList.openDocuments;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateActiveDocument, (updateActiveDocument) => {
		update((state) => {
			// Assume we receive a correct document id
			const activeId = state.documents.findIndex((doc) => doc.id === updateActiveDocument.documentId);
			state.activeDocumentIndex = activeId;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(TriggerFetchAndOpenDocument, async (triggerFetchAndOpenDocument) => {
		try {
			const { name, filename } = triggerFetchAndOpenDocument;
			const url = new URL(`demo-artwork/${filename}`, document.location.href);
			const data = await fetch(url);
			const content = await data.text();

			editor.handle.openDocumentFile(name, content);
		} catch {
			// Needs to be delayed until the end of the current call stack so the existing demo artwork dialog can be closed first, otherwise this dialog won't show
			setTimeout(() => {
				editor.handle.errorDialog("Failed to open document", "The file could not be reached over the internet. You may be offline, or it may be missing.");
			}, 0);
		}
	});
	editor.subscriptions.subscribeJsMessage(TriggerOpenDocument, async () => {
		const suffix = "." + editor.handle.fileExtension();
		const data = await upload(suffix, "text");

		// Use filename as document name, removing the extension if it exists
		let documentName = data.filename;
		if (documentName.endsWith(suffix)) {
			documentName = documentName.slice(0, -suffix.length);
		}

		editor.handle.openDocumentFile(documentName, data.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerImport, async () => {
		const data = await upload("image/*", "both");

		if (data.type.includes("svg")) {
			const svg = new TextDecoder().decode(data.content.data);
			editor.handle.pasteSvg(data.filename, svg);
			return;
		}

		// In case the user accidentally uploads a Graphite file, open it instead of failing to import it
		const graphiteFileSuffix = "." + editor.handle.fileExtension();
		if (data.filename.endsWith(graphiteFileSuffix)) {
			const documentName = data.filename.slice(0, -graphiteFileSuffix.length);
			editor.handle.openDocumentFile(documentName, data.content.text);
			return;
		}

		const imageData = await extractPixelData(new Blob([new Uint8Array(data.content.data)], { type: data.type }));
		editor.handle.pasteImage(data.filename, new Uint8Array(imageData.data), imageData.width, imageData.height);
	});
	editor.subscriptions.subscribeJsMessage(TriggerSaveDocument, (triggerSaveDocument) => {
		downloadFile(triggerSaveDocument.name, triggerSaveDocument.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerSaveFile, (triggerFileDownload) => {
		downloadFile(triggerFileDownload.name, triggerFileDownload.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerExportImage, async (TriggerExportImage) => {
		const { svg, name, mime, size } = TriggerExportImage;

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
	editor.subscriptions.subscribeJsMessage(UpdateDataPanelState, async (updateDataPanelState) => {
		update((state) => {
			state.dataPanelOpen = updateDataPanelState.open;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdatePropertiesPanelState, async (updatePropertiesPanelState) => {
		update((state) => {
			state.propertiesPanelOpen = updatePropertiesPanelState.open;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateLayersPanelState, async (updateLayersPanelState) => {
		update((state) => {
			state.layersPanelOpen = updateLayersPanelState.open;
			return state;
		});
	});

	return {
		subscribe,
	};
}
export type PortfolioState = ReturnType<typeof createPortfolioState>;
