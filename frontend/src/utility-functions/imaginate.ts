/* eslint-disable camelcase */

// import { escapeJSON } from "~/src/utility-functions/escape";
import { blobToBase64 } from "~/src/utility-functions/files";
import { type RequestResult, requestWithUploadDownloadProgress } from "~/src/utility-functions/network";
import { type Editor } from "~/src/wasm-communication/editor";
import type { XY } from "~/src/wasm-communication/messages";
import { type ImaginateGenerationParameters } from "~/src/wasm-communication/messages";

const MAX_POLLING_RETRIES = 4;
const SERVER_STATUS_CHECK_TIMEOUT = 5000;
const PROGRESS_EVERY_N_STEPS = 5;

let timer: NodeJS.Timeout | undefined;
let terminated = false;

let generatingAbortRequest: XMLHttpRequest | undefined;
let pollingAbortController = new AbortController();
let statusAbortController = new AbortController();

// PUBLICLY CALLABLE FUNCTIONS

export async function imaginateGenerate(
	parameters: ImaginateGenerationParameters,
	image: Blob | undefined,
	mask: Blob | undefined,
	maskPaintMode: string,
	maskBlurPx: number,
	maskFillContent: string,
	hostname: string,
	refreshFrequency: number,
	documentId: bigint,
	layerPath: BigUint64Array,
	nodePath: BigUint64Array,
	editor: Editor
): Promise<void> {
	// Ignore a request to generate a new image while another is already being generated
	if (generatingAbortRequest !== undefined) return;

	terminated = false;

	// Immediately set the progress to 0% so the backend knows to update its layout
	editor.instance.setImaginateGeneratingStatus(documentId, layerPath, nodePath, 0, "Beginning");

	// Initiate a request to the computation server
	const discloseUploadingProgress = (progress: number): void => {
		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, nodePath, progress * 100, "Uploading");
	};
	const { uploaded, result, xhr } = await generate(discloseUploadingProgress, hostname, image, mask, maskPaintMode, maskBlurPx, maskFillContent, parameters);
	generatingAbortRequest = xhr;

	try {
		// Wait until the request is fully uploaded, which could be slow if the img2img source is large and the user is on a slow connection
		await uploaded;
		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, nodePath, 0, "Generating");

		// Begin polling for updates to the in-progress image generation at the specified interval
		// Don't poll if the chosen interval is 0, or if the chosen sampling method does not support polling
		if (refreshFrequency > 0) {
			const interval = Math.max(refreshFrequency * 1000, 500);
			scheduleNextPollingUpdate(interval, Date.now(), 0, editor, hostname, documentId, layerPath, nodePath, parameters.resolution);
		}

		// Wait for the final image to be returned by the initial request containing either the full image or the last frame if it was terminated by the user
		const { body, status } = await result;
		if (status < 200 || status > 299) {
			throw new Error(`Request to server failed to return a 200-level status code (${status})`);
		}

		// Extract the final image from the response and convert it to a data blob
		const base64Data = JSON.parse(body)?.images?.[0] as string | undefined;
		const base64 = typeof base64Data === "string" && base64Data.length > 0 ? `data:image/png;base64,${base64Data}` : undefined;
		if (!base64) throw new Error("Could not read final image result from server response");
		const blob = await (await fetch(base64)).blob();

		// Send the backend an updated status
		const percent = terminated ? undefined : 100;
		const newStatus = terminated ? "Terminated" : "Idle";
		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, nodePath, percent, newStatus);

		// Send the backend a blob URL for the final image
		updateBackendImage(editor, blob, documentId, layerPath, nodePath);
	} catch {
		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, nodePath, undefined, "Terminated");

		await imaginateCheckConnection(hostname, editor);
	}

	abortAndResetGenerating();
	abortAndResetPolling();
}

export async function imaginateTerminate(hostname: string, documentId: bigint, layerPath: BigUint64Array, nodePath: BigUint64Array, editor: Editor): Promise<void> {
	terminated = true;
	abortAndResetPolling();

	try {
		await terminate(hostname);

		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, nodePath, undefined, "Terminating");
	} catch {
		abortAndResetGenerating();
		abortAndResetPolling();

		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, nodePath, undefined, "Terminated");

		await imaginateCheckConnection(hostname, editor);
	}
}

export async function imaginateCheckConnection(hostname: string, editor: Editor): Promise<void> {
	const serverReached = await checkConnection(hostname);
	editor.instance.setImaginateServerStatus(serverReached);
}

