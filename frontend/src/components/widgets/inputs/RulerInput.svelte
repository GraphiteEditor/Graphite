<script lang="ts" context="module">
	export type RulerDirection = "Horizontal" | "Vertical";
</script>

<script lang="ts">
	import { onMount } from "svelte";

	const RULER_THICKNESS = 16;
	const MAJOR_MARK_THICKNESS = 16;
	const MINOR_MARK_THICKNESS = 6;
	const MICRO_MARK_THICKNESS = 3;

	export let direction: RulerDirection = "Vertical";
	export let origin: number;
	export let numberInterval: number;
	export let majorMarkSpacing: number;
	export let minorDivisions = 5;
	export let microDivisions = 2;

	let rulerInput: HTMLDivElement | undefined;
	let rulerLength = 0;
	let svgBounds = { width: "0px", height: "0px" };

	$: svgPath = computeSvgPath(direction, origin, majorMarkSpacing, minorDivisions, microDivisions, rulerLength);
	$: svgTexts = computeSvgTexts(direction, origin, majorMarkSpacing, numberInterval, rulerLength);

	function computeSvgPath(direction: RulerDirection, origin: number, majorMarkSpacing: number, minorDivisions: number, microDivisions: number, rulerLength: number): string {
		const isVertical = direction === "Vertical";
		const lineDirection = isVertical ? "H" : "V";

		const offsetStart = mod(origin, majorMarkSpacing);
		const shiftedOffsetStart = offsetStart - majorMarkSpacing;

		const divisions = majorMarkSpacing / minorDivisions / microDivisions;
		const majorMarksFrequency = minorDivisions * microDivisions;

		let dPathAttribute = "";
		let i = 0;
		for (let location = shiftedOffsetStart; location < rulerLength; location += divisions) {
			let length;
			if (i % majorMarksFrequency === 0) length = MAJOR_MARK_THICKNESS;
			else if (i % microDivisions === 0) length = MINOR_MARK_THICKNESS;
			else length = MICRO_MARK_THICKNESS;
			i += 1;

			const destination = Math.round(location) + 0.5;
			const startPoint = isVertical ? `${RULER_THICKNESS - length},${destination}` : `${destination},${RULER_THICKNESS - length}`;
			dPathAttribute += `M${startPoint}${lineDirection}${RULER_THICKNESS} `;
		}

		return dPathAttribute;
	}

	function computeSvgTexts(direction: RulerDirection, origin: number, majorMarkSpacing: number, numberInterval: number, rulerLength: number): { transform: string; text: string }[] {
		const isVertical = direction === "Vertical";

		const offsetStart = mod(origin, majorMarkSpacing);
		const shiftedOffsetStart = offsetStart - majorMarkSpacing;

		const svgTextCoordinates = [];

		let labelNumber = (Math.ceil(-origin / majorMarkSpacing) - 1) * numberInterval;

		for (let location = shiftedOffsetStart; location < rulerLength; location += majorMarkSpacing) {
			const destination = Math.round(location);
			const x = isVertical ? 9 : destination + 2;
			const y = isVertical ? destination + 1 : 9;

			let transform = `translate(${x} ${y})`;
			if (isVertical) transform += " rotate(270)";

			const text = numberInterval >= 1 ? `${labelNumber}` : labelNumber.toFixed(Math.abs(Math.log10(numberInterval))).replace(/\.0+$/, "");

			svgTextCoordinates.push({ transform, text });

			labelNumber += numberInterval;
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
		background: var(--color-2-mildblack);
		overflow: hidden;
		position: relative;
		box-sizing: border-box;

		&.horizontal {
			height: 16px;
			border-bottom: 1px solid var(--color-5-dullgray);
		}

		&.vertical {
			width: 16px;
			border-right: 1px solid var(--color-5-dullgray);

			svg text {
				text-anchor: end;
			}
		}

		svg {
			position: absolute;

			path {
				stroke-width: 1px;
				stroke: var(--color-5-dullgray);
			}

			text {
				font-size: 12px;
				fill: var(--color-8-uppergray);
			}
		}
	}
</style>
