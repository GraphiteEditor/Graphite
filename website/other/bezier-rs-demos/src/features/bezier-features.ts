import { WasmBezier } from "@/../wasm/pkg";
import { tSliderOptions, errorOptions, minimumSeparationOptions } from "@/utils/options";
import { TVariant, BezierDemoOptions, WasmBezierInstance, BezierCallback } from "@/utils/types";

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
				sliderOptions: [
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
				sliderOptions: [
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
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => bezier.evaluate(options.t, tVariant),
		demoOptions: {
			Quadratic: {
				sliderOptions: [tSliderOptions],
			},
		},
		chooseTVariant: true,
	},
	"lookup-table": {
		name: "Lookup Table",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.compute_lookup_table(options.steps),
		demoOptions: {
			Quadratic: {
				sliderOptions: [
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
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => bezier.tangent(options.t, tVariant),
		demoOptions: {
			Quadratic: {
				sliderOptions: [tSliderOptions],
			},
		},
		chooseTVariant: true,
	},
	normal: {
		name: "Normal",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => bezier.normal(options.t, tVariant),
		demoOptions: {
			Quadratic: {
				sliderOptions: [tSliderOptions],
			},
		},
		chooseTVariant: true,
	},
	curvature: {
		name: "Curvature",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => bezier.curvature(options.t, tVariant),
		demoOptions: {
			Linear: {
				disabled: true,
			},
			Quadratic: {
				sliderOptions: [tSliderOptions],
			},
		},
		chooseTVariant: true,
	},
	split: {
		name: "Split",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => bezier.split(options.t, tVariant),
		demoOptions: {
			Quadratic: {
				sliderOptions: [tSliderOptions],
			},
		},
		chooseTVariant: true,
	},
	trim: {
		name: "Trim",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => bezier.trim(options.t1, options.t2, tVariant),
		demoOptions: {
			Quadratic: {
				sliderOptions: [
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
		chooseTVariant: true,
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
				sliderOptions: [
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
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.outline(options.distance),
		demoOptions: {
			Quadratic: {
				sliderOptions: [
					{
						variable: "distance",
						min: 0,
						max: 30,
						step: 1,
						default: 15,
					},
				],
			},
		},
	},
	"graduated-outline": {
		name: "Graduated Outline",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.graduated_outline(options.start_distance, options.end_distance),
		demoOptions: {
			Quadratic: {
				sliderOptions: [
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
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.skewed_outline(options.distance1, options.distance2, options.distance3, options.distance4),
		demoOptions: {
			Quadratic: {
				sliderOptions: [
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
				],
			},
		},
	},
	arcs: {
		name: "Arcs",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.arcs(options.error, options.max_iterations, options.strategy),
		demoOptions: ((): Omit<BezierDemoOptions, "Linear"> => {
			const sliderOptions = [
				{
					variable: "strategy",
					min: 0,
					max: 2,
					step: 1,
					default: 0,
					unit: [": Automatic", ": FavorLargerArcs", ": FavorCorrectness"],
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
				Quadratic: {
					customPoints: [
						[50, 50],
						[85, 65],
						[100, 100],
					],
					sliderOptions,
					disabled: false,
				},
				Cubic: {
					customPoints: [
						[160, 180],
						[170, 10],
						[30, 90],
						[180, 160],
					],
					sliderOptions,
					disabled: false,
				},
			};
		})(),
	},
	"intersect-linear": {
		name: "Intersect (Line Segment)",
		callback: (bezier: WasmBezierInstance): string => {
			const line = [
				[150, 150],
				[20, 20],
			];
			return bezier.intersect_line_segment(line);
		},
	},
	"intersect-quadratic": {
		name: "Intersect (Quadratic)",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => {
			const quadratic = [
				[20, 80],
				[180, 10],
				[90, 120],
			];
			return bezier.intersect_quadratic_segment(quadratic, options.error, options.minimum_separation);
		},
		demoOptions: {
			Quadratic: {
				sliderOptions: [errorOptions, minimumSeparationOptions],
			},
		},
	},
	"intersect-cubic": {
		name: "Intersect (Cubic)",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => {
			const cubic = [
				[40, 20],
				[100, 40],
				[40, 120],
				[175, 140],
			];
			return bezier.intersect_cubic_segment(cubic, options.error, options.minimum_separation);
		},
		demoOptions: {
			Quadratic: {
				sliderOptions: [errorOptions, minimumSeparationOptions],
			},
		},
	},
	"intersect-self": {
		name: "Intersect (Self)",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.intersect_self(options.error),
		demoOptions: {
			Quadratic: {
				sliderOptions: [errorOptions],
			},
			Cubic: {
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
				[50, 50],
				[150, 150],
			]),
	},
	rotate: {
		name: "Rotate",
		callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.rotate(options.angle * Math.PI, 100, 100),
		demoOptions: {
			Quadratic: {
				sliderOptions: [
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
		callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined, tVariant: TVariant): string => bezier.de_casteljau_points(options.t, tVariant),
		demoOptions: {
			Quadratic: {
				sliderOptions: [tSliderOptions],
			},
		},
		chooseTVariant: true,
	},
	join: {
		name: "Join",
		callback: (bezier: WasmBezierInstance): string => {
			const points = JSON.parse(bezier.get_points());
			let examplePoints = [];
			if (points.length === 2) {
				examplePoints = [
					[120, 155],
					[40, 155],
				];
			} else if (points.length === 3) {
				examplePoints = [
					[40, 150],
					[95, 195],
					[155, 145],
				];
			} else {
				examplePoints = [
					[140, 150],
					[85, 110],
					[65, 180],
					[30, 140],
				];
			}
			return bezier.join(examplePoints);
		},
		demoOptions: {
			Linear: {
				customPoints: [
					[45, 40],
					[130, 90],
				],
			},
			Quadratic: {
				customPoints: [
					[153, 40],
					[40, 20],
					[75, 85],
				],
			},
			Cubic: {
				customPoints: [
					[20, 80],
					[40, 20],
					[90, 100],
					[130, 55],
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
	chooseTVariant?: boolean;
};
export default bezierFeatures as Record<BezierFeatureKey, BezierFeatureOptions>;
