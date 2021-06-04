<template>
	<MainWindow />
</template>

<style lang="scss">
:root {
	--color-0-black: #000;
	--color-1-nearblack: #111;
	--color-2-mildblack: #222;
	--color-3-darkgray: #333;
	--color-4-dimgray: #444;
	--color-5-dullgray: #555;
	--color-6-lowergray: #666;
	--color-7-middlegray: #777;
	--color-8-uppergray: #888;
	--color-9-palegray: #999;
	--color-a-softgray: #aaa;
	--color-b-lightgray: #bbb;
	--color-c-brightgray: #ccc;
	--color-d-mildwhite: #ddd;
	--color-e-nearwhite: #eee;
	--color-f-white: #fff;
	--color-accent: #3194d6;
	--color-accent-hover: #49a5e2;

	// TODO: Replace with CSS color() function to calculate alpha when browsers support it
	// See https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/color() and https://caniuse.com/css-color-function
	// E6 = 90% alpha
	--floating-menu-opacity-color-2-mildblack: #222222e6;
	--floating-menu-shadow: rgba(0, 0, 0, 50%);
}

html,
body,
#app {
	margin: 0;
	height: 100%;
	background: var(--color-2-mildblack);
	user-select: none;
}

body,
input,
textarea,
button {
	font-family: "Source Sans Pro", Arial, sans-serif;
	font-size: 14px;
	line-height: 1;
	color: var(--color-e-nearwhite);
}

svg,
img {
	display: block;
}

// For placeholder messages (remove eventually)
.floating-menu {
	h1,
	h2,
	h3,
	h4,
	h5,
	h6,
	p {
		margin: 0;
	}

	p {
		margin-top: 8px;
	}
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
