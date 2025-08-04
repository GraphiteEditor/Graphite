/* eslint-disable max-classes-per-file */

import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import {
	type FrontendDocumentDetails,
	TriggerFetchAndOpenDocument,
	TriggerSaveDocument,
	TriggerDownloadImage,
	TriggerDownloadTextFile,
	TriggerImport,
	TriggerOpenDocument,
	UpdateActiveDocument,
	UpdateOpenDocumentsList,
	UpdateSpreadsheetState,
	defaultWidgetLayout,
	patchWidgetLayout,
	UpdateSpreadsheetLayout,
} from "@graphite/messages";
import { downloadFileText, downloadFileBlob, upload } from "@graphite/utility-functions/files";
import { extractPixelData, rasterizeSVG } from "@graphite/utility-functions/rasterization";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createPortfolioState(editor: Editor) {
	const { subscribe, update } = writable({
		unsaved: false,
		documents: [] as FrontendDocumentDetails[],
		activeDocumentIndex: 0,
		spreadsheetOpen: false,
		spreadsheetNode: BigInt(0) as bigint | undefined,
		spreadsheetWidgets: defaultWidgetLayout(),
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
			const url = new URL(filename, document.location.href);
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
		const extension = editor.handle.fileSaveSuffix();
		const data = await upload(extension, "text");
		editor.handle.openDocumentFile(data.filename, data.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerImport, async () => {
		const data = await upload("image/*", "both");

		if (data.type.includes("svg")) {
			const svg = new TextDecoder().decode(data.content.data);
			editor.handle.pasteSvg(data.filename, svg);
			return;
		}

		// In case the user accidentally uploads a Graphite file, open it instead of failing to import it
		if (data.filename.endsWith(".graphite")) {
			editor.handle.openDocumentFile(data.filename, data.content.text);
			return;
		}

		const imageData = await extractPixelData(new Blob([data.content.data], { type: data.type }));
		editor.handle.pasteImage(data.filename, new Uint8Array(imageData.data), imageData.width, imageData.height);
	});
	editor.subscriptions.subscribeJsMessage(TriggerSaveDocument, (triggerSaveDocument) => {
		downloadFileText(triggerSaveDocument.name, triggerSaveDocument.document);
	});
	editor.subscriptions.subscribeJsMessage(TriggerDownloadTextFile, (triggerFileDownload) => {
		downloadFileText(triggerFileDownload.name, triggerFileDownload.document);
	});
	editor.subscriptions.subscribeJsMessage(TriggerDownloadImage, async (triggerDownloadImage) => {
		const { svg, name, mime, size } = triggerDownloadImage;

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
	editor.subscriptions.subscribeJsMessage(UpdateSpreadsheetState, async (updateSpreadsheetState) => {
		update((state) => {
			state.spreadsheetOpen = updateSpreadsheetState.open;
			state.spreadsheetNode = updateSpreadsheetState.node;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateSpreadsheetLayout, (updateSpreadsheetLayout) => {
		update((state) => {
			patchWidgetLayout(state.spreadsheetWidgets, updateSpreadsheetLayout);
			return state;
		});
	});

	return {
		subscribe,
	};
}
export type PortfolioState = ReturnType<typeof createPortfolioState>;
