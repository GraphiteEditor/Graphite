/* eslint-disable camelcase */

// import { escapeJSON } from "@/utility-functions/escape";
import { blobToBase64 } from "@/utility-functions/files";
import { type RequestResult, requestWithUploadDownloadProgress } from "@/utility-functions/network";
import { type Editor } from "@/wasm-communication/editor";
import type { XY } from "@/wasm-communication/messages";
import { type ImaginateGenerationParameters } from "@/wasm-communication/messages";

const MAX_POLLING_RETRIES = 4;
const SERVER_STATUS_CHECK_TIMEOUT = 5000;

let timer: NodeJS.Timeout | undefined;
let terminated = false;

let generatingAbortRequest: XMLHttpRequest | undefined;
let pollingAbortController = new AbortController();
let statusAbortController = new AbortController();

// PUBLICLY CALLABLE FUNCTIONS

export async function imaginateGenerate(
	parameters: ImaginateGenerationParameters,
	image: Blob | undefined,
	hostname: string,
	refreshFrequency: number,
	documentId: bigint,
	layerPath: BigUint64Array,
	editor: Editor
): Promise<void> {
	// Ignore a request to generate a new image while another is already being generated
	if (generatingAbortRequest !== undefined) return;

	terminated = false;

	// Immediately set the progress to 0% so the backend knows to update its layout
	editor.instance.setImaginateGeneratingStatus(documentId, layerPath, 0, "Beginning");

	// Initiate a request to the computation server
	const discloseUploadingProgress = (progress: number): void => {
		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, progress * 100, "Uploading");
	};
	const { uploaded, result, xhr } = await generate(discloseUploadingProgress, hostname, image, parameters);
	generatingAbortRequest = xhr;

	try {
		// Wait until the request is fully uploaded, which could be slow if the img2img source is large and the user is on a slow connection
		await uploaded;
		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, 0, "Generating");

		// Begin polling for updates to the in-progress image generation at the specified interval
		// Don't poll if the chosen interval is 0, or if the chosen sampling method does not support polling
		if (refreshFrequency > 0) {
			const interval = Math.max(refreshFrequency * 1000, 500);
			scheduleNextPollingUpdate(interval, Date.now(), 0, editor, hostname, documentId, layerPath, parameters.resolution);
		}

		// Wait for the final image to be returned by the initial request containing either the full image or the last frame if it was terminated by the user
		const { body, status } = await result;
		if (status < 200 || status > 299) {
			throw new Error(`Request to server failed to return a 200-level status code (${status})`);
		}

		// Extract the final image from the response and convert it to a data blob
		const base64 = JSON.parse(body)?.images?.[0] as string | undefined;
		if (typeof base64 !== "string" || !base64.startsWith("data:image/png;base64,")) throw new Error("Could not read final image result from server response");
		const blob = await (await fetch(base64)).blob();

		// Send the backend an updated status
		const percent = terminated ? undefined : 100;
		const newStatus = terminated ? "Terminated" : "Idle";
		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, percent, newStatus);

		// Send the backend a blob URL for the final image
		preloadAndSetImaginateBlobURL(editor, blob, documentId, layerPath, parameters.resolution.x, parameters.resolution.y);

		// Send the backend the blob data to be stored persistently in the layer
		const u8Array = new Uint8Array(await blob.arrayBuffer());
		editor.instance.setImaginateImageData(documentId, layerPath, u8Array);
	} catch {
		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, undefined, "Terminated");

		await imaginateCheckConnection(hostname, editor);
	}

	abortAndResetGenerating();
	abortAndResetPolling();
}

export async function imaginateTerminate(hostname: string, documentId: bigint, layerPath: BigUint64Array, editor: Editor): Promise<void> {
	terminated = true;
	abortAndResetPolling();

	try {
		await terminate(hostname);

		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, undefined, "Terminating");
	} catch {
		abortAndResetGenerating();
		abortAndResetPolling();

		editor.instance.setImaginateGeneratingStatus(documentId, layerPath, undefined, "Terminated");

		await imaginateCheckConnection(hostname, editor);
	}
}

export async function imaginateCheckConnection(hostname: string, editor: Editor): Promise<void> {
	const serverReached = await checkConnection(hostname);
	editor.instance.setImaginateServerStatus(serverReached);
}

