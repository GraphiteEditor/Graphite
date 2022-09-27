<template>
	<div>
		<h4 class="example-header">{{ title }}</h4>
		<figure class="example-figure" ref="drawing"></figure>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import BezierDrawing from "@/components/BezierDrawing";
import { Callback, WasmBezierInstance } from "@/utils/types";

export default defineComponent({
	props: {
		title: String,
		bezier: {
			type: Object as PropType<WasmBezierInstance>,
			required: true,
		},
		callback: {
			type: Function as PropType<Callback>,
			required: true,
		},
		options: {
			type: Object as PropType<Record<string, number>>,
			default: () => ({}),
		},
		createThroughPoints: {
			type: Boolean as PropType<boolean>,
			default: false,
		},
	},
	data() {
		return {
			bezierDrawing: new BezierDrawing(this.bezier, this.callback, this.options, this.createThroughPoints),
		};
	},
	mounted() {
		const drawing = this.$refs.drawing as HTMLElement;
		drawing.appendChild(this.bezierDrawing.getCanvas());
		this.bezierDrawing.updateBezier();
	},
	watch: {
		options: {
			deep: true,
			handler() {
				this.bezierDrawing.updateBezier(undefined, this.options);
			},
		},
	},
});
</script>

<style scoped>
.example-header {
	margin-bottom: 0;
}

.example-figure {
	margin-top: 0.5em;
}
</style>
