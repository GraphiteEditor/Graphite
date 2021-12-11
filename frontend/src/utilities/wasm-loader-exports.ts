/* eslint-disable @typescript-eslint/no-explicit-any */
import { JsMessageType } from "@/state/js-dispatcher";

// These functions are exported to the wasm module.
// See /frontend/wasm/src/lib.rs
type FrontendMessageHandler = (messageType: string, message: any) => void;
export function handleJsMessage(callback: FrontendMessageHandler, responseType: JsMessageType, responseData: any) {
	callback(responseType, responseData);
}
