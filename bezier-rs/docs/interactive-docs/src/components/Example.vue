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
		bezier: Object,
	},
	mounted() {
		const bezierDrawing = new BezierDrawing(this.bezier);
		this.$refs.drawing.appendChild(bezierDrawing.getCanvas());
		bezierDrawing.drawBezier();
	},
};
</script>

<style scoped>
</style>
