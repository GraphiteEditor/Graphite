<template>
	<div>
		<Example :title="title" :bezier="bezier" :callback="callback" :options="$data" />
		<div v-for="(slider, index) in templateOptions.sliders" :key="index">
			<div class="slider_label">{{ slider.variable }} = {{ $data[slider.variable] }}</div>
			<input class="slider" v-model.number="$data[slider.variable]" type="range" :step="slider.step" :min="slider.min" :max="slider.max" />
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { BezierCallback, SliderOption } from "@/utils/types";
import { WasmBezierInstance } from "@/utils/wasm-comm";

import Example from "@/components/Example.vue";

export default defineComponent({
	name: "SliderExample",
	components: {
		Example,
	},
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
			type: Object,
			default: () => ({}),
		},
	},
	data() {
		const sliders: SliderOption[] = this.templateOptions.sliders;
		return Object.assign({}, ...sliders.map((s) => ({ [s.variable]: s.default })));
	},
});
</script>

<style scoped></style>
