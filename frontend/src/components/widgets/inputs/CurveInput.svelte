<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { Curve, CurveManipulatorGroup } from "@graphite/messages";
	import { clamp } from "@graphite/utility-functions/math";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	const dispatch = createEventDispatcher<{
		value: Curve;
	}>();

	export let classes: Record<string, boolean> = {};
	let styleName = "";
	export { styleName as style };
	export let styles: Record<string, string | number | undefined> = {};
	export let value: Curve;
	export let disabled = false;
	export let tooltip: string | undefined = undefined;

	const GRID_SIZE = 4;

	let manipulatorsList: CurveManipulatorGroup[] = [
		{
			anchor: [0, 0],
			handles: [
				[-1, -1],
				[0.25, 0.25],
			],
		},
		{
			anchor: [0.5, 0.5],
			handles: [
				[0.25, 0.25],
				[0.75, 0.75],
			],
		},
		{
			anchor: [1, 1],
			handles: [
				[0.75, 0.75],
				[2, 2],
			],
		},
	];
	let selectedNodeIndex: number | undefined = undefined;
	let draggedNodeIndex: number | undefined = undefined;
	let dAttribute = recalculateSvgPath();

	$: {
		manipulatorsList = [manipulatorsList[0]].concat(value.manipulatorGroups).concat([manipulatorsList[manipulatorsList.length - 1]]);
		manipulatorsList[0].handles[1] = value.firstHandle;
		manipulatorsList[manipulatorsList.length - 1].handles[0] = value.lastHandle;
		dAttribute = recalculateSvgPath();
	}

	function updateCurve() {
		dispatch("value", {
			manipulatorGroups: manipulatorsList.slice(1, manipulatorsList.length - 1),
			firstHandle: manipulatorsList[0].handles[1],
			lastHandle: manipulatorsList[manipulatorsList.length - 1].handles[0],
		});
	}

	function recalculateSvgPath() {
		let dAttribute = "";
		let anchor = manipulatorsList[0].anchor;
		let handle = manipulatorsList[0].handles[1];

		manipulatorsList.slice(1).forEach((m) => {
			dAttribute += `M${anchor[0]} ${1 - anchor[1]} C${handle[0]} ${1 - handle[1]}, ${m.handles[0][0]} ${1 - m.handles[0][1]}, ${m.anchor[0]} ${1 - m.anchor[1]} `;
			anchor = m.anchor;
			handle = m.handles[1];
		});

		return dAttribute;
	}

	function handleManipulatorPointerDown(e: PointerEvent, i: number) {
		// Delete an anchor with RMB or MMB
		if (e.button > 0 && i > 0 && i < manipulatorsList.length - 1) {
			draggedNodeIndex = undefined;
			selectedNodeIndex = undefined;

			manipulatorsList.splice(i, 1);
			manipulatorsList = manipulatorsList;

			dAttribute = recalculateSvgPath();

			updateCurve();

			return;
		}

		draggedNodeIndex = i;
		if (i >= 0) selectedNodeIndex = i;
	}

	function getSvgPositionFromPointerEvent(e: PointerEvent): [number, number] | undefined {
		if (!(e.target instanceof SVGElement)) return undefined;

		const target = e.target?.closest("svg") || undefined;
		if (!target) return undefined;

		const rect = target.getBoundingClientRect();
		const x = (e.x - rect.x) / rect.width;
		const y = 1 - (e.y - rect.y) / rect.height;
		return [clamp(x), clamp(y)];
	}

	function clampHandles() {
		for (let i = 0; i < manipulatorsList.length - 1; i++) {
			const [min, max] = [manipulatorsList[i].anchor[0], manipulatorsList[i + 1].anchor[0]];

			for (let j = 0; j < 2; j++) {
				manipulatorsList[i + j].handles[1 - j][0] = clamp(manipulatorsList[i + j].handles[1 - j][0], min, max);
				manipulatorsList[i + j].handles[1 - j][1] = clamp(manipulatorsList[i + j].handles[1 - j][1]);
			}
		}
	}

	function handlePointerUp(e: PointerEvent) {
		if (draggedNodeIndex !== undefined) {
			draggedNodeIndex = undefined;
			return;
		}
		if (e.button !== 0) return;
		const anchor = getSvgPositionFromPointerEvent(e);
		if (!anchor) return;

		let nodeIndex = manipulatorsList.findIndex((manipulators) => manipulators.anchor[0] > anchor[0]);
		if (nodeIndex === -1) nodeIndex = manipulatorsList.length;

		manipulatorsList.splice(nodeIndex, 0, {
			anchor: anchor,
			handles: [
				[anchor[0] - 0.05, anchor[1]],
				[anchor[0] + 0.05, anchor[1]],
			],
		});
		selectedNodeIndex = nodeIndex;
		clampHandles();
		dAttribute = recalculateSvgPath();
		updateCurve();
	}

	function setHandlePosition(anchorIndex: number, handleIndex: number, position: [number, number]) {
		const { anchor, handles } = manipulatorsList[anchorIndex];
		const otherHandle = handles[1 - handleIndex];

		const handleVector = [anchor[0] - position[0], anchor[1] - position[1]];
		const handleVectorLength = Math.hypot(...handleVector);
		const handleVectorNormalized = [handleVector[0] / handleVectorLength, handleVector[1] / handleVectorLength];
		const otherHandleVectorLength = Math.hypot(anchor[0] - otherHandle[0], anchor[1] - otherHandle[1]);

		handles[handleIndex] = position;
		handles[1 - handleIndex] = [anchor[0] + handleVectorNormalized[0] * otherHandleVectorLength, anchor[1] + handleVectorNormalized[1] * otherHandleVectorLength];
	}

	function handlePointerMove(e: PointerEvent) {
		if (draggedNodeIndex === undefined || draggedNodeIndex === 0 || draggedNodeIndex === manipulatorsList.length - 1) return;
		const position = getSvgPositionFromPointerEvent(e);
		if (!position) return;

		if (draggedNodeIndex > 0) {
			position[0] = clamp(position[0], manipulatorsList[draggedNodeIndex - 1].anchor[0], manipulatorsList[draggedNodeIndex + 1].anchor[0]);

			const manipulators = manipulatorsList[draggedNodeIndex];
			manipulators.handles = [
				[manipulators.handles[0][0] + position[0] - manipulators.anchor[0], manipulators.handles[0][1] + position[1] - manipulators.anchor[1]],
				[manipulators.handles[1][0] + position[0] - manipulators.anchor[0], manipulators.handles[1][1] + position[1] - manipulators.anchor[1]],
			];
			manipulators.anchor = position;
		} else {
			if (selectedNodeIndex === undefined) return;
			setHandlePosition(selectedNodeIndex, -draggedNodeIndex - 1, position);

			const manipulators = manipulatorsList[selectedNodeIndex];
			if (manipulators.handles[0][0] > manipulators.anchor[0]) {
				manipulators.handles = [manipulators.handles[1], manipulators.handles[0]];
				draggedNodeIndex = -3 - draggedNodeIndex;
			}
		}

		clampHandles();
		dAttribute = recalculateSvgPath();
		updateCurve();
	}
