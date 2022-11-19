<template>
	<div>
		<h4 class="example-header">{{ title }}</h4>
		<figure @mousedown="onMouseDown" @mouseup="onMouseUp" @mousemove="onMouseMove" class="example-figure" v-html="subpathSVG"></figure>
		<div v-for="(slider, index) in sliderOptions" :key="index">
			<div class="slider-label">{{ slider.variable }} = {{ sliderData[slider.variable] }}{{ getSliderValue(sliderData[slider.variable], sliderUnits[slider.variable]) }}</div>
			<input class="slider" v-model.number="sliderData[slider.variable]" type="range" :step="slider.step" :min="slider.min" :max="slider.max" />
		</div>
	</div>
</template>

<style></style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { WasmSubpath } from "@/../wasm/pkg";
import { SubpathCallback, WasmSubpathInstance, WasmSubpathManipulatorKey, SliderOption } from "@/utils/types";

const SELECTABLE_RANGE = 10;
const POINT_INDEX_TO_MANIPULATOR: WasmSubpathManipulatorKey[] = ["set_anchor", "set_in_handle", "set_out_handle"];

export default defineComponent({
	props: {
		title: { type: String as PropType<string>, required: true },
		triples: { type: Array as PropType<Array<Array<number[] | undefined>>>, mutable: true, required: true },
		closed: { type: Boolean as PropType<boolean>, default: false },
		callback: { type: Function as PropType<SubpathCallback>, required: true },
		sliderOptions: { type: Object as PropType<Array<SliderOption>>, default: () => ({}) },
	},
	data() {
		const subpath = WasmSubpath.from_triples(this.triples, this.closed) as WasmSubpathInstance;

		const sliderData = Object.assign({}, ...this.sliderOptions.map((s) => ({ [s.variable]: s.default })));
		const sliderUnits = Object.assign({}, ...this.sliderOptions.map((s) => ({ [s.variable]: s.unit })));

		return {
			subpath,
			subpathSVG: this.callback(subpath, sliderData),
			activeIndex: undefined as number[] | undefined,
			mutableTriples: JSON.parse(JSON.stringify(this.triples)),
			sliderData,
			sliderUnits,
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
				this.subpathSVG = this.callback(this.subpath, this.sliderData);
			}
		},
		getSliderValue: (sliderValue: number, sliderUnit?: string | string[]) => (Array.isArray(sliderUnit) ? sliderUnit[sliderValue] : sliderUnit),
	},
	watch: {
		sliderData: {
			handler() {
				this.subpathSVG = this.callback(this.subpath, this.sliderData);
			},
			deep: true,
		},
	},
});
</script>
