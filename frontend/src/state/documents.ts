/* eslint-disable max-classes-per-file */
import { reactive, readonly } from "vue";

import { TriggerFileDownload, TriggerRasterDownload, FrontendDocumentDetails, TriggerFileUpload, UpdateActiveDocument, UpdateOpenDocumentsList } from "@/dispatcher/js-messages";
import { EditorState } from "@/state/wasm-loader";
import { download, downloadBlob, upload } from "@/utilities/files";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDocumentsState(editor: EditorState) {
	const state = reactive({
		unsaved: false,
		documents: [] as FrontendDocumentDetails[],
		activeDocumentIndex: 0,
	});

	// Set up message subscriptions on creation
	editor.dispatcher.subscribeJsMessage(UpdateOpenDocumentsList, (updateOpenDocumentList) => {
		state.documents = updateOpenDocumentList.open_documents;
	});

	editor.dispatcher.subscribeJsMessage(UpdateActiveDocument, (updateActiveDocument) => {
		// Assume we receive a correct document id
		const activeId = state.documents.findIndex((doc) => doc.id === updateActiveDocument.document_id);
		state.activeDocumentIndex = activeId;
	});

	editor.dispatcher.subscribeJsMessage(TriggerFileUpload, async () => {
		const extension = editor.rawWasm.file_save_suffix();
		const data = await upload(extension);
		editor.instance.open_document_file(data.filename, data.content);
	});

	editor.dispatcher.subscribeJsMessage(TriggerFileDownload, (triggerFileDownload) => {
		download(triggerFileDownload.name, triggerFileDownload.document);
	});

	editor.dispatcher.subscribeJsMessage(TriggerRasterDownload, (triggerRasterDownload) => {
		// A canvas to render our svg to in order to get a raster image
		// https://stackoverflow.com/questions/3975499/convert-svg-to-image-jpeg-png-etc-in-the-browser
		const canvas = document.createElement("canvas");
		canvas.width = triggerRasterDownload.size.x;
		canvas.height = triggerRasterDownload.size.y;
		const ctx = canvas.getContext("2d");
		if (!ctx) return;

		// Fill the canvas with white if jpeg (does not support transparency and defaults to black)
		if (triggerRasterDownload.mime.endsWith("jpeg")) {
			ctx.fillStyle = "white";
			ctx.fillRect(0, 0, triggerRasterDownload.size.x, triggerRasterDownload.size.y);
		}

		// Create a blob url for our svg
		const img = new Image();
		const svgBlob = new Blob([triggerRasterDownload.document], { type: "image/svg+xml;charset=utf-8" });
		const url = URL.createObjectURL(svgBlob);
		img.onload = (): void => {
			// Draw our svg to the canvas
			ctx?.drawImage(img, 0, 0, triggerRasterDownload.size.x, triggerRasterDownload.size.y);

			// Convert the canvas to an image of the correct mime
			const imgURI = canvas.toDataURL(triggerRasterDownload.mime);
			// Download our canvas
			downloadBlob(imgURI, triggerRasterDownload.name);

			// Cleanup resources
			URL.revokeObjectURL(url);
		};
		img.src = url;
	});

	// TODO(mfish33): Replace with initialization system Issue:#524
	// Get the initial documents
	editor.instance.get_open_documents_list();

	return {
		state: readonly(state),
	};
}
export type DocumentsState = ReturnType<typeof createDocumentsState>;
