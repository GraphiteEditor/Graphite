<template>
	<div>
		<h3 class="example-pane-header">{{ name }}</h3>
		<div class="example-row">
			<div v-for="(example, index) in examples" :key="index">
				<BezierExample :title="example.title" :points="example.points" :callback="callback" />
			</div>
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent, PropType } from "vue";

import { BezierCallback } from "@/utils/types";

import BezierExample from "@/components/BezierExample.vue";

export default defineComponent({
	props: {
		name: String,
		callback: {
			type: Function as PropType<BezierCallback>,
			required: true,
		},
	},
	data() {
		return {
			examples: [
				{
					title: "Linear",
					points: [
						[30, 60],
						[140, 120],
					],
				},
				{
					title: "Quadratic",
					points: [
						[30, 50],
						[140, 30],
						[160, 170],
					],
				},
				{
					title: "Quadratic",
					points: [
						[30, 30],
						[60, 140],
						[150, 30],
						[160, 160],
					],
				},
			],
		};
	},
	components: {
		BezierExample,
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
