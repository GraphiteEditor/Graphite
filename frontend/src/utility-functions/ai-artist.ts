import { escapeJSON } from "@/utility-functions/escape";
import { blobToBase64 } from "@/utility-functions/files";
import { stripIndents } from "@/utility-functions/strip-indents";
import type { Editor } from "@/wasm-communication/editor";
import type { XY } from "@/wasm-communication/messages";

const MAX_POLLING_RETRIES = 10;
const SERVER_STATUS_CHECK_TIMEOUT = 5000;

let mainRequest: Promise<Blob> | undefined;
let pollingRetries = 0;
let interval: NodeJS.Timer | undefined;
let mainRequestController = new AbortController();
let pollingRequestController = new AbortController();
let timeoutRequestController = new AbortController();

function hostInfo(hostname: string): { hostname: string; endpoint: string } {
	const endpoint = `${hostname}api/predict/`;
	return { hostname, endpoint };
}

export async function terminateAIArtist(hostname: string, documentId: bigint, layerPath: BigUint64Array, editor: Editor): Promise<void> {
	await terminate(hostname);

	abortAndResetPolling();

	editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, undefined, false);
}

export async function checkAIArtist(hostname: string, editor: Editor): Promise<void> {
	const serverReached = await checkServerStatus(hostname);
	editor.instance.setAiArtistServerStatus(serverReached);
}

export async function callAIArtist(
	hostname: string,
	refreshFrequency: number,
	prompt: string,
	negativePrompt: string,
	resolution: XY,
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
	if (mainRequest) return;

	// Immediately set the progress to 0% so the backend knows to update its layout
	editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, 0, true);

	// Initiate a request to the computation server
	const [width, height] = [resolution.x, resolution.y];
	if (image === undefined || denoisingStrength === undefined) {
		mainRequest = txt2img(hostname, prompt, negativePrompt, seed, samples, cfgScale, restoreFaces, tiling, width, height);
	} else {
		mainRequest = img2img(hostname, image, prompt, negativePrompt, seed, samples, cfgScale, denoisingStrength, restoreFaces, tiling, width, height);
	}

	// Begin polling every second for updates to the in-progress image generation
	if (refreshFrequency > 0) {
		pollingRetries = 0;

		const timeInterval = Math.max(refreshFrequency * 1000, 500);
		interval = setInterval(async () => {
			try {
				const [blob, percentComplete] = await pollImage(hostname);

				const blobURL = URL.createObjectURL(blob);
				editor.instance.setAIArtistBlobURL(documentId, layerPath, blobURL, width, height);
				editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, percentComplete, true);
			} catch {
				pollingRetries += 1;

				if (pollingRetries > MAX_POLLING_RETRIES) abortAndReset();
			}
		}, timeInterval);
	}

	// Wait for the final image to be returned by the initial request containing either the full image or the last frame if it was terminated by the user
	const blob = await mainRequest;

	const blobURL = URL.createObjectURL(blob);
	editor.instance.setAIArtistBlobURL(documentId, layerPath, blobURL, width, height);
	editor.instance.setAIArtistGeneratingStatus(documentId, layerPath, 100, false);
	abortAndReset();
}

function abortAndReset(): void {
	mainRequestController.abort();
	mainRequestController = new AbortController();
	mainRequest = undefined;

	abortAndResetPolling();
}

function abortAndResetPolling(): void {
	pollingRequestController.abort();
	pollingRequestController = new AbortController();
	clearInterval(interval);
	pollingRetries = 0;
}

async function pollImage(hostname: string): Promise<[Blob, number]> {
	const server = hostInfo(hostname);

	// Highly unstable API
	const result = await fetch(server.endpoint, {
		signal: pollingRequestController.signal,
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

async function txt2img(
	hostname: string,
	prompt: string,
	negativePrompt: string,
	seed: number,
	samples: number,
	cfgScale: number,
	restoreFaces: boolean,
	tiling: boolean,
	width: number,
	height: number
): Promise<Blob> {
	const server = hostInfo(hostname);

	// Highly unstable API
	const result = await fetch(server.endpoint, {
		signal: mainRequestController.signal,
		headers: {
			accept: "*/*",
			"accept-language": "en-US,en;q=0.9",
			"content-type": "application/json",
		},
		referrer: hostname,
		referrerPolicy: "strict-origin-when-cross-origin",
		body: stripIndents`
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

async function img2img(
	hostname: string,
	image: Blob,
	prompt: string,
	negativePrompt: string,
	seed: number,
	samples: number,
	cfgScale: number,
	denoisingStrength: number,
	restoreFaces: boolean,
	tiling: boolean,
	width: number,
	height: number
): Promise<Blob> {
	const sourceImageBase64 = await blobToBase64(image);

	const server = hostInfo(hostname);

	const result = await fetch(server.endpoint, {
		signal: mainRequestController.signal,
		headers: {
			accept: "*/*",
			"accept-language": "en-US,en;q=0.9",
			"content-type": "application/json",
		},
		referrer: hostname,
		referrerPolicy: "strict-origin-when-cross-origin",
		body: stripIndents`
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

async function terminate(hostname: string): Promise<void> {
	const server = hostInfo(hostname);

	await fetch(server.endpoint, {
		headers: {
			accept: "*/*",
			"accept-language": "en-US,en;q=0.9",
			"content-type": "application/json",
		},
		referrer: hostname,
		referrerPolicy: "strict-origin-when-cross-origin",
		body: stripIndents`
			{
				"fn_index":1,
				"data":[],
				"session_hash":"0000000000"
			}`,
		method: "POST",
		mode: "cors",
		credentials: "omit",
	});
}

async function checkServerStatus(hostname: string): Promise<boolean> {
	timeoutRequestController.abort();
	timeoutRequestController = new AbortController();

	const timeout = setTimeout(() => timeoutRequestController.abort(), SERVER_STATUS_CHECK_TIMEOUT);

	const server = hostInfo(hostname);

	try {
		await fetch(server.endpoint, {
			signal: timeoutRequestController.signal,
			headers: {
				accept: "*/*",
				"accept-language": "en-US,en;q=0.9",
				"content-type": "application/json",
			},
			referrer: hostname,
			referrerPolicy: "strict-origin-when-cross-origin",
			body: stripIndents`
				{
					"fn_index":54,
					"data":[],
					"session_hash":"0000000000"
				}`,
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
