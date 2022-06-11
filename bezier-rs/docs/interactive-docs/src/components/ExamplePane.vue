<template>
	<div class="example_row">
		<div v-for="example in exampleData" :key="example.id">
			<Example :title="example.title" :bezier="example.bezier" />
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent } from "vue";

import { WasmBezierInstance } from "../utils/wasm-comm";

import Example from "./Example.vue";
// import wasm from "bezier-rs-wasm";

type ExampleData = {
	id: number;
	title: string;
	bezier: WasmBezierInstance;
};

export default defineComponent({
	name: "ExamplePane",
	components: {
		Example,
	},
	data() {
		return {
			exampleData: [] as ExampleData[],
		};
	},
	mounted() {
		// eslint-disable-next-line
		import("../../wasm/pkg").then((wasm) => {
			this.exampleData = [
				{
					id: 0,
					title: "Quadratic Bezier",
					bezier: wasm.WasmBezier.new_quad(30, 30, 140, 20, 160, 170),
				},
				{
					id: 1,
					title: "Cubic Bezier",
					bezier: wasm.WasmBezier.new_cubic(30, 30, 60, 140, 150, 30, 160, 160),
				},
			];
		});
	},
});
</script>

<style>
.example_row {
	display: flex; /* or inline-flex */
	flex-direction: row;
	justify-content: center;
}
</style>
