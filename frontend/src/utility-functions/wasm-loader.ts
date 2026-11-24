import init from "/wrapper/pkg/graphite_wasm_wrapper";
import wasmBinaryUrl from "/wrapper/pkg/graphite_wasm_wrapper_bg.wasm?url";

// Initializes the editor's Wasm module, rejoining the parts that CI deployments split the binary into
// to fit under the single-file size limit (see `wasmSplitting` in `vite.config.ts`)
export async function initWasm() {
	if (__WASM_PART_COUNT__ <= 1 && __WASM_SIZE__ === 0) return init();

	// Fetch all parts in parallel (served from the service worker's precache once it is installed)
	const partUrls = __WASM_PART_COUNT__ <= 1 ? [wasmBinaryUrl] : Array.from({ length: __WASM_PART_COUNT__ }, (_, index) => wasmBinaryUrl.replace(/\.wasm$/, `-part${index}.wasm`));
	const partResponses = await Promise.all(partUrls.map((url) => fetch(url)));

	const failedResponse = partResponses.find((response) => !response.ok);
	if (failedResponse) throw new Error(`Failed to fetch Wasm binary part (status ${failedResponse.status}): ${failedResponse.url}`);

	const trackedResponses = trackDownloadProgress(partResponses);

	// eslint-disable-next-line camelcase
	if (trackedResponses.length === 1) return init({ module_or_path: trackedResponses[0] });

	// Rejoin the parts and hand them to wasm-bindgen as a single response, with the MIME type needed for streaming compilation
	const parts = await Promise.all(trackedResponses.map((response) => response.blob()));
	const joined = new Response(new Blob(parts), { headers: { "Content-Type": "application/wasm" } });
	// eslint-disable-next-line camelcase
	return init({ module_or_path: joined });
}

function trackDownloadProgress(responses: Response[]): Response[] {
	let loadedBytes = 0;
	document.body.style.setProperty("--loading-progress", "0");
	document.body.classList.add("loading-progress");

	return responses.map((response) => {
		if (!response.body) return response;

		const counter = new TransformStream<Uint8Array, Uint8Array>({
			transform(chunk, controller) {
				loadedBytes += chunk.byteLength;
				document.body.style.setProperty("--loading-progress", String(Math.min(loadedBytes / __WASM_SIZE__, 1)));
				controller.enqueue(chunk);
			},
		});
		return new Response(response.body.pipeThrough(counter), { status: response.status, statusText: response.statusText, headers: response.headers });
	});
}
