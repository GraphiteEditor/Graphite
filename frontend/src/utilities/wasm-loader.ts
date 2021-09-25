import { parseResponse, Response, ResponseType } from "@/state/response-handler";

type ResponseCallback = (responseData: Response) => void;

type WasmInstance = typeof import("@/../wasm/pkg");
type EditorWasmBase = InstanceType<WasmInstance["Editor"]>;
type StaticFunctions = Omit<WasmInstance, "Editor" | "init">;

// We expose a type that's a collection of:
// * methods found on our wrapper class (see below)
// * methods from wasm, found on the exported Editor class
// * static functions from wasm.
export type EditorWasm = EditorWasmWrapper & EditorWasmBase & StaticFunctions;

let instance: WasmInstance | null = null;
export async function initWasm() {
	if (instance !== null) return;

	const module = await import("@/../wasm/pkg");
	instance = module;
}

// WasmBindgen is doing some tricky things on instantiation
// The constructor doesn't initialize 'this', but creates a new object with the base class'es prototype
// we cannot extend this class by the usual means.
//
// We cannot extend a Proxy, either.
//
// So we do these kinds of hacks. Sorry.

// Keep track of all active instances for the panic hook.
// Yes, communicating via a global variable is a sin. If you know of a better way, patches welcome.
// Eventually, we will (hopefully) have one Wasm VM per document, thus a separate panic hook per instance.
const editorInstances = new Set<EditorWasmWrapper>();
// eslint-disable-next-line @typescript-eslint/no-explicit-any, no-underscore-dangle
(window as any)._graphiteActiveEditorInstances = editorInstances;
class EditorWasmWrapper {
	readonly wasmEditor: EditorWasmBase;

	private responseMap: Partial<Record<ResponseType, ResponseCallback>> = {};

	constructor() {
		if (!instance) throw new Error("The wasm module wasn't initialized. Call initWasm() first.");
		const boundResponseHandler = (responseType: ResponseType, responseData: Response) => this.handleResponse(responseType, responseData);
		this.wasmEditor = new instance.Editor(boundResponseHandler);
		editorInstances.add(this);
	}

	registerResponseHandler(responseType: ResponseType, callback: ResponseCallback) {
		this.responseMap[responseType] = callback;
	}

	handleResponse(responseType: ResponseType, responseData: Response) {
		if (!instance) throw new Error("The wasm module wasn't initialized. Call initWasm() first.");
		const callback = this.responseMap[responseType];
		const data = parseResponse(instance.wasm_memory, responseType, responseData);

		if (callback && data) {
			callback(data);
		} else if (data) {
			// eslint-disable-next-line no-console
			console.error(`Received a Response of type "${responseType}" but no handler was registered for it from the client.`);
		} else {
			// eslint-disable-next-line no-console
			console.error(`Received a Response of type "${responseType}" but but was not able to parse the data.`);
		}
	}

	free() {
		this.wasmEditor.free();
		editorInstances.delete(this);
		console.log("Freed, editor instances", editorInstances);
	}
}

export default function createEditor(): EditorWasm {
	const editor = new EditorWasmWrapper();

	// we wrap the class in a proxy.
	const proxyHandler = {
		get(target: EditorWasmWrapper, propKey: string | symbol, receiver: unknown) {
			// Look for the property on the wrapper class first
			let targetValue = Reflect.get(target, propKey, receiver);
			if (targetValue !== undefined) return targetValue;

			// Look on the wrapped wasm class next
			const { wasmEditor } = target;
			targetValue = Reflect.get(wasmEditor, propKey);
			if (targetValue !== undefined) {
				// Always call methods on the wasm_editor with this = wasm_editor.
				return proxyMethod(targetValue, wasmEditor);
			}

			// Last, check for static functions on the wasm instance
			// ..except these two. They remain private.
			if (propKey === "Editor" || propKey === "init") return undefined;
			if (instance) {
				targetValue = Reflect.get(instance, propKey);
				if (targetValue !== undefined) {
					return proxyMethod(targetValue, null);
				}
			}

			return undefined;
		},
	};

	// There's no way to explain this proxy-magic to typescript, so we have to use a cast here.
	return new Proxy<EditorWasmWrapper>(editor, proxyHandler) as unknown as EditorWasm;
}

function proxyMethod(targetValue: unknown, _this: unknown): unknown {
	// Anything not callable is returned as is
	if (typeof targetValue !== "function") return targetValue;

	// Class constructors are returned as is
	// TODO: Figure out how to also wrap class constructor functions instead of skipping them for now
	// Note that this is a really expensive check, and we don't currently expose any classes.
	// if (/^\s*class\s+/.test('' + targetValue))
	//	return targetValue;

	// Replace the original function with a wrapper function that runs the original in a try-catch block
	return (...args: unknown[]) => {
		let result;
		try {
			result = targetValue.apply(_this, args);
		} catch (err: unknown) {
			// Suppress `unreachable` WebAssembly.RuntimeError exceptions
			if (!`${err}`.startsWith("RuntimeError: unreachable")) throw err;
		}
		return result;
	};
}
