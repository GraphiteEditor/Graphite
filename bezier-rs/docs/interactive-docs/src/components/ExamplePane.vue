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
		curveDegrees: {
			type: Set as PropType<Set<number>>,
			default: () => new Set([1, 2, 3]),
		},
	},
	data() {
		return {
			exampleData: [] as ExampleData[],
		};
	},
	mounted() {
		import("@/../wasm/pkg").then((wasm: WasmRawInstance) => {
			const linearPoints = [
				[30, 60],
				[140, 120],
			];
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
			this.exampleData = [];
			if (this.curveDegrees.has(1)) {
				this.exampleData.push({
					title: "Linear",
					bezier: wasm.WasmBezier.new_linear(linearPoints),
					templateOptions: this.templateOptions as TemplateOption,
				});
			}
			if (this.curveDegrees.has(2)) {
				this.exampleData.push({
					title: "Quadratic",
					bezier: wasm.WasmBezier.new_quadratic(quadraticPoints),
					templateOptions: this.templateOptions as TemplateOption,
				});
			}
			if (this.curveDegrees.has(3)) {
				this.exampleData.push({
					title: "Cubic",
					bezier: wasm.WasmBezier.new_cubic(cubicPoints),
					templateOptions: (this.cubicOptions || this.templateOptions) as TemplateOption,
				});
			}
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
