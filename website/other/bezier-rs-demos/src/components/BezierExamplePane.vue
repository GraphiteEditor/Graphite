<template>
	<div class="example-pane-container">
		<h3 class="example-pane-header">{{ name }}</h3>
		<div v-if="euclideanParameterizationEnabled" class="euclidean-switch">
			<label class="switch-label">Euclidean Parameterization:</label>
			<label class="switch">
				<input v-model.number="isEuclidean" type="checkbox" />
				<span class="switch-slider"></span>
			</label>
		</div>
		<div class="example-row">
			<div v-for="(example, index) in examples" :key="index">
				<BezierExample
					v-if="!example.disabled"
					:title="example.title"
					:points="example.points"
					:callback="callback"
					:sliderOptions="example.sliderOptions"
					:triggerOnMouseMove="triggerOnMouseMove"
					:isEuclidean="isEuclidean"
				/>
			</div>
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { BezierCallback, BezierCurveType, ExampleOptions, SliderOption } from "@/utils/types";

import BezierExample from "@/components/BezierExample.vue";

export default defineComponent({
	props: {
		name: String,
		callback: {
			type: Function as PropType<BezierCallback>,
			required: true,
		},
		exampleOptions: {
			type: Object as PropType<ExampleOptions>,
			default: () => ({}),
		},
		triggerOnMouseMove: {
			type: Boolean,
			default: false,
		},
		euclideanParameterizationEnabled: {
			type: Boolean,
			default: false,
		},
	},
	data() {
		const exampleDefaults = {
			[BezierCurveType.Linear]: {
				points: [
					[30, 60],
					[140, 120],
				],
			},
			[BezierCurveType.Quadratic]: {
				points: [
					[30, 50],
					[140, 30],
					[160, 170],
				],
			},
			[BezierCurveType.Cubic]: {
				points: [
					[30, 30],
					[60, 140],
					[150, 30],
					[160, 160],
				],
			},
		};

		// Use quadratic slider options as a default if sliders are not provided for the other curve types.
		const defaultSliderOptions: SliderOption[] = this.exampleOptions[BezierCurveType.Quadratic]?.sliderOptions || [];

		return {
			examples: Object.values(BezierCurveType).map((curveType: BezierCurveType) => {
				const givenData = this.exampleOptions[curveType];
				const defaultData = exampleDefaults[curveType];
				return {
					title: curveType,
					disabled: givenData?.disabled || false,
					points: givenData?.customPoints || defaultData.points,
					sliderOptions: givenData?.sliderOptions || defaultSliderOptions,
				};
			}),
			isEuclidean: false,
		};
	},
	components: {
		BezierExample,
	},
});
</script>
