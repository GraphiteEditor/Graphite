import init from "/wrapper/pkg/graphite_wasm_wrapper";
import wasmBinaryUrl from "/wrapper/pkg/graphite_wasm_wrapper_bg.wasm?url";

// Initializes the editor's Wasm module, rejoining the parts that CI deployments split the binary into
// to fit under the single-file size limit (see `wasmSplitting` in `vite.config.ts`)
export async function initWasm() {
	if (__WASM_PART_COUNT__ <= 1 && __WASM_SIZE__ === 0) return init();

	const partUrls = __WASM_PART_COUNT__ <= 1 ? [wasmBinaryUrl] : Array.from({ length: __WASM_PART_COUNT__ }, (_, index) => wasmBinaryUrl.replace(/\.wasm$/, `-part${index}.wasm`));
	// Read the parts straight from the service worker's precache when it has them
	const partResponses = (await cachedParts(partUrls)) || downloadParts(partUrls);

	// eslint-disable-next-line camelcase
	if (partResponses.length === 1) return init({ module_or_path: await partResponses[0] });

	// Rejoin the parts and hand them to wasm-bindgen as a single response, with the MIME type needed for streaming compilation
	const parts = await Promise.all(partResponses.map(async (response) => (await response).blob()));
	const joined = new Response(new Blob(parts), { headers: { "Content-Type": "application/wasm" } });
	// eslint-disable-next-line camelcase
	return init({ module_or_path: joined });
}

async function cachedParts(urls: string[]): Promise<Response[] | undefined> {
	try {
		const responses = await Promise.all(urls.map((url) => caches.match(url)));
		if (responses.every((response): response is Response => response !== undefined)) return responses;
	} catch {
		// Cache Storage is unavailable, such as in some private windows
	}
	return undefined;
}

function downloadParts(urls: string[]): Promise<Response>[] {
	const trackProgress = trackDownloadProgress();

	return urls.map(async (url) => {
		const response = await fetch(url);
		if (!response.ok) throw new Error(`Failed to fetch Wasm binary part (status ${response.status}): ${response.url}`);
		return trackProgress(response);
	});
}

function trackDownloadProgress(): (response: Response) => Response {
	let loadedBytes = 0;
	document.body.style.setProperty("--loading-progress", "0");
	document.body.classList.add("loading-progress");

	return (response) => {
		if (!response.body) return response;

		const counter = new TransformStream<Uint8Array, Uint8Array>({
			transform(chunk, controller) {
				loadedBytes += chunk.byteLength;
				document.body.style.setProperty("--loading-progress", String(Math.min(loadedBytes / __WASM_SIZE__, 1)));
				controller.enqueue(chunk);
			},
		});
		return new Response(response.body.pipeThrough(counter), { status: response.status, statusText: response.statusText, headers: response.headers });
	};
}
