<template>
	<div class="App">
		<h1>Bezier-rs Interactive Documentation</h1>
		<p>This is the interactive documentation for the <b>bezier-rs</b> library. Click and drag on the endpoints of the example curves to visualize the various Bezier utilities and functions.</p>
		<div v-for="feature in features" :key="feature.id">
			<ExamplePane :template="feature.template" :templateOptions="feature.templateOptions" :name="feature.name" :callback="feature.callback" />
		</div>
		<br />
		<div id="svg-test" />
	</div>
</template>

<script lang="ts">
import { defineComponent, markRaw } from "vue";

import { drawText, drawPoint, drawLine, getContextFromCanvas } from "@/utils/drawing";
import { WasmBezierInstance } from "@/utils/types";

import ExamplePane from "@/components/ExamplePane.vue";
import SliderExample from "@/components/SliderExample.vue";

// eslint-disable-next-line
const testBezierLib = async () => {
	import("@/../wasm/pkg").then((wasm) => {
		const bezier = wasm.WasmBezier.new_quad([
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
					id: 0,
					name: "Constructor",
					// eslint-disable-next-line
					callback: (): void => {},
				},
				{
					id: 2,
					name: "Length",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance): void => {
						drawText(getContextFromCanvas(canvas), `Length: ${bezier.length().toFixed(2)}`, 5, canvas.height - 7);
					},
				},
				{
					id: 3,
					name: "Compute",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: string): void => {
						const point = JSON.parse(bezier.compute(parseFloat(options)));
						drawPoint(getContextFromCanvas(canvas), point, 4, "Red");
					},
					template: markRaw(SliderExample),
					templateOptions: tSliderOptions,
				},
				{
					id: 4,
					name: "Lookup Table",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: string): void => {
						const lookupPoints = bezier.compute_lookup_table(Number(options));
						lookupPoints.forEach((serialisedPoint, index) => {
							if (index !== 0 && index !== lookupPoints.length - 1) {
								drawPoint(getContextFromCanvas(canvas), JSON.parse(serialisedPoint), 3, "Red");
							}
						});
					},
					template: markRaw(SliderExample),
					templateOptions: {
						min: 2,
						max: 15,
						step: 1,
						default: 5,
						variable: "Steps",
					},
				},
				{
					id: 5,
					name: "Derivative",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: string): void => {
						const t = parseFloat(options);
						const context = getContextFromCanvas(canvas);

						const intersection = JSON.parse(bezier.compute(t));
						const derivative = JSON.parse(bezier.derivative(t));
						const curveFactor = bezier.get_points().length - 1;

						const tangentStart = {
							x: intersection.x - derivative.x / curveFactor,
							y: intersection.y - derivative.y / curveFactor,
						};
						const tangentEnd = {
							x: intersection.x + derivative.x / curveFactor,
							y: intersection.y + derivative.y / curveFactor,
						};

						drawLine(context, tangentStart, tangentEnd, "Red");
						drawPoint(context, tangentStart, 3, "Red");
						drawPoint(context, intersection, 3, "Red");
						drawPoint(context, tangentEnd, 3, "Red");
					},
					template: markRaw(SliderExample),
					templateOptions: tSliderOptions,
				},
				{
					id: 6,
					name: "Normal",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: string): void => {
						const t = parseFloat(options);
						const context = getContextFromCanvas(canvas);

						const intersection = JSON.parse(bezier.compute(t));
						const normal = JSON.parse(bezier.normal(t));

						const normalStart = {
							x: intersection.x - normal.x * 20,
							y: intersection.y - normal.y * 20,
						};
						const normalEnd = {
							x: intersection.x + normal.x * 20,
							y: intersection.y + normal.y * 20,
						};

						drawLine(context, normalStart, normalEnd, "Red");
						drawPoint(context, normalStart, 3, "Red");
						drawPoint(context, intersection, 3, "Red");
						drawPoint(context, normalEnd, 3, "Red");
					},
					template: markRaw(SliderExample),
					templateOptions: tSliderOptions,
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
