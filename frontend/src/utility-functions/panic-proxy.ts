// This works by proxying every function call and wrapping a try-catch block to filter out redundant and confusing
// `RuntimeError: unreachable` exceptions that would normally be printed in the browser's JS console upon a panic.
export function panicProxy<T extends object>(module: T): T {
	const proxyHandler = {
		get(target: T, propKey: string | symbol, receiver: unknown): unknown {
			const targetValue = Reflect.get(target, propKey, receiver);

			// Keep the original value being accessed if it isn't a function
			const isFunction = typeof targetValue === "function";
			if (!isFunction) return targetValue;

			// Special handling to wrap the return of a constructor in the proxy
			const isClass = isFunction && /^\s*class\s+/.test(targetValue.toString());
			if (isClass) {
				// eslint-disable-next-line func-names
				return function (...args: unknown[]): unknown {
					// eslint-disable-next-line new-cap
					const result = new targetValue(...args);
					return panicProxy(result);
				};
			}

			// Replace the original function with a wrapper function that runs the original in a try-catch block
			// eslint-disable-next-line func-names
			return function (...args: unknown[]): unknown {
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
