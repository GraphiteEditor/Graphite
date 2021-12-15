import { JsMessageType } from "@/state/js-dispatcher";

// These functions are exported to the wasm module.
// See /frontend/wasm/src/lib.rs
type FrontendMessageHandler = (messageType: JsMessageType, message: Record<string, unknown>) => void;
export function handleJsMessage(callback: FrontendMessageHandler, responseType: JsMessageType, responseData: Record<string, unknown>) {
	callback(responseType, responseData);
}
