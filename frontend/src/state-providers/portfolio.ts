/* eslint-disable max-classes-per-file */
import { reactive, readonly } from "vue";

import { callAIArtist } from "@/utility-functions/ai-artist";
import { downloadFileText, downloadFileBlob, upload } from "@/utility-functions/files";
import { rasterizeSVG } from "@/utility-functions/rasterization";
import { type Editor } from "@/wasm-communication/editor";
import {
	type FrontendDocumentDetails,
	TriggerFileDownload,
	TriggerImport,
	TriggerOpenDocument,
	TriggerRasterDownload,
	TriggerAiArtistRasterizeAndGenerateImg2Img,
	TriggerAiArtistGenerateTxt2Img,
	UpdateActiveDocument,
	UpdateOpenDocumentsList,
	UpdateImageData,
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
		downloadFileText(triggerFileDownload.name, triggerFileDownload.document);
	});
	editor.subscriptions.subscribeJsMessage(TriggerRasterDownload, async (triggerRasterDownload) => {
		const { svg, name, mime, size } = triggerRasterDownload;

		// Fill the canvas with white if it'll be a JPEG (which does not support transparency and defaults to black)
		const backgroundColor = mime.endsWith("jpeg") ? "white" : undefined;

		// Rasterize the SVG to an image file
		const blob = await rasterizeSVG(svg, size.x, size.y, mime, backgroundColor);

		// Have the browser download the file to the user's disk
		downloadFileBlob(name, blob);
	});
	editor.subscriptions.subscribeJsMessage(TriggerAiArtistGenerateTxt2Img, async (triggerAiArtistGenerateTxt2Img) => {
		const { layerPath, prompt, samples, cfgScale } = triggerAiArtistGenerateTxt2Img;

		callAIArtist(prompt, samples, cfgScale, undefined, undefined, layerPath, editor);
	});
	editor.subscriptions.subscribeJsMessage(TriggerAiArtistRasterizeAndGenerateImg2Img, async (triggerAiArtistRasterizeAndGenerateImg2Img) => {
		const { svg, size, layerPath, prompt, samples, cfgScale, denoisingStrength } = triggerAiArtistRasterizeAndGenerateImg2Img;

		// Rasterize the SVG to an image file
		const blob = await rasterizeSVG(svg, size.x, size.y, "image/png");

		// TODO: Call `URL.revokeObjectURL` at the appropriate time to avoid a memory leak
		const blobURL = URL.createObjectURL(blob);

		editor.instance.setImageBlobUrl(layerPath, blobURL, size.x, size.y);

		callAIArtist(prompt, samples, cfgScale, denoisingStrength, blob, layerPath, editor);
	});
	editor.subscriptions.subscribeJsMessage(UpdateImageData, (updateImageData) => {
		updateImageData.imageData.forEach(async (element) => {
			const buffer = new Uint8Array(element.imageData.values()).buffer;
			const blob = new Blob([buffer], { type: element.mime });

			// TODO: Call `URL.revokeObjectURL` at the appropriate time to avoid a memory leak
			const blobURL = URL.createObjectURL(blob);

			const image = await createImageBitmap(blob);

			editor.instance.setImageBlobUrl(element.path, blobURL, image.width, image.height);
		});
	});

	return {
		state: readonly(state) as typeof state,
	};
}
export type PortfolioState = ReturnType<typeof createPortfolioState>;