export async function preloadAndSetImaginateBlobURL(editor: Editor, blob: Blob, documentId: bigint, layerPath: BigUint64Array, width: number, height: number): Promise<void> {
	const blobURL = URL.createObjectURL(blob);

	// Pre-decode the image so it is ready to be drawn instantly once it's placed into the viewport SVG
	const image = new Image();
	image.src = blobURL;
	await image.decode();

	editor.instance.setImaginateBlobURL(documentId, layerPath, blobURL, width, height);
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

			if (blob) preloadAndSetImaginateBlobURL(editor, blob, documentId, layerPath, resolution.x, resolution.y);
			editor.instance.setImaginateGeneratingStatus(documentId, layerPath, percentComplete, "Generating");

			scheduleNextPollingUpdate(interval, nextTimeoutBegan, 0, editor, hostname, documentId, layerPath, resolution);
		} catch {
			if (generatingAbortRequest === undefined) return;

			if (pollingRetries + 1 > MAX_POLLING_RETRIES) {
				abortAndResetGenerating();
				abortAndResetPolling();

				await imaginateCheckConnection(hostname, editor);
			} else {
				scheduleNextPollingUpdate(interval, nextTimeoutBegan, pollingRetries + 1, editor, hostname, documentId, layerPath, resolution);
			}
		}
	}, timeFromNow);
}

// API COMMUNICATION FUNCTIONS

async function pollImage(hostname: string): Promise<[Blob | undefined, number]> {
	// Fetch the percent progress and in-progress image from the API
	const result = await fetch(`${hostname}sdapi/v1/progress`, { signal: pollingAbortController.signal, method: "GET" });
	const { current_image, progress } = await result.json();
	const progressPercent = progress * 100;

	// The image is not ready yet (because it's only had a few samples since beginning)
	if (current_image === null) {
		return [undefined, progressPercent];
	}
	// Something's wrong and the image wasn't provided as expected
	if (typeof current_image !== "string" || !current_image.startsWith("data:image/png;base64,")) {
		return Promise.reject();
	}

	// The image was provided so we turn it into a data blob
	const blob = await (await fetch(current_image)).blob();
	return [blob, progressPercent];
}

async function generate(
	discloseUploadingProgress: (progress: number) => void,
	hostname: string,
	image: Blob | undefined,
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

		// TODO: Temporarily set `show_progress_every_n_steps`
		body = {
			// enable_hr: false,
			// denoising_strength: 0,
			// firstphase_width: 0,
			// firstphase_height: 0,
			prompt: parameters.prompt, // TODO: Escape?
			styles: [], // TODO: Remove?
			seed: Number(parameters.seed), // TODO: Validate
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
			negative_prompt: parameters.negativePrompt, // TODO: Escape?
			eta: 0, // TODO: Remove?
			// s_churn: 0,
			s_tmax: 0, // TODO: Remove?
			// s_tmin: 0,
			// s_noise: 1,
			override_settings: {
				show_progress_every_n_steps: 1,
			},
			sampler_index: parameters.samplingMethod, // sampler_index: "Euler", // TODO: Validate
		};
	} else {
		const sourceImageBase64 = await blobToBase64(image);

		endpoint = `${hostname}sdapi/v1/img2img`;

		body = {
			init_images: [sourceImageBase64],
			// resize_mode: 0,
			denoising_strength: parameters.denoisingStrength,
			mask: "", // TODO: Remove?
			// mask_blur: 4,
			// inpainting_fill: 0,
			// inpaint_full_res: true,
			// inpaint_full_res_padding: 0,
			// inpainting_mask_invert: 0,
			prompt: parameters.prompt, // TODO: Escape?
			styles: [], // TODO: Remove?
			seed: Number(parameters.seed), // TODO: Validate
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
			negative_prompt: parameters.negativePrompt, // TODO: Escape?
			eta: 0, // TODO: Remove?
			// s_churn: 0,
			s_tmax: 0, // TODO: Remove?
			// s_tmin: 0,
			// s_noise: 1,
			override_settings: {
				show_progress_every_n_steps: 5,
			},
			sampler_index: parameters.samplingMethod, // sampler_index: "Euler", // TODO: Validate
			// include_init_images: false, // TODO: Set to true?
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
		const { status } = await fetch(`${hostname}sdapi/v1/progress/?skip_current_image=true`, { signal: statusAbortController.signal, method: "GET", mode: "cors" });

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
