import { WasmBezier } from "@/../wasm/pkg";
import { capOptions, tSliderOptions, bezierTValueVariantOptions, errorOptions, minimumSeparationOptions } from "@/utils/options";
import type { BezierDemoOptions, WasmBezierInstance, BezierCallback, InputOption } from "@/utils/types";
import { BEZIER_T_VALUE_VARIANTS } from "@/utils/types";

const bezierFeatures = {
	constructor: {
		name: "Constructor",
		callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.to_svg(),
	},
	"bezier-through-points": {
		name: "Bezier Through Points",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => {
			const points = JSON.parse(bezier.get_points());
			if (Object.values(options).length === 1) {
				return WasmBezier.quadratic_through_points(points, options.t);
			}
			return WasmBezier.cubic_through_points(points, options.t, options["midpoint separation"]);
		},
		demoOptions: {
			Linear: {
				disabled: true,
			},
			Quadratic: {
				customPoints: [
					[30, 50],
					[120, 70],
					[160, 170],
				],
				inputOptions: [
					{
						min: 0.01,
						max: 0.99,
						step: 0.01,
						default: 0.5,
						variable: "t",
					},
				],
			},
			Cubic: {
				customPoints: [
					[30, 50],
					[120, 70],
					[160, 170],
				],
				inputOptions: [
					{
						min: 0.01,
						max: 0.99,
						step: 0.01,
						default: 0.5,
						variable: "t",
					},
					{
						min: 0,
						max: 100,
						step: 2,
						default: 30,
						variable: "midpoint separation",
					},
				],
			},
		},
	},
	length: {
		name: "Length",
		callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.length(),
	},
	evaluate: {
		name: "Evaluate",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined): string => bezier.evaluate(options.t, BEZIER_T_VALUE_VARIANTS[options.TVariant]),
		demoOptions: {
			Quadratic: {
				inputOptions: [bezierTValueVariantOptions, tSliderOptions],
			},
		},
	},
	"lookup-table": {
		name: "Lookup Table",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined): string => bezier.compute_lookup_table(options.steps, BEZIER_T_VALUE_VARIANTS[options.TVariant]),
		demoOptions: {
			Quadratic: {
				inputOptions: [
					bezierTValueVariantOptions,
					{
						min: 2,
						max: 15,
						step: 1,
						default: 5,
						variable: "steps",
					},
				],
			},
		},
	},
	derivative: {
		name: "Derivative",
		callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.derivative(),
		demoOptions: {
			Linear: {
				disabled: true,
			},
			Quadratic: {
				customPoints: [
					[30, 40],
					[110, 50],
					[120, 130],
				],
			},
			Cubic: {
				customPoints: [
					[50, 50],
					[60, 100],
					[100, 140],
					[140, 150],
				],
			},
		},
	},
	tangent: {
		name: "Tangent",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined): string => bezier.tangent(options.t, BEZIER_T_VALUE_VARIANTS[options.TVariant]),
		demoOptions: {
			Quadratic: {
				inputOptions: [bezierTValueVariantOptions, tSliderOptions],
			},
		},
	},
	"tangents-to-point": {
		name: "Tangents To Point",
		callback: (bezier: WasmBezierInstance, _: Record<string, number>, mouseLocation?: [number, number]): string =>
			mouseLocation ? bezier.tangents_to_point(mouseLocation[0], mouseLocation[1]) : bezier.to_svg(),
		triggerOnMouseMove: true,
		demoOptions: { Linear: { disabled: true } },
	},
	normal: {
		name: "Normal",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined): string => bezier.normal(options.t, BEZIER_T_VALUE_VARIANTS[options.TVariant]),
		demoOptions: {
			Quadratic: {
				inputOptions: [bezierTValueVariantOptions, tSliderOptions],
			},
		},
	},
	"normals-to-point": {
		name: "Normals To Point",
		callback: (bezier: WasmBezierInstance, _: Record<string, number>, mouseLocation?: [number, number]): string =>
			mouseLocation ? bezier.normals_to_point(mouseLocation[0], mouseLocation[1]) : bezier.to_svg(),
		triggerOnMouseMove: true,
	},
	curvature: {
		name: "Curvature",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined): string => bezier.curvature(options.t, BEZIER_T_VALUE_VARIANTS[options.TVariant]),
		demoOptions: {
			Linear: {
				disabled: true,
			},
			Quadratic: {
				inputOptions: [bezierTValueVariantOptions, tSliderOptions],
			},
			Cubic: {
				inputOptions: [bezierTValueVariantOptions, { ...tSliderOptions, default: 0.7 }],
			},
		},
	},
	split: {
		name: "Split",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined): string => bezier.split(options.t, BEZIER_T_VALUE_VARIANTS[options.TVariant]),
		demoOptions: {
			Quadratic: {
				inputOptions: [bezierTValueVariantOptions, tSliderOptions],
			},
		},
	},
	trim: {
		name: "Trim",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined): string => bezier.trim(options.t1, options.t2, BEZIER_T_VALUE_VARIANTS[options.TVariant]),
		demoOptions: {
			Quadratic: {
				inputOptions: [
					bezierTValueVariantOptions,
					{
						variable: "t1",
						min: 0,
						max: 1,
						step: 0.01,
						default: 0.25,
					},
					{
						variable: "t2",
						min: 0,
						max: 1,
						step: 0.01,
						default: 0.75,
					},
				],
			},
		},
	},
	project: {
		name: "Project",
		callback: (bezier: WasmBezierInstance, _: Record<string, number>, mouseLocation?: [number, number]): string =>
			mouseLocation ? bezier.project(mouseLocation[0], mouseLocation[1]) : bezier.to_svg(),
		triggerOnMouseMove: true,
	},
	"local-extrema": {
		name: "Local Extrema",
		callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.local_extrema(),
		demoOptions: {
			Linear: {
				disabled: true,
			},
			Quadratic: {
				customPoints: [
					[40, 40],
					[160, 30],
					[110, 150],
				],
			},
			Cubic: {
				customPoints: [
					[160, 180],
					[170, 10],
					[30, 90],
					[180, 160],
				],
			},
		},
	},
	"bounding-box": {
		name: "Bounding Box",
		callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.bounding_box(),
	},
	inflections: {
		name: "Inflections",
		callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.inflections(),
		demoOptions: {
			Linear: {
				disabled: true,
			},
			Quadratic: {
				disabled: true,
			},
		},
	},
	reduce: {
		name: "Reduce",
		callback: (bezier: WasmBezierInstance): string => bezier.reduce(),
	},
	offset: {
		name: "Offset",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.offset(options.distance),
		demoOptions: {
			Quadratic: {
				inputOptions: [
					{
						variable: "distance",
						min: -30,
						max: 30,
						step: 1,
						default: 15,
					},
				],
			},
		},
	},
	outline: {
		name: "Outline",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.outline(options.distance, options.cap),
		demoOptions: {
			Quadratic: {
				inputOptions: [
					{
						variable: "distance",
						min: 0,
						max: 30,
						step: 1,
						default: 15,
					},
					capOptions,
				],
			},
		},
	},
	"graduated-outline": {
		name: "Graduated Outline",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.graduated_outline(options.start_distance, options.end_distance, options.cap),
		demoOptions: {
			Quadratic: {
				inputOptions: [
					{
						variable: "start_distance",
						min: 0,
						max: 30,
						step: 1,
						default: 5,
					},
					{
						variable: "end_distance",
						min: 0,
						max: 30,
						step: 1,
						default: 15,
					},
					capOptions,
				],
			},
		},
		customPoints: {
			Cubic: [
				[31, 94],
				[40, 40],
				[107, 107],
				[106, 106],
			],
		},
	},
	"skewed-outline": {
		name: "Skewed Outline",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string =>
			bezier.skewed_outline(options.distance1, options.distance2, options.distance3, options.distance4, options.cap),
		demoOptions: {
			Quadratic: {
				inputOptions: [
					{
						variable: "distance1",
						min: 0,
						max: 30,
						step: 1,
						default: 20,
					},
					{
						variable: "distance2",
						min: 0,
						max: 30,
						step: 1,
						default: 10,
					},
					{
						variable: "distance3",
						min: 0,
						max: 30,
						step: 1,
						default: 30,
					},
					{
						variable: "distance4",
						min: 0,
						max: 30,
						step: 1,
						default: 5,
					},
					capOptions,
				],
			},
		},
	},
	arcs: {
		name: "Arcs",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.arcs(options.error, options.max_iterations, options.strategy),
		demoOptions: ((): BezierDemoOptions => {
			const inputOptions: InputOption[] = [
				{
					variable: "strategy",
					default: 0,
					inputType: "dropdown",
					options: ["Automatic", "FavorLargerArcs", "FavorCorrectness"],
				},
				{
					variable: "error",
					min: 0.05,
					max: 1,
					step: 0.05,
					default: 0.5,
				},
				{
					variable: "max_iterations",
					min: 50,
					max: 200,
					step: 1,
					default: 100,
				},
			];

			return {
				Linear: {
					disabled: true,
				},
				Quadratic: {
					customPoints: [
						[70, 40],
						[180, 50],
						[160, 150],
					],
					inputOptions,
					disabled: false,
				},
				Cubic: {
					customPoints: [
						[160, 180],
						[170, 10],
						[30, 90],
						[180, 160],
					],
					inputOptions,
					disabled: false,
				},
			};
		})(),
	},
	"intersect-linear": {
		name: "Intersect (Linear Segment)",
		callback: (bezier: WasmBezierInstance): string => {
			const line = [
				[45, 30],
				[195, 160],
			];
			return bezier.intersect_line_segment(line);
		},
	},
	"intersect-quadratic": {
		name: "Intersect (Quadratic Segment)",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => {
			const quadratic = [
				[45, 80],
				[205, 10],
				[115, 120],
			];
			return bezier.intersect_quadratic_segment(quadratic, options.error, options.minimum_separation);
		},
		demoOptions: {
			Quadratic: {
				inputOptions: [errorOptions, minimumSeparationOptions],
			},
		},
	},
	"intersect-cubic": {
		name: "Intersect (Cubic Segment)",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => {
			const cubic = [
				[65, 20],
				[125, 40],
				[65, 120],
				[200, 140],
			];
			return bezier.intersect_cubic_segment(cubic, options.error, options.minimum_separation);
		},
		demoOptions: {
			Quadratic: {
				inputOptions: [errorOptions, minimumSeparationOptions],
			},
		},
	},
	"intersect-self": {
		name: "Intersect (Self)",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.intersect_self(options.error),
		demoOptions: {
			Linear: {
				disabled: true,
			},
			Quadratic: {
				disabled: true,
			},
			Cubic: {
				inputOptions: [errorOptions],
				customPoints: [
					[160, 180],
					[170, 10],
					[30, 90],
					[180, 140],
				],
			},
		},
	},
	"intersect-rectangle": {
		name: "Intersect (Rectangle)",
		callback: (bezier: WasmBezierInstance): string =>
			bezier.intersect_rectangle([
				[75, 50],
				[175, 150],
			]),
	},
	rotate: {
		name: "Rotate",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.rotate(options.angle * Math.PI, 125, 100),
		demoOptions: {
			Quadratic: {
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
		},
	},
	"de-casteljau-points": {
		name: "De Casteljau Points",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined): string => bezier.de_casteljau_points(options.t, BEZIER_T_VALUE_VARIANTS[options.TVariant]),
		demoOptions: {
			Quadratic: {
				inputOptions: [bezierTValueVariantOptions, tSliderOptions],
			},
		},
	},
	join: {
		name: "Join",
		callback: (bezier: WasmBezierInstance): string => {
			const points = JSON.parse(bezier.get_points());
			let examplePoints = [];
			if (points.length === 2) {
				examplePoints = [
					[145, 155],
					[65, 155],
				];
			} else if (points.length === 3) {
				examplePoints = [
					[65, 150],
					[120, 195],
					[190, 145],
				];
			} else {
				examplePoints = [
					[165, 150],
					[110, 110],
					[90, 180],
					[55, 140],
				];
			}
			return bezier.join(examplePoints);
		},
		demoOptions: {
			Linear: {
				customPoints: [
					[70, 40],
					[155, 90],
				],
			},
			Quadratic: {
				customPoints: [
					[185, 40],
					[65, 20],
					[100, 85],
				],
			},
			Cubic: {
				customPoints: [
					[45, 80],
					[65, 20],
					[115, 100],
					[155, 55],
				],
			},
		},
	},
};

export type BezierFeatureKey = keyof typeof bezierFeatures;
export type BezierFeatureOptions = {
	name: string;
	callback: BezierCallback;
	demoOptions?: Partial<BezierDemoOptions>;
	triggerOnMouseMove?: boolean;
};
export default bezierFeatures as Record<BezierFeatureKey, BezierFeatureOptions>;
