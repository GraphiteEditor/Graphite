<template>
	<h1>Bezier-rs Interactive Documentation</h1>
	<p>
		This is the interactive documentation for the <a href="https://crates.io/crates/bezier-rs"><b>Bezier-rs</b></a> library. View the
		<a href="https://docs.rs/bezier-rs/latest/bezier_rs">crate documentation</a>
		for detailed function descriptions and API usage. Click and drag on the endpoints of the example curves to visualize the various Bezier utilities and functions.
	</p>

	<h2>Beziers</h2>
	<div v-for="[name, feature] in Object.entries(bezierFeatures)" :key="name">
		<BezierExamplePane
			:name="name"
			:callback="feature.callback"
			:exampleOptions="feature.exampleOptions"
			:triggerOnMouseMove="feature.triggerOnMouseMove"
			:chooseComputeType="feature.chooseComputeType"
		/>
	</div>

	<h2>Subpaths</h2>
	<div v-for="[name, feature] in Object.entries(subpathFeatures)" :key="name">
		<SubpathExamplePane
			:name="name"
			:callback="feature.callback"
			:sliderOptions="feature.sliderOptions"
			:triggerOnMouseMove="feature.triggerOnMouseMove"
			:chooseComputeType="feature.chooseComputeType"
		/>
	</div>
</template>

<style>
#app {
	font-family: Arial, sans-serif;
	text-align: center;
	margin: 40px 0;
}

#app > h1 + p {
	max-width: 768px;
	line-height: 1.4;
	margin: auto;
	text-align: justify;
}

#app > h2 {
	margin-top: 40px;
}

/* Example Pane styles */
.example-row {
	display: flex;
	flex-direction: row;
	justify-content: center;
}

.compute-type-choice {
	margin-top: 20px;
}

.example-pane-header {
	margin-top: 2em;
	margin-bottom: 0;
}

.example-pane-container {
	position: relative;
	width: fit-content;
	margin: auto;
}

/* Example styles */
.example-header {
	margin: 20px 0;
}

.example-figure {
	width: 200px;
	height: 200px;
	margin-bottom: 20px;
	border: solid 1px black;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import bezierFeatures from "@/features/bezierFeatures";
import subpathFeatures from "@/features/subpathFeatures";

import BezierExamplePane from "@/components/BezierExamplePane.vue";
import SubpathExamplePane from "@/components/SubpathExamplePane.vue";

export default defineComponent({
	data() {
		return {
			bezierFeatures,
			subpathFeatures,
		};
	},
	components: {
		BezierExamplePane,
		SubpathExamplePane,
	},
});
</script>
