/* eslint-disable @typescript-eslint/no-explicit-any, func-names */

// Import this function and chain it on all `wasm` imports like: const wasm = import("@/../wasm/pkg").then(panicProxy);
// This works by proxying every function call wrapping a try-catch block to filter out redundant and confusing `RuntimeError: unreachable` exceptions sent to the console
export function panicProxy(module: any) {
	const proxyHandler = {
		get(target: any, propKey: any, receiver: any) {
			const targetValue = Reflect.get(target, propKey, receiver);

			// Keep the original value being accessed if it isn't a function or it is a class
			// TODO: Figure out how to also wrap class constructor functions instead of skipping them for now
			const isFunction = typeof targetValue === "function";
			const isClass = isFunction && /^\s*class\s+/.test(targetValue.toString());
			if (!isFunction || isClass) return targetValue;

			// Replace the original function with a wrapper function that runs the original in a try-catch block
			return function (...args: any) {
				let result;
				try {
					// @ts-expect-error
					result = targetValue.apply(this, args);
				} catch (err: any) {
					// Suppress `unreachable` WebAssembly.RuntimeError exceptions
					if (!`${err}`.startsWith("RuntimeError: unreachable")) throw err;
				}
				return result;
			};
		},
	};

	return new Proxy(module, proxyHandler);
}
