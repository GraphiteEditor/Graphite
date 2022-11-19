<template>
	<div class="example-pane-container">
		<h3 class="example-pane-header">{{ name }}</h3>
		<div v-if="chooseComputeType" class="compute-type-choice">
			<strong>ComputeType:</strong>

			<input type="radio" :id="`${id}-parametric`" value="Parametric" v-model="computeTypeChoice" />
			<label :for="`${id}-parametric`">Parametric</label>

			<input type="radio" :id="`${id}-euclidean`" value="Euclidean" v-model="computeTypeChoice" />
			<label :for="`${id}-euclidean`">Euclidean</label>
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
					:computeType="computeTypeChoice"
				/>
			</div>
		</div>
	</div>
</template>

<style></style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { BezierCallback, BezierCurveType, BEZIER_CURVE_TYPE, ComputeType, ExampleOptions, SliderOption } from "@/utils/types";

import BezierExample from "@/components/BezierExample.vue";

export default defineComponent({
	props: {
		name: { type: String as PropType<string>, required: true },
		callback: { type: Function as PropType<BezierCallback>, required: true },
		exampleOptions: { type: Object as PropType<ExampleOptions>, default: () => ({}) },
		triggerOnMouseMove: { type: Boolean as PropType<boolean>, default: false },
		chooseComputeType: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		const exampleDefaults = {
			Linear: {
				points: [
					[30, 60],
					[140, 120],
				],
			},
			Quadratic: {
				points: [
					[30, 50],
					[140, 30],
					[160, 170],
				],
			},
			Cubic: {
				points: [
					[30, 30],
					[60, 140],
					[150, 30],
					[160, 160],
				],
			},
		};

		// Use quadratic slider options as a default if sliders are not provided for the other curve types.
		const defaultSliderOptions: SliderOption[] = this.exampleOptions.Quadratic?.sliderOptions || [];

		return {
			examples: BEZIER_CURVE_TYPE.map((curveType: BezierCurveType) => {
				const givenData = this.exampleOptions[curveType];
				const defaultData = exampleDefaults[curveType];
				return {
					title: curveType,
					disabled: givenData?.disabled || false,
					points: givenData?.customPoints || defaultData.points,
					sliderOptions: givenData?.sliderOptions || defaultSliderOptions,
				};
			}),
			id: `${Math.random()}`.substring(2),
			computeTypeChoice: "Parametric" as ComputeType,
		};
	},
	components: {
		BezierExample,
	},
});
</script>
