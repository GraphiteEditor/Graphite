/* eslint-disable max-classes-per-file */

import { writable } from "svelte/store";

import { copyToClipboardFileURL } from "@graphite/io-managers/clipboard";
import { downloadFileText, downloadFileBlob, upload } from "@graphite/utility-functions/files";
import { extractPixelData, imageToPNG, rasterizeSVG } from "@graphite/utility-functions/rasterization";
import { type Editor } from "@graphite/wasm-communication/editor";
import {
	type FrontendDocumentDetails,
	TriggerCopyToClipboardBlobUrl,
	TriggerFetchAndOpenDocument,
	TriggerDownloadBlobUrl,
	TriggerDownloadImage,
	TriggerDownloadTextFile,
	TriggerImport,
	TriggerOpenDocument,
	TriggerRevokeBlobUrl,
	UpdateActiveDocument,
	UpdateOpenDocumentsList,
} from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createPortfolioState(editor: Editor) {
	const { subscribe, update } = writable({
		unsaved: false,
		documents: [] as FrontendDocumentDetails[],
		activeDocumentIndex: 0,
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

			editor.instance.openDocumentFile(name, content);
		} catch {
			// Needs to be delayed until the end of the current call stack so the existing demo artwork dialog can be closed first, otherwise this dialog won't show
			setTimeout(() => {
				editor.instance.errorDialog("Failed to open document", "The file could not be reached over the internet. You may be offline, or it may be missing.");
			}, 0);
		}
	});
	editor.subscriptions.subscribeJsMessage(TriggerOpenDocument, async () => {
		const extension = editor.instance.fileSaveSuffix();
		const data = await upload(extension, "text");
		editor.instance.openDocumentFile(data.filename, data.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerImport, async () => {
		const data = await upload("image/*", "data");

		if (data.type.includes("svg")) {
			const svg = new TextDecoder().decode(data.content);
			editor.instance.pasteSvg(svg);

			return;
		}

		const imageData = await extractPixelData(new Blob([data.content], { type: data.type }));
		editor.instance.pasteImage(new Uint8Array(imageData.data), imageData.width, imageData.height);
	});
	editor.subscriptions.subscribeJsMessage(TriggerDownloadTextFile, (triggerFileDownload) => {
		downloadFileText(triggerFileDownload.name, triggerFileDownload.document);
	});
	editor.subscriptions.subscribeJsMessage(TriggerDownloadBlobUrl, async (triggerDownloadBlobUrl) => {
		const data = await fetch(triggerDownloadBlobUrl.blobUrl);
		const blob = await data.blob();

		// TODO: Remove this if/when we end up returning PNG directly from the backend
		const pngBlob = await imageToPNG(blob);

		downloadFileBlob(triggerDownloadBlobUrl.layerName, pngBlob);
	});
	editor.subscriptions.subscribeJsMessage(TriggerCopyToClipboardBlobUrl, (triggerDownloadBlobUrl) => {
		copyToClipboardFileURL(triggerDownloadBlobUrl.blobUrl);
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
	editor.subscriptions.subscribeJsMessage(TriggerRevokeBlobUrl, async (triggerRevokeBlobUrl) => {
		URL.revokeObjectURL(triggerRevokeBlobUrl.url);
	});

	return {
		subscribe,
	};
}
export type PortfolioState = ReturnType<typeof createPortfolioState>;
