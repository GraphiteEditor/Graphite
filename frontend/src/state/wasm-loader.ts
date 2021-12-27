/* eslint-disable func-names */

import { createJsDispatcher } from "@/dispatcher/js-dispatcher";
import { JsMessageType } from "@/dispatcher/js-messages";

export type WasmInstance = typeof import("@/../wasm/pkg");
export type RustEditorInstance = InstanceType<WasmInstance["JsEditorHandle"]>;

let wasmImport: WasmInstance | null = null;
export async function initWasm() {
	if (wasmImport !== null) return;

	// Separating in two lines satisfies typescript when used below
	const importedWasm = await import("@/../wasm/pkg").then(panicProxy);
	wasmImport = importedWasm;

	const randomSeed = BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
	importedWasm.set_random_seed(randomSeed);
}

// This works by proxying every function call wrapping a try-catch block to filter out redundant and confusing
// `RuntimeError: unreachable` exceptions sent to the console
function panicProxy<T extends object>(module: T): T {
	const proxyHandler = {
		get(target: T, propKey: string | symbol, receiver: unknown): unknown {
			const targetValue = Reflect.get(target, propKey, receiver);

			// Keep the original value being accessed if it isn't a function
			const isFunction = typeof targetValue === "function";
			if (!isFunction) return targetValue;

			// Special handling to wrap the return of a constructor in the proxy
			const isClass = isFunction && /^\s*class\s+/.test(targetValue.toString());
			if (isClass) {
				return function (...args: unknown[]) {
					// eslint-disable-next-line new-cap
					const result = new targetValue(...args);
					return panicProxy(result);
				};
			}

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
	if (wasmImport) return wasmImport;
	throw new Error("Editor WASM backend was not initialized at application startup");
}

export function createEditorState() {
	const dispatcher = createJsDispatcher();
	const rawWasm = getWasmInstance();

	const rustCallback = (messageType: JsMessageType, data: Record<string, unknown>) => {
		dispatcher.handleJsMessage(messageType, data, rawWasm, instance);
	};

	const instance = new rawWasm.JsEditorHandle(rustCallback);

	return {
		dispatcher,
		rawWasm,
		instance,
	};
}
export type EditorState = Readonly<ReturnType<typeof createEditorState>>;
