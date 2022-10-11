import { blobToBase64 } from "@/utility-functions/files";
import type { Editor } from "@/wasm-communication/editor";
import type { XY } from "@/wasm-communication/messages";

export async function callAIArtist(
	prompt: string,
	resolution: XY,
	samples: number,
	cfgScale: number,
	denoisingStrength: number | undefined,
	image: Blob | undefined,
	layerPath: BigUint64Array,
	editor: Editor
): Promise<void> {
	const width = resolution.x;
	const height = resolution.y;

	let final;
	if (image === undefined || denoisingStrength === undefined) final = txt2img(prompt, samples, cfgScale, width, height);
	else final = img2img(image, prompt, samples, cfgScale, denoisingStrength, width, height);

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

async function txt2img(prompt: string, samples: number, cfgScale: number, width: number, height: number): Promise<Blob> {
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
				${cfgScale},
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

async function img2img(image: Blob, prompt: string, samples: number, cfgScale: number, denoisingStrength: number, width: number, height: number): Promise<Blob> {
	const sourceImageBase64 = await blobToBase64(image);

	const result = await fetch("http://192.168.1.10:7860/api/predict/", {
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
				"",
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
				-1,
				-1,
				0,
				0,
				0,
				false,
				${width},
				${height},
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
