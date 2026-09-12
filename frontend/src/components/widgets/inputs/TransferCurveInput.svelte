<script lang="ts">
	import { createEventDispatcher, onDestroy } from "svelte";
	import { preventEscapeClosingParentFloatingMenu } from "/src/components/layout/FloatingMenu.svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import type { TransferCurveInputUpdate } from "/wrapper/pkg/graphite_wasm_wrapper";

	const BUTTON_LEFT = 0;
	const BUTTON_RIGHT = 2;
	// Smallest horizontal gap kept between neighboring points, in box-normalized units, so the curve stays a function
	const MINIMUM_X_GAP = 1 / 1024;
	// How far from a point, in pixels, a press still takes it rather than inserting another
	const GRAB_RADIUS = 16;

	const dispatch = createEventDispatcher<{ update: TransferCurveInputUpdate; commit: undefined }>();

	export let points: [number, number][];
	export let samples: [number, number][];
	export let domain: [number, number];
	export let range: [number, number];
	export let clampToRange = true;
	export let allowInsert = true;
	export let allowDelete = true;
	export let disabled = false;

	// Reference to the box DOM element so pointer coordinates can be converted to box-normalized positions
	let boxElement: HTMLDivElement | undefined = undefined;

	// The point a drag is carrying, held by the frontend for the drag's length; Rust owns the authoritative point data
	let activePointIndex: number | undefined = undefined;
	// Where the dragged point began, restored if the drag is cancelled, and whether this drag created it
	let dragRestore: [number, number] | undefined = undefined;
	let dragInserted = false;
	// Whether the press moved the point, so the double-click a second press can produce deletes nothing
	let dragMoved = false;
	// An insert waiting to be reported back, with the point asked for and the count before it, so its index can be read off the reply
	let pendingInsert: { point: [number, number]; priorCount: number; abandoned: boolean } | undefined = undefined;
	// The curve's place under the pointer, previewed while the pointer sits over empty space
	let insertPreview: [number, number] | undefined = undefined;
	// The point a press would take, lit so it is clear which one a click affects
	let targetPointIndex: number | undefined = undefined;
	// The points the last two presses took, which a double-click needs to know landed on the same one
	let pressIndex: number | undefined = undefined;
	let previousPressIndex: number | undefined = undefined;

	function emit(update: TransferCurveInputUpdate) {
		dispatch("update", update);
	}

	function normalizedX(x: number): number {
		return (x - domain[0]) / (domain[1] - domain[0] || 1);
	}

	function normalizedY(y: number): number {
		return (y - range[0]) / (range[1] - range[0] || 1);
	}

	function fromNormalized(nx: number, ny: number): [number, number] {
		return [domain[0] + nx * (domain[1] - domain[0]), range[0] + ny * (range[1] - range[0])];
	}

	function pointerNormalized(e: MouseEvent): [number, number] | undefined {
		const rect = boxElement?.getBoundingClientRect();
		if (!rect || rect.width === 0 || rect.height === 0) return undefined;

		const nx = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
		let ny = 1 - (e.clientY - rect.top) / rect.height;
		if (clampToRange) ny = Math.max(0, Math.min(1, ny));
		return [nx, ny];
	}

	// A dragged point passes the others rather than stopping at them, stepping over the hair of space each keeps to stay solvable
	function clearOfOtherPoints(index: number, nx: number): number {
		const others = points.filter((_, other) => other !== index).map((point) => normalizedX(point[0]));
		const isClear = (x: number) => x >= 0 && x <= 1 && others.every((at) => Math.abs(x - at) >= MINIMUM_X_GAP - 1e-12);
		if (isClear(nx)) return nx;

		// The nearest clear spot sits at the edge of some point's gap, the higher side winning a tie
		const candidates = others.flatMap((at) => [at + MINIMUM_X_GAP, at - MINIMUM_X_GAP]).filter(isClear);
		let nearest: number | undefined = undefined;
		for (let i = 0; i < candidates.length; i += 1) {
			const candidate = candidates[i];
			const distance = Math.abs(candidate - nx);
			if (nearest === undefined || distance < Math.abs(nearest - nx) || (distance === Math.abs(nearest - nx) && candidate > nearest)) nearest = candidate;
		}

		return nearest ?? nx;
	}

	// The point a press takes: the nearest within the grab radius, measured in pixels so points bunched
	// together resolve by true distance rather than by which of them is drawn on top
	function nearestPointIndex(position: [number, number]): number | undefined {
		const rect = boxElement?.getBoundingClientRect();
		if (!rect) return undefined;

		let nearest: number | undefined = undefined;
		let nearestDistance = GRAB_RADIUS;

		points.forEach((point, index) => {
			const x = (normalizedX(point[0]) - position[0]) * rect.width;
			const y = (normalizedY(point[1]) - position[1]) * rect.height;
			const distance = Math.sqrt(x * x + y * y);
			if (distance < nearestDistance) {
				nearestDistance = distance;
				nearest = index;
			}
		});

		return nearest;
	}

	function beginPointDrag(index: number) {
		activePointIndex = index;
		dragRestore = [...points[index]];
		dragInserted = false;
		dragMoved = false;
		dispatch("commit");
		addEvents();
	}

	function insertPoint(position: [number, number]) {
		// Kept clear of the others' x from the start, as a drag would keep it, since the index it will take is the one past the end
		const point = fromNormalized(clearOfOtherPoints(points.length, position[0]), position[1]);

		dispatch("commit");
		emit({ InsertPoint: { x: point[0], y: point[1] } });

		// The drag waits for the reply, since until then an index would name a neighbor in the curve as it stands without the point
		pendingInsert = { point, priorCount: points.length, abandoned: false };
		activePointIndex = undefined;
		dragRestore = point;
		dragInserted = true;
		dragMoved = false;
		addEvents();
	}

	function boxPointerDown(e: PointerEvent) {
		if (disabled) return;

		const position = pointerNormalized(e);
		if (!position) return;

		// Resolved again here, since a pen or touch press arrives with no hover to have settled it
		const index = nearestPointIndex(position);
		targetPointIndex = index;
		insertPreview = undefined;
		previousPressIndex = pressIndex;
		pressIndex = index;

		if (index !== undefined) {
			if (e.button === BUTTON_LEFT) beginPointDrag(index);
			else if (e.button === BUTTON_RIGHT) removePoint(index);
			return;
		}

		if (e.button === BUTTON_LEFT && allowInsert) insertPoint(insertionAt(position));
	}

	// Acts only where both presses took the same point, so the one an empty-space click inserts is not deleted by the click after it
	function boxDoubleClick() {
		if (disabled || dragMoved || pressIndex === undefined || pressIndex !== previousPressIndex) return;
		removePoint(pressIndex);
	}

	// A right-click or double-click removes a point, except that the outermost points anchor the corners and return to their own instead
	function removePoint(index: number) {
		const pressed = points[index];
		if (!allowDelete || !pressed) return;

		let end: number | undefined = undefined;
		if (points.every((point) => point[0] >= pressed[0])) end = 0;
		else if (points.every((point) => point[0] <= pressed[0])) end = 1;

		dispatch("commit");
		if (end !== undefined) {
			const corner = fromNormalized(end, end);
			emit({ MovePoint: { index, x: corner[0], y: corner[1] } });
		} else {
			deletePoint(index);
		}
	}

	// Takes up the dragging of an inserted point once the reply carries it, found by position since Rust chooses where it lands
	function adoptInsertedPoint(reported: [number, number][]) {
		if (!pendingInsert || reported.length <= pendingInsert.priorCount) return;

		const [x, y] = pendingInsert.point;
		const distanceSquared = (point: [number, number]) => (point[0] - x) ** 2 + (point[1] - y) ** 2;
		let nearest = 0;
		for (let i = 1; i < reported.length; i += 1) {
			if (distanceSquared(reported[i]) < distanceSquared(reported[nearest])) nearest = i;
		}

		if (pendingInsert.abandoned) deletePoint(nearest);
		else activePointIndex = nearest;
		pendingInsert = undefined;
	}
	$: adoptInsertedPoint(points);

	// A drag already owns the pointer, so the hover it left behind stands
	function boxPointerMove(e: PointerEvent) {
		if (disabled || dragRestore !== undefined) return;

		const position = pointerNormalized(e);
		targetPointIndex = position ? nearestPointIndex(position) : undefined;
		insertPreview = allowInsert && targetPointIndex === undefined && position ? insertionAt(position) : undefined;
	}

	function boxPointerLeave() {
		targetPointIndex = undefined;
		insertPreview = undefined;
	}

	function deletePoint(index: number) {
		emit({ DeletePoint: { index } });
		if (activePointIndex === index) activePointIndex = undefined;
		else if (activePointIndex !== undefined && activePointIndex > index) activePointIndex -= 1;
	}

	function onPointerMove(e: PointerEvent) {
		if (activePointIndex === undefined) return;
		if (e.buttons === 0) {
			stopDrag();
			return;
		}

		const position = pointerNormalized(e);
		if (!position) return;
		const point = fromNormalized(clearOfOtherPoints(activePointIndex, position[0]), position[1]);

		dragMoved = true;
		emit({ MovePoint: { index: activePointIndex, x: point[0], y: point[1] } });
	}

	function abortDrag() {
		if (activePointIndex !== undefined) {
			if (dragInserted) deletePoint(activePointIndex);
			else if (dragRestore) emit({ MovePoint: { index: activePointIndex, x: dragRestore[0], y: dragRestore[1] } });
		} else if (pendingInsert) {
			// The reply has yet to name the inserted point, so it is deleted when that arrives
			pendingInsert.abandoned = true;
		}
		stopDrag();
	}

	function stopDrag() {
		removeEvents();
		activePointIndex = undefined;
		dragRestore = undefined;
		dragInserted = false;
		if (!pendingInsert?.abandoned) pendingInsert = undefined;
	}

	function onPointerUp() {
		stopDrag();
	}

	function onMouseDown(e: MouseEvent) {
		const BUTTONS_RIGHT = 0b0000_0010;
		if (e.buttons & BUTTONS_RIGHT) abortDrag();
	}

	function onKeyDown(e: KeyboardEvent) {
		if (e.key !== "Escape") return;
		if (boxElement) preventEscapeClosingParentFloatingMenu(boxElement);
		abortDrag();
	}

	function addEvents() {
		document.addEventListener("pointermove", onPointerMove);
		document.addEventListener("pointerup", onPointerUp);
		document.addEventListener("mousedown", onMouseDown);
		document.addEventListener("keydown", onKeyDown);
	}

	function removeEvents() {
		document.removeEventListener("pointermove", onPointerMove);
		document.removeEventListener("pointerup", onPointerUp);
		document.removeEventListener("mousedown", onMouseDown);
		document.removeEventListener("keydown", onKeyDown);
	}

	// The polyline in the box's unit square, with SVG's downward y flipped to point upward
	$: pathData = samples.map((sample, i) => `${i === 0 ? "M" : "L"}${sample[0]} ${1 - sample[1]}`).join(" ");

	// Where a click would leave a new point: the curve's own height below or above a pointer near it, read off the evenly
	// spaced samples the editor bakes, or the pointer's own place when it sits further off
	function insertionAt(position: [number, number]): [number, number] {
		const height = boxElement?.getBoundingClientRect().height || 0;
		if (samples.length < 2 || height === 0) return position;

		const along = Math.max(0, Math.min(1, position[0])) * (samples.length - 1);
		const lower = Math.floor(along);
		const upper = Math.min(lower + 1, samples.length - 1);
		const y = samples[lower][1] + (samples[upper][1] - samples[lower][1]) * (along - lower);

		return Math.abs(y - position[1]) * height <= GRAB_RADIUS ? [position[0], y] : position;
	}

	onDestroy(removeEvents);
