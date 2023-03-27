import { capOptions, joinOptions, tSliderOptions, subpathTValueVariantOptions, intersectionErrorOptions, minimumSeparationOptions } from "@/utils/options";
import { SubpathCallback, SubpathInputOption, WasmSubpathInstance, SUBPATH_T_VALUE_VARIANTS } from "@/utils/types";

const subpathFeatures = {
	constructor: {
		name: "Constructor",
		callback: (subpath: WasmSubpathInstance): string => subpath.to_svg(),
	},
	insert: {
		name: "Insert",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined): string => subpath.insert(options.t, SUBPATH_T_VALUE_VARIANTS[options.TVariant]),
		inputOptions: [subpathTValueVariantOptions, tSliderOptions],
	},
	length: {
		name: "Length",
		callback: (subpath: WasmSubpathInstance): string => subpath.length(),
	},
	evaluate: {
		name: "Evaluate",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined): string => subpath.evaluate(options.t, SUBPATH_T_VALUE_VARIANTS[options.TVariant]),
		inputOptions: [subpathTValueVariantOptions, tSliderOptions],
	},
	project: {
		name: "Project",
		callback: (subpath: WasmSubpathInstance, _: Record<string, number>, mouseLocation?: [number, number]): string =>
			mouseLocation ? subpath.project(mouseLocation[0], mouseLocation[1]) : subpath.to_svg(),
		triggerOnMouseMove: true,
	},
	tangent: {
		name: "Tangent",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined): string => subpath.tangent(options.t, SUBPATH_T_VALUE_VARIANTS[options.TVariant]),
		inputOptions: [subpathTValueVariantOptions, tSliderOptions],
	},
	normal: {
		name: "Normal",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined): string => subpath.normal(options.t, SUBPATH_T_VALUE_VARIANTS[options.TVariant]),
		inputOptions: [subpathTValueVariantOptions, tSliderOptions],
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
					[80, 30],
					[210, 150],
				],
				options.error,
				options.minimum_separation
			),
		inputOptions: [intersectionErrorOptions, minimumSeparationOptions],
	},
	"intersect-quadratic": {
		name: "Intersect (Quadratic Segment)",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string =>
			subpath.intersect_quadratic_segment(
				[
					[25, 50],
					[205, 10],
					[135, 180],
				],
				options.error,
				options.minimum_separation
			),
		inputOptions: [intersectionErrorOptions, minimumSeparationOptions],
	},
	"intersect-cubic": {
		name: "Intersect (Cubic Segment)",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string =>
			subpath.intersect_cubic_segment(
				[
					[65, 20],
					[125, 40],
					[65, 120],
					[200, 140],
				],
				options.error,
				options.minimum_separation
			),
		inputOptions: [intersectionErrorOptions, minimumSeparationOptions],
	},
	"self-intersect": {
		name: "Self Intersect",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string => subpath.self_intersections(options.error, options.minimum_separation),
		inputOptions: [intersectionErrorOptions, minimumSeparationOptions],
	},
	split: {
		name: "Split",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined): string => subpath.split(options.t, SUBPATH_T_VALUE_VARIANTS[options.TVariant]),
		inputOptions: [subpathTValueVariantOptions, tSliderOptions],
	},
	trim: {
		name: "Trim",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined): string => subpath.trim(options.t1, options.t2, SUBPATH_T_VALUE_VARIANTS[options.TVariant]),
		inputOptions: [subpathTValueVariantOptions, { ...tSliderOptions, default: 0.2, variable: "t1" }, { ...tSliderOptions, variable: "t2" }],
	},
	offset: {
		name: "Offset",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string => subpath.offset(options.distance, options.join),
		inputOptions: [
			{
				variable: "distance",
				min: -25,
				max: 25,
				step: 1,
				default: 10,
			},
			joinOptions,
		],
	},
	outline: {
		name: "Outline",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string => subpath.outline(options.distance, options.join, options.cap),
		inputOptions: [
			{
				variable: "distance",
				min: 0,
				max: 25,
				step: 1,
				default: 10,
			},
			joinOptions,
			{ ...capOptions, isDisabledForClosed: true },
		],
	},
};

export type SubpathFeatureKey = keyof typeof subpathFeatures;
export type SubpathFeatureOptions = {
	name: string;
	callback: SubpathCallback;
	inputOptions?: SubpathInputOption[];
	triggerOnMouseMove?: boolean;
};
export default subpathFeatures as Record<SubpathFeatureKey, SubpathFeatureOptions>;
