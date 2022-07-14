<template>
	<div>
		<h4 class="example-header">{{ name }}</h4>
		<figure @mousedown="onMouseDown" @mouseup="onMouseUp" @mousemove="onMouseMove" class="example-figure" ref="drawing"></figure>
	</div>
</template>

<script lang="ts">
import { defineComponent } from "vue";

import { WasmSubPath } from "@/../wasm/pkg";
import { WasmSubPathInstance, WasmSubPathManipulatorKey } from "@/utils/types";

const SELECTABLE_RANGE = 10;
const pointIndexToManipulator: WasmSubPathManipulatorKey[] = ["set_anchor", "set_in_handle", "set_out_handle"];

export default defineComponent({
	name: "SubPathComponent",
	data() {
		const triples = [
			[[20, 20], null, [10, 90]],
			[[150, 40], [60, 40], null],
			[[175, 175], null, null],
			[
				[100, 100],
				[40, 120],
				[20, 60],
			],
		];
		return {
			triples,
			subPath: WasmSubPath.from_triples(triples) as WasmSubPathInstance,
			activeControllerIndex: null as number | null,
			activePointIndex: null as number | null,
		};
	},
	props: {
		name: String,
	},
	mounted() {
		this.updateDrawing();
	},
	methods: {
		onMouseDown(event: MouseEvent) {
			const mx = event.offsetX;
			const my = event.offsetY;
			for (let controllerIndex = 0; controllerIndex < this.triples.length; controllerIndex += 1) {
				for (let pointIndex = 0; pointIndex < 3; pointIndex += 1) {
					const point = this.triples[controllerIndex][pointIndex];
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
				this.triples[this.activeControllerIndex][this.activePointIndex] = [mx, my];
				this.updateDrawing();
			}
		},
		updateDrawing() {
			const drawing = this.$refs.drawing as HTMLElement;
			drawing.innerHTML = this.subPath.to_svg();
		},
	},
});
</script>

<style scoped>
.example-header {
	margin-bottom: 0.5em;
}

.example-figure {
	margin-top: 0.5em;
	border: solid 1px black;
	width: 200px;
	height: 200px;
	margin: auto;
}
</style>
