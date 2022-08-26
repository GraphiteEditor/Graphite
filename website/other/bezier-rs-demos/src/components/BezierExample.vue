<template>
	<div>
		<h4 class="example-header">{{ title }}</h4>
		<figure @mousedown="onMouseDown" @mouseup="onMouseUp" @mousemove="onMouseMove" class="example-figure" v-html="bezierSVG"></figure>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { WasmBezier } from "@/../wasm/pkg";
import { getConstructorKey, getCurveType } from "@/utils/helpers";
import { BezierCallback, BezierCurveType, WasmBezierManipulatorKey } from "@/utils/types";

const SELECTABLE_RANGE = 10;

// Given the number of points in the curve, map the index of a point to the correct manipulator key
const MANIPULATOR_KEYS_FROM_BEZIER_TYPE: { [key in BezierCurveType]: WasmBezierManipulatorKey[] } = {
	[BezierCurveType.Linear]: ["set_start", "set_end"],
	[BezierCurveType.Quadratic]: ["set_start", "set_handle_start", "set_end"],
	[BezierCurveType.Cubic]: ["set_start", "set_handle_start", "set_handle_end", "set_end"],
};

export default defineComponent({
	props: {
		title: String,
		points: {
			type: Array as PropType<Array<Array<number>>>,
			required: true,
			mutable: true,
		},
		callback: {
			type: Function as PropType<BezierCallback>,
			required: true,
		},
	},
	data() {
		const curveType = getCurveType(this.points.length);
		const manipulatorKeys = MANIPULATOR_KEYS_FROM_BEZIER_TYPE[curveType];
		const bezier = WasmBezier[getConstructorKey(curveType)](this.points);
		return {
			bezier,
			bezierSVG: this.callback(bezier),
			manipulatorKeys,
			activeIndex: undefined as number | undefined,
			mutablePoints: JSON.parse(JSON.stringify(this.points)),
		};
	},
	methods: {
		onMouseDown(event: MouseEvent) {
			const mx = event.offsetX;
			const my = event.offsetY;
			for (let pointIndex = 0; pointIndex < this.points.length; pointIndex += 1) {
				const point = this.mutablePoints[pointIndex];
				if (point && Math.abs(mx - point[0]) < SELECTABLE_RANGE && Math.abs(my - point[1]) < SELECTABLE_RANGE) {
					this.activeIndex = pointIndex;
					return;
				}
			}
		},
		onMouseUp() {
			this.activeIndex = undefined;
		},
		onMouseMove(event: MouseEvent) {
			const mx = event.offsetX;
			const my = event.offsetY;
			if (this.activeIndex !== undefined) {
				this.bezier[this.manipulatorKeys[this.activeIndex]](mx, my);
				this.mutablePoints[this.activeIndex] = [mx, my];
				this.bezierSVG = this.callback(this.bezier);
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
