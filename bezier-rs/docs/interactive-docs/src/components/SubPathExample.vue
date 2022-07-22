<template>
	<div>
		<h4 class="example-header">{{ title }}</h4>
		<figure @mousedown="onMouseDown" @mouseup="onMouseUp" @mousemove="onMouseMove" class="example-figure" ref="drawing"></figure>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { WasmSubPath } from "@/../wasm/pkg";
import { SubPathCallback, WasmSubPathInstance, WasmSubPathManipulatorKey } from "@/utils/types";

const SELECTABLE_RANGE = 10;
const pointIndexToManipulator: WasmSubPathManipulatorKey[] = ["set_anchor", "set_in_handle", "set_out_handle"];

export default defineComponent({
	name: "SubPathComponent",
	props: {
		title: String,
		triples: {
			type: Array as PropType<Array<Array<number[] | null>>>,
			required: true,
			mutable: true,
		},
		closed: Boolean,
		callback: {
			type: Function as PropType<SubPathCallback>,
			required: true,
		},
	},
	data() {
		return {
			mutableTriples: JSON.parse(JSON.stringify(this.triples)),
			subPath: WasmSubPath.from_triples(this.triples, this.closed) as WasmSubPathInstance,
			activeControllerIndex: null as number | null,
			activePointIndex: null as number | null,
		};
	},
	mounted() {
		this.updateDrawing();
	},
	methods: {
		onMouseDown(event: MouseEvent) {
			const mx = event.offsetX;
			const my = event.offsetY;
			for (let controllerIndex = 0; controllerIndex < this.mutableTriples.length; controllerIndex += 1) {
				for (let pointIndex = 0; pointIndex < 3; pointIndex += 1) {
					const point = this.mutableTriples[controllerIndex][pointIndex];
					if (point != null && Math.abs(mx - point[0]) < SELECTABLE_RANGE && Math.abs(my - point[1]) < SELECTABLE_RANGE) {
						this.activeControllerIndex = controllerIndex;
						this.activePointIndex = pointIndex;
						return;
					}
				}
			}
		},
		onMouseUp() {
			this.activeControllerIndex = null;
			this.activePointIndex = null;
		},
		onMouseMove(event: MouseEvent) {
			const mx = event.offsetX;
			const my = event.offsetY;
			if (this.activeControllerIndex != null && this.activePointIndex != null) {
				this.subPath[pointIndexToManipulator[this.activePointIndex]](this.activeControllerIndex, mx, my);
				this.mutableTriples[this.activeControllerIndex][this.activePointIndex] = [mx, my];
				this.updateDrawing();
			}
		},
		updateDrawing() {
			const drawing = this.$refs.drawing as HTMLElement;
			drawing.innerHTML = this.callback(this.subPath);
		},
	},
});
</script>

<style scoped>
.example-figure {
	border: solid 1px black;
	width: 200px;
	height: 200px;
}
</style>
