/* eslint-disable max-classes-per-file */
import { reactive, readonly } from "vue";

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
	editor.subscriptions.subscribeJsMessage(TriggerAiArtistRasterizeAndGenerateImg2Img, async (triggerAiArtistRasterizeAndGenerateImg2Img) => {
		const { svg, size, layerPath, prompt } = triggerAiArtistRasterizeAndGenerateImg2Img;

		// Rasterize the SVG to an image file
		const blob = await rasterizeSVG(svg, size.x, size.y, "image/png");

		// TODO: Call `URL.revokeObjectURL` at the appropriate time to avoid a memory leak
		const blobURL = URL.createObjectURL(blob);

		editor.instance.setImageBlobUrl(layerPath, blobURL, size.x, size.y);

		callStableDiffusion(prompt, blob, layerPath, editor);
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

async function callStableDiffusion(prompt: string, _blob: Blob, layerPath: BigUint64Array, editor: Editor): Promise<void> {
	const samples = 30;
	const width = 512;
	const height = 512;

	const final = txt2img(prompt, samples, width, height);

	const interval = setInterval(async () => {
		const blob = await pollImage();

		const blobURL = URL.createObjectURL(blob);
		editor.instance.setImageBlobUrl(layerPath, blobURL, width, height);
	}, 1000);

	const blob = await final;

	clearInterval(interval);

	const blobURL = URL.createObjectURL(blob);
	editor.instance.setImageBlobUrl(layerPath, blobURL, width, height);
}

async function txt2img(prompt: string, samples: number, width: number, height: number): Promise<Blob> {
	// Highly unstable API
	const result = await fetch("http://192.168.1.10:7860/api/predict/", {
		headers: {
			accept: "*/*",
			"accept-language": "en-US,en;q=0.9",
			"content-type": "application/json",
		},
		referrer: "http://192.168.1.10:7860/",
		referrerPolicy: "strict-origin-when-cross-origin",
		body: `{
			"fn_index":12,
			"data":[
				"${prompt}",
				"",
				"None",
				"None",
				${samples},
				"Euler a",
				false,
				false,
				1,
				1,
				7,
				-1,
				-1,
				0,
				0,
				0,
				false,
				${width},
				${height},
				false,
				false,
				0.7,
				"None",
				false,
				false,
				null,
				"",
				"Seed",
				"",
				"Steps",
				"",
				true,
				false,
				null,
				"",
				""
			],
			"session_hash":"0000000000"
		}`,
		method: "POST",
		mode: "cors",
		credentials: "omit",
	});
	const json = await result.json();
	const base64 = json.data[0]?.[0] as string; // Highly unstable API

	if (typeof base64 !== "string" || !base64.startsWith("data:image/png;base64,")) return Promise.reject();

	return (await fetch(base64)).blob();
}

async function pollImage(): Promise<Blob> {
	// Highly unstable API
	const result = await fetch("http://192.168.1.10:7860/api/predict/", {
		headers: {
			accept: "*/*",
			"accept-language": "en-US,en;q=0.9",
			"content-type": "application/json",
		},
		referrer: "http://192.168.1.10:7860/",
		referrerPolicy: "strict-origin-when-cross-origin",
		body: `{
			"fn_index":2,
			"data":[],
			"session_hash":"0000000000"
		}`,
		method: "POST",
		mode: "cors",
		credentials: "omit",
	});
	const json = await result.json();
	const base64 = json.data[2]; // Highly unstable API

	if (typeof base64 !== "string" || !base64.startsWith("data:image/png;base64,")) return Promise.reject();

	return (await fetch(base64)).blob();
}
