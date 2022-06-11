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

<style scoped>
.example_header {
	margin-bottom: 0;
}
.example_figure {
	margin-top: 0.5em;
}
</style>
