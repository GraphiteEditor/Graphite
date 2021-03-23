<template>
	<MainWindow />
</template>

<style lang="scss">
html, body, #app {
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

export default defineComponent({
	components: { MainWindow },
	created() {
		this.greet();
	},
	methods: {
		async greet() {
			const wasm = import("../wasm/pkg");
			const { greet } = await wasm;
			console.log(greet("Graphite"));
		},
	},
});
</script>
