<template>
	<div class="App">
		<h1>Bezier-rs Interactive Documentation</h1>
		<p>This is the interactive documentation for the <b>bezier-rs</b> library. Click and drag on the endpoints of the example curves to visualize the various Bezier utilities and functions.</p>
		<h2>Beziers</h2>
		<div v-for="(feature, index) in bezierFeatures" :key="index">
			<BezierExamplePane :name="feature.name" :callback="feature.callback" :exampleOptions="feature.exampleOptions" :triggerOnMouseMove="feature.triggerOnMouseMove" />
		</div>
		<div v-for="(feature, index) in features" :key="index">
			<ExamplePane
				:template="feature.template"
				:templateOptions="feature.templateOptions"
				:name="feature.name"
				:callback="feature.callback"
				:curveDegrees="feature.curveDegrees"
				:customPoints="feature.customPoints"
			/>
		</div>
		<h2>Subpaths</h2>
		<div v-for="(feature, index) in subpathFeatures" :key="index">
			<SubpathExamplePane :name="feature.name" :callback="feature.callback" />
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, markRaw } from "vue";

import { WasmBezier } from "@/../wasm/pkg";
import { drawBezier, drawCircleSector, drawCurve, drawLine, drawPoint, getContextFromCanvas, COLORS } from "@/utils/drawing";
import { BezierCurveType, CircleSector, Point, WasmBezierInstance, WasmSubpathInstance } from "@/utils/types";

import BezierExamplePane from "@/components/BezierExamplePane.vue";
import ExamplePane from "@/components/ExamplePane.vue";
import SliderExample from "@/components/SliderExample.vue";
import SubpathExamplePane from "@/components/SubpathExamplePane.vue";

const tSliderOptions = {
	min: 0,
	max: 1,
	step: 0.01,
	default: 0.5,
	variable: "t",
};

