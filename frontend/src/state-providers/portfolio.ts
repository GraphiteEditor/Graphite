/* eslint-disable max-classes-per-file */
import { reactive, readonly } from "vue";

import { download, downloadBlob, upload } from "@/utility-functions/files";
import { type Editor } from "@/wasm-communication/editor";
import {
	type FrontendDocumentDetails,
	TriggerFileDownload,
	TriggerImport,
	TriggerOpenDocument,
	TriggerRasterDownload,
	UpdateActiveDocument,
	UpdateOpenDocumentsList,
} from "@/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createPortfolioState(editor: Editor) {
	const state = reactive({
		unsaved: false,
		documents: [] as FrontendDocumentDetails[],
		activeDocumentIndex: 0,
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdateOpenDocumentsList, (updateOpenDocumentList) => {
		state.documents = updateOpenDocumentList.openDocuments;
	});
	editor.subscriptions.subscribeJsMessage(UpdateActiveDocument, (updateActiveDocument) => {
		// Assume we receive a correct document id
		const activeId = state.documents.findIndex((doc) => doc.id === updateActiveDocument.documentId);
		state.activeDocumentIndex = activeId;
	});
	editor.subscriptions.subscribeJsMessage(TriggerOpenDocument, async () => {
		const extension = editor.instance.fileSaveSuffix();
		const data = await upload(extension, "text");
		editor.instance.openDocumentFile(data.filename, data.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerImport, async () => {
		const data = await upload("image/*", "data");
		editor.instance.pasteImage(data.type, Uint8Array.from(data.content));
	});
	editor.subscriptions.subscribeJsMessage(TriggerFileDownload, (triggerFileDownload) => {
		download(triggerFileDownload.name, triggerFileDownload.document);
	});
	editor.subscriptions.subscribeJsMessage(TriggerRasterDownload, (triggerRasterDownload) => {
		// A canvas to render our svg to in order to get a raster image
		// https://stackoverflow.com/questions/3975499/convert-svg-to-image-jpeg-png-etc-in-the-browser
		const canvas = document.createElement("canvas");
		canvas.width = triggerRasterDownload.size.x;
		canvas.height = triggerRasterDownload.size.y;
		const context = canvas.getContext("2d");
		if (!context) return;

		// Fill the canvas with white if jpeg (does not support transparency and defaults to black)
		if (triggerRasterDownload.mime.endsWith("jpeg")) {
			context.fillStyle = "white";
			context.fillRect(0, 0, triggerRasterDownload.size.x, triggerRasterDownload.size.y);
		}

		// Create a blob url for our svg
		const img = new Image();
		const svgBlob = new Blob([triggerRasterDownload.document], { type: "image/svg+xml;charset=utf-8" });
		const url = URL.createObjectURL(svgBlob);
		img.onload = (): void => {
			// Draw our svg to the canvas
			context?.drawImage(img, 0, 0, triggerRasterDownload.size.x, triggerRasterDownload.size.y);

			// Convert the canvas to an image of the correct mime
			const imgURI = canvas.toDataURL(triggerRasterDownload.mime);
			// Download our canvas
			downloadBlob(imgURI, triggerRasterDownload.name);

			// Cleanup resources
			URL.revokeObjectURL(url);
		};
		img.src = url;
	});

	return {
		state: readonly(state) as typeof state,
	};
}
export type PortfolioState = ReturnType<typeof createPortfolioState>;
