/* eslint-disable max-classes-per-file */

import {writable} from "svelte/store";

import { downloadFileText, downloadFileBlob, upload } from "@graphite/utility-functions/files";
import { imaginateGenerate, imaginateCheckConnection, imaginateTerminate, updateBackendImage } from "@graphite/utility-functions/imaginate";
import { extractPixelData, rasterizeSVG, rasterizeSVGCanvas } from "@graphite/utility-functions/rasterization";
import { type Editor } from "@graphite/wasm-communication/editor";
import {
	type FrontendDocumentDetails,
	TriggerFileDownload,
	TriggerImport,
	TriggerOpenDocument,
	TriggerRasterDownload,
	TriggerImaginateGenerate,
	TriggerImaginateTerminate,
	TriggerImaginateCheckServerStatus,
	TriggerNodeGraphFrameGenerate,
	UpdateActiveDocument,
	UpdateOpenDocumentsList,
	UpdateImageData,
	TriggerRevokeBlobUrl,
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
		})
	});
	editor.subscriptions.subscribeJsMessage(UpdateActiveDocument, (updateActiveDocument) => {
		update((state) => {
			// Assume we receive a correct document id
			const activeId = state.documents.findIndex((doc) => doc.id === updateActiveDocument.documentId);
			state.activeDocumentIndex = activeId;
			return state;
		})
	});
	editor.subscriptions.subscribeJsMessage(TriggerOpenDocument, async () => {
		const extension = editor.instance.fileSaveSuffix();
		const data = await upload(extension, "text");
		editor.instance.openDocumentFile(data.filename, data.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerImport, async () => {
		const data = await upload("image/*", "data");
		const imageData = await extractPixelData(new Blob([data.content], { type: data.type }));
		editor.instance.pasteImage(new Uint8Array(imageData.data), imageData.width, imageData.height);
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
	editor.subscriptions.subscribeJsMessage(TriggerImaginateCheckServerStatus, async (triggerImaginateCheckServerStatus) => {
		const { hostname } = triggerImaginateCheckServerStatus;

		imaginateCheckConnection(hostname, editor);
	});
	editor.subscriptions.subscribeJsMessage(TriggerImaginateGenerate, async (triggerImaginateGenerate) => {
		const { documentId, layerPath, nodePath, hostname, refreshFrequency, baseImage, maskImage, maskPaintMode, maskBlurPx, maskFillContent, parameters } = triggerImaginateGenerate;

		// Handle img2img mode
		let image: Blob | undefined;
		if (parameters.denoisingStrength !== undefined && baseImage !== undefined) {
			const buffer = new Uint8Array(baseImage.imageData.values()).buffer;

			image = new Blob([buffer], { type: baseImage.mime });
			updateBackendImage(editor, image, documentId, layerPath, nodePath);
		}

		// Handle layer mask
		let mask: Blob | undefined;
		if (maskImage !== undefined) {
			// Rasterize the SVG to an image file
			mask = await rasterizeSVG(maskImage.svg, maskImage.size[0], maskImage.size[1], "image/png");
		}

		imaginateGenerate(parameters, image, mask, maskPaintMode, maskBlurPx, maskFillContent, hostname, refreshFrequency, documentId, layerPath, nodePath, editor);
	});
	editor.subscriptions.subscribeJsMessage(TriggerImaginateTerminate, async (triggerImaginateTerminate) => {
		const { documentId, layerPath, nodePath, hostname } = triggerImaginateTerminate;

		imaginateTerminate(hostname, documentId, layerPath, nodePath, editor);
	});
	editor.subscriptions.subscribeJsMessage(UpdateImageData, (updateImageData) => {
		updateImageData.imageData.forEach(async (element) => {
			const buffer = new Uint8Array(element.imageData.values()).buffer;
			const blob = new Blob([buffer], { type: element.mime });

			const blobURL = URL.createObjectURL(blob);

			// Pre-decode the image so it is ready to be drawn instantly once it's placed into the viewport SVG
			const image = new Image();
			image.src = blobURL;
			await image.decode();

			editor.instance.setImageBlobURL(updateImageData.documentId, element.path, blobURL, image.naturalWidth, image.naturalHeight, element.transform);
		});
	});
	editor.subscriptions.subscribeJsMessage(TriggerNodeGraphFrameGenerate, async (triggerNodeGraphFrameGenerate) => {
		const { documentId, layerPath, svg, size, imaginateNode } = triggerNodeGraphFrameGenerate;

		// Rasterize the SVG to an image file
		let imageData;
		try {
			// getImageData may throw an exception if the resolution is too high
			if (size[0] >= 1 && size[1] >= 1) {
				imageData = (await rasterizeSVGCanvas(svg, size[0], size[1])).getContext("2d")?.getImageData(0, 0, size[0], size[1]);
			}
		} catch (e) {
			console.error("Failed to rasterize the SVG canvas in JS to be sent back to Rust:", e);
		}

		if (imageData) editor.instance.processNodeGraphFrame(documentId, layerPath, new Uint8Array(imageData.data), imageData.width, imageData.height, imaginateNode);
	});
	editor.subscriptions.subscribeJsMessage(TriggerRevokeBlobUrl, async (triggerRevokeBlobUrl) => {
		URL.revokeObjectURL(triggerRevokeBlobUrl.url);
	});

	return {
		subscribe,
	};
}
export type PortfolioState = ReturnType<typeof createPortfolioState>;
