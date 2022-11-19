<template>
	<div class="App">
		<h1>Bezier-rs Interactive Documentation</h1>
		<p>This is the interactive documentation for the <b>bezier-rs</b> library. Click and drag on the endpoints of the example curves to visualize the various Bezier utilities and functions.</p>
		<h2>Beziers</h2>
		<div v-for="(feature, index) in bezierFeatures" :key="index">
			<BezierExamplePane
				:name="feature.name"
				:callback="feature.callback"
				:exampleOptions="feature.exampleOptions"
				:triggerOnMouseMove="feature.triggerOnMouseMove"
				:euclideanParameterizationEnabled="feature.euclideanParameterizationEnabled"
			/>
		</div>
		<h2>Subpaths</h2>
		<div v-for="(feature, index) in subpathFeatures" :key="index">
			<SubpathExamplePane :name="feature.name" :callback="feature.callback" />
		</div>
	</div>
</template>

<script lang="ts">
import { defineComponent } from "vue";

import { WasmBezier } from "@/../wasm/pkg";
import "@/styles.css";
import { BezierCurveType, ExampleOptions, WasmBezierInstance, WasmSubpathInstance } from "@/utils/types";

import BezierExamplePane from "@/components/BezierExamplePane.vue";
import SubpathExamplePane from "@/components/SubpathExamplePane.vue";

const tSliderOptions = {
	min: 0,
	max: 1,
	step: 0.01,
	default: 0.5,
	variable: "t",
};

const tErrorOptions = {
	variable: "error",
	min: 0.1,
	max: 2,
	step: 0.1,
	default: 0.5,
};

