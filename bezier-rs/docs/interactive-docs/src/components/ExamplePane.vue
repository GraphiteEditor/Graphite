<template>
	<div>
		<h3 class="example-pane-header">{{ name }}</h3>
		<div class="example-row">
			<div v-for="(example, index) in exampleData" :key="index">
				<component :is="template" :templateOptions="example.templateOptions" :title="example.title" :bezier="example.bezier" :callback="callback" :createThroughPoints="createThroughPoints" />
			</div>
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType, Component } from "vue";

import { BezierCallback, BezierCurveType, TemplateOption, WasmBezierConstructorKey, WasmBezierInstance, WasmRawInstance } from "@/utils/types";

import Example from "@/components/Example.vue";

type ExampleData = {
	title: string;
	bezier: WasmBezierInstance;
	templateOptions: TemplateOption;
};

type CustomTemplateOptions = {
	[key in BezierCurveType]?: TemplateOption;
};

type CustomPoints = {
	[key in BezierCurveType]?: number[][];
};

const CurveTypeMapping = {
	[BezierCurveType.Linear]: {
		points: [
			[30, 60],
			[140, 120],
		],
		constructor: "new_linear" as WasmBezierConstructorKey,
	},
	[BezierCurveType.Quadratic]: {
		points: [
			[30, 50],
			[140, 30],
			[160, 170],
		],
		constructor: "new_quadratic" as WasmBezierConstructorKey,
	},
	[BezierCurveType.Cubic]: {
		points: [
			[30, 30],
			[60, 140],
			[150, 30],
			[160, 160],
		],
		constructor: "new_cubic" as WasmBezierConstructorKey,
	},
};

export default defineComponent({
	props: {
		name: {
			type: String as PropType<string>,
			required: true,
		},
		callback: {
			type: Function as PropType<BezierCallback>,
			required: true,
		},
		template: {
			type: Object as PropType<Component>,
			default: Example,
		},
		templateOptions: {
			type: Object as PropType<TemplateOption>,
			required: false,
		},
		customOptions: {
			type: Object as PropType<CustomTemplateOptions>,
			default: () => ({}),
		},
		createThroughPoints: {
			type: Boolean as PropType<boolean>,
			default: false,
		},
		curveDegrees: {
			type: Set as PropType<Set<BezierCurveType>>,
			default: () => new Set(Object.values(BezierCurveType)),
		},
		customPoints: {
			type: Object as PropType<CustomPoints>,
			default: () => ({}),
		},
	},
	data() {
		return {
			exampleData: [] as ExampleData[],
		};
	},
	mounted() {
		import("@/../wasm/pkg").then((wasm: WasmRawInstance) => {
			this.exampleData = [];
			// Only add example for BezierCurveType that is in the curveDegrees set
			Object.values(BezierCurveType).forEach((bezierType) => {
				if (this.curveDegrees.has(bezierType)) {
					const { points, constructor } = CurveTypeMapping[bezierType];
					this.exampleData.push({
						title: bezierType,
						// Use custom options if they were provided for the current BezierCurveType
						bezier: wasm.WasmBezier[constructor](this.customPoints[bezierType] || points),
						templateOptions: (this.customOptions[bezierType] || this.templateOptions) as TemplateOption,
					});
				}
			});
		});
	},
	components: {
		Example,
	},
});
</script>

<style>
.example-row {
	display: flex;
	flex-direction: row;
	justify-content: center;
}

.example-pane-header {
	margin-bottom: 0;
}
</style>
