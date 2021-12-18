/* eslint-disable func-names */

import { createJsDispatcher } from "@/dispatcher/js-dispatcher";
import { JsMessageType } from "@/dispatcher/js-messages";

export type WasmInstance = typeof import("@/../wasm/pkg");
export type RustEditorInstance = InstanceType<WasmInstance["Editor"]>;

let wasmImport: WasmInstance | null = null;
export async function initWasm() {
	if (wasmImport !== null) return;

	wasmImport = await import("@/../wasm/pkg").then(panicProxy);
}

// This works by proxying every function call wrapping a try-catch block to filter out redundant and confusing
// `RuntimeError: unreachable` exceptions sent to the console
function panicProxy<T extends object>(module: T): T {
	const proxyHandler = {
		get(target: T, propKey: string | symbol, receiver: unknown): unknown {
			const targetValue = Reflect.get(target, propKey, receiver);

			// Keep the original value being accessed if it isn't a function or it is a class
			// TODO: Figure out how to also wrap class constructor functions instead of skipping them for now
			const isFunction = typeof targetValue === "function";
			const isClass = isFunction && /^\s*class\s+/.test(targetValue.toString());
			if (!isFunction || isClass) return targetValue;

			// Replace the original function with a wrapper function that runs the original in a try-catch block
			return function (...args: unknown[]) {
				let result;
				try {
					// @ts-expect-error TypeScript does not know what `this` is, since it should be able to be anything
					result = targetValue.apply(this, args);
				} catch (err) {
					// Suppress `unreachable` WebAssembly.RuntimeError exceptions
					if (!`${err}`.startsWith("RuntimeError: unreachable")) throw err;
				}
				return result;
			};
		},
	};

	return new Proxy<T>(module, proxyHandler);
}

function getWasmInstance() {
	if (!wasmImport) {
		throw new Error("Editor WASM backend was not initialized at application startup");
	}
	return wasmImport;
}

export type EditorState = Readonly<ReturnType<typeof createEditorState>>;
export function createEditorState() {
	const dispatcher = createJsDispatcher();
	const rawWasm = getWasmInstance();

	// Use an arrow function to preserve `this` context
	const instance = new rawWasm.Editor((messageType: JsMessageType, data: Record<string, unknown>) => {
		dispatcher.handleJsMessage(messageType, data, rawWasm, instance);
	});

	return {
		dispatcher,
		rawWasm,
		instance,
	};
}
