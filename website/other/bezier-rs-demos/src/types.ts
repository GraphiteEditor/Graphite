import type * as WasmPkg from "@/../wasm/pkg";

type WasmRawInstance = typeof WasmPkg;
export type WasmBezierInstance = InstanceType<WasmRawInstance["WasmBezier"]>;

export type WasmSubpathInstance = InstanceType<WasmRawInstance["WasmSubpath"]>;
export type WasmSubpathManipulatorKey = "set_anchor" | "set_in_handle" | "set_out_handle";
type WasmBezierConstructorKey = "new_linear" | "new_quadratic" | "new_cubic";
type WasmBezierManipulatorKey = "set_start" | "set_handle_start" | "set_handle_end" | "set_end";

type DemoDataCommon = {
	title: string;
	element: HTMLDivElement;
	inputOptions: InputOption[];
	locked: boolean;
	triggerOnMouseMove: boolean;
	sliderData: Record<string, number>;
	sliderUnits: Record<string, string | string[]>;
	activePointIndex: number | undefined;
};
export type DemoDataBezier = DemoDataCommon & {
	kind: "bezier";
	manipulatorKeys: WasmBezierManipulatorKey[];
	bezier: WasmBezierInstance;
	points: number[][];
	callback: BezierCallback;
};
export type DemoDataSubpath = DemoDataCommon & {
	kind: "subpath";
	activeManipulatorIndex: number | undefined;
	manipulatorKeys: WasmSubpathManipulatorKey[] | undefined;
	subpath: WasmSubpathInstance;
	triples: (number[] | undefined)[][];
	callback: SubpathCallback;
};
export type DemoData = DemoDataBezier | DemoDataSubpath;

export const BEZIER_CURVE_TYPE = ["Linear", "Quadratic", "Cubic"] as const;
export type BezierCurveType = (typeof BEZIER_CURVE_TYPE)[number];

export type BezierCallback = (bezier: WasmBezierInstance, options: Record<string, number>, mouseLocation?: [number, number]) => string;
export type SubpathCallback = (subpath: WasmSubpathInstance, options: Record<string, number>, mouseLocation?: [number, number]) => string;

export type BezierDemoOptions = {
	[key in BezierCurveType]: {
		disabled?: boolean;
		inputOptions?: InputOption[];
		customPoints?: number[][];
	};
};

export type InputOption = {
	variable: string;
	min?: number;
	max?: number;
	step?: number;
	default?: number;
	unit?: string | string[];
	inputType?: "slider" | "dropdown";
	options?: string[];
	disabled?: boolean;
};
export type SubpathInputOption = InputOption & {
	isDisabledForClosed?: boolean;
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

export type DemoArgs = {
	title: string;
	disabled?: boolean;
};

export type BezierDemoArgs = {
	points: number[][];
	inputOptions: InputOption[];
} & DemoArgs;

export type SubpathDemoArgs = {
	triples: (number[] | undefined)[][];
	closed: boolean;
} & DemoArgs;

export const BEZIER_T_VALUE_VARIANTS = ["Parametric", "Euclidean"] as const;
export const SUBPATH_T_VALUE_VARIANTS = ["GlobalParametric", "GlobalEuclidean"] as const;

const CAP_VARIANTS = ["Butt", "Round", "Square"] as const;
const JOIN_VARIANTS = ["Bevel", "Miter", "Round"] as const;

export const POINT_INDEX_TO_MANIPULATOR: WasmSubpathManipulatorKey[] = ["set_anchor", "set_in_handle", "set_out_handle"];

// Given the number of points in the curve, map the index of a point to the correct manipulator key
export const MANIPULATOR_KEYS_FROM_BEZIER_TYPE: { [key in BezierCurveType]: WasmBezierManipulatorKey[] } = {
	Linear: ["set_start", "set_end"],
	Quadratic: ["set_start", "set_handle_start", "set_end"],
	Cubic: ["set_start", "set_handle_start", "set_handle_end", "set_end"],
};

export function getBezierDemoPointDefaults() {
	// We use a function to generate a new object each time it is called
	// to prevent one instance from being shared and modified across demos
	return {
		Linear: [
			[55, 60],
			[165, 120],
		],
		Quadratic: [
			[55, 50],
			[165, 30],
			[185, 170],
		],
		Cubic: [
			[55, 30],
			[85, 140],
			[175, 30],
			[185, 160],
		],
	};
}

export function getSubpathDemoArgs(): SubpathDemoArgs[] {
	// We use a function to generate a new object each time it is called
	// to prevent one instance from being shared and modified across demos
	return [
		{
			title: "Open Subpath",
			triples: [
				[[45, 20], undefined, [35, 90]],
				[[175, 40], [85, 40], undefined],
				[[200, 175], undefined, undefined],
				[[125, 100], [65, 120], undefined],
			],
			closed: false,
		},
		{
			title: "Closed Subpath",
			triples: [
				[[60, 125], undefined, [65, 40]],
				[[155, 30], [145, 120], undefined],
				[
					[170, 150],
					[200, 90],
					[95, 185],
				],
			],
			closed: true,
		},
	];
}

export const tSliderOptions = {
	variable: "t",
	inputType: "slider",
	min: -0.01,
	max: 1.01,
	step: 0.01,
	default: 0.5,
};

export const errorOptions = {
	variable: "error",
	inputType: "slider",
	min: 0.1,
	max: 2,
	step: 0.1,
	default: 0.5,
};

export const minimumSeparationOptions = {
	variable: "minimum_separation",
	inputType: "slider",
	min: 0.001,
	max: 0.25,
	step: 0.001,
	default: 0.05,
};

export const intersectionErrorOptions = {
	variable: "error",
	inputType: "slider",
	min: 0.001,
	max: 0.525,
	step: 0.0025,
	default: 0.02,
};

export const separationDiskDiameter = {
	variable: "separation_disk_diameter",
	inputType: "slider",
	min: 2.5,
	max: 25,
	step: 0.1,
	default: 5,
};

export const bezierTValueVariantOptions = {
	variable: "TVariant",
	inputType: "dropdown",
	default: 0,
	options: BEZIER_T_VALUE_VARIANTS,
};

export const subpathTValueVariantOptions = {
	variable: "TVariant",
	inputType: "dropdown",
	default: 0,
	options: SUBPATH_T_VALUE_VARIANTS,
};

export const joinOptions = {
	variable: "join",
	inputType: "dropdown",
	default: 0,
	options: JOIN_VARIANTS,
};

export const capOptions = {
	variable: "cap",
	inputType: "dropdown",
	default: 0,
	options: CAP_VARIANTS,
};
