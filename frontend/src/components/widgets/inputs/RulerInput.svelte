<script lang="ts">
	import { onMount } from "svelte";

	const RULER_THICKNESS = 16;
	const MAJOR_MARK_THICKNESS = 16;
	const MINOR_MARK_THICKNESS = 6;
	const MICRO_MARK_THICKNESS = 3;
	const TAU = 2 * Math.PI;

	type RulerDirection = "Horizontal" | "Vertical";

	export let direction: RulerDirection = "Vertical";
	export let originX: number;
	export let originY: number;
	export let tilt: number;
	export let flip: boolean = false;
	export let numberInterval: number;
	export let majorMarkSpacing: number;
	export let minorDivisions = 5;
	export let microDivisions = 2;
	export let cursorPosition: { x: number; y: number } | undefined = undefined;

	let rulerInput: HTMLDivElement | undefined;
	let rulerLength = 0;
	let svgBounds = { width: "0px", height: "0px" };

	type Axis = { sign: number; vec: [number, number] };

	$: axes = computeAxes(tilt);
	$: isHorizontal = direction === "Horizontal";
	$: trackedAxis = isHorizontal ? axes.horiz : axes.vert;
	$: otherAxis = isHorizontal ? axes.vert : axes.horiz;
	$: crossAxisDirection = flipVector(otherAxis.vec, flip);
	$: stretchFactor = 1 / Math.max(Math.abs(isHorizontal ? trackedAxis.vec[0] : trackedAxis.vec[1]), 1e-10);
	$: stretchedSpacing = majorMarkSpacing * stretchFactor;
	$: effectiveOrigin = projectOntoRuler(direction, originX, originY, crossAxisDirection);
	$: svgPath = computeSvgPath(direction, effectiveOrigin, stretchedSpacing, stretchFactor, minorDivisions, microDivisions, rulerLength, crossAxisDirection);
	$: svgTexts = computeSvgTexts(direction, effectiveOrigin, stretchedSpacing, numberInterval, rulerLength, trackedAxis, crossAxisDirection);
	$: cursorIndicatorPath = computeCursorIndicator(direction, cursorPosition, crossAxisDirection);

	function computeAxes(tilt: number): { horiz: Axis; vert: Axis } {
		const normTilt = ((tilt % TAU) + TAU) % TAU;
		const octant = Math.floor((normTilt + Math.PI / 4) / (Math.PI / 2)) % 4;

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

	function flipVector(vec: [number, number], flipped: boolean): [number, number] {
		return flipped ? [-vec[0], vec[1]] : vec;
	}

	function projectOntoRuler(direction: RulerDirection, x: number, y: number, vec: [number, number]): number {
		const [vx, vy] = vec;
		if (direction === "Horizontal") return Math.abs(vy) < 1e-10 ? x : x - y * (vx / vy);
		return Math.abs(vx) < 1e-10 ? y : y - x * (vy / vx);
	}

	function tickMarkGeometry(direction: RulerDirection, vx: number, vy: number): { dx: number; dy: number; sxBase: number; syBase: number } {
		const reversal = direction === "Horizontal" ? (vy > 0 ? -1 : 1) : vx > 0 ? -1 : 1;
		return {
			dx: vx * reversal,
			dy: vy * reversal,
			sxBase: direction === "Horizontal" ? 0 : RULER_THICKNESS,
			syBase: direction === "Horizontal" ? RULER_THICKNESS : 0,
		};
	}

	function computeSvgPath(
		direction: RulerDirection,
		effectiveOrigin: number,
		stretchedSpacing: number,
		stretchFactor: number,
		minorDivisions: number,
		microDivisions: number,
		rulerLength: number,
		crossAxisDirection: [number, number],
	): string {
		const adaptive = stretchFactor > 1.3 ? { minor: minorDivisions, micro: 1 } : { minor: minorDivisions, micro: microDivisions };
		const divisions = stretchedSpacing / adaptive.minor / adaptive.micro;
		const majorMarksFrequency = adaptive.minor * adaptive.micro;
		const shiftedOffsetStart = mod(effectiveOrigin, stretchedSpacing) - stretchedSpacing;

		const { dx, dy, sxBase, syBase } = tickMarkGeometry(direction, crossAxisDirection[0], crossAxisDirection[1]);

		let path = "";
		let i = 0;
		for (let location = shiftedOffsetStart; location < rulerLength + RULER_THICKNESS; location += divisions) {
			let length;
			if (i % majorMarksFrequency === 0) length = MAJOR_MARK_THICKNESS;
			else if (i % adaptive.micro === 0) length = MINOR_MARK_THICKNESS;
			else length = MICRO_MARK_THICKNESS;
			i += 1;

			const destination = Math.round(location) + 0.5;
			const [sx, sy] = direction === "Horizontal" ? [destination, syBase] : [sxBase, destination];
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
		crossAxisDirection: [number, number],
	): { transform: string; text: string }[] {
		const isVertical = direction === "Vertical";

		const { dx: tipDx, dy: tipDy } = tickMarkGeometry(direction, crossAxisDirection[0], crossAxisDirection[1]);
		const forwardTip = isVertical ? -tipDy : tipDx;
		const tiltScale = forwardTip >= 0 ? 1 : 0.5;
		const tipOffsetX = tipDx * MAJOR_MARK_THICKNESS * tiltScale;
		const tipOffsetY = tipDy * MAJOR_MARK_THICKNESS * tiltScale;

		const shiftedOffsetStart = mod(effectiveOrigin, stretchedSpacing) - stretchedSpacing;
		const increments = Math.round((shiftedOffsetStart - effectiveOrigin) / stretchedSpacing);
		let labelNumber = increments * numberInterval * trackedAxis.sign;

		const results: { transform: string; text: string }[] = [];

		for (let location = shiftedOffsetStart; location < rulerLength; location += stretchedSpacing) {
			const destination = Math.round(location);
			const x = isVertical ? 9 : destination + 2 + tipOffsetX;
			const y = isVertical ? destination + 1 + tipOffsetY : 9;

			let transform = `translate(${x} ${y})`;
			if (isVertical) transform += " rotate(270)";

			const num = Math.abs(labelNumber) < 1e-9 ? 0 : labelNumber;
			const text = numberInterval >= 1 ? `${num}` : num.toFixed(Math.abs(Math.log10(numberInterval))).replace(/\.0+$/, "");

			results.push({ transform, text });

			labelNumber += numberInterval * trackedAxis.sign;
		}

		return results;
	}

	function computeCursorIndicator(direction: RulerDirection, cursor: { x: number; y: number } | undefined, crossAxisDirection: [number, number]): string {
		if (cursor === undefined) return "";

		const projected = projectOntoRuler(direction, cursor.x, cursor.y, crossAxisDirection);
		const { dx, dy, sxBase, syBase } = tickMarkGeometry(direction, crossAxisDirection[0], crossAxisDirection[1]);

		// Scale the line so it spans the full ruler bar thickness
		const thicknessComponent = Math.abs(direction === "Horizontal" ? dy : dx);
		const length = thicknessComponent < 1e-10 ? RULER_THICKNESS : RULER_THICKNESS / thicknessComponent;

		const destination = Math.round(projected) + 0.5;
		const [sx, sy] = direction === "Horizontal" ? [destination, syBase] : [sxBase, destination];
		return `M${sx},${sy}l${dx * length},${dy * length}`;
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
		{#if cursorIndicatorPath}
			<path class="cursor-indicator" d={cursorIndicatorPath} />
		{/if}
	</svg>
</div>

<style lang="scss">
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

				&.cursor-indicator {
					stroke: var(--color-8-uppergray);
				}
			}

			text {
				font-size: 12px;
				fill: var(--color-8-uppergray);
			}
		}
	}
</style>
