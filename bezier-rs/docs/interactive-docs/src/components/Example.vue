<template>
	<div>
		<h3>{{ title }}</h3>
		<figure ref="drawing"></figure>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { BezierCallback } from "../utils/types";
import { WasmBezierInstance } from "../utils/wasm-comm";

import BezierDrawing from "./BezierDrawing";

export default defineComponent({
	name: "ExampleComponent",
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
	},
	mounted() {
		const bezierDrawing = new BezierDrawing(this.bezier, this.callback);
		const drawing = this.$refs.drawing as HTMLElement;
		drawing.appendChild(bezierDrawing.getCanvas());
		bezierDrawing.updateBezier();
	},
});
</script>

<style scoped></style>
