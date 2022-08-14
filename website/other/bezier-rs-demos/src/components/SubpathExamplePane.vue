<template>
	<div>
		<h3 class="example-pane-header">{{ name }}</h3>
		<div class="example-row">
			<div v-for="(example, index) in examples" :key="index">
				<SubpathExample :title="example.title" :triples="example.triples" :closed="example.closed" :callback="callback" />
			</div>
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { SubpathCallback } from "@/utils/types";

import SubpathExample from "@/components/SubpathExample.vue";

export default defineComponent({
	props: {
		name: String,
		callback: {
			type: Function as PropType<SubpathCallback>,
			required: true,
		},
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
		};
	},
	components: {
		SubpathExample,
	},
});
</script>

<style scoped>
.example-row {
	display: flex;
	flex-direction: row;
	justify-content: center;
}

.example-pane-header {
	margin-bottom: 0;
}
</style>
