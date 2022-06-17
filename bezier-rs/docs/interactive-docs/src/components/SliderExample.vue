<template>
	<div>
		<Example :title="title" :bezier="bezier" :callback="callback" :options="value.toString()" />
		<div class="slider_label">{{ templateOptions.variable }} = {{ value }}</div>
		<input class="slider" v-model="value" type="range" :step="templateOptions.step" :min="templateOptions.min" :max="templateOptions.max" />
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { BezierCallback } from "@/utils/types";
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
		return {
			value: this.templateOptions.default,
		};
	},
});
</script>

<style scoped></style>