</script>

<LayoutCol class="transfer-curve-input" classes={{ disabled }}>
	<div class="pointer-field" on:pointerdown={boxPointerDown} on:pointermove={boxPointerMove} on:pointerleave={boxPointerLeave} on:dblclick={boxDoubleClick}>
		<div class="curve-box" bind:this={boxElement}>
			<svg viewBox="0 0 1 1" preserveAspectRatio="none" xmlns="http://www.w3.org/2000/svg">
				<line class="identity-line" x1="0" y1="1" x2="1" y2="0" vector-effect="non-scaling-stroke" />
				<path class="curve-path" d={pathData} vector-effect="non-scaling-stroke" />
			</svg>
			{#each points as point, index}
				<div
					class="curve-point"
					class:active={index === activePointIndex}
					class:targeted={index === targetPointIndex}
					style:--point-x={normalizedX(point[0])}
					style:--point-y={normalizedY(point[1])}
				></div>
			{/each}
			{#if insertPreview}
				<div class="curve-point preview" style:--point-x={insertPreview[0]} style:--point-y={insertPreview[1]}></div>
			{/if}
		</div>
	</div>
</LayoutCol>

<style lang="scss">
	.transfer-curve-input {
		// The grid is inset by the handle radius so a point sitting in a corner stays within the widget's field
		--handle-radius: 4px;
		--grid-size: 256px;
		flex: 1 1 100%;
		box-sizing: border-box;
		max-width: calc(var(--grid-size) + 2 * var(--handle-radius));
		border-radius: 2px;
		background-color: var(--color-2-mildblack);

		// The inset belongs to the pointer target, so a press in it reaches the points whose handles reach out into it
		.pointer-field {
			padding: var(--handle-radius);

			.curve-box {
				position: relative;
				width: 100%;
				max-width: var(--grid-size);
				aspect-ratio: 1;
				// Quarter grid lines, with an inset frame supplying the outermost ones on all four edges
				background-image: linear-gradient(var(--color-3-darkgray) 1px, transparent 1px), linear-gradient(90deg, var(--color-3-darkgray) 1px, transparent 1px);
				background-size: 25% 25%;
				box-shadow: inset 0 0 0 1px var(--color-3-darkgray);

				svg {
					position: absolute;
					top: 0;
					left: 0;
					width: 100%;
					height: 100%;
					overflow: visible;
					pointer-events: none;

					.identity-line {
						stroke: var(--color-3-darkgray);
						stroke-width: 1px;
					}

					.curve-path {
						fill: none;
						stroke: var(--color-e-nearwhite);
						stroke-width: 1.5px;
					}
				}

				.curve-point {
					position: absolute;
					left: calc(var(--point-x) * 100%);
					top: calc((1 - var(--point-y)) * 100%);
					width: calc(2 * var(--handle-radius));
					height: calc(2 * var(--handle-radius));
					margin: calc(-1 * var(--handle-radius));
					box-sizing: border-box;
					border-radius: 50%;
					background: var(--color-e-nearwhite);
					pointer-events: none;

					// A ring around the dot marks the point a click would take
					&.targeted {
						z-index: 1;
						outline: 1px solid var(--color-e-nearwhite);
						outline-offset: 2px;
					}

					&.preview {
						background: var(--color-8-uppergray);
					}

					&.active {
						z-index: 1;
						background: var(--color-f-white);
						outline: 1px solid var(--color-f-white);
						outline-offset: 2px;
					}
				}
			}
		}

		&.disabled {
			.pointer-field {
				pointer-events: none;
			}

			.curve-path {
				stroke: var(--color-8-uppergray);
			}

			.curve-point {
				background: var(--color-8-uppergray);
			}
		}
	}
</style>
