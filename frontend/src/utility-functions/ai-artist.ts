import { blobToBase64 } from "@/utility-functions/files";
import type { Editor } from "@/wasm-communication/editor";
import type { XY } from "@/wasm-communication/messages";

const MAX_POLLING_RETRIES = 10;

let mainRequest: Promise<Blob> | undefined;
let pollingRetries = 0;
let interval: NodeJS.Timer | undefined;
let mainRequestController = new AbortController();
let pollingRequestController = new AbortController();

export async function terminateAIArtist(layerPath: BigUint64Array, editor: Editor): Promise<void> {
	await terminate();

	abortAndResetPolling();

	editor.instance.setAIArtistTerminated(layerPath);
}

export async function callAIArtist(
	prompt: string,
	negativePrompt: string,
	resolution: XY,
	seed: number,
	samples: number,
	cfgScale: number,
	denoisingStrength: number | undefined,
	image: Blob | undefined,
	layerPath: BigUint64Array,
	editor: Editor
): Promise<void> {
	// Ignore a request to generate a new image while another is already being generated
	if (mainRequest) return;

	// Immediately set the progress to 0% so the backend knows to update its layout
	editor.instance.setAIArtistPercentComplete(layerPath, 0);

	// Initiate a request to the computation server
	const [width, height] = [resolution.x, resolution.y];
	if (image === undefined || denoisingStrength === undefined) {
		mainRequest = txt2img(prompt, negativePrompt, seed, samples, cfgScale, width, height);
	} else {
		mainRequest = img2img(image, prompt, negativePrompt, seed, samples, cfgScale, denoisingStrength, width, height);
	}
	pollingRetries = 0;

	// Begin polling every second for updates to the in-progress image generation
	interval = setInterval(async () => {
		try {
			const [blob, percentComplete] = await pollImage();

			const blobURL = URL.createObjectURL(blob);
			editor.instance.setImageBlobUrl(layerPath, blobURL, width, height);
			editor.instance.setAIArtistPercentComplete(layerPath, percentComplete);
		} catch {
			pollingRetries += 1;

			if (pollingRetries > MAX_POLLING_RETRIES) abortAndReset();
		}
	}, 1000);

	// Wait for the final image to be returned by the initial request containing either the full image or the last frame if it was terminated by the user
	const blob = await mainRequest;

	const blobURL = URL.createObjectURL(blob);
	editor.instance.setImageBlobUrl(layerPath, blobURL, width, height);
	editor.instance.setAIArtistPercentComplete(layerPath, 100);
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

async function pollImage(): Promise<[Blob, number]> {
	// Highly unstable API
	const result = await fetch("http://192.168.1.10:7860/api/predict/", {
		signal: pollingRequestController.signal,
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
	const percentComplete = Number(json.data[0].match(/(?<="width:).*?(?=%")/)[0]); // Highly unstable API
	const base64 = json.data[2]; // Highly unstable API

	if (typeof base64 !== "string" || !base64.startsWith("data:image/png;base64,")) return Promise.reject();

	const blob = await (await fetch(base64)).blob();

	return [blob, percentComplete];
}

async function txt2img(prompt: string, negativePrompt: string, seed: number, samples: number, cfgScale: number, width: number, height: number): Promise<Blob> {
	// Highly unstable API
	const result = await fetch("http://192.168.1.10:7860/api/predict/", {
		signal: mainRequestController.signal,
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
				"${negativePrompt}",
				"None",
				"None",
				${samples},
				"Euler a",
				false,
				false,
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

async function img2img(image: Blob, prompt: string, negativePrompt: string, seed: number, samples: number, cfgScale: number, denoisingStrength: number, width: number, height: number): Promise<Blob> {
	const sourceImageBase64 = await blobToBase64(image);

	const result = await fetch("http://192.168.1.10:7860/api/predict/", {
		signal: mainRequestController.signal,
		headers: {
			accept: "*/*",
			"accept-language": "en-US,en;q=0.9",
			"content-type": "application/json",
		},
		referrer: "http://192.168.1.10:7860/",
		referrerPolicy: "strict-origin-when-cross-origin",
		body: `{
			"fn_index":31,
			"data":[
				0,
				"${prompt}",
				"${negativePrompt}",
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
				false,
				false,
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

async function terminate(): Promise<void> {
	await fetch("http://192.168.1.10:7860/api/predict/", {
		headers: {
			accept: "*/*",
			"accept-language": "en-US,en;q=0.9",
			"content-type": "application/json",
		},
		referrer: "http://192.168.1.10:7860/",
		referrerPolicy: "strict-origin-when-cross-origin",
		body: `{
			"fn_index":1,
			"data":[],
			"session_hash":"0000000000"
		}`,
		method: "POST",
		mode: "cors",
		credentials: "omit",
	});
}
