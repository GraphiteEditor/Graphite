// This file is instantiated by wasm-bindgen in `/frontend/wasm/src/lib.rs` and re-exports the `handleResponse` function to
// provide access to the global copy of `response-handler.ts` with its shared state, not an isolated duplicate with empty state

import wasm from "@/utilities/wasm-loader";
import { Response, ResponseType } from "./response-handler";

export function handleResponse(responseType: ResponseType, responseData: Response) {
	return wasm().handleResponse(responseType, responseData);
}
