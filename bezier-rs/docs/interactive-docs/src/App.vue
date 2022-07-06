<template>
	<div class="App">
		<h1>Bezier-rs Interactive Documentation</h1>
		<p>This is the interactive documentation for the <b>bezier-rs</b> library. Click and drag on the endpoints of the example curves to visualize the various Bezier utilities and functions.</p>
		<div v-for="(feature, index) in features" :key="index">
			<ExamplePane
				:template="feature.template"
				:templateOptions="feature.templateOptions"
				:name="feature.name"
				:callback="feature.callback"
				:createThroughPoints="feature.createThroughPoints"
				:cubicOptions="feature.cubicOptions"
			/>
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, markRaw } from "vue";

import { drawText, drawPoint, drawBezier, drawLine, getContextFromCanvas, drawBezierHelper, COLORS } from "@/utils/drawing";
import { Point, WasmBezierInstance } from "@/utils/types";

import ExamplePane from "@/components/ExamplePane.vue";
import SliderExample from "@/components/SliderExample.vue";

const tSliderOptions = {
	min: 0,
	max: 1,
	step: 0.01,
	default: 0.5,
	variable: "t",
};

const SCALE_UNIT_VECTOR_FACTOR = 50;

export default defineComponent({
	name: "App",
	components: {
		ExamplePane,
	},
	data() {
		return {
			features: [
				{
					name: "Constructor",
					// eslint-disable-next-line
					callback: (): void => {},
				},
				{
					name: "Bezier Through Points",
					// eslint-disable-next-line
					callback: (): void => {},
					createThroughPoints: true,
					template: markRaw(SliderExample),
					templateOptions: {
						sliders: [
							{
								min: 0.01,
								max: 0.99,
								step: 0.01,
								default: 0.5,
								variable: "t",
							},
						],
					},
					cubicOptions: {
						sliders: [
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
								step: 5,
								default: 10,
								variable: "midpoint separation",
							},
						],
					},
				},
				{
					name: "Length",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance): void => {
						drawText(getContextFromCanvas(canvas), `Length: ${bezier.length().toFixed(2)}`, 5, canvas.height - 7);
					},
				},
				{
					name: "Compute",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const point = JSON.parse(bezier.compute(options.t));
						drawPoint(getContextFromCanvas(canvas), point, 4, COLORS.NON_INTERACTIVE.STROKE_1);
					},
					template: markRaw(SliderExample),
					templateOptions: { sliders: [tSliderOptions] },
				},
				{
					name: "Lookup Table",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const lookupPoints = bezier.compute_lookup_table(options.steps);
						lookupPoints.forEach((serialisedPoint, index) => {
							if (index !== 0 && index !== lookupPoints.length - 1) {
								drawPoint(getContextFromCanvas(canvas), JSON.parse(serialisedPoint), 3, COLORS.NON_INTERACTIVE.STROKE_1);
							}
						});
					},
					template: markRaw(SliderExample),
					templateOptions: {
						sliders: [
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
				{
					name: "Tangent",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);

						const intersection = JSON.parse(bezier.compute(options.t));
						const tangent = JSON.parse(bezier.tangent(options.t));

						const tangentEnd = {
							x: intersection.x + tangent.x * SCALE_UNIT_VECTOR_FACTOR,
							y: intersection.y + tangent.y * SCALE_UNIT_VECTOR_FACTOR,
						};

						drawPoint(context, intersection, 3, COLORS.NON_INTERACTIVE.STROKE_1);
						drawLine(context, intersection, tangentEnd, COLORS.NON_INTERACTIVE.STROKE_1);
						drawPoint(context, tangentEnd, 3, COLORS.NON_INTERACTIVE.STROKE_1);
					},
					template: markRaw(SliderExample),
					templateOptions: { sliders: [tSliderOptions] },
				},
				{
					name: "Normal",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);

						const intersection = JSON.parse(bezier.compute(options.t));
						const normal = JSON.parse(bezier.normal(options.t));

						const normalEnd = {
							x: intersection.x - normal.x * SCALE_UNIT_VECTOR_FACTOR,
							y: intersection.y - normal.y * SCALE_UNIT_VECTOR_FACTOR,
						};

						drawPoint(context, intersection, 3, COLORS.NON_INTERACTIVE.STROKE_1);
						drawLine(context, intersection, normalEnd, COLORS.NON_INTERACTIVE.STROKE_1);
						drawPoint(context, normalEnd, 3, COLORS.NON_INTERACTIVE.STROKE_1);
					},
					template: markRaw(SliderExample),
					templateOptions: { sliders: [tSliderOptions] },
				},
				{
					name: "Split",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);
						const bezierPairPoints = JSON.parse(bezier.split(options.t));

						drawBezier(context, bezierPairPoints[0], null, { curveStrokeColor: COLORS.NON_INTERACTIVE.STROKE_2, radius: 3.5 });
						drawBezier(context, bezierPairPoints[1], null, { curveStrokeColor: COLORS.NON_INTERACTIVE.STROKE_1, radius: 3.5 });
					},
					template: markRaw(SliderExample),
					templateOptions: { sliders: [tSliderOptions] },
				},
				{
					name: "Trim",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);
						const trimmedBezier = bezier.trim(options.t1, options.t2);
						drawBezierHelper(context, trimmedBezier, { curveStrokeColor: COLORS.NON_INTERACTIVE.STROKE_1, radius: 3.5 });
					},
					template: markRaw(SliderExample),
					templateOptions: {
						sliders: [
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
				{
					name: "Project",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>, mouseLocation?: Point): void => {
						if (mouseLocation != null) {
							const context = getContextFromCanvas(canvas);
							const closestPoint = JSON.parse(bezier.project(mouseLocation.x, mouseLocation.y));
							drawLine(context, mouseLocation, closestPoint, COLORS.NON_INTERACTIVE.STROKE_1);
						}
					},
				},
				{
					name: "Local Extrema",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance): void => {
						const context = getContextFromCanvas(canvas);
						const dimensionColors = ["red", "green"];
						const extrema: number[][] = JSON.parse(bezier.local_extrema());
						extrema.forEach((tValues, index) => {
							tValues.forEach((t) => {
								const point: Point = JSON.parse(bezier.compute(t));
								drawPoint(context, point, 4, dimensionColors[index]);
							});
						});
						drawText(getContextFromCanvas(canvas), "X extrema", 5, canvas.height - 20, dimensionColors[0]);
						drawText(getContextFromCanvas(canvas), "Y extrema", 5, canvas.height - 5, dimensionColors[1]);
					},
				},
				{
					name: "Rotate",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);
						const rotatedBezier = bezier
							.rotate(options.angle * Math.PI)
							.get_points()
							.map((p) => JSON.parse(p));
						drawBezier(context, rotatedBezier, null, { curveStrokeColor: COLORS.NON_INTERACTIVE.STROKE_1, radius: 3.5 });
					},
					template: markRaw(SliderExample),
					templateOptions: {
						sliders: [
							{
								variable: "angle",
								min: 0,
								max: 2,
								step: 1 / 16,
								default: 1 / 8,
								unit: "π",
							},
						],
					},
				},
				{
					name: "Intersect Line Segment",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance): void => {
						const context = getContextFromCanvas(canvas);
						const line = [
							{ x: 150, y: 150 },
							{ x: 30, y: 30 },
						];
						const mappedLine = line.map((p) => [p.x, p.y]);
						drawLine(context, line[0], line[1], COLORS.NON_INTERACTIVE.STROKE_1);
						const intersections: Point[] = bezier.intersect_line_segment(mappedLine).map((p) => JSON.parse(p));
						intersections.forEach((p: Point) => {
							drawPoint(context, p, 3, COLORS.NON_INTERACTIVE.STROKE_2);
						});
					},
				},
			],
		};
	},
});
</script>

<style>
#app {
	font-family: Avenir, Helvetica, Arial, sans-serif;
	-webkit-font-smoothing: antialiased;
	-moz-osx-font-smoothing: grayscale;
	text-align: center;
	color: #2c3e50;
	margin-top: 60px;
}
</style>
