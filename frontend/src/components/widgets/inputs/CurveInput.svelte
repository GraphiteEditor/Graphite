<script lang="ts">
	// TODO: add a way to interact with keyboard and touch.

	import { createEventDispatcher } from "svelte";

	import type { Curve, CurveManipulatorGroup } from "@graphite/wasm-communication/messages";

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

	let groups: CurveManipulatorGroup[] = [
		{
			anchor: [0, 0],
			handles: [[-1, -1], [0.25, 0.25]]
		},
		{
			anchor: [0.5, 0.5],
			handles: [[0.25, 0.25], [0.75, 0.75]]
		},
		{
			anchor: [1, 1],
			handles: [[0.75, 0.75], [2, 2]]
		}
	];

	let selectedNodeIndex: number | undefined  = undefined;
	let draggedNodeIndex: number | undefined  = undefined;
	let gridSize: number = 4;

	function updateCurve() {
		dispatch("value", {
			manipulator_groups: groups.slice(1, groups.length - 1),
			first_handle: groups[0].handles[1],
			last_handle: groups[groups.length - 1].handles[0],
		} );
	}

	function recalculateSvgPath() {
		let d: string = "";
		let anchor: [number, number] = groups[0].anchor;
		let handle: [number, number] = groups[0].handles[1];
		for (const group of groups.slice(1)) {
			d += " M " + anchor[0] + " " + (1 - anchor[1]);
			d += (" C " + handle[0] + " " + (1 - handle[1])
				+ ", " + group.handles[0][0] + " " + (1 - group.handles[0][1])
				+ ", " + group.anchor[0] + " " + (1 - group.anchor[1]));
			anchor = group.anchor;
			handle = group.handles[1];
		}
		return d;
	}

	let d: string = recalculateSvgPath();

	$: {
		groups = [groups[0]].concat(value.manipulator_groups).concat([groups[groups.length - 1]]);
		groups[0].handles[1] = value.first_handle;
		groups[groups.length - 1].handles[0] = value.last_handle;
		d = recalculateSvgPath();
	}

	function handleManipulatorMouseDown(e: MouseEvent, i: number) {
		// delete an anchor with right- or middle-click
		if (e.button > 0 && i > 0 && i < groups.length - 1) {
			draggedNodeIndex = undefined;
			selectedNodeIndex = undefined;
			// somehow svelte doesn't recognize a change in `groups`,
			// when we do `groups.splice(i, 1)`, so here we are:
			groups = groups.slice(0, i).concat(groups.slice(i + 1));
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

	function clampHandles() {
		for (let i = 0; i < groups.length - 1; ++i) {
			const [min, max] = [groups[i].anchor[0], groups[i + 1].anchor[0]];
			for (let j = 0; j < 2; ++j) {
				groups[i + j].handles[1 - j][0] = clamp(groups[i + j].handles[1 - j][0], min, max);
				groups[i + j].handles[1 - j][1] = clamp(groups[i + j].handles[1 - j][1], 0, 1);
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
		const anchor: [number, number] = getSvgPositionFromMouseEvent(e);
		let nodeIndex: number = -1;
		// search for the first anchor at the right of the mouse
		while (nodeIndex + 1 < groups.length && groups[++nodeIndex].anchor[0] <= anchor[0]);
		groups.splice(nodeIndex, 0, {
			anchor: anchor,
			handles: [[anchor[0] - 0.05, anchor[1]], [anchor[0] + 0.05, anchor[1]]]
		});
		selectedNodeIndex = nodeIndex;
		clampHandles();
		d = recalculateSvgPath();
		updateCurve();
	}

	function vectorLength(vec: [number, number]): number {
		return Math.sqrt(vec[0] * vec[0] + vec[1] * vec[1]);
	}

	function setHandlePos(anchor: number, handle: number, pos: [number, number]) {
		groups[anchor].handles[handle] = pos;

		const center = groups[anchor].anchor;
		const other = groups[anchor].handles[1 - handle];

		const thisHandleVec = pos.map((c, i) => center[i] - c);
		const thisHandleVecLen = vectorLength(thisHandleVec);
		const thisHandleVecNorm = thisHandleVec.map(c => c / thisHandleVecLen);
		const otherHandleVecLen = vectorLength(other.map((c, i) => center[i] - c));

		groups[anchor].handles[1 - handle] = center.map((c, i) => c + thisHandleVecNorm[i] * otherHandleVecLen);
	}

	function handleMouseMove(e: MouseEvent) {
		if (typeof draggedNodeIndex === "undefined" || draggedNodeIndex === 0 || draggedNodeIndex === groups.length - 1)
			return;
		const pos: [number, number] = getSvgPositionFromMouseEvent(e);
		if (draggedNodeIndex > 0) {
			pos[0] = clamp(pos[0], groups[draggedNodeIndex - 1].anchor[0], groups[draggedNodeIndex + 1].anchor[0])
			groups[draggedNodeIndex].handles = groups[draggedNodeIndex].handles
				.map(p => p.map((c, i) => c + pos[i] - groups[draggedNodeIndex].anchor[i]));
			groups[draggedNodeIndex].anchor = pos;
		} else {
			setHandlePos(selectedNodeIndex, -draggedNodeIndex - 1, pos);

			if (groups[selectedNodeIndex].handles[0][0] > groups[selectedNodeIndex].anchor[0]) {
				groups[selectedNodeIndex].handles = [groups[selectedNodeIndex].handles[1], groups[selectedNodeIndex].handles[0]];
				draggedNodeIndex = -3 - draggedNodeIndex;
			}
		}
		clampHandles();
		d = recalculateSvgPath();
		updateCurve();
	}

</script>

<LayoutRow class={`curve-input`} classes={{ disabled, "sharp-right-corners": sharpRightCorners, ...classes }} style={styleName} {styles} {tooltip}>
	<svg viewBox="0 0 1 1" class="curve-input-svg"
			on:mousemove={handleMouseMove}
			on:mouseup={handleMouseUp} >
		{#each {length: gridSize - 1} as _, i}
			<path d={"M 0 " + ((i + 1) / gridSize) + " L 1 " + ((i + 1) / gridSize) } class="grid pointer-redirect" />
			<path d={"M " + ((i + 1) / gridSize) + " 0 L " + ((i + 1) / gridSize) + " 1" } class="grid pointer-redirect" />
		{/each}
		<path fill="transparent" class="curve pointer-redirect" d={d} />
		{#each [0, 1] as i}
			<path d={(typeof selectedNodeIndex === "undefined") ? "" : ("M " + groups[selectedNodeIndex].anchor[0]
					+ " " + (1 - groups[selectedNodeIndex].anchor[1])
					+ " L " + groups[selectedNodeIndex].handles[i][0]
					+ " " + (1 - groups[selectedNodeIndex].handles[i][1]))}
				style={"visibility: " + ((typeof selectedNodeIndex === "undefined") ? "hidden;" : "visible;")}
				class="marker-line pointer-redirect" />
			<circle cx={(typeof selectedNodeIndex === "undefined") ? 0 : groups[selectedNodeIndex].handles[i][0]}
					cy={(typeof selectedNodeIndex === "undefined") ? 0 : (1 - groups[selectedNodeIndex].handles[i][1])}
					style={"visibility: " + ((typeof selectedNodeIndex === "undefined") ? "hidden;" : "visible;")}
					r="0.02" class="manipulator handle pointer-redirect"
					on:mousedown={e => handleManipulatorMouseDown(e, -i - 1)} />
		{/each}
		{#each groups as group, i}
			<circle cx={group.anchor[0]} cy={1 - group.anchor[1]} r="0.025" class="manipulator pointer-redirect"
				on:mousedown={e => handleManipulatorMouseDown(e, i)} />
		{/each}
		<style>
			.curve {
				stroke: var(--color-e-nearwhite);
				stroke-width: 0.01;
			}

			.manipulator {
				fill: var(--color-1-nearblack);
				stroke: var(--color-e-nearwhite);
				stroke-width: 0.01;
				cursor: grab;
			}

			.manipulator:hover {
				stroke: var(--color-f-white);
				fill: var(--color-f-white);
			}

			.handle {
				fill: var(--color-1-nearblack);
				stroke: var(--color-c-brightgray);
			}

			.handle:hover {
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

		.curve-input-svg {
			z-index: 1;
		}
	}
</style>
