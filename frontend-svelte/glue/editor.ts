import { invoke } from "@tauri-apps/api";
import { createNanoEvents, Emitter } from "nanoevents";

import * as graphite from "graphite-wasm";
import { JsEditorHandle } from "graphite-wasm";
export { JsEditorHandle } from "graphite-wasm";

import type { GraphiteEmitter } from "./emitter_type";

/* init wasm module */
// Provide a random starter seed which must occur after initializing the WASM module, since WASM can't generate its own random numbers
const randomSeedFloat = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);
const randomSeed = BigInt(randomSeedFloat);
graphite.setRandomSeed(randomSeed);

export const editor_pubsub = createNanoEvents() as GraphiteEmitter;
export const editor: JsEditorHandle = new JsEditorHandle((messageType: string, messageData: any): void => {
	editor_pubsub.emit(messageType as any, messageData);
});

export async function updateImage(path: BigUint64Array, mime: string, imageData: Uint8Array, documentId: bigint): Promise<void> {
	const blob = new Blob([imageData], { type: mime });

	const blobURL = URL.createObjectURL(blob);

	// Pre-decode the image so it is ready to be drawn instantly once it's placed into the viewport SVG
	const image = new Image();
	image.src = blobURL;
	await image.decode();

	editor.setImageBlobURL(documentId, path, blobURL, image.naturalWidth, image.naturalHeight);
}

export async function fetchImage(path: BigUint64Array, mime: string, documentId: bigint, url: string): Promise<void> {
	const data = await fetch(url);
	const blob = await data.blob();

	const blobURL = URL.createObjectURL(blob);

	// Pre-decode the image so it is ready to be drawn instantly once it's placed into the viewport SVG
	const image = new Image();
	image.src = blobURL;
	await image.decode();

	editor.setImageBlobURL(documentId, path, blobURL, image.naturalWidth, image.naturalHeight);
}

export async function dispatchTauri(message: unknown): Promise<void> {
	try {
		const response = await invoke("handle_message", { message });
		editor.tauriResponse(response);
	} catch {
		console.error("Failed to dispatch Tauri message");
	}
}
