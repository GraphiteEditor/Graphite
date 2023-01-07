<template>
	<div class="example-pane-container">
		<h3 class="example-pane-header">{{ name }}</h3>
		<div v-if="chooseComputeType" class="compute-type-choice">
			<strong>ComputeType:</strong>

			<input type="radio" :id="`${id}-parametric`" value="Parametric" v-model="computeType" />
			<label :for="`${id}-parametric`">Parametric</label>

			<input type="radio" :id="`${id}-euclidean`" value="Euclidean" v-model="computeType" />
			<label :for="`${id}-euclidean`">Euclidean</label>
		</div>
		<div class="example-row">
			<div v-for="(example, index) in examples" :key="index">
				<subpath-example
					:title="example.title"
					:name="name"
					:triples="JSON.stringify(example.triples)"
					:closed="example.closed"
					:sliderOptions="JSON.stringify(sliderOptions)"
					:triggerOnMouseMove="triggerOnMouseMove"
					:computetype="computeType"
				/>
			</div>
		</div>
	</div>
</template>

<style></style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

// eslint-disable-next-line no-restricted-imports
import SubpathExample from "./SubpathExample";

import { SubpathCallback, SliderOption, ComputeType } from "@/utils/types";

export default defineComponent({
	props: {
		name: { type: String as PropType<string>, required: true },
		callback: { type: Function as PropType<SubpathCallback>, required: true },
		sliderOptions: { type: Array as PropType<Array<SliderOption>>, default: () => [] },
		triggerOnMouseMove: { type: Boolean as PropType<boolean>, default: false },
		chooseComputeType: { type: Boolean as PropType<boolean>, default: false },
	},
	data() {
		return {
			examples: [
				{
					title: "Open Subpath",
					triples: [
						[[20, 20], undefined, [10, 90]],
						[[150, 40], [60, 40], undefined],
						[[175, 175], undefined, undefined],
						[[100, 100], [40, 120], undefined],
					],
					closed: false,
				},
				{
					title: "Closed Subpath",
					triples: [
						[[35, 125], undefined, [40, 40]],
						[[130, 30], [120, 120], undefined],
						[
							[145, 150],
							[175, 90],
							[70, 185],
						],
					],
					closed: true,
				},
			],
			id: `${Math.random()}`.substring(2),
			computeType: "Parametric" as ComputeType,
		};
	},
	components: {
		SubpathExample,
	},
});
</script>
