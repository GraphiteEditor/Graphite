import { JsMessageType } from "@/utilities/js-messages";

// This file is instantiated by wasm-bindgen in `/frontend/wasm/src/lib.rs`
type FrontendMessageHandler = (messageType: JsMessageType, message: Record<string, unknown>) => void;
export function handleJsMessage(callback: FrontendMessageHandler, responseType: JsMessageType, responseData: Record<string, unknown>) {
	callback(responseType, responseData);
}
