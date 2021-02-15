const wasm = import("./pkg");

wasm
	.then(wasm => wasm.greet())
	.catch(console.error);