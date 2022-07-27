<template>
	<div>
		<h4 class="example-header">{{ title }}</h4>
		<figure @mousedown="onMouseDown" @mouseup="onMouseUp" @mousemove="onMouseMove" class="example-figure" v-html="subpathSVG"></figure>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { WasmSubpath } from "@/../wasm/pkg";
import { SubpathCallback, WasmSubpathInstance, WasmSubpathManipulatorKey } from "@/utils/types";

const SELECTABLE_RANGE = 10;
const POINT_INDEX_TO_MANIPULATOR: WasmSubpathManipulatorKey[] = ["set_anchor", "set_in_handle", "set_out_handle"];

export default defineComponent({
	props: {
		title: String,
		triples: {
			type: Array as PropType<Array<Array<number[] | undefined>>>,
			required: true,
			mutable: true,
		},
		closed: Boolean,
		callback: {
			type: Function as PropType<SubpathCallback>,
			required: true,
		},
	},
	data() {
		const subpath = WasmSubpath.from_triples(this.triples, this.closed) as WasmSubpathInstance;
		return {
			subpath,
			subpathSVG: this.callback(subpath),
			activeIndex: undefined as number[] | undefined,
			mutableTriples: JSON.parse(JSON.stringify(this.triples)),
		};
	},
	methods: {
		onMouseDown(event: MouseEvent) {
			const mx = event.offsetX;
			const my = event.offsetY;
			for (let controllerIndex = 0; controllerIndex < this.mutableTriples.length; controllerIndex += 1) {
				for (let pointIndex = 0; pointIndex < 3; pointIndex += 1) {
					const point = this.mutableTriples[controllerIndex][pointIndex];
					if (point && Math.abs(mx - point[0]) < SELECTABLE_RANGE && Math.abs(my - point[1]) < SELECTABLE_RANGE) {
						this.activeIndex = [controllerIndex, pointIndex];
						return;
					}
				}
			}
		},
		onMouseUp() {
			this.activeIndex = undefined;
		},
		onMouseMove(event: MouseEvent) {
			const mx = event.offsetX;
			const my = event.offsetY;
			if (this.activeIndex) {
				this.subpath[POINT_INDEX_TO_MANIPULATOR[this.activeIndex[1]]](this.activeIndex[0], mx, my);
				this.mutableTriples[this.activeIndex[0]][this.activeIndex[1]] = [mx, my];
				this.subpathSVG = this.callback(this.subpath);
			}
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
