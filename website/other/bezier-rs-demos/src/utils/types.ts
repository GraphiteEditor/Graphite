export type WasmRawInstance = typeof import("@/../wasm/pkg");
export type WasmBezierInstance = InstanceType<WasmRawInstance["WasmBezier"]>;

export type WasmBezierKey = keyof WasmBezierInstance;
export type WasmBezierConstructorKey = "new_linear" | "new_quadratic" | "new_cubic";
export type WasmBezierManipulatorKey = "set_start" | "set_handle_start" | "set_handle_end" | "set_end";

export type WasmSubpathInstance = InstanceType<WasmRawInstance["WasmSubpath"]>;
export type WasmSubpathManipulatorKey = "set_anchor" | "set_in_handle" | "set_out_handle";

export const BEZIER_CURVE_TYPE = ["Linear", "Quadratic", "Cubic"] as const;
export type BezierCurveType = typeof BEZIER_CURVE_TYPE[number];

export type ComputeType = "Euclidean" | "Parametric";

export type BezierCallback = (bezier: WasmBezierInstance, options: Record<string, number>, mouseLocation?: [number, number], computeType?: ComputeType) => string;
export type SubpathCallback = (subpath: WasmSubpathInstance, options: Record<string, number>, mouseLocation?: [number, number], computeType?: ComputeType) => string;

export type BezierExampleOptions = {
	[key in BezierCurveType]: {
		disabled?: boolean;
		sliderOptions?: SliderOption[];
		customPoints?: number[][];
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

export function getCurveType(numPoints: number): BezierCurveType {
	const mapping: Record<number, BezierCurveType> = {
		2: "Linear",
		3: "Quadratic",
		4: "Cubic",
	};

	if (!(numPoints in mapping)) throw new Error("Invalid number of points for a bezier");

	return mapping[numPoints];
}

export function getConstructorKey(bezierCurveType: BezierCurveType): WasmBezierConstructorKey {
	const mapping: Record<BezierCurveType, WasmBezierConstructorKey> = {
		Linear: "new_linear",
		Quadratic: "new_quadratic",
		Cubic: "new_cubic",
	};
	return mapping[bezierCurveType];
}

export interface ExampleArgs {
	title: string;
	disabled?: boolean;
}

export interface BezierExampleArgs extends ExampleArgs {
	points: number[][];
	sliderOptions: SliderOption[];
}

export interface SubpathExampleArgs extends ExampleArgs {
	triples: (number[] | undefined)[][];
	closed: boolean;
}

export interface Example extends HTMLElement {
	sliderOptions: SliderOption[];
	sliderData: Record<string, number>;
	sliderUnits: Record<string, string | string[]>;

	drawExample(figure: HTMLElement, mouseLocation?: [number, number]): void;
	onMouseDown(event: MouseEvent): void;
	onMouseUp(): void;
	onMouseMove(event: MouseEvent): void;
	getSliderUnit(sliderValue: number, variable: string): string;
}

export interface ExamplePane extends HTMLElement {
	name: string;
	examples: ExampleArgs[];
	id: string;
	chooseComputeType: boolean;
	computeType: ComputeType;
	buildExample(example: ExampleArgs): Example;
}
