<template>
	<div>
		<h4 class="example_header">{{ title }}</h4>
		<figure class="example_figure" ref="drawing"></figure>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import BezierDrawing from "@/components/BezierDrawing";
import { BezierCallback } from "@/utils/types";
import { WasmBezierInstance } from "@/utils/wasm-comm";

export default defineComponent({
	name: "ExampleComponent",
	data() {
		return {
			bezierDrawing: new BezierDrawing(this.bezier, this.callback, this.options),
		};
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
		options: {
			type: String,
			default: "",
		},
	},
	mounted() {
		const drawing = this.$refs.drawing as HTMLElement;
		drawing.appendChild(this.bezierDrawing.getCanvas());
		this.bezierDrawing.updateBezier();
	},
	watch: {
		options() {
			this.bezierDrawing.updateBezier(this.options);
		},
	},
});
</script>

<style scoped>
.example_header {
	margin-bottom: 0;
}
.example_figure {
	margin-top: 0.5em;
	border: 1px black;
}
</style>
