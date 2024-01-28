import { capOptions, joinOptions, tSliderOptions, subpathTValueVariantOptions, intersectionErrorOptions, minimumSeparationOptions, separationDiskDiameter } from "@/utils/options";
import type { SubpathCallback, SubpathInputOption, WasmSubpathInstance } from "@/utils/types";
import { SUBPATH_T_VALUE_VARIANTS } from "@/utils/types";

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
	"lookup-table": {
		name: "Lookup Table",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined): string => subpath.compute_lookup_table(options.steps, SUBPATH_T_VALUE_VARIANTS[options.TVariant]),
		inputOptions: [
			subpathTValueVariantOptions,
			{
				min: 2,
				max: 30,
				step: 1,
				default: 5,
				variable: "steps",
			},
		],
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
	"poisson-disk-points": {
		name: "Poisson-Disk Points",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined): string => subpath.poisson_disk_points(options.separation_disk_diameter),
		inputOptions: [separationDiskDiameter],
	},
	inflections: {
		name: "Inflections",
		callback: (subpath: WasmSubpathInstance): string => subpath.inflections(),
	},
	"intersect-linear": {
		name: "Intersect (Linear Segment)",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string =>
			subpath.intersect_line_segment(
				[
					[80, 30],
					[210, 150],
				],
				options.error,
				options.minimum_separation,
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
				options.minimum_separation,
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
				options.minimum_separation,
			),
		inputOptions: [intersectionErrorOptions, minimumSeparationOptions],
	},
	"intersect-self": {
		name: "Intersect (Self)",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string => subpath.self_intersections(options.error, options.minimum_separation),
		inputOptions: [intersectionErrorOptions, minimumSeparationOptions],
	},
	"intersect-rectangle": {
		name: "Intersect (Rectangle)",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string =>
			subpath.intersect_rectangle(
				[
					[75, 50],
					[175, 150],
				],
				options.error,
				options.minimum_separation,
			),
		inputOptions: [intersectionErrorOptions, minimumSeparationOptions],
	},
	curvature: {
		name: "Curvature",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined): string => subpath.curvature(options.t, SUBPATH_T_VALUE_VARIANTS[options.TVariant]),
		inputOptions: [subpathTValueVariantOptions, { ...tSliderOptions, default: 0.2 }],
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
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string => subpath.offset(options.distance, options.join, options.miter_limit),
		inputOptions: [
			{
				variable: "distance",
				min: -25,
				max: 25,
				step: 1,
				default: 10,
			},
			joinOptions,
			{
				variable: "join: Miter - limit",
				min: 1,
				max: 10,
				step: 0.25,
				default: 4,
			},
		],
	},
	outline: {
		name: "Outline",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string => subpath.outline(options.distance, options.join, options.cap, options.miter_limit),
		inputOptions: [
			{
				variable: "distance",
				min: 0,
				max: 25,
				step: 1,
				default: 10,
			},
			joinOptions,
			{
				variable: "join: Miter - limit",
				min: 1,
				max: 10,
				step: 0.25,
				default: 4,
			},
			{ ...capOptions, isDisabledForClosed: true },
		],
	},
	rotate: {
		name: "Rotate",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>): string => subpath.rotate(options.angle * Math.PI, 125, 100),
		inputOptions: [
			{
				variable: "angle",
				min: 0,
				max: 2,
				step: 1 / 50,
				default: 0.12,
				unit: "π",
			},
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
