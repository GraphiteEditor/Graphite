import init from "/wrapper/pkg/graphite_wasm_wrapper";
import wasmBinaryUrl from "/wrapper/pkg/graphite_wasm_wrapper_bg.wasm?url";

// Initializes the editor's Wasm module, rejoining the parts that CI deployments split the binary into
// to fit under the single-file size limit (see `wasmSplitting` in `vite.config.ts`)
export async function initWasm() {
	// Local and native builds keep the binary whole, letting the wasm-bindgen glue code load it directly
	if (__WASM_PART_COUNT__ <= 1) return init();

	// Fetch all parts in parallel (served from the service worker's precache once it is installed)
	const partRequests = [];
	for (let index = 0; index < __WASM_PART_COUNT__; index += 1) {
		partRequests.push(fetch(wasmBinaryUrl.replace(/\.wasm$/, `-part${index}.wasm`)));
	}
	const partResponses = await Promise.all(partRequests);

	const failedResponse = partResponses.find((response) => !response.ok);
	if (failedResponse) throw new Error(`Failed to fetch Wasm binary part (status ${failedResponse.status}): ${failedResponse.url}`);

	// Rejoin the parts and hand them to wasm-bindgen as a single response, with the MIME type needed for streaming compilation
	const parts = await Promise.all(partResponses.map((response) => response.blob()));
	const joined = new Response(new Blob(parts), { headers: { "Content-Type": "application/wasm" } });
	// eslint-disable-next-line camelcase
	return init({ module_or_path: joined });
}