export default defineComponent({
	data() {
		return {
			bezierFeatures: [
				{
					name: "Constructor",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.to_svg(),
				},
				{
					name: "Bezier Through Points",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => {
						const points: Point[] = JSON.parse(bezier.get_points());
						const formattedPoints: number[][] = points.map((p) => [p.x, p.y]);
						if (Object.values(options).length === 1) {
							return WasmBezier.quadratic_through_points(formattedPoints, options.t);
						}
						return WasmBezier.cubic_through_points(formattedPoints, options.t, options["midpoint separation"]);
					},
					exampleOptions: {
						[BezierCurveType.Linear]: {
							disabled: true,
						},
						[BezierCurveType.Quadratic]: {
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
						[BezierCurveType.Cubic]: {
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
				{
					name: "Length",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.length(),
				},
				{
					name: "Evaluate",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.evaluate(options.t),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tSliderOptions],
						},
					},
				},
				{
					name: "Lookup Table",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.compute_lookup_table(options.steps),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
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
				{
					name: "Derivative",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.derivative(),
					exampleOptions: {
						[BezierCurveType.Linear]: {
							disabled: true,
						},
						[BezierCurveType.Quadratic]: {
							customPoints: [
								[30, 40],
								[110, 50],
								[120, 130],
							],
						},
						[BezierCurveType.Cubic]: {
							customPoints: [
								[50, 50],
								[60, 100],
								[100, 140],
								[140, 150],
							],
						},
					},
				},
				{
					name: "Tangent",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.tangent(options.t),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tSliderOptions],
						},
					},
				},

				{
					name: "Normal",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.normal(options.t),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tSliderOptions],
						},
					},
				},
				{
					name: "Curvature",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.curvature(options.t),
					exampleOptions: {
						[BezierCurveType.Linear]: {
							disabled: true,
						},
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tSliderOptions],
						},
					},
				},
				{
					name: "Split",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.split(options.t),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tSliderOptions],
						},
					},
				},
				{
					name: "Trim",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.trim(options.t1, options.t2),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
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
				},
				{
					name: "Project",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>, mouseLocation: Point): string =>
						mouseLocation ? bezier.project(mouseLocation.x, mouseLocation.y) : bezier.to_svg(),
					triggerOnMouseMove: true,
				},
				{
					name: "Local Extrema",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.local_extrema(),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							customPoints: [
								[40, 40],
								[160, 30],
								[110, 150],
							],
						},
						[BezierCurveType.Cubic]: {
							customPoints: [
								[160, 180],
								[170, 10],
								[30, 90],
								[180, 160],
							],
						},
					},
				},
				{
					name: "Bounding Box",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.bounding_box(),
				},
				{
					name: "Inflections",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.inflections(),
					exampleOptions: {
						[BezierCurveType.Linear]: {
							disabled: true,
						},
						[BezierCurveType.Quadratic]: {
							disabled: true,
						},
					},
				},
				{
					name: "Reduce",
					callback: (bezier: WasmBezierInstance): string => bezier.reduce(),
				},
				{
					name: "Offset",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.offset(options.distance),
				},
				{
					name: "Outline",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.outline(options.distance),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [
								{
									variable: "distance",
									min: 0,
									max: 50,
									step: 1,
									default: 20,
								},
							],
						},
					},
				},
				{
					name: "Graduated Outline",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.graduated_outline(options.start_distance, options.end_distance),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [
								{
									variable: "start_distance",
									min: 0,
									max: 50,
									step: 1,
									default: 30,
								},
								{
									variable: "end_distance",
									min: 0,
									max: 50,
									step: 1,
									default: 30,
								},
							],
							customPoints: [
								[30, 50],
								[83, 99],
								[160, 170],
							],
						},
					},
				},
				{
					name: "Skewed Outline",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string =>
						bezier.skewed_outline(options.distance1, options.distance2, options.distance3, options.distance4),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [
								{
									variable: "distance1",
									min: 0,
									max: 50,
									step: 1,
									default: 20,
								},
								{
									variable: "distance2",
									min: 0,
									max: 50,
									step: 1,
									default: 10,
								},
								{
									variable: "distance3",
									min: 0,
									max: 50,
									step: 1,
									default: 30,
								},
								{
									variable: "distance4",
									min: 0,
									max: 50,
									step: 1,
									default: 5,
								},
							],
						},
					},
				},
			],
			features: [
				{
					name: "De Casteljau Points",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const hullPoints: Point[][] = JSON.parse(bezier.de_casteljau_points(options.t));
						hullPoints.reverse().forEach((iteration: Point[], iterationIndex) => {
							const colorLight = `hsl(${90 * iterationIndex}, 100%, 50%)`;

							iteration.forEach((point: Point, index) => {
								// Skip the anchor and handle points which are already drawn in black
								if (iterationIndex !== hullPoints.length - 1) {
									drawPoint(getContextFromCanvas(canvas), point, 4, colorLight);
								}

								if (index !== 0) {
									const prevPoint: Point = iteration[index - 1];
									drawLine(getContextFromCanvas(canvas), point, prevPoint, colorLight);
								}
							});
						});
					},
					template: markRaw(SliderExample),
					templateOptions: { sliders: [tSliderOptions] },
				},
				{
					name: "Rotate",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);
						const rotatedBezier = JSON.parse(bezier.rotate(options.angle * Math.PI).get_points());
						drawBezier(context, rotatedBezier, null, { curveStrokeColor: COLORS.NON_INTERACTIVE.STROKE_1, radius: 3.5 });
					},
					template: markRaw(SliderExample),
					templateOptions: {
						sliders: [
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
				{
					name: "Intersect (Line Segment)",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance): void => {
						const context = getContextFromCanvas(canvas);
						const line = [
							{ x: 150, y: 150 },
							{ x: 20, y: 20 },
						];
						const mappedLine = line.map((p) => [p.x, p.y]);
						drawLine(context, line[0], line[1], COLORS.NON_INTERACTIVE.STROKE_1);
						const intersections: Float64Array = bezier.intersect_line_segment(mappedLine);
						intersections.forEach((t: number) => {
							const p = JSON.parse(bezier.evaluate_value(t));
							drawPoint(context, p, 3, COLORS.NON_INTERACTIVE.STROKE_2);
						});
					},
				},
				{
					name: "Intersect (Quadratic Segment)",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);
						const points = [
							{ x: 20, y: 80 },
							{ x: 180, y: 10 },
							{ x: 90, y: 120 },
						];
						const mappedPoints = points.map((p) => [p.x, p.y]);
						drawCurve(context, points, COLORS.NON_INTERACTIVE.STROKE_1, 1);
						const intersections: Float64Array = bezier.intersect_quadratic_segment(mappedPoints, options.error);
						intersections.forEach((t: number) => {
							const p = JSON.parse(bezier.evaluate_value(t));
							drawPoint(context, p, 3, COLORS.NON_INTERACTIVE.STROKE_2);
						});
					},
					template: markRaw(SliderExample),
					templateOptions: {
						sliders: [
							{
								variable: "error",
								min: 0.1,
								max: 2,
								step: 0.1,
								default: 0.5,
							},
						],
					},
				},
				{
					name: "Intersect (Cubic Segment)",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);
						const points = [
							{ x: 40, y: 20 },
							{ x: 100, y: 40 },
							{ x: 40, y: 120 },
							{ x: 175, y: 140 },
						];
						const mappedPoints = points.map((p) => [p.x, p.y]);
						drawCurve(context, points, COLORS.NON_INTERACTIVE.STROKE_1, 1);
						const intersections: Float64Array = bezier.intersect_cubic_segment(mappedPoints, options.error);
						intersections.forEach((t: number) => {
							const p = JSON.parse(bezier.evaluate_value(t));
							drawPoint(context, p, 3, COLORS.NON_INTERACTIVE.STROKE_2);
						});
					},
					template: markRaw(SliderExample),
					templateOptions: {
						sliders: [
							{
								variable: "error",
								min: 0.1,
								max: 2,
								step: 0.1,
								default: 0.5,
							},
						],
					},
				},
				{
					name: "Intersect (Self)",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);
						const intersections: number[][] = JSON.parse(bezier.intersect_self(options.error));
						intersections.forEach((tValues: number[]) => {
							const p = JSON.parse(bezier.evaluate_value(tValues[0]));
							drawPoint(context, p, 3, COLORS.NON_INTERACTIVE.STROKE_2);
						});
					},
					template: markRaw(SliderExample),
					templateOptions: {
						sliders: [
							{
								variable: "error",
								min: 0.01,
								max: 1,
								step: 0.05,
								default: 0.5,
							},
						],
					},
					customPoints: {
						[BezierCurveType.Cubic]: [
							[160, 180],
							[170, 10],
							[30, 90],
							[180, 140],
						],
					},
					curveDegrees: new Set([BezierCurveType.Cubic]),
				},
				{
					name: "Arcs",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);
						const arcs: CircleSector[] = JSON.parse(bezier.arcs(options.error, options.max_iterations, options.strategy));
						arcs.forEach((circleSector, index) => {
							drawCircleSector(context, circleSector, `hsl(${40 * index}, 100%, 50%, 75%)`, `hsl(${40 * index}, 100%, 50%, 37.5%)`);
						});
					},
					template: markRaw(SliderExample),
					templateOptions: {
						sliders: [
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
						],
					},
					curveDegrees: new Set([BezierCurveType.Quadratic, BezierCurveType.Cubic]),
					customPoints: {
						[BezierCurveType.Quadratic]: [
							[50, 50],
							[85, 65],
							[100, 100],
						],
						[BezierCurveType.Cubic]: [
							[160, 180],
							[170, 10],
							[30, 90],
							[180, 160],
						],
					},
				},
			],
			subpathFeatures: [
				{
					name: "Constructor",
					callback: (subpath: WasmSubpathInstance): string => subpath.to_svg(),
				},
				{
					name: "Length",
					callback: (subpath: WasmSubpathInstance): string => subpath.length(),
				},
			],
		};
	},
	components: {
		BezierExamplePane,
		ExamplePane,
		SubpathExamplePane,
	},
});
</script>

<style>
#app {
	font-family: Arial, sans-serif;
	text-align: center;
	color: #2c3e50;
	margin-top: 60px;
}
</style>
