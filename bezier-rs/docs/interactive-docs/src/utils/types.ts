export type WasmRawInstance = typeof import("../../wasm/pkg");
export type WasmBezierInstance = InstanceType<WasmRawInstance["WasmBezier"]>;

export type WasmBezierKey = keyof WasmBezierInstance;
export type WasmBezierMutatorKey = "set_start" | "set_handle_start" | "set_handle_end" | "set_end";

export type BezierCallback = (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: string) => void;

export type Point = {
	x: number;
	y: number;
};

export type BezierPoint = Point & {
	mutator: WasmBezierMutatorKey;
};
