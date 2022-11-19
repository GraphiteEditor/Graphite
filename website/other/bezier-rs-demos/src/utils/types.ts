export type WasmRawInstance = typeof import("@/../wasm/pkg");
export type WasmBezierInstance = InstanceType<WasmRawInstance["WasmBezier"]>;

export type WasmBezierKey = keyof WasmBezierInstance;
export type WasmBezierConstructorKey = "new_linear" | "new_quadratic" | "new_cubic";
export type WasmBezierManipulatorKey = "set_start" | "set_handle_start" | "set_handle_end" | "set_end";

export type WasmSubpathInstance = InstanceType<WasmRawInstance["WasmSubpath"]>;
export type WasmSubpathManipulatorKey = "set_anchor" | "set_in_handle" | "set_out_handle";

export enum BezierCurveType {
	Linear = "Linear",
	Quadratic = "Quadratic",
	Cubic = "Cubic",
}

export type BezierCallback = (bezier: WasmBezierInstance, options: Record<string, number>, mouseLocation?: [number, number], computeType?: boolean) => string;
export type SubpathCallback = (subpath: WasmSubpathInstance) => string;

export type ExampleOptions = {
	[key in BezierCurveType]: {
		disabled: boolean;
		sliderOptions: SliderOption[];
		customPoints: number[][];
	};
};

export type SliderOption = {
	min: number;
	max: number;
	step: number;
	default: number;
	variable: string;
	unit?: string | string[];
};
