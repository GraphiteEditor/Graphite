import { escapeJSON } from "@/utility-functions/escape";
import { blobToBase64 } from "@/utility-functions/files";
import { type RequestResult, requestWithUploadDownloadProgress } from "@/utility-functions/network";
import { stripIndents } from "@/utility-functions/strip-indents";
import { type Editor } from "@/wasm-communication/editor";
import { type AiArtistGenerationParameters } from "@/wasm-communication/messages";

const MAX_POLLING_RETRIES = 4;
const SERVER_STATUS_CHECK_TIMEOUT = 5000;
const SAMPLING_MODES_POLLING_UNSUPPORTED = ["DPM fast", "DPM adaptive"];

let timer: NodeJS.Timeout | undefined;
let terminated = false;

let generatingAbortRequest: XMLHttpRequest | undefined;
let pollingAbortController = new AbortController();
let statusAbortController = new AbortController();

// PUBLICLY CALLABLE FUNCTIONS

export async function aiArtistGenerate(
	parameters: AiArtistGenerationParameters,
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
	editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, 0, "Beginning");

	// Initiate a request to the computation server
	const discloseUploadingProgress = (progress: number): void => {
		editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, progress * 100, "Uploading");
	};
	const { uploaded, result, xhr } = await generate(discloseUploadingProgress, hostname, image, parameters);
	generatingAbortRequest = xhr;

	try {
		// Wait until the request is fully uploaded, which could be slow if the img2img source is large and the user is on a slow connection
		await uploaded;
		editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, 0, "Generating");

		// Begin polling for updates to the in-progress image generation at the specified interval
		// Don't poll if the chosen interval is 0, or if the chosen sampling method does not support polling
		if (refreshFrequency > 0 && !SAMPLING_MODES_POLLING_UNSUPPORTED.includes(parameters.samplingMethod)) {
			const interval = Math.max(refreshFrequency * 1000, 500);
			scheduleNextPollingUpdate(interval, Date.now(), 0, editor, hostname, documentId, layerPath, parameters.resolution);
		}

		// Wait for the final image to be returned by the initial request containing either the full image or the last frame if it was terminated by the user
		const { body, status } = await result;
		if (status < 200 || status > 299) {
			throw new Error(`Request to server failed to return a 200-level status code (${status})`);
		}

		// Extract the final image from the response and convert it to a data blob
		// Highly unstable API
		const base64 = JSON.parse(body)?.data[0]?.[0] as string | undefined;
		if (typeof base64 !== "string" || !base64.startsWith("data:image/png;base64,")) throw new Error("Could not read final image result from server response");
		const blob = await (await fetch(base64)).blob();

		// Send the backend an updated status
		const percent = terminated ? undefined : 100;
		const newStatus = terminated ? "Terminated" : "Idle";
		editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, percent, newStatus);

		// Send the backend a blob URL for the final image
		const blobURL = URL.createObjectURL(blob);
		editor.instance.setAIArtistBlobURL(documentId, layerPath, blobURL, parameters.resolution[0], parameters.resolution[1]);

		// Send the backend the blob data to be stored persistently in the layer
		const u8Array = new Uint8Array(await blob.arrayBuffer());
		editor.instance.setAIArtistImageData(documentId, layerPath, u8Array);
	} catch {
		editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, undefined, "Terminated");

		await aiArtistCheckConnection(hostname, editor);
	}

	abortAndResetGenerating();
	abortAndResetPolling();
}

export async function aiArtistTerminate(hostname: string, documentId: bigint, layerPath: BigUint64Array, editor: Editor): Promise<void> {
	terminated = true;
	abortAndResetPolling();

	try {
		await terminate(hostname);

		editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, undefined, "Terminating");
	} catch {
		abortAndResetGenerating();
		abortAndResetPolling();

		editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, undefined, "Terminated");

		await aiArtistCheckConnection(hostname, editor);
	}
}

export async function aiArtistCheckConnection(hostname: string, editor: Editor): Promise<void> {
	const serverReached = await checkConnection(hostname);
	editor.instance.setAiArtistServerStatus(serverReached);
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
	resolution: [number, number]
): void {
	// Pick a future time that keeps to the user-requested interval if possible, but on slower connections will go as fast as possible without overlapping itself
	const nextPollTimeGoal = timeoutBegan + interval;
	const timeFromNow = Math.max(0, nextPollTimeGoal - Date.now());

	timer = setTimeout(async () => {
		const nextTimeoutBegan = Date.now();

		try {
			const [blob, percentComplete] = await pollImage(hostname);
			if (terminated) return;

			const blobURL = URL.createObjectURL(blob);
			editor.instance.setAIArtistBlobURL(documentId, layerPath, blobURL, resolution[0], resolution[1]);
			editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, percentComplete, "Generating");

			scheduleNextPollingUpdate(interval, nextTimeoutBegan, 0, editor, hostname, documentId, layerPath, resolution);
		} catch {
			if (generatingAbortRequest === undefined) return;

			if (pollingRetries + 1 > MAX_POLLING_RETRIES) {
				abortAndResetGenerating();
				abortAndResetPolling();

				await aiArtistCheckConnection(hostname, editor);
			} else {
				scheduleNextPollingUpdate(interval, nextTimeoutBegan, pollingRetries + 1, editor, hostname, documentId, layerPath, resolution);
			}
		}
	}, timeFromNow);
}

// API COMMUNICATION FUNCTIONS
// These are highly unstable APIs that will need to be updated very frequently, so we currently assume usage of this exact commit from the server:
// https://github.com/AUTOMATIC1111/stable-diffusion-webui/commit/7d6042b908c064774ee10961309d396eabdc6c4a

