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
		<br />
		<div id="svg-test" />
	</div>
</template>

<script lang="ts">
import { defineComponent, markRaw } from "vue";

import { drawText, drawPoint, drawBezier, drawLine, getContextFromCanvas, drawBezierHelper, COLORS } from "@/utils/drawing";
import { WasmBezierInstance } from "@/utils/types";

import ExamplePane from "@/components/ExamplePane.vue";
import SliderExample from "@/components/SliderExample.vue";

// eslint-disable-next-line
const testBezierLib = async () => {
	import("@/../wasm/pkg").then((wasm) => {
		const bezier = wasm.WasmBezier.new_quadratic([
			[0, 0],
			[50, 0],
			[100, 100],
		]);
		const svgContainer = document.getElementById("svg-test");
		if (svgContainer) {
			svgContainer.innerHTML = bezier.to_svg();
		}
	});
};

const tSliderOptions = {
	min: 0,
	max: 1,
	step: 0.01,
	default: 0.5,
	variable: "t",
};

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
					name: "Bezier from points",
					// eslint-disable-next-line
					callback: (): void => {},
					createThroughPoints: true,
					template: markRaw(SliderExample),
					templateOptions: { sliders: [{ ...tSliderOptions }] },
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
								variable: "strut",
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
					name: "Derivative",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);

						const intersection = JSON.parse(bezier.compute(options.t));
						const derivative = JSON.parse(bezier.derivative(options.t));
						const curveFactor = bezier.get_points().length - 1;

						const tangentStart = {
							x: intersection.x - derivative.x / curveFactor,
							y: intersection.y - derivative.y / curveFactor,
						};
						const tangentEnd = {
							x: intersection.x + derivative.x / curveFactor,
							y: intersection.y + derivative.y / curveFactor,
						};

						drawLine(context, tangentStart, tangentEnd, COLORS.NON_INTERACTIVE.STROKE_1);
						drawPoint(context, tangentStart, 3, COLORS.NON_INTERACTIVE.STROKE_1);
						drawPoint(context, intersection, 3, COLORS.NON_INTERACTIVE.STROKE_1);
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

						const normalStart = {
							x: intersection.x - normal.x * 20,
							y: intersection.y - normal.y * 20,
						};
						const normalEnd = {
							x: intersection.x + normal.x * 20,
							y: intersection.y + normal.y * 20,
						};

						drawLine(context, normalStart, normalEnd, COLORS.NON_INTERACTIVE.STROKE_1);
						drawPoint(context, normalStart, 3, COLORS.NON_INTERACTIVE.STROKE_1);
						drawPoint(context, intersection, 3, COLORS.NON_INTERACTIVE.STROKE_1);
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

						drawBezier(context, bezierPairPoints[0], null, COLORS.NON_INTERACTIVE.STROKE_2, 3.5);
						drawBezier(context, bezierPairPoints[1], null, COLORS.NON_INTERACTIVE.STROKE_1, 3.5);
					},
					template: markRaw(SliderExample),
					templateOptions: { sliders: [tSliderOptions] },
				},
				{
					name: "Trim",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: Record<string, number>): void => {
						const context = getContextFromCanvas(canvas);
						const trimmedBezier = bezier.trim(options.t1, options.t2);
						drawBezierHelper(context, trimmedBezier, COLORS.NON_INTERACTIVE.STROKE_1, 3.5);
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
