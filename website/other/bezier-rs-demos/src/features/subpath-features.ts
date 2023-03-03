import { tSliderOptions, intersectionErrorOptions, minimumSeparationOptions } from "@/utils/options";
import { TVariant, SliderOption, SubpathCallback, WasmSubpathInstance } from "@/utils/types";

const subpathFeatures = {
	constructor: {
		name: "Constructor",
		callback: (subpath: WasmSubpathInstance): string => subpath.to_svg(),
	},
	insert: {
		name: "Insert",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => subpath.insert(options.t, tVariant),
		sliderOptions: [tSliderOptions],
		chooseTVariant: true,
	},
	length: {
		name: "Length",
		callback: (subpath: WasmSubpathInstance): string => subpath.length(),
	},
	evaluate: {
		name: "Evaluate",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => subpath.evaluate(options.t, tVariant),
		sliderOptions: [tSliderOptions],
		chooseTVariant: true,
	},
	project: {
		name: "Project",
		callback: (subpath: WasmSubpathInstance, _: Record<string, number>, mouseLocation?: [number, number]): string =>
			mouseLocation ? subpath.project(mouseLocation[0], mouseLocation[1]) : subpath.to_svg(),
		triggerOnMouseMove: true,
	},
	tangent: {
		name: "Tangent",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => subpath.tangent(options.t, tVariant),
		sliderOptions: [tSliderOptions],
		chooseTVariant: true,
	},
	normal: {
		name: "Normal",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => subpath.normal(options.t, tVariant),
		sliderOptions: [tSliderOptions],
		chooseTVariant: true,
	},
	"local-extrema": {
		name: "Local Extrema",
		callback: (subpath: WasmSubpathInstance): string => subpath.local_extrema(),
	},
	"bounding-box": {
		name: "Bounding Box",
		callback: (subpath: WasmSubpathInstance): string => subpath.bounding_box(),
	},
	inflections: {
		name: "Inflections",
		callback: (subpath: WasmSubpathInstance): string => subpath.inflections(),
	},
	"intersect-linear": {
		name: "Intersect (Line Segment)",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string =>
			subpath.intersect_line_segment(
				[
					[150, 150],
					[20, 20],
				],
				options.error,
				options.minimum_seperation
			),
		sliderOptions: [intersectionErrorOptions, minimumSeparationOptions],
	},
	"intersect-quadratic": {
		name: "Intersect (Quadratic Segment)",
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
		sliderOptions: [intersectionErrorOptions, minimumSeparationOptions],
	},
	"intersect-cubic": {
		name: "Intersect (Cubic Segment)",
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
		sliderOptions: [intersectionErrorOptions, minimumSeparationOptions],
	},
	"self-intersect": {
		name: "Self Intersect",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string => subpath.self_intersections(options.error, options.minimum_seperation),
		sliderOptions: [intersectionErrorOptions, minimumSeparationOptions],
	},
	split: {
		name: "Split",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => subpath.split(options.t, tVariant),
		sliderOptions: [tSliderOptions],
		chooseTVariant: true,
	},
	trim: {
		name: "Trim",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => subpath.trim(options.tVariant1, options.tVariant2, tVariant),
		sliderOptions: [
			{ ...tSliderOptions, default: 0.2, variable: "tVariant1" },
			{ ...tSliderOptions, variable: "tVariant2" },
		],
		chooseTVariant: true,
	},
	offset: {
		name: "Offset",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string => subpath.offset(options.distance),
		sliderOptions: [
			{
				variable: "distance",
				min: -25,
				max: 25,
				step: 1,
				default: 10,
			},
		],
	},
	outline: {
		name: "Outline",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string => subpath.outline(options.distance),
		sliderOptions: [
			{
				variable: "distance",
				min: 0,
				max: 25,
				step: 1,
				default: 10,
			},
		],
	},
};

export type SubpathFeatureKey = keyof typeof subpathFeatures;
export type SubpathFeatureOptions = {
	name: string;
	callback: SubpathCallback;
	sliderOptions?: SliderOption[];
	triggerOnMouseMove?: boolean;
	chooseTVariant?: boolean;
};
export default subpathFeatures as Record<SubpathFeatureKey, SubpathFeatureOptions>;
