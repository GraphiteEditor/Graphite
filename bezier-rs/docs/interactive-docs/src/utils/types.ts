export type WasmRawInstance = typeof import("../../wasm/pkg");
export type WasmBezierInstance = InstanceType<WasmRawInstance["WasmBezier"]>;

export type WasmBezierKey = keyof WasmBezierInstance;
export type WasmBezierMutatorKey = "set_start" | "set_handle1" | "set_handle2" | "set_end";

export type Point = {
	x: number;
	y: number;
	r: number;
	mutator: WasmBezierMutatorKey;
	selected?: boolean;
};
