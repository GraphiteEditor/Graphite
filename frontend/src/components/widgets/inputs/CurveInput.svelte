<script lang="ts">
	// TODO: add a way to interact with keyboard and touch.

	import { createEventDispatcher } from "svelte";

	import type { Curve, CurveSample } from "@graphite/wasm-communication/messages";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	// emits: ["update:value"],
	const dispatch = createEventDispatcher<{
		value: Curve;
	}>();

	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number | undefined> = {};
	export let disabled = false;
	export let tooltip: string | undefined = undefined;
	export let sharpRightCorners = false;
	export let value: Curve;

	let samples: CurveSample[] = [
		{
			pos: [0, 0],
			params: [[-1, -1], [0.25, 0.25]]
		},
		{
			pos: [0.5, 0.5],
			params: [[0.25, 0.25], [0.75, 0.75]]
		},
		{
			pos: [1, 1],
			params: [[0.75, 0.75], [2, 2]]
		}
	];

	let selectedNodeIndex: number | undefined  = undefined;
	let draggedNodeIndex: number | undefined  = undefined;
	let gridSize: number = 4;

	function updateCurve() {
		dispatch("value", {
			samples: samples.slice(1, samples.length - 1),
			start_params: samples[0].params[1],
			end_params: samples[samples.length - 1].params[0],
		} );
	}

	function recalculateSvgPath() {
		let d: string = "";
		let pos: [number, number] = samples[0].pos;
		let param: [number, number] = samples[0].params[1];
		for (const sample of samples.slice(1)) {
			d += " M " + pos[0] + " " + (1 - pos[1]);
			d += (" C " + param[0] + " " + (1 - param[1])
				+ ", " + sample.params[0][0] + " " + (1 - sample.params[0][1])
				+ ", " + sample.pos[0] + " " + (1 - sample.pos[1]));
			pos = sample.pos;
			param = sample.params[1];
		}
		return d;
	}

	let d: string = recalculateSvgPath();

	$: {
		samples = [samples[0]].concat(value.samples).concat([samples[samples.length - 1]]);
		samples[0].params[1] = value.start_params;
		samples[samples.length - 1].params[0] = value.end_params;
		d = recalculateSvgPath();
	}

	function handleSampleMouseDown(e: MouseEvent, i: number) {
		// delete a sample with right- or middle-click
		if (e.button > 0 && i > 0 && i < samples.length - 1) {
			draggedNodeIndex = undefined;
			selectedNodeIndex = undefined;
			// somehow svelte doesn't recognize a change in `samples`,
			// when we do `samples.splice(i, 1)`, so here we are:
			samples = samples.slice(0, i).concat(samples.slice(i + 1));
			d = recalculateSvgPath();
			updateCurve();
			return;
		}
		draggedNodeIndex = i;
		if (i >= 0)
			selectedNodeIndex = i;
	}

	function clamp(x: number, min: number, max: number): number {
		return Math.min(Math.max(x, min), max);
	}

	function getSvgPositionFromMouseEvent(e: MouseEvent): [number, number] {
		// mouse events may also occur on the child elements of the svg element.
		// In this case redirect event to its parent.
		const target = e.target.classList.contains("pointer-redirect") ? e.target.parentElement : e.target;
		const rect: DOMRect = target.getBoundingClientRect();
		const x: number = (e.x - rect.x) / rect.width;
		const y: number = 1 - (e.y - rect.y) / rect.height;
		return [clamp(x, 0, 1), clamp(y, 0, 1)];
	}

	function clampParameters() {
		for (let i = 0; i < samples.length - 1; ++i) {
			const [min, max] = [samples[i].pos[0], samples[i + 1].pos[0]];
			for (let j = 0; j < 2; ++j) {
				samples[i + j].params[1 - j][0] = clamp(samples[i + j].params[1 - j][0], min, max);
				samples[i + j].params[1 - j][1] = clamp(samples[i + j].params[1 - j][1], 0, 1);
			}
		}
	}

	function handleMouseUp(e: MouseEvent) {
		if (typeof draggedNodeIndex !== "undefined") {
			draggedNodeIndex = undefined;
			return;
		}
		if (e.button !== 0)
			return;
		const pos: [number, number] = getSvgPositionFromMouseEvent(e);
		let nodeIndex: number = -1;
		// search for the first sample at the right of the mouse
		while (nodeIndex + 1 < samples.length && samples[++nodeIndex].pos[0] <= pos[0]);
		samples.splice(nodeIndex, 0, {
			pos: pos,
			params: [[pos[0] - 0.05, pos[1]], [pos[0] + 0.05, pos[1]]]
		});
		selectedNodeIndex = nodeIndex;
		clampParameters();
		d = recalculateSvgPath();
		updateCurve();
	}

	function vectorLength(vec: [number, number]): number {
		return Math.sqrt(vec[0] * vec[0] + vec[1] * vec[1]);
	}

	function setHandlePos(sample: number, handle: number, pos: [number, number]) {
		samples[sample].params[handle] = pos;

		const center = samples[sample].pos;
		const other = samples[sample].params[1 - handle];

		const thisHandleVec = pos.map((c, i) => center[i] - c);
		const thisHandleVecLen = vectorLength(thisHandleVec);
		const thisHandleVecNorm = thisHandleVec.map(c => c / thisHandleVecLen);
		const otherHandleVecLen = vectorLength(other.map((c, i) => center[i] - c));

		samples[sample].params[1 - handle] = center.map((c, i) => c + thisHandleVecNorm[i] * otherHandleVecLen);
	}

	function handleMouseMove(e: MouseEvent) {
		if (typeof draggedNodeIndex === "undefined" || draggedNodeIndex === 0 || draggedNodeIndex === samples.length - 1)
			return;
		const pos: [number, number] = getSvgPositionFromMouseEvent(e);
		if (draggedNodeIndex > 0) {
			pos[0] = clamp(pos[0], samples[draggedNodeIndex - 1].pos[0], samples[draggedNodeIndex + 1].pos[0])
			samples[draggedNodeIndex].params = samples[draggedNodeIndex].params
				.map(p => p.map((c, i) => c + pos[i] - samples[draggedNodeIndex].pos[i]));
			samples[draggedNodeIndex].pos = pos;
		} else {
			setHandlePos(selectedNodeIndex, -draggedNodeIndex - 1, pos);

			if (samples[selectedNodeIndex].params[0][0] > samples[selectedNodeIndex].pos[0]) {
				samples[selectedNodeIndex].params = [samples[selectedNodeIndex].params[1], samples[selectedNodeIndex].params[0]];
				draggedNodeIndex = -3 - draggedNodeIndex;
			}
		}
		clampParameters();
		d = recalculateSvgPath();
		updateCurve();
	}

