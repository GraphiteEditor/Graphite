import { JsDispatcher, JsMessageType } from "@/state/js-dispatcher";
import { WasmInstance } from "./js-messages";

let wasmImport: WasmInstance | null = null;
export async function initWasm() {
	if (wasmImport !== null) return;

	wasmImport = await import("@/../wasm/pkg");
}

function getWasmInstance() {
	if (!wasmImport) {
		throw new Error("Wasm was not initialized at application startup");
	}
	return wasmImport;
}

export class EditorState {
	readonly dispatcher = new JsDispatcher();

	readonly instance: InstanceType<WasmInstance["Editor"]>;

	readonly rawWasm: WasmInstance;

	constructor() {
		const wasm = getWasmInstance();
		// Use an arrow function to preserve this context
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		this.instance = new wasm.Editor((messageType: JsMessageType, data: any) => this.dispatcher.handleJsMessage(messageType, data, this.rawWasm));
		this.rawWasm = wasm;
	}
}
