<template>
	<MainWindow />
</template>

<style lang="scss">
html,
body,
#app {
	margin: 0;
	height: 100%;
	background: #222;
	user-select: none;
}

body,
input,
textarea,
button {
	font-family: "Source Sans Pro", Arial, sans-serif;
	font-size: 14px;
	line-height: 1;
	color: #ddd;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import MainWindow from "./components/window/MainWindow.vue";

const wasm = import("../wasm/pkg");

export default defineComponent({
	components: { MainWindow },
	created() {
		this.greet();
	},
	methods: {
		async greet() {
			const { greet } = await wasm;
			greet("Graphite");
		},
	},
});
</script>
