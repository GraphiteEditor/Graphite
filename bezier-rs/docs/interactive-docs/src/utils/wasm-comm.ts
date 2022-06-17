export type WasmRawInstance = typeof import("../../wasm/pkg");
export type WasmBezierInstance = InstanceType<WasmRawInstance["WasmBezier"]>;
