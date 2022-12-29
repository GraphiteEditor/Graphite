<template>
	<div>
		<h3 class="example-pane-header">{{ name }}</h3>
		<div v-if="chooseComputeType" class="compute-type-choice">
			<strong>ComputeType:</strong>

			<input type="radio" :id="`${id}-parametric`" value="Parametric" v-model="computeTypeChoice" />
			<label :for="`${id}-parametric`">Parametric</label>

			<input type="radio" :id="`${id}-euclidean`" value="Euclidean" v-model="computeTypeChoice" />
			<label :for="`${id}-euclidean`">Euclidean</label>
		</div>
		<div class="example-row">
			<div v-for="(example, index) in examples" :key="index">
				<SubpathExample
					:title="example.title"
					:triples="example.triples"
					:closed="example.closed"
					:callback="callback"
					:sliderOptions="sliderOptions"
					:triggerOnMouseMove="triggerOnMouseMove"
					:computeType="computeTypeChoice"
				/>
			</div>
		</div>
	</div>
</template>

<style></style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { SubpathCallback, SliderOption, ComputeType } from "@/utils/types";

import SubpathExample from "@/components/SubpathExample.vue";

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
			computeTypeChoice: "Parametric" as ComputeType,
		};
	},
	components: {
		SubpathExample,
	},
});
</script>
