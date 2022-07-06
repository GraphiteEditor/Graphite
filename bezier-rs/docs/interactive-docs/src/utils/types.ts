export type WasmRawInstance = typeof import("../../wasm/pkg");
export type WasmBezierInstance = InstanceType<WasmRawInstance["WasmBezier"]>;

export type WasmBezierKey = keyof WasmBezierInstance;
export type WasmBezierConstructorKey = "new_linear" | "new_quadratic" | "new_cubic";
export type WasmBezierMutatorKey = "set_start" | "set_handle_start" | "set_handle_end" | "set_end";

export enum BezierCurveType {
	Linear = "linear",
	Quadratic = "quadratic",
	Cubic = "cubic",
}

export type BezierCallback = (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>, mouseLocation?: Point) => void;

export type SliderOption = {
	min: number;
	max: number;
	step: number;
	default: number;
	variable: string;
	unit?: string;
};

export type TemplateOption = {
	sliders: SliderOption[];
};

export type Point = {
	x: number;
	y: number;
};

export type BezierPoint = Point & {
	mutator: WasmBezierMutatorKey;
};

export type BezierStyleConfig = {
	curveStrokeColor: string;
	handleStrokeColor: string;
	handleLineStrokeColor: string;
	radius: number;
	drawHandles: boolean;
};
