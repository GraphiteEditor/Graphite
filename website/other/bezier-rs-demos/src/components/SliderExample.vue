<template>
	<div>
		<Example :title="title" :bezier="bezier" :callback="callback" :options="sliderData" :createThroughPoints="createThroughPoints" />
		<div v-for="(slider, index) in templateOptions.sliders" :key="index">
			<div class="slider-label">{{ slider.variable }} = {{ sliderData[slider.variable] }}{{ getSliderValue(sliderData[slider.variable], sliderUnits[slider.variable]) }}</div>
			<input class="slider" v-model.number="sliderData[slider.variable]" type="range" :step="slider.step" :min="slider.min" :max="slider.max" />
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { BezierCallback, TemplateOption, WasmBezierInstance } from "@/utils/types";

import Example from "@/components/Example.vue";

export default defineComponent({
	props: {
		title: String,
		bezier: {
			type: Object as PropType<WasmBezierInstance>,
			required: true,
		},
		callback: {
			type: Function as PropType<BezierCallback>,
			required: true,
		},
		templateOptions: {
			type: Object as PropType<TemplateOption>,
			default: () => ({}),
		},
		createThroughPoints: {
			type: Boolean as PropType<boolean>,
			default: false,
		},
	},
	data() {
		const sliders = this.templateOptions.sliders;
		return {
			sliderData: Object.assign({}, ...sliders.map((s) => ({ [s.variable]: s.default }))),
			sliderUnits: Object.assign({}, ...sliders.map((s) => ({ [s.variable]: s.unit }))),
		};
	},
	components: {
		Example,
	},
	methods: {
		getSliderValue: (sliderValue: number, sliderUnit?: string | string[]) => (Array.isArray(sliderUnit) ? sliderUnit[sliderValue] : sliderUnit),
	},
});
</script>

<style scoped></style>
