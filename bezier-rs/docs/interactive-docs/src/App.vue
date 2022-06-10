<template>
	<div class="App">
		<h1>Bezier-rs Interactive Documentation</h1>
		<p>This is the interactive documentation for the <b>bezier-rs</b> library. Click and drag on the endpoints of the example curves to visualize the various Bezier utilities and functions.</p>
		<div v-for="feature in features" :key="feature.id">
			<ExamplePane :name="feature.name" :callback="feature.callback" />
		</div>
		<div id="svg-test" />
	</div>
</template>

<script lang="ts">
import { defineComponent } from "vue";

import ExamplePane from "./components/ExamplePane.vue";
import { drawText, getContextFromCanvas } from "./utils/drawing";
import { WasmBezierInstance } from "./utils/types";

// eslint-disable-next-line
const testBezierLib = async () => {
	// TODO: Fix below
	// eslint seems to think this pkg is the one in the frontend folder, not the one in interactive-docs (which is not what is actually imported)
	// eslint-disable-next-line
	import("../wasm/pkg").then((wasm) => {
		// eslint-disable-next-line
		const bezier = wasm.WasmBezier.new_quad(0, 0, 50, 0, 100, 100);
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