export default defineComponent({
	data() {
		return {
			bezierFeatures: [
				{
					name: "Constructor",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.to_svg(),
				},
				{
					name: "Bezier Through Points",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => {
						const points = JSON.parse(bezier.get_points());
						if (Object.values(options).length === 1) {
							return WasmBezier.quadratic_through_points(points, options.t);
						}
						return WasmBezier.cubic_through_points(points, options.t, options["midpoint separation"]);
					},
					exampleOptions: {
						[BezierCurveType.Linear]: {
							disabled: true,
						},
						[BezierCurveType.Quadratic]: {
							customPoints: [
								[30, 50],
								[120, 70],
								[160, 170],
							],
							sliderOptions: [
								{
									min: 0.01,
									max: 0.99,
									step: 0.01,
									default: 0.5,
									variable: "t",
								},
							],
						},
						[BezierCurveType.Cubic]: {
							customPoints: [
								[30, 50],
								[120, 70],
								[160, 170],
							],
							sliderOptions: [
								{
									min: 0.01,
									max: 0.99,
									step: 0.01,
									default: 0.5,
									variable: "t",
								},
								{
									min: 0,
									max: 100,
									step: 2,
									default: 30,
									variable: "midpoint separation",
								},
							],
						},
					},
				},
				{
					name: "Length",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.length(),
				},
				{
					name: "Evaluate",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>, _: undefined, isEuclidean: boolean): string => bezier.evaluate(options.computeArgument, isEuclidean),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [{ ...tSliderOptions, variable: "computeArgument" }],
						},
					},
					euclideanParameterizationEnabled: true,
				},
				{
					name: "Lookup Table",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.compute_lookup_table(options.steps),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [
								{
									min: 2,
									max: 15,
									step: 1,
									default: 5,
									variable: "steps",
								},
							],
						},
					},
				},
				{
					name: "Derivative",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.derivative(),
					exampleOptions: {
						[BezierCurveType.Linear]: {
							disabled: true,
						},
						[BezierCurveType.Quadratic]: {
							customPoints: [
								[30, 40],
								[110, 50],
								[120, 130],
							],
						},
						[BezierCurveType.Cubic]: {
							customPoints: [
								[50, 50],
								[60, 100],
								[100, 140],
								[140, 150],
							],
						},
					},
				},
				{
					name: "Tangent",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.tangent(options.t),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tSliderOptions],
						},
					},
				},

				{
					name: "Normal",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.normal(options.t),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tSliderOptions],
						},
					},
				},
				{
					name: "Curvature",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.curvature(options.t),
					exampleOptions: {
						[BezierCurveType.Linear]: {
							disabled: true,
						},
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tSliderOptions],
						},
					},
				},
				{
					name: "Split",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.split(options.t),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tSliderOptions],
						},
					},
				},
				{
					name: "Trim",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.trim(options.t1, options.t2),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [
								{
									variable: "t1",
									min: 0,
									max: 1,
									step: 0.01,
									default: 0.25,
								},
								{
									variable: "t2",
									min: 0,
									max: 1,
									step: 0.01,
									default: 0.75,
								},
							],
						},
					},
				},
				{
					name: "Project",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>, mouseLocation?: [number, number]): string =>
						mouseLocation ? bezier.project(mouseLocation[0], mouseLocation[1]) : bezier.to_svg(),
					triggerOnMouseMove: true,
				},
				{
					name: "Local Extrema",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.local_extrema(),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							customPoints: [
								[40, 40],
								[160, 30],
								[110, 150],
							],
						},
						[BezierCurveType.Cubic]: {
							customPoints: [
								[160, 180],
								[170, 10],
								[30, 90],
								[180, 160],
							],
						},
					},
				},
				{
					name: "Bounding Box",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.bounding_box(),
				},
				{
					name: "Inflections",
					callback: (bezier: WasmBezierInstance, _: Record<string, number>): string => bezier.inflections(),
					exampleOptions: {
						[BezierCurveType.Linear]: {
							disabled: true,
						},
						[BezierCurveType.Quadratic]: {
							disabled: true,
						},
					},
				},
				{
					name: "Reduce",
					callback: (bezier: WasmBezierInstance): string => bezier.reduce(),
				},
				{
					name: "Offset",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.offset(options.distance),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [
								{
									variable: "distance",
									min: -50,
									max: 50,
									step: 1,
									default: 20,
								},
							],
						},
					},
				},
				{
					name: "Outline",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.outline(options.distance),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [
								{
									variable: "distance",
									min: 0,
									max: 50,
									step: 1,
									default: 20,
								},
							],
						},
					},
				},
				{
					name: "Graduated Outline",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.graduated_outline(options.start_distance, options.end_distance),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [
								{
									variable: "start_distance",
									min: 0,
									max: 50,
									step: 1,
									default: 30,
								},
								{
									variable: "end_distance",
									min: 0,
									max: 50,
									step: 1,
									default: 30,
								},
							],
						},
					},
				},
				{
					name: "Skewed Outline",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string =>
						bezier.skewed_outline(options.distance1, options.distance2, options.distance3, options.distance4),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [
								{
									variable: "distance1",
									min: 0,
									max: 50,
									step: 1,
									default: 20,
								},
								{
									variable: "distance2",
									min: 0,
									max: 50,
									step: 1,
									default: 10,
								},
								{
									variable: "distance3",
									min: 0,
									max: 50,
									step: 1,
									default: 30,
								},
								{
									variable: "distance4",
									min: 0,
									max: 50,
									step: 1,
									default: 5,
								},
							],
						},
					},
				},
				{
					name: "Arcs",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.arcs(options.error, options.max_iterations, options.strategy),
					exampleOptions: ((): Omit<ExampleOptions, "Linear"> => {
						const sliderOptions = [
							{
								variable: "strategy",
								min: 0,
								max: 2,
								step: 1,
								default: 0,
								unit: [": Automatic", ": FavorLargerArcs", ": FavorCorrectness"],
							},
							{
								variable: "error",
								min: 0.05,
								max: 1,
								step: 0.05,
								default: 0.5,
							},
							{
								variable: "max_iterations",
								min: 50,
								max: 200,
								step: 1,
								default: 100,
							},
						];

						return {
							[BezierCurveType.Quadratic]: {
								customPoints: [
									[50, 50],
									[85, 65],
									[100, 100],
								],
								sliderOptions,
								disabled: false,
							},
							[BezierCurveType.Cubic]: {
								customPoints: [
									[160, 180],
									[170, 10],
									[30, 90],
									[180, 160],
								],
								sliderOptions,
								disabled: false,
							},
						};
					})(),
				},
				{
					name: "Intersect (Line Segment)",
					callback: (bezier: WasmBezierInstance): string => {
						const line = [
							[150, 150],
							[20, 20],
						];
						return bezier.intersect_line_segment(line);
					},
				},
				{
					name: "Intersect (Quadratic)",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => {
						const quadratic = [
							[20, 80],
							[180, 10],
							[90, 120],
						];
						return bezier.intersect_quadratic_segment(quadratic, options.error);
					},
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tErrorOptions],
						},
					},
				},
				{
					name: "Intersect (Cubic)",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => {
						const cubic = [
							[40, 20],
							[100, 40],
							[40, 120],
							[175, 140],
						];
						return bezier.intersect_cubic_segment(cubic, options.error);
					},
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tErrorOptions],
						},
					},
				},
				{
					name: "Intersect (Self)",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.intersect_self(options.error),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tErrorOptions],
						},
						[BezierCurveType.Cubic]: {
							customPoints: [
								[160, 180],
								[170, 10],
								[30, 90],
								[180, 140],
							],
						},
					},
				},
				{
					name: "Rotate",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.rotate(options.angle * Math.PI, 100, 100),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [
								{
									variable: "angle",
									min: 0,
									max: 2,
									step: 1 / 50,
									default: 0.12,
									unit: "π",
								},
							],
						},
					},
				},
				{
					name: "De Casteljau Points",
					callback: (bezier: WasmBezierInstance, options: Record<string, number>): string => bezier.de_casteljau_points(options.t),
					exampleOptions: {
						[BezierCurveType.Quadratic]: {
							sliderOptions: [tSliderOptions],
						},
					},
				},
			],
			subpathFeatures: [
				{
					name: "Constructor",
					callback: (subpath: WasmSubpathInstance): string => subpath.to_svg(),
				},
				{
					name: "Length",
					callback: (subpath: WasmSubpathInstance): string => subpath.length(),
				},
			],
		};
	},
	components: {
		BezierExamplePane,
		SubpathExamplePane,
	},
});
</script>

<style>
#app {
	font-family: Arial, sans-serif;
	text-align: center;
	color: #2c3e50;
	margin-top: 60px;
}
</style>
