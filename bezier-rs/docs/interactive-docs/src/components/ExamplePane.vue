<template>
	<div>
		<h2 class="example_pane_header">{{ name }}</h2>
		<div class="example_row">
			<div v-for="(example, index) in exampleData" :key="index">
				<component :is="template" :templateOptions="example.templateOptions" :title="example.title" :bezier="example.bezier" :callback="callback" :createFromPoints="createFromPoints" />
			</div>
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType, Component } from "vue";

import { BezierCallback, SliderOption } from "@/utils/types";
import { WasmBezierInstance, WasmRawInstance } from "@/utils/wasm-comm";

import Example from "@/components/Example.vue";

type ExampleData = {
	title: string;
	bezier: WasmBezierInstance;
	templateOptions: SliderOption;
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
		cubicOptions: {
			type: Object,
			default: null,
		},
		createFromPoints: {
			type: Boolean,
			default: false,
		},
	},
	data() {
		return {
			exampleData: [] as ExampleData[],
		};
	},
	mounted() {
		import("@/../wasm/pkg").then((wasm: WasmRawInstance) => {
			const quadraticPoints = [
				[30, 50],
				[140, 30],
				[160, 170],
			];
			const cubicPoints = [
				[30, 30],
				[60, 140],
				[150, 30],
				[160, 160],
			];
			this.exampleData = [
				{
					title: "Quadratic",
					bezier: wasm.WasmBezier.new_quadratic(quadraticPoints),
					templateOptions: this.templateOptions as SliderOption,
				},
				{
					title: "Cubic",
					bezier: wasm.WasmBezier.new_cubic(cubicPoints),
					templateOptions: (this.cubicOptions || this.templateOptions) as SliderOption,
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
