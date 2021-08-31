// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function panicProxy(module: any) {
	const proxyHandler = {
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		get(target: any, propKey: any, receiver: any) {
			const interceptedFunction = target[propKey];

			// TODO: Figure out how to wrap constructors, instead of skipping them for now
			const isClass = typeof interceptedFunction === "function" && /^\s*class\s+/.test(interceptedFunction.toString());
			if (isClass) return interceptedFunction;

			const targetValue = Reflect.get(target, propKey, receiver);
			if (typeof targetValue === "function") {
				// eslint-disable-next-line @typescript-eslint/no-explicit-any, func-names
				return function (...args: any) {
					let returned;
					try {
						// @ts-expect-error
						returned = targetValue.apply(this, args);
					} catch (err: any) {
						// Suppress `unreachable` WebAssembly.RuntimeError exceptions
						if (!`${err}`.startsWith("RuntimeError: unreachable")) throw err;
					}
					return returned;
				};
			}
			return targetValue;
		},
	};

	return new Proxy(module, proxyHandler);
}

// Intercept console.error() for panic messages sent by code in the WASM toolchain
let panicDetails = "";

// eslint-disable-next-line no-console
const error = console.error.bind(console);
// eslint-disable-next-line no-console
console.error = (...args) => {
	const details = "".concat(...args).trim();
	if (details.startsWith("panicked at")) panicDetails = details;

	error(...args);
};

export function getPanicDetails(): string {
	return panicDetails;
}
