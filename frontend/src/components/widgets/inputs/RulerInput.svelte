<script lang="ts" context="module">
	export type RulerDirection = "Horizontal" | "Vertical";
</script>

<script lang="ts">
	import { onMount } from "svelte";

	const RULER_THICKNESS = 16;
	const MAJOR_MARK_THICKNESS = 16;
	const MEDIUM_MARK_THICKNESS = 6;
	const MINOR_MARK_THICKNESS = 3;

	export let direction: RulerDirection = "Vertical";
	export let origin: number;
	export let numberInterval: number;
	export let majorMarkSpacing: number;
	export let mediumDivisions = 5;
	export let minorDivisions = 2;

	let rulerInput: HTMLDivElement | undefined;
	let rulerLength = 0;
	let svgBounds = { width: "0px", height: "0px" };

	$: svgPath = computeSvgPath(direction, origin, majorMarkSpacing, mediumDivisions, minorDivisions, rulerLength);
	$: svgTexts = computeSvgTexts(direction, origin, majorMarkSpacing, numberInterval, rulerLength);

	function computeSvgPath(direction: RulerDirection, origin: number, majorMarkSpacing: number, mediumDivisions: number, minorDivisions: number, rulerLength: number): string {
		const isVertical = direction === "Vertical";
		const lineDirection = isVertical ? "H" : "V";

		const offsetStart = mod(origin, majorMarkSpacing);
		const shiftedOffsetStart = offsetStart - majorMarkSpacing;

		const divisions = majorMarkSpacing / mediumDivisions / minorDivisions;
		const majorMarksFrequency = mediumDivisions * minorDivisions;

		let dPathAttribute = "";
		let i = 0;
		for (let location = shiftedOffsetStart; location < rulerLength; location += divisions) {
			let length;
			if (i % majorMarksFrequency === 0) length = MAJOR_MARK_THICKNESS;
			else if (i % minorDivisions === 0) length = MEDIUM_MARK_THICKNESS;
			else length = MINOR_MARK_THICKNESS;
			i += 1;

			const destination = Math.round(location) + 0.5;
			const startPoint = isVertical ? `${RULER_THICKNESS - length},${destination}` : `${destination},${RULER_THICKNESS - length}`;
			dPathAttribute += `M${startPoint}${lineDirection}${RULER_THICKNESS} `;
		}

		return dPathAttribute;
	}

	function computeSvgTexts(direction: RulerDirection, origin: number, majorMarkSpacing: number, numberInterval: number, rulerLength: number): { transform: string; text: number }[] {
		const isVertical = direction === "Vertical";

		const offsetStart = mod(origin, majorMarkSpacing);
		const shiftedOffsetStart = offsetStart - majorMarkSpacing;

		const svgTextCoordinates = [];

		let text = (Math.ceil(-origin / majorMarkSpacing) - 1) * numberInterval;

		for (let location = shiftedOffsetStart; location < rulerLength; location += majorMarkSpacing) {
			const destination = Math.round(location);
			const x = isVertical ? 9 : destination + 2;
			const y = isVertical ? destination + 1 : 9;

			let transform = `translate(${x} ${y})`;
			if (isVertical) transform += " rotate(270)";

			svgTextCoordinates.push({ transform, text });

			text += numberInterval;
		}

		return svgTextCoordinates;
	}

	export function resize() {
		if (!rulerInput) return;

		const isVertical = direction === "Vertical";

		const newLength = isVertical ? rulerInput.clientHeight : rulerInput.clientWidth;
		const roundedUp = (Math.floor(newLength / majorMarkSpacing) + 1) * majorMarkSpacing;

		if (roundedUp !== rulerLength) {
			rulerLength = roundedUp;
			const thickness = `${RULER_THICKNESS}px`;
			const length = `${roundedUp}px`;
			svgBounds = isVertical ? { width: thickness, height: length } : { width: length, height: thickness };
		}
	}

	// Modulo function that works for negative numbers, unlike the JS `%` operator
	function mod(n: number, m: number): number {
		const remainder = n % m;
		return Math.floor(remainder >= 0 ? remainder : remainder + m);
	}

	onMount(resize);
</script>

<div class={`ruler-input ${direction.toLowerCase()}`} bind:this={rulerInput}>
	<svg style:width={svgBounds.width} style:height={svgBounds.height}>
		<path d={svgPath} />
		{#each svgTexts as svgText}
			<text transform={svgText.transform}>{svgText.text}</text>
		{/each}
	</svg>
</div>

<style lang="scss" global>
	.ruler-input {
		flex: 1 1 100%;
		background: var(--color-4-dimgray);
		overflow: hidden;
		position: relative;

		&.horizontal {
			height: 16px;
		}

		&.vertical {
			width: 16px;

			svg text {
				text-anchor: end;
			}
		}

		svg {
			position: absolute;

			path {
				stroke-width: 1px;
				stroke: var(--color-6-lowergray);
			}

			text {
				font-size: 12px;
				fill: var(--color-8-uppergray);
			}
		}
	}
</style>
