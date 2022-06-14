<template>
	<div>
		<h2 class="example_pane_header">{{ name }}</h2>
		<div class="example_row">
			<div v-for="example in exampleData" :key="example.id">
				<component :is="template" :templateOptions="templateOptions" :title="example.title" :bezier="example.bezier" :callback="callback" />
			</div>
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType, Component } from "vue";

import { BezierCallback } from "@/utils/types";
import { WasmBezierInstance } from "@/utils/wasm-comm";

import Example from "@/components/Example.vue";

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
	props: {
		name: String,
		callback: {
			type: Function as PropType<BezierCallback>,
			required: true,
		},
		template: {
			type: Object as PropType<Component>,
			default: Example,
		},
		templateOptions: Object,
	},
	data() {
		return {
			exampleData: [] as ExampleData[],
		};
	},
	mounted() {
		import("@/../wasm/pkg").then((wasm) => {
			this.exampleData = [
				{
					id: 0,
					title: "Quadratic",
					bezier: wasm.WasmBezier.new_quad([
						[30, 30],
						[140, 20],
						[160, 170],
					]),
				},
				{
					id: 1,
					title: "Cubic",
					bezier: wasm.WasmBezier.new_cubic([
						[30, 30],
						[60, 140],
						[150, 30],
						[160, 160],
					]),
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

.example_pane_header {
	margin-bottom: 0;
}
</style>
