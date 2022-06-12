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

import { drawText, drawPoint, getContextFromCanvas } from "@/utils/drawing";
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
					name: "Get / Compute",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: string): void => {
						const point = JSON.parse(bezier.compute(parseFloat(options)));
						point.r = 4;
						point.selected = false;
						drawPoint(getContextFromCanvas(canvas), point, "DarkBlue");
					},
					template: markRaw(SliderExample),
					templateOptions: {
						min: 0,
						max: 1,
						step: 0.01,
						default: 0.5,
						variable: "t",
					},
				},
				{
					id: 4,
					name: "Lookup Table",
					callback: (canvas: HTMLCanvasElement, bezier: WasmBezierInstance, options: string): void => {
						const lookupPoints = bezier.get_lookup_table(Number(options));
						lookupPoints.forEach((serPoint, index) => {
							if (index !== 0 && index !== lookupPoints.length - 1) {
								const point = JSON.parse(serPoint);
								point.r = 3;
								point.selected = false;
								drawPoint(getContextFromCanvas(canvas), point, "DarkBlue");
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
