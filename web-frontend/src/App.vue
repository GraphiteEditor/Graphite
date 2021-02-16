<template>
	<MainWindow />
</template>

<script lang="ts">
import { Options, Vue } from "vue-class-component";
import MainWindow from "./components/window/MainWindow.vue";

@Options({
	components: {
		MainWindow,
	},
})
export default class App extends Vue {}
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
