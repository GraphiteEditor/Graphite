import { tSliderOptions } from "@/utils/options";
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
	"bounding-box": {
		name: "Bounding Box",
		callback: (subpath: WasmSubpathInstance): string => subpath.bounding_box(),
	},
	"intersect-linear": {
		name: "Intersect (Line Segment)",
		callback: (subpath: WasmSubpathInstance): string =>
			subpath.intersect_line_segment([
				[150, 150],
				[20, 20],
			]),
	},
	"intersect-quadratic": {
		name: "Intersect (Quadratic segment)",
		callback: (subpath: WasmSubpathInstance): string =>
			subpath.intersect_quadratic_segment([
				[20, 80],
				[180, 10],
				[90, 120],
			]),
	},
	"intersect-cubic": {
		name: "Intersect (Cubic segment)",
		callback: (subpath: WasmSubpathInstance): string =>
			subpath.intersect_cubic_segment([
				[40, 20],
				[100, 40],
				[40, 120],
				[175, 140],
			]),
	},
	split: {
		name: "Split",
		callback: (subpath: WasmSubpathInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => subpath.split(options.t, tVariant),
		sliderOptions: [tSliderOptions],
		chooseTVariant: true,
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
