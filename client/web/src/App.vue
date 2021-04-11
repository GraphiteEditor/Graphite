<template>
	<MainWindow />
</template>

<style lang="scss">
html,
body,
#app {
	margin: 0;
	height: 100%;
	font-family: "Source Sans Pro", Arial, sans-serif;
	font-size: 14px;
	line-height: 1;
	color: #ddd;
	background: #222;
	user-select: none;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import MainWindow from "./components/window/MainWindow.vue";
import { NC } from "./events/NotificationCenter";

const _wasm = import("../wasm/pkg");
type InferPromise<T> = T extends Promise<infer U> ? U : any;
type Wasm = InferPromise<typeof _wasm>;

export default defineComponent({
	components: { MainWindow },
	created() {
		this.greet();
	},
	methods: {
		async greet() {
			const {
				greet,
				Color,
				update_primary_color,
				update_secondary_color
			} = await _wasm;
			console.log(greet("Graphite"));

			NC.on("update_primary_color", ({ value }) => {
				update_primary_color(
					new Color(value.color.r, value.color.g, value.color.b, 1)
				);
			});

			NC.on("update_secondary_color", ({ value }) => {
				update_secondary_color(
					new Color(value.color.r, value.color.g, value.color.b, 1)
				);
			});
		}
	}
});
</script>
