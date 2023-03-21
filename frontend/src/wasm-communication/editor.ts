import type WasmBindgenPackage from "@/../wasm/pkg";
import { panicProxy } from "@/utility-functions/panic-proxy";
import { type JsMessageType } from "@/wasm-communication/messages";
import { createSubscriptionRouter, type SubscriptionRouter } from "@/wasm-communication/subscription-router";

export type WasmRawInstance = typeof WasmBindgenPackage;
export type WasmEditorInstance = InstanceType<WasmRawInstance["JsEditorHandle"]>;
export type Editor = Readonly<ReturnType<typeof createEditor>>;

// `wasmImport` starts uninitialized because its initialization needs to occur asynchronously, and thus needs to occur by manually calling and awaiting `initWasm()`
let wasmImport: WasmRawInstance | undefined;
let editorInstance: WasmEditorInstance | undefined;

export async function updateImage(path: BigUint64Array, mime: string, imageData: Uint8Array, documentId: bigint): Promise<void> {
	const blob = new Blob([imageData], { type: mime });

	const blobURL = URL.createObjectURL(blob);

	// Pre-decode the image so it is ready to be drawn instantly once it's placed into the viewport SVG
	const image = new Image();
	image.src = blobURL;
	await image.decode();

	editorInstance?.setImageBlobURL(documentId, path, blobURL, image.naturalWidth, image.naturalHeight);
}

export async function fetchImage(path: BigUint64Array, mime: string, documentId: bigint, url: string): Promise<void> {
	const data = await fetch(url);
	const blob = await data.blob();

	const blobURL = URL.createObjectURL(blob);

	// Pre-decode the image so it is ready to be drawn instantly once it's placed into the viewport SVG
	const image = new Image();
	image.src = blobURL;
	await image.decode();

	editorInstance?.setImageBlobURL(documentId, path, blobURL, image.naturalWidth, image.naturalHeight);
}

const tauri = "__TAURI_METADATA__" in window && import("@tauri-apps/api");
export async function dispatchTauri(message: unknown): Promise<void> {
	if (!tauri) return;

	try {
		const response = await (await tauri).invoke("handle_message", { message });
		editorInstance?.tauriResponse(response);
	} catch {
		// eslint-disable-next-line no-console
		console.error("Failed to dispatch Tauri message");
	}
}

// Should be called asynchronously before `createEditor()`
export async function initWasm(): Promise<void> {
	// Skip if the WASM module is already initialized
	if (wasmImport !== undefined) return;

	// Import the WASM module JS bindings and wrap them in the panic proxy
	// eslint-disable-next-line import/no-cycle
	wasmImport = await import("@/../wasm/pkg").then(panicProxy);

	// Provide a random starter seed which must occur after initializing the WASM module, since WASM can't generate its own random numbers
	const randomSeedFloat = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);
	const randomSeed = BigInt(randomSeedFloat);
	wasmImport?.setRandomSeed(randomSeed);
	// TODO: Tauri: reenable this
	// try {
	// 	await invoke("set_random_seed", { seed: randomSeedFloat });
	// } catch {
	// 	// Ignore errors
	// }
}

// Should be called after running `initWasm()` and its promise resolving
// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createEditor() {
	// Raw: Object containing several callable functions from `editor_api.rs` defined directly on the WASM module, not the editor instance (generated by wasm-bindgen)
	if (!wasmImport) throw new Error("Editor WASM backend was not initialized at application startup");
	const raw: WasmRawInstance = wasmImport;

	// Instance: Object containing many functions from `editor_api.rs` that are part of the editor instance (generated by wasm-bindgen)
	const instance: WasmEditorInstance = new raw.JsEditorHandle((messageType: JsMessageType, messageData: Record<string, unknown>): void => {
		// This callback is called by WASM when a FrontendMessage is received from the WASM wrapper editor instance
		// We pass along the first two arguments then add our own `raw` and `instance` context for the last two arguments
		subscriptions.handleJsMessage(messageType, messageData, raw, instance);
	});
	editorInstance = instance;

	// Subscriptions: Allows subscribing to messages in JS that are sent from the WASM backend
	const subscriptions: SubscriptionRouter = createSubscriptionRouter();

	return {
		raw,
		instance,
		subscriptions,
	};
}
