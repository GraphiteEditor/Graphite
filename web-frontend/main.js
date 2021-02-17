const wasm = import("./pkg");

wasm.then((binding) => binding.greet()).catch(console.error);
