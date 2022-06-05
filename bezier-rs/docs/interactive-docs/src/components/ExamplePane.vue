<template>
	<div class="example_row">
		<div v-for="example in examples" :key="example.id">
			<Example :title="example.title" :bezier="example.bezier" />
		</div>
	</div>
</template>

<script>
import Example from "./Example.vue";

export default {
	name: "ExamplePane",
	components: {
		Example,
	},
	data() {
		return {
			examples: [],
		};
	},
	created() {
		this.getCurve();
	},
	methods: {
		getCurve() {
			// eslint-disable-next-line
			import("@/../wasm/pkg").then((wasm) => {
				// eslint-disable-next-line
				this.examples = [
					{
						id: 0,
						title: "Quadratic Bezier",
						bezier: wasm.WasmBezier.new_quad(30, 30, 140, 20, 160, 170),
					},
					{
						id: 1,
						title: "Cubic Bezier",
						bezier: wasm.WasmBezier.new_cubic(30, 30, 60, 140, 150, 30, 160, 160),
					},
				];
			});
		},
	},
};
</script>

<style>
.example_row {
	display: flex; /* or inline-flex */
	flex-direction: row;
	justify-content: center;
}
</style>
