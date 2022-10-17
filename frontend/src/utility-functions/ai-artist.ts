import { escapeJSON } from "@/utility-functions/escape";
import { blobToBase64 } from "@/utility-functions/files";
import { type RequestResult, requestWithUploadDownloadProgress } from "@/utility-functions/network";
import { stripIndents } from "@/utility-functions/strip-indents";
import { type Editor } from "@/wasm-communication/editor";

type UploadedAndResult = {
	uploaded: Promise<void>;
	result: Promise<RequestResult>;
	xhr: XMLHttpRequest | undefined;
};

const MAX_POLLING_RETRIES = 4;
const SERVER_STATUS_CHECK_TIMEOUT = 5000;

let timer: NodeJS.Timeout | undefined;
let terminated = false;

let generatingAbortRequest: XMLHttpRequest | undefined;
let pollingAbortController = new AbortController();
let statusAbortController = new AbortController();

export async function aiArtistGenerate(
	hostname: string,
	refreshFrequency: number,
	prompt: string,
	negativePrompt: string,
	resolution: [number, number],
	seed: number,
	samples: number,
	cfgScale: number,
	denoisingStrength: number | undefined,
	restoreFaces: boolean,
	tiling: boolean,
	image: Blob | undefined,
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
	const { uploaded, result, xhr } = await generate(discloseUploadingProgress, hostname, image, prompt, negativePrompt, seed, samples, cfgScale, denoisingStrength, restoreFaces, tiling, resolution);
	generatingAbortRequest = xhr;

	try {
		// Wait until the request is fully uploaded, which could be slow if the img2img source is large and the user is on a slow connection
		await uploaded;
		editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, 0, "Generating");

		// Begin polling every second for updates to the in-progress image generation
		if (refreshFrequency > 0) {
			const interval = Math.max(refreshFrequency * 1000, 500);
			scheduleNextPollingUpdate(interval, Date.now(), 0, editor, hostname, documentId, layerPath, resolution);
		}

		// Wait for the final image to be returned by the initial request containing either the full image or the last frame if it was terminated by the user
		const { body, status } = await result;
		if (status < 200 || status > 299) {
			throw new Error(`Request to server failed to return a 200-level status code (${status})`);
		}

		// Extract the final image from the response and convert it to a data blob
		const base64 = JSON.parse(body)?.data[0]?.[0] as string | undefined; // Highly unstable API
		if (typeof base64 !== "string" || !base64.startsWith("data:image/png;base64,")) throw new Error("Could not read final image result from server response");
		const blob = await (await fetch(base64)).blob();

		// Send the backend an updated status
		const percent = terminated ? undefined : 100;
		const newStatus = terminated ? "Terminated" : "Idle";
		editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, percent, newStatus);

		// Send the backend a blob URL for the final image
		const blobURL = URL.createObjectURL(blob);
		editor.instance.setAIArtistBlobURL(documentId, layerPath, blobURL, resolution[0], resolution[1]);

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

function hostInfo(hostname: string): { hostname: string; endpoint: string } {
	const endpoint = `${hostname}api/predict/`;
	return { hostname, endpoint };
}

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

function abortAndResetGenerating(): void {
	generatingAbortRequest?.abort();
	generatingAbortRequest = undefined;
}

function abortAndResetPolling(): void {
	pollingAbortController.abort();
	pollingAbortController = new AbortController();
	clearTimeout(timer);
}

async function pollImage(hostname: string): Promise<[Blob, number]> {
	const server = hostInfo(hostname);

	// Highly unstable API
	const result = await fetch(server.endpoint, {
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
				"fn_index":2,
				"data":[],
				"session_hash":"0000000000"
			}`,
		method: "POST",
		mode: "cors",
		credentials: "omit",
	});
	const json = await result.json();
	const percentComplete = Number(json.data[0].match(/(?<="width:).*?(?=%")/)[0]); // Highly unstable API
	const base64 = json.data[2]; // Highly unstable API

	if (typeof base64 !== "string" || !base64.startsWith("data:image/png;base64,")) return Promise.reject();

	const blob = await (await fetch(base64)).blob();

	return [blob, percentComplete];
}

async function generate(
	discloseUploadingProgress: (progress: number) => void,
	hostname: string,
	image: Blob | undefined,
	prompt: string,
	negativePrompt: string,
	seed: number,
	samples: number,
	cfgScale: number,
	denoisingStrength: number | undefined,
	restoreFaces: boolean,
	tiling: boolean,
	[width, height]: [number, number]
): Promise<UploadedAndResult> {
	let body;
	if (image === undefined || denoisingStrength === undefined) {
		// Highly unstable API
		body = stripIndents`
		{
			"fn_index":12,
			"data":[
				"${escapeJSON(prompt)}",
				"${escapeJSON(negativePrompt)}",
				"None",
				"None",
				${samples},
				"Euler a",
				${restoreFaces},
				${tiling},
				1,
				1,
				${cfgScale},
				${seed},
				-1,
				0,
				0,
				0,
				false,
				${height},
				${width},
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
		}`;
	} else {
		const sourceImageBase64 = await blobToBase64(image);

		// Highly unstable API
		body = stripIndents`
		{
			"fn_index":31,
			"data":[
				0,
				"${escapeJSON(prompt)}",
				"${escapeJSON(negativePrompt)}",
				"None",
				"None",
				"${sourceImageBase64}",
				null,
				null,
				null,
				"Draw mask",
				${samples},
				"Euler a",
				4,
				"fill",
				${restoreFaces},
				${tiling},
				1,
				1,
				${cfgScale},
				${denoisingStrength},
				${seed},
				-1,
				0,
				0,
				0,
				false,
				${height},
				${width},
				"Just resize",
				false,
				32,
				"Inpaint masked",
				"",
				"",
				"None",
				"",
				"",
				1,
				50,
				0,
				false,
				4,
				1,
				"<p style=\\"margin-bottom:0.75em\\">Recommended settings: Sampling Steps: 80-100, Sampler: Euler a, Denoising strength: 0.8</p>",
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
				"<p style=\\"margin-bottom:0.75em\\">Will upscale the image to twice the dimensions; use width and height sliders to set tile size</p>",
				64,
				"None",
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
	const server = hostInfo(hostname);
	const uploadProgress = (progress: number): void => {
		if (progress < 1) {
			discloseUploadingProgress(progress);
		} else {
			uploadedResolve();
		}
	};
	const [result, xhr] = requestWithUploadDownloadProgress(server.endpoint, "POST", body, uploadProgress, abortAndResetPolling);
	result.catch(() => uploadedReject());

	// Return the promise that resolves when the request upload is complete, the promise that resolves when the response download is complete, and the XHR so it can be aborted
	return { uploaded, result, xhr };
}

async function terminate(hostname: string): Promise<void> {
	const server = hostInfo(hostname);

	const body = stripIndents`
		{
			"fn_index":1,
			"data":[],
			"session_hash":"0000000000"
		}`;

	await fetch(server.endpoint, {
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

	const server = hostInfo(hostname);

	const body = stripIndents`
		{
			"fn_index":54,
			"data":[],
			"session_hash":"0000000000"
		}`;

	try {
		await fetch(server.endpoint, {
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