function endpoint(hostname: string): string {
	// Highly unstable API
	return `${hostname}api/predict/`;
}

async function pollImage(hostname: string): Promise<[Blob, number]> {
	// Highly unstable API
	const result = await fetch(endpoint(hostname), {
		signal: pollingAbortController.signal,
		headers: {
			accept: "*/*",
			"accept-language": "en-US,en;q=0.9",
			"content-type": "application/json",
		},
		referrer: hostname,
		referrerPolicy: "strict-origin-when-cross-origin",
		body: stripIndents`
			{
				"fn_index":3,
				"data":[],
				"session_hash":"0000000000"
			}`,
		method: "POST",
		mode: "cors",
		credentials: "omit",
	});
	const json = await result.json();
	// Highly unstable API
	const percentComplete = Math.abs(Number(json.data[0].match(/(?<="width:).*?(?=%")/)[0])); // The API sometimes returns negative values presumably due to a bug
	// Highly unstable API
	const base64 = json.data[2];

	if (typeof base64 !== "string" || !base64.startsWith("data:image/png;base64,")) return Promise.reject();

	const blob = await (await fetch(base64)).blob();

	return [blob, percentComplete];
}

async function generate(
	discloseUploadingProgress: (progress: number) => void,
	hostname: string,
	image: Blob | undefined,
	parameters: AiArtistGenerationParameters
): Promise<{
	uploaded: Promise<void>;
	result: Promise<RequestResult>;
	xhr?: XMLHttpRequest;
}> {
	let body;
	if (image === undefined || parameters.denoisingStrength === undefined) {
		// Highly unstable API
		body = stripIndents`
		{
			"fn_index":13,
			"data":[
				"${escapeJSON(parameters.prompt)}",
				"${escapeJSON(parameters.negativePrompt)}",
				"None",
				"None",
				${parameters.samples},
				"${parameters.samplingMethod}",
				${parameters.restoreFaces},
				${parameters.tiling},
				1,
				1,
				${parameters.cfgScale},
				${parameters.seed},
				-1,
				0,
				0,
				0,
				false,
				${parameters.resolution[1]},
				${parameters.resolution[0]},
				false,
				0.7,
				0,
				0,
				"None",
				false,
				false,
				null,
				"",
				"Seed",
				"",
				"Nothing",
				"",
				true,
				false,
				false,
				null,
				""
			],
			"session_hash":"0000000000"
		}`;
	} else {
		const sourceImageBase64 = await blobToBase64(image);

		// Highly unstable API
		body = stripIndents`
		{
			"fn_index":33,
			"data":[
				0,
				"${escapeJSON(parameters.prompt)}",
				"${escapeJSON(parameters.negativePrompt)}",
				"None",
				"None",
				"${sourceImageBase64}",
				null,
				null,
				null,
				"Draw mask",
				${parameters.samples},
				"${parameters.samplingMethod}",
				4,
				"fill",
				${parameters.restoreFaces},
				${parameters.tiling},
				1,
				1,
				${parameters.cfgScale},
				${parameters.denoisingStrength},
				${parameters.seed},
				-1,
				0,
				0,
				0,
				false,
				${parameters.resolution[1]},
				${parameters.resolution[0]},
				"Just resize",
				false,
				32,
				"Inpaint masked",
				"",
				"",
				"None",
				"",
				true,
				true,
				"",
				"",
				true,
				50,
				true,
				1,
				0,
				false,
				4,
				1,
				"",
				128,
				8,
				["left","right","up","down"],
				1,
				0.05,
				128,
				4,
				"fill",
				["left","right","up","down"],
				false,
				false,
				null,
				"",
				"",
				64,
				"None",
				"Seed",
				"",
				"Nothing",
				"",
				true,
				false,
				false,
				null,
				"",
				""
			],
			"session_hash":"0000000000"
		}`;
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
	const [result, xhr] = requestWithUploadDownloadProgress(endpoint(hostname), "POST", body, uploadProgress, abortAndResetPolling);
	result.catch(() => uploadedReject());

	// Return the promise that resolves when the request upload is complete, the promise that resolves when the response download is complete, and the XHR so it can be aborted
	return { uploaded, result, xhr };
}

async function terminate(hostname: string): Promise<void> {
	const body = stripIndents`
		{
			"fn_index":2,
			"data":[],
			"session_hash":"0000000000"
		}`;

	await fetch(endpoint(hostname), {
		headers: {
			accept: "*/*",
			"accept-language": "en-US,en;q=0.9",
			"content-type": "application/json",
		},
		referrer: hostname,
		referrerPolicy: "strict-origin-when-cross-origin",
		body,
		method: "POST",
		mode: "cors",
		credentials: "omit",
	});
}

async function checkConnection(hostname: string): Promise<boolean> {
	statusAbortController.abort();
	statusAbortController = new AbortController();

	const timeout = setTimeout(() => statusAbortController.abort(), SERVER_STATUS_CHECK_TIMEOUT);

	const body = stripIndents`
		{
			"fn_index":100,
			"data":[],
			"session_hash":"0000000000"
		}`;

	try {
		await fetch(endpoint(hostname), {
			signal: statusAbortController.signal,
			headers: {
				accept: "*/*",
				"accept-language": "en-US,en;q=0.9",
				"content-type": "application/json",
			},
			referrer: hostname,
			referrerPolicy: "strict-origin-when-cross-origin",
			body,
			method: "POST",
			mode: "cors",
			credentials: "omit",
		});

		clearTimeout(timeout);

		return true;
	} catch (_) {
		return false;
	}
}
