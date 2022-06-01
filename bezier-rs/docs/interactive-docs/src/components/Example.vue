<template>
	<div>
		<h3>{{ title }}</h3>
		<figure ref="drawing"></figure>
	</div>
</template>

<script>
import { BezierDrawing } from "./BezierDrawing";

export default {
	name: "ExampleComponent",
	props: {
		title: String,
		points: Object,
	},
	mounted() {
		// TODO: Figure out to handle this properly
		// eslint-disable-next-line
		const bezier = new BezierDrawing(this.$refs.drawing, this.points);
	},
};
</script>

<style scoped>
</style>
