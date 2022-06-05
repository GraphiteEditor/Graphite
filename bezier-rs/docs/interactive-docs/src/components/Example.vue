<template>
	<div>
		<h3>{{ title }}</h3>
		<figure ref="drawing"></figure>
	</div>
</template>

<script>
import BezierDrawing from "./BezierDrawing";

export default {
	name: "ExampleComponent",
	props: {
		title: String,
		bezier: Object,
		points: Object,
	},
	mounted() {
		// eslint-disable-next-line
		import("@/../wasm/pkg").then((wasm) => {
			const bezierDrawing = new BezierDrawing(this.points, wasm);
			this.$refs.drawing.appendChild(bezierDrawing.getCanvas());
			bezierDrawing.updateBezier();
		});
	},
};
</script>

<style scoped>
</style>
