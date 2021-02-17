<template>
	<MainWindow />
</template>

<script lang="ts">
import { defineComponent } from "vue";
import MainWindow from "./components/window/MainWindow.vue";

export default defineComponent({
	name: "App",
	components: {
		MainWindow,
	},
});
</script>

<style lang="scss">
html, body, #app {
	margin: 0;
	height: 100%;
	font-family: "Segoe UI", Arial, sans-serif;
	font-size: 14px;
	line-height: 1;
	color: #ddd;
	background: #222;
	user-select: none;
}
</style>
