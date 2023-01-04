import * as graphite from "graphite-wasm";
export { JsEditorHandle } from "graphite-wasm";

// Provide a random starter seed which must occur after initializing the WASM module, since WASM can't generate its own random numbers
const randomSeedFloat = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);
const randomSeed = BigInt(randomSeedFloat);
graphite.setRandomSeed(randomSeed);