// Converts the blob image into a list of pixels using an invisible canvas.
export async function updateBackendImage(editor: Editor, blob: Blob, documentId: bigint, layerPath: BigUint64Array, nodePath: BigUint64Array): Promise<void> {
	const image = await createImageBitmap(blob);
	const canvas = document.createElement("canvas");
	canvas.width = image.width;
	canvas.height = image.height;
	const ctx = canvas.getContext("2d");
	if (!ctx) throw new Error("Could not create canvas context");
	ctx.drawImage(image, 0, 0);

	// Send the backend the blob data to be stored persistently in the layer
	const imageData = ctx.getImageData(0, 0, image.width, image.height);
	const u8Array = new Uint8Array(imageData.data);

	editor.instance.setImaginateImageData(documentId, layerPath, nodePath, u8Array, imageData.width, imageData.height);
}

// ABORTING AND RESETTING HELPERS

function abortAndResetGenerating(): void {
	generatingAbortRequest?.abort();
	generatingAbortRequest = undefined;
}

function abortAndResetPolling(): void {
	pollingAbortController.abort();
	pollingAbortController = new AbortController();
	clearTimeout(timer);
}

// POLLING IMPLEMENTATION DETAILS

function scheduleNextPollingUpdate(
	interval: number,
	timeoutBegan: number,
	pollingRetries: number,
	editor: Editor,
	hostname: string,
	documentId: bigint,
	layerPath: BigUint64Array,
	nodePath: BigUint64Array,
	resolution: XY
): void {
	// Pick a future time that keeps to the user-requested interval if possible, but on slower connections will go as fast as possible without overlapping itself
	const nextPollTimeGoal = timeoutBegan + interval;
	const timeFromNow = Math.max(0, nextPollTimeGoal - Date.now());

	timer = setTimeout(async () => {
		const nextTimeoutBegan = Date.now();

		try {
			const [blob, percentComplete] = await pollImage(hostname);

			// After waiting for the polling result back from the server, if during that intervening time the user has terminated the generation, exit so we don't overwrite that terminated status
			if (terminated) return;

			if (blob) updateBackendImage(editor, blob, documentId, layerPath, nodePath);
			editor.instance.setImaginateGeneratingStatus(documentId, layerPath, nodePath, percentComplete, "Generating");

			scheduleNextPollingUpdate(interval, nextTimeoutBegan, 0, editor, hostname, documentId, layerPath, nodePath, resolution);
		} catch {
			if (generatingAbortRequest === undefined) return;

			if (pollingRetries + 1 > MAX_POLLING_RETRIES) {
				abortAndResetGenerating();
				abortAndResetPolling();

				await imaginateCheckConnection(hostname, editor);
			} else {
				scheduleNextPollingUpdate(interval, nextTimeoutBegan, pollingRetries + 1, editor, hostname, documentId, layerPath, nodePath, resolution);
			}
		}
	}, timeFromNow);
}

// API COMMUNICATION FUNCTIONS

async function pollImage(hostname: string): Promise<[Blob | undefined, number]> {
	// Fetch the percent progress and in-progress image from the API
	const result = await fetch(`${hostname}sdapi/v1/progress`, { signal: pollingAbortController.signal, method: "GET" });
	const { current_image, progress } = await result.json();

	// Convert to a usable format
	const progressPercent = progress * 100;
	const base64 = typeof current_image === "string" && current_image.length > 0 ? `data:image/png;base64,${current_image}` : undefined;

	// Deal with a missing image
	if (!base64) {
		// The image is not ready yet (because it's only had a few samples since generation began), but we do have a progress percentage
		if (!Number.isNaN(progressPercent) && progressPercent >= 0 && progressPercent <= 100) {
			return [undefined, progressPercent];
		}

		// Something else is wrong and the image wasn't provided as expected
		return Promise.reject();
	}

	// The image was provided so we turn it into a data blob
	const blob = await (await fetch(base64)).blob();
	return [blob, progressPercent];
}