</script>

<LayoutRow class={`curve-input`} classes={{ disabled, "sharp-right-corners": sharpRightCorners, ...classes }} style={styleName} {styles} {tooltip}>
	<svg viewBox="0 0 1 1" class="curve-input-samples"
			on:mousemove={handleMouseMove}
			on:mouseup={handleMouseUp} >
		{#each {length: gridSize - 1} as _, i}
			<path d={"M 0 " + ((i + 1) / gridSize) + " L 1 " + ((i + 1) / gridSize) } class="grid pointer-redirect" />
			<path d={"M " + ((i + 1) / gridSize) + " 0 L " + ((i + 1) / gridSize) + " 1" } class="grid pointer-redirect" />
		{/each}
		<path fill="transparent" class="curve pointer-redirect" d={d} />
		{#each [0, 1] as i}
			<path d={(typeof selectedNodeIndex === "undefined") ? "" : ("M " + samples[selectedNodeIndex].pos[0]
					+ " " + (1 - samples[selectedNodeIndex].pos[1])
					+ " L " + samples[selectedNodeIndex].params[i][0]
					+ " " + (1 - samples[selectedNodeIndex].params[i][1]))}
				style={"visibility: " + ((typeof selectedNodeIndex === "undefined") ? "hidden;" : "visible;")}
				class="marker-line pointer-redirect" />
			<circle cx={(typeof selectedNodeIndex === "undefined") ? 0 : samples[selectedNodeIndex].params[i][0]}
					cy={(typeof selectedNodeIndex === "undefined") ? 0 : (1 - samples[selectedNodeIndex].params[i][1])}
					style={"visibility: " + ((typeof selectedNodeIndex === "undefined") ? "hidden;" : "visible;")}
					r="0.02" class="sample marker pointer-redirect"
					on:mousedown={e => handleSampleMouseDown(e, -i - 1)} />
		{/each}
		{#each samples as sample, i}
			<circle cx={sample.pos[0]} cy={1 - sample.pos[1]} r="0.025" class="sample pointer-redirect"
				on:mousedown={e => handleSampleMouseDown(e, i)} />
		{/each}
		<style>
			.curve {
				stroke: var(--color-e-nearwhite);
				stroke-width: 0.01;
			}

			.sample {
				fill: var(--color-1-nearblack);
				stroke: var(--color-e-nearwhite);
				stroke-width: 0.01;
				cursor: grab;
			}

			.sample:hover {
				stroke: var(--color-f-white);
				fill: var(--color-f-white);
			}

			.marker {
				fill: var(--color-1-nearblack);
				stroke: var(--color-c-brightgray);
			}

			.marker:hover {
				stroke: var(--color-a-softgray);
				fill: var(--color-a-softgray);
			}

			.marker-line {
				stroke: var(--color-7-middlegray);
				stroke-width: 0.005;
				pointer-events: none;
			}
			.grid {
				stroke: var(--color-5-dullgray);
				stroke-width: 0.005;
				pointer-events: none;
			}
		</style>
	</svg>
	<slot />
</LayoutRow>

<style lang="scss" global>
	.curve-input {
		background: var(--color-1-nearblack);
		display: flex;
		position: relative;
		min-width: calc(2 * var(--widget-height)) !important;
		max-width: calc(8 * var(--widget-height)) !important;

		.curve-input-samples {
			z-index: 1;
		}
	}
</style>
