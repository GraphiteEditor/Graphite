import { tSliderOptions } from "@/utils/options";
import { ComputeType, SliderOption, SubpathCallback, WasmSubpathInstance } from "@/utils/types";

const tErrorOptions = {
	variable: "error",
	min: 0.001,
	max: 0.525,
	step: 0.0025,
	default: 0.02,
};

const tMinimumSeperationOptions = {
	variable: "minimum_seperation",
	min: 0.001,
	max: 0.25,
	step: 0.001,
	default: 0.05,
};

const subpathFeatures = {
	Constructor: {
		callback: (subpath: WasmSubpathInstance): string => subpath.to_svg(),
	},
	Insert: {
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined, computeType: ComputeType): string => subpath.insert(options.computeArgument, computeType),
		sliderOptions: [{ ...tSliderOptions, variable: "computeArgument" }],
		// TODO: Uncomment this after implementing the Euclidean version
		// chooseComputeType: true,
	},
	Length: {
		callback: (subpath: WasmSubpathInstance): string => subpath.length(),
	},
	Evaluate: {
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined, computeType: ComputeType): string => subpath.evaluate(options.computeArgument, computeType),
		sliderOptions: [{ ...tSliderOptions, variable: "computeArgument" }],
		chooseComputeType: true,
	},
	Project: {
		callback: (subpath: WasmSubpathInstance, _: Record<string, number>, mouseLocation?: [number, number]): string =>
			mouseLocation ? subpath.project(mouseLocation[0], mouseLocation[1]) : subpath.to_svg(),
		triggerOnMouseMove: true,
	},
	"Intersect (Line Segment)": {
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string =>
			subpath.intersect_line_segment(
				[
					[150, 150],
					[20, 20],
				],
				options.error,
				options.minimum_seperation
			),
		sliderOptions: [tErrorOptions, tMinimumSeperationOptions],
	},
	"Intersect (Quadratic segment)": {
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string =>
			subpath.intersect_quadratic_segment(
				[
					[20, 80],
					[180, 10],
					[90, 120],
				],
				options.error,
				options.minimum_seperation
			),
		sliderOptions: [tErrorOptions, tMinimumSeperationOptions],
	},
	"Intersect (Cubic segment)": {
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string =>
			subpath.intersect_cubic_segment(
				[
					[40, 20],
					[100, 40],
					[40, 120],
					[175, 140],
				],
				options.error,
				options.minimum_seperation
			),
		sliderOptions: [tErrorOptions, tMinimumSeperationOptions],
	},
	"Self Intersect": {
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string => subpath.self_intersections(options.error, options.minimum_seperation),
		sliderOptions: [tErrorOptions, tMinimumSeperationOptions],
	},
};

export type SubpathFeatureName = keyof typeof subpathFeatures;
export type SubpathFeatureOptions = {
	callback: SubpathCallback;
	sliderOptions?: SliderOption[];
	triggerOnMouseMove?: boolean;
	chooseComputeType?: boolean;
};
export default subpathFeatures as Record<SubpathFeatureName, SubpathFeatureOptions>;
