<script lang="ts">
	import { onMount } from "svelte";

	const RULER_THICKNESS = 16;
	const MAJOR_MARK_THICKNESS = 16;
	const MINOR_MARK_THICKNESS = 6;
	const MICRO_MARK_THICKNESS = 3;

	type RulerDirection = "Horizontal" | "Vertical";

	export let direction: RulerDirection = "Vertical";
	export let originX: number;
	export let originY: number;
	export let numberInterval: number;
	export let majorMarkSpacing: number;
	export let minorDivisions = 5;
	export let microDivisions = 2;
	export let tilt: number = 0;

	let rulerInput: HTMLDivElement | undefined;
	let rulerLength = 0;
	let svgBounds = { width: "0px", height: "0px" };

	type Axis = {
		sign: number;
		vec: [number, number];
	};

	$: axes = computeAxes(tilt);
	$: isHorizontal = direction === "Horizontal";
	$: trackedAxis = isHorizontal ? axes.horiz : axes.vert;
	$: otherAxis = isHorizontal ? axes.vert : axes.horiz;
	$: stretchFactor = 1 / (isHorizontal ? trackedAxis.vec[0] : trackedAxis.vec[1]);
	$: stretchedSpacing = majorMarkSpacing * stretchFactor;
	$: effectiveOrigin = computeEffectiveOrigin(direction, originX, originY, otherAxis);
	$: svgPath = computeSvgPath(direction, effectiveOrigin, stretchedSpacing, minorDivisions, microDivisions, rulerLength, otherAxis);
	$: svgTexts = computeSvgTexts(direction, effectiveOrigin, stretchedSpacing, numberInterval, rulerLength, trackedAxis, otherAxis);

	function computeAxes(tilt: number): { horiz: Axis; vert: Axis } {
		const HALF_PI = Math.PI / 2;
		const normTilt = ((tilt % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
		const octant = Math.floor((normTilt + Math.PI / 4) / HALF_PI) % 4;

		const [c, s] = [Math.cos(tilt), Math.sin(tilt)];
		const posX: Axis = { sign: 1, vec: [c, s] };
		const posY: Axis = { sign: 1, vec: [-s, c] };
		const negX: Axis = { sign: -1, vec: [-c, -s] };
		const negY: Axis = { sign: -1, vec: [s, -c] };

		if (octant === 0) return { horiz: posX, vert: posY };
		if (octant === 1) return { horiz: negY, vert: posX };
		if (octant === 2) return { horiz: negX, vert: negY };
		return { horiz: posY, vert: negX };
	}

	function computeEffectiveOrigin(direction: RulerDirection, ox: number, oy: number, otherAxis: Axis): number {
		const [vx, vy] = otherAxis.vec;
		return direction === "Horizontal" ? ox - oy * (vx / vy) : oy - ox * (vy / vx);
	}

	function computeSvgPath(
		direction: RulerDirection,
		effectiveOrigin: number,
		stretchedSpacing: number,
		minorDivisions: number,
		microDivisions: number,
		rulerLength: number,
		otherAxis: Axis,
	): string {
		const adaptive = stretchFactor > 1.3 ? { minor: minorDivisions, micro: 1 } : { minor: minorDivisions, micro: microDivisions };
		const divisions = stretchedSpacing / adaptive.minor / adaptive.micro;
		const majorMarksFrequency = adaptive.minor * adaptive.micro;
		const shiftedOffsetStart = mod(effectiveOrigin, stretchedSpacing) - stretchedSpacing;

		const [vx, vy] = otherAxis.vec;
		// Tick direction: project outward from viewport edge into the ruler strip
		const flip = direction === "Horizontal" ? (vy > 0 ? -1 : 1) : vx > 0 ? -1 : 1;
		const [dx, dy] = [vx * flip, vy * flip];
		const [sxBase, syBase] = direction === "Horizontal" ? [0, RULER_THICKNESS] : [RULER_THICKNESS, 0];

		let path = "";
		let i = 0;
		for (let loc = shiftedOffsetStart; loc < rulerLength + RULER_THICKNESS; loc += divisions) {
			const length = i % majorMarksFrequency === 0 ? MAJOR_MARK_THICKNESS : i % adaptive.micro === 0 ? MINOR_MARK_THICKNESS : MICRO_MARK_THICKNESS;
			i += 1;

			const pos = Math.round(loc) + 0.5;
			const [sx, sy] = direction === "Horizontal" ? [pos, syBase] : [sxBase, pos];
			path += `M${sx},${sy}l${dx * length},${dy * length} `;
		}

		return path;
	}

	function computeSvgTexts(
		direction: RulerDirection,
		effectiveOrigin: number,
		stretchedSpacing: number,
		numberInterval: number,
		rulerLength: number,
		trackedAxis: Axis,
		otherAxis: Axis,
	): { transform: string; text: string }[] {
		const isVertical = direction === "Vertical";

		// Compute the tick tip offset so labels align with the top of the slanted tick
		const [vx, vy] = otherAxis.vec;
		const flip = isVertical ? (vx > 0 ? -1 : 1) : vy > 0 ? -1 : 1;
		const tipOffsetX = vx * flip * MAJOR_MARK_THICKNESS;
		const tipOffsetY = vy * flip * MAJOR_MARK_THICKNESS;

		const shiftedOffsetStart = mod(effectiveOrigin, stretchedSpacing) - stretchedSpacing;
		const increments = Math.round((shiftedOffsetStart - effectiveOrigin) / stretchedSpacing);
		let labelNumber = increments * numberInterval * trackedAxis.sign;

		const results: { transform: string; text: string }[] = [];

		for (let loc = shiftedOffsetStart; loc < rulerLength; loc += stretchedSpacing) {
			const destination = Math.round(loc);
			const x = isVertical ? 9 : destination + 2 + tipOffsetX;
			const y = isVertical ? destination + 1 + tipOffsetY : 9;

			let transform = `translate(${x} ${y})`;
			if (isVertical) transform += " rotate(-90)";

			const num = Math.abs(labelNumber) < 1e-9 ? 0 : labelNumber;
			const text = numberInterval >= 1 ? `${num}` : num.toFixed(Math.abs(Math.log10(numberInterval))).replace(/\.0+$/, "");

			results.push({ transform, text });
			labelNumber += numberInterval * trackedAxis.sign;
		}

		return results;
	}

	export function resize() {
		if (!rulerInput) return;

		const isVertical = direction === "Vertical";
		const newLength = isVertical ? rulerInput.clientHeight : rulerInput.clientWidth;
		const roundedUp = (Math.floor(newLength / stretchedSpacing) + 2) * stretchedSpacing;

		if (roundedUp !== rulerLength) {
			rulerLength = roundedUp;
			const thickness = `${RULER_THICKNESS}px`;
			const length = `${roundedUp}px`;
			svgBounds = isVertical ? { width: thickness, height: length } : { width: length, height: thickness };
		}
	}

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
