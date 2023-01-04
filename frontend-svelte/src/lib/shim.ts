import * as graphite from "graphite-wasm";
export { JsEditorHandle } from "graphite-wasm";

// a.setRandomSeed()
// a.init()
// new a.JsEditorHandle().
// Skip if the WASM module is already initialized

// Provide a random starter seed which must occur after initializing the WASM module, since WASM can't generate its own random numbers
const randomSeedFloat = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);
const randomSeed = BigInt(randomSeedFloat);
graphite.setRandomSeed(randomSeed);
graphite.init();
