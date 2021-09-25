import { parseResponse, Response, ResponseType } from "@/state/response-handler";
import { panicProxy } from "@/utilities/panic-proxy";

type ResponseCallback = (responseData: Response) => void;

interface AdditionalProperties {
	registerResponseHandler: (responseType: ResponseType, callback: ResponseCallback) => void;
	handleResponse: (responseType: ResponseType, responseData: Response) => void;
}

export type EditorWasm = typeof import("@/../wasm/pkg") & AdditionalProperties;
let instance: EditorWasm | null = null;
export default function wasm(): EditorWasm {
	if (instance === null) throw new Error("The wasm module wasn't initialized. Call initWasm() first.");
	return instance;
}

export async function initWasm() {
	if (instance !== null) return;

	// This cycle is harmless, because one of the imports in the cycle is asynchronous.
	// Also, this cycle should disappear soon.
	/* eslint-disable-next-line import/no-cycle */
	const module = await import("@/../wasm/pkg");

	const responseMap: Partial<Record<ResponseType, ResponseCallback>> = {};
	// eslint-disable-next-line no-shadow
	function registerResponseHandler(responseType: ResponseType, callback: ResponseCallback) {
		responseMap[responseType] = callback;
	}
	function handleResponse(responseType: ResponseType, responseData: Response) {
		const callback = responseMap[responseType];
		const data = parseResponse(module.wasm_memory, responseType, responseData);

		if (callback && data) {
			callback(data);
		} else if (data) {
			// eslint-disable-next-line no-console
			console.error(`Received a Response of type "${responseType}" but no handler was registered for it from the client.`);
		} else {
			// eslint-disable-next-line no-console
			console.error(`Received a Response of type "${responseType}" but but was not able to parse the data.`);
		}
	}

	const extendedModule: EditorWasm = {
		...module,
		registerResponseHandler,
		handleResponse,
	};
	instance = panicProxy(extendedModule);
}
