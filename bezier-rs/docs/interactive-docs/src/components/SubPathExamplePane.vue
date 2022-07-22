<template>
	<div>
		<h3 class="example-pane-header">{{ name }}</h3>
		<div class="example-row">
			<div v-for="(example, index) in examples" :key="index">
				<SubPathExample :title="example.title" :triples="example.triples" :closed="example.closed" :callback="callback" />
			</div>
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { SubPathCallback } from "@/utils/types";

import SubPathExample from "@/components/SubPathExample.vue";

export default defineComponent({
	name: "SubPathPane",
	components: {
		SubPathExample,
	},
	data() {
		return {
			examples: [
				{
					title: "Open SubPath",
					triples: [
						[[20, 20], null, [10, 90]],
						[[150, 40], [60, 40], null],
						[[175, 175], null, null],
						[[100, 100], [40, 120], null],
					],
					closed: false,
				},
				{
					title: "Closed SubPath",
					triples: [
						[[35, 125], null, [40, 40]],
						[[130, 30], [120, 120], null],
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
	props: {
		name: String,
		callback: {
			type: Function as PropType<SubPathCallback>,
			required: true,
		},
	},
});
</script>

<style scoped>
.example-row {
	display: flex; /* or inline-flex */
	flex-direction: row;
	justify-content: center;
}

.example-pane-header {
	margin-bottom: 0;
}
</style>
