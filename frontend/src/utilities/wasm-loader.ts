import { panicProxy } from "@/utilities/panic-proxy";

export type EditorWasm = typeof import("@/../wasm/pkg");
let instance: EditorWasm | null = null;
export default function wasm(): EditorWasm {
	if (instance === null) throw new Error("The wasm module wasn't initialized. Call initWasm() first.");
	return instance;
}

export async function initWasm() {
	if (instance !== null) return;
	const module = await import("@/../wasm/pkg");
	instance = panicProxy(module);
}