</script>

<LayoutRow class="curve-input" classes={{ disabled, ...classes }} style={styleName} {styles} {tooltip}>
	<svg viewBox="0 0 1 1" on:pointermove={handlePointerMove} on:pointerup={handlePointerUp}>
		{#each { length: GRID_SIZE - 1 } as _, i}
			<path class="grid" d={`M 0 ${(i + 1) / GRID_SIZE} L 1 ${(i + 1) / GRID_SIZE}`} />
			<path class="grid" d={`M ${(i + 1) / GRID_SIZE} 0 L ${(i + 1) / GRID_SIZE} 1`} />
		{/each}
		<path class="curve" d={dAttribute} />
		{#if selectedNodeIndex !== undefined}
			{@const m = manipulatorsList[selectedNodeIndex]}
			{#each [0, 1] as i}
				<path d={`M ${m.anchor[0]} ${1 - m.anchor[1]} L ${m.handles[i][0]} ${1 - m.handles[i][1]}`} class="handle-line" />
				<circle cx={m.handles[i][0]} cy={1 - m.handles[i][1]} class="manipulator handle" r="0.02" on:pointerdown={(e) => handleManipulatorPointerDown(e, -i - 1)} />
			{/each}
		{/if}
		{#each manipulatorsList as manipulators, i}
			<circle cx={manipulators.anchor[0]} cy={1 - manipulators.anchor[1]} class="manipulator" r="0.02" on:pointerdown={(e) => handleManipulatorPointerDown(e, i)} />
		{/each}
	</svg>
	<slot />
</LayoutRow>

<style lang="scss" global>
	.curve-input {
		background: var(--color-1-nearblack);
		display: flex;
		position: relative;
		min-width: calc(2 * var(--widget-height));
		max-width: calc(8 * var(--widget-height));

		.grid {
			stroke: var(--color-5-dullgray);
			stroke-width: 0.005;
			pointer-events: none;
		}

		.curve {
			fill: none;
			stroke: var(--color-e-nearwhite);
			stroke-width: 0.01;
		}

		.manipulator {
			fill: var(--color-1-nearblack);
			stroke: var(--color-e-nearwhite);
			stroke-width: 0.01;

			&:hover {
				fill: var(--color-f-white);
				stroke: var(--color-f-white);
			}

			&.handle {
				fill: var(--color-1-nearblack);
				stroke: var(--color-c-brightgray);

				&:hover {
					fill: var(--color-a-softgray);
					stroke: var(--color-a-softgray);
				}
			}
		}

		.handle-line {
			stroke: var(--color-5-dullgray);
			stroke-width: 0.005;
			pointer-events: none;
		}
	}
</style>
