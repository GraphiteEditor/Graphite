<template>
	<div>
		<h2 class="example_pane_header">{{ name }}</h2>
		<div class="example_row">
			<div v-for="(example, index) in exampleData" :key="index">
				<component :is="template" :templateOptions="example.templateOptions" :title="example.title" :bezier="example.bezier" :callback="callback" :createThroughPoints="createThroughPoints" />
			</div>
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType, Component } from "vue";

import { BezierCallback, TemplateOption, WasmBezierInstance, WasmRawInstance } from "@/utils/types";

import Example from "@/components/Example.vue";

type ExampleData = {
	title: string;
	bezier: WasmBezierInstance;
	templateOptions: TemplateOption;
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
		templateOptions: Object as PropType<TemplateOption>,
		cubicOptions: {
			type: Object as PropType<TemplateOption>,
			default: null,
		},
		createThroughPoints: {
			type: Boolean as PropType<boolean>,
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
					templateOptions: this.templateOptions as TemplateOption,
				},
				{
					title: "Cubic",
					bezier: wasm.WasmBezier.new_cubic(cubicPoints),
					templateOptions: (this.cubicOptions || this.templateOptions) as TemplateOption,
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
