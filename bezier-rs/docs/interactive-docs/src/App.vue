<template>
	<div class="App">
		<h1>Bezier-rs Interactive Documentation</h1>
		<p>This is the interactive documentation for the <b>bezier-rs</b> library. Click and drag on the endpoints of the example curves to visualize the various Bezier utilities and functions.</p>
		<ExamplePane />
		<div id="svg-test" />
	</div>
</template>

<script>
import ExamplePane from "./components/ExamplePane.vue";

const testBezierLib = async () => {
	// TODO: Fix below
	// eslint seems to think this pkg is the one in the frontend folder, not the one in interactive-docs (which is not what is actually imported)
	// eslint-disable-next-line
	import("@/../wasm/pkg").then((wasm) => {
		const svg = wasm.quad_to_svg(0, 0, 50, 0, 100, 100);
		const svgContainer = document.getElementById("svg-test");
		svgContainer.innerHTML = svg;
	});
};

testBezierLib();

export default {
	name: "App",
	components: {
		ExamplePane,
	},
};
</script>

<style>
#app {
	font-family: Avenir, Helvetica, Arial, sans-serif;
	-webkit-font-smoothing: antialiased;
	-moz-osx-font-smoothing: grayscale;
	text-align: center;
	color: #2c3e50;
	margin-top: 60px;
}
h1 {
	margin: 40px 0 0;
}
</style>