async function generate(
	discloseUploadingProgress: (progress: number) => void,
	hostname: string,
	image: Blob | undefined,
	mask: Blob | undefined,
	maskPaintMode: string,
	maskBlurPx: number,
	maskFillContent: string,
	parameters: ImaginateGenerationParameters
): Promise<{
	uploaded: Promise<void>;
	result: Promise<RequestResult>;
	xhr?: XMLHttpRequest;
}> {
	let body;
	let endpoint;
	if (image === undefined || parameters.denoisingStrength === undefined) {
		endpoint = `${hostname}sdapi/v1/txt2img`;

		body = {
			// enable_hr: false,
			// denoising_strength: 0,
			// firstphase_width: 0,
			// firstphase_height: 0,
			prompt: parameters.prompt,
			// styles: [],
			seed: Number(parameters.seed),
			// subseed: -1,
			// subseed_strength: 0,
			// seed_resize_from_h: -1,
			// seed_resize_from_w: -1,
			// batch_size: 1,
			// n_iter: 1,
			steps: parameters.samples,
			cfg_scale: parameters.cfgScale,
			width: parameters.resolution.x,
			height: parameters.resolution.y,
			restore_faces: parameters.restoreFaces,
			tiling: parameters.tiling,
			negative_prompt: parameters.negativePrompt,
			// eta: 0,
			// s_churn: 0,
			// s_tmax: 0,
			// s_tmin: 0,
			// s_noise: 1,
			override_settings: {
				show_progress_every_n_steps: PROGRESS_EVERY_N_STEPS,
			},
			sampler_index: parameters.samplingMethod,
		};
	} else {
		const sourceImageBase64 = await blobToBase64(image);
		const maskImageBase64 = mask ? await blobToBase64(mask) : "";

		const maskFillContentIndexes = ["Fill", "Original", "LatentNoise", "LatentNothing"];
		const maskFillContentIndexFound = maskFillContentIndexes.indexOf(maskFillContent);
		const maskFillContentIndex = maskFillContentIndexFound === -1 ? undefined : maskFillContentIndexFound;

		const maskInvert = maskPaintMode === "Inpaint" ? 1 : 0;

		endpoint = `${hostname}sdapi/v1/img2img`;

		body = {
			init_images: [sourceImageBase64],
			// resize_mode: 0,
			denoising_strength: parameters.denoisingStrength,
			mask: mask && maskImageBase64,
			mask_blur: mask && maskBlurPx,
			inpainting_fill: mask && maskFillContentIndex,
			inpaint_full_res: mask && false,
			// inpaint_full_res_padding: 0,
			inpainting_mask_invert: mask && maskInvert,
			prompt: parameters.prompt,
			// styles: [],
			seed: Number(parameters.seed),
			// subseed: -1,
			// subseed_strength: 0,
			// seed_resize_from_h: -1,
			// seed_resize_from_w: -1,
			// batch_size: 1,
			// n_iter: 1,
			steps: parameters.samples,
			cfg_scale: parameters.cfgScale,
			width: parameters.resolution.x,
			height: parameters.resolution.y,
			restore_faces: parameters.restoreFaces,
			tiling: parameters.tiling,
			negative_prompt: parameters.negativePrompt,
			// eta: 0,
			// s_churn: 0,
			// s_tmax: 0,
			// s_tmin: 0,
			// s_noise: 1,
			override_settings: {
				show_progress_every_n_steps: PROGRESS_EVERY_N_STEPS,
				img2img_fix_steps: true,
			},
			sampler_index: parameters.samplingMethod,
			// include_init_images: false,
		};
	}

	// Prepare a promise that will resolve after the outbound request upload is complete
	let uploadedResolve: () => void;
	let uploadedReject: () => void;
	const uploaded = new Promise<void>((resolve, reject): void => {
		uploadedResolve = resolve;
		uploadedReject = reject;
	});

	// Fire off the request and, once the outbound request upload is complete, resolve the promise we defined above
	const uploadProgress = (progress: number): void => {
		if (progress < 1) {
			discloseUploadingProgress(progress);
		} else {
			uploadedResolve();
		}
	};
	const [result, xhr] = requestWithUploadDownloadProgress(endpoint, "POST", JSON.stringify(body), uploadProgress, abortAndResetPolling);
	result.catch(() => uploadedReject());

	// Return the promise that resolves when the request upload is complete, the promise that resolves when the response download is complete, and the XHR so it can be aborted
	return { uploaded, result, xhr };
}

async function terminate(hostname: string): Promise<void> {
	await fetch(`${hostname}sdapi/v1/interrupt`, { method: "POST" });
}

async function checkConnection(hostname: string): Promise<boolean> {
	statusAbortController.abort();
	statusAbortController = new AbortController();

	const timeout = setTimeout(() => statusAbortController.abort(), SERVER_STATUS_CHECK_TIMEOUT);

	try {
		// Intentionally misuse this API endpoint by using it just to check for a code 200 response, regardless of what the result is
		const { status } = await fetch(`${hostname}sdapi/v1/progress?skip_current_image=true`, { signal: statusAbortController.signal, method: "GET" });

		// This code means the server has indeed responded and the endpoint exists (otherwise it would be 404)
		if (status === 200) {
			clearTimeout(timeout);
			return true;
		}
	} catch {
		// Do nothing here
	}

	return false;
}
