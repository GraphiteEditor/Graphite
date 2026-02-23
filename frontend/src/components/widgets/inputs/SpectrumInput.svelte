<script lang="ts" context="module">
	export const MIN_MIDPOINT = 0.01;
	export const MAX_MIDPOINT = 0.99;
</script>

<script lang="ts">
	import { createEventDispatcher, onDestroy } from "svelte";

	import { evaluateGradientAtPosition } from "@graphite/../wasm/pkg/graphite_wasm";
	import type { Gradient } from "@graphite/messages";
	import { Color } from "@graphite/messages";

	import { preventEscapeClosingParentFloatingMenu } from "@graphite/components/layout/FloatingMenu.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	const BUTTON_LEFT = 0;
	const BUTTON_RIGHT = 2;

	const dispatch = createEventDispatcher<{ activeMarkerIndexChange: { activeMarkerIndex: number | undefined; activeMarkerIsMidpoint: boolean }; gradient: Gradient; dragging: boolean }>();

	export let gradient: Gradient;
	export let disabled = false;
	export let activeMarkerIndex = 0 as number | undefined;
	export let activeMarkerIsMidpoint = false;
	// export let disabled = false;
	// export let tooltipLabel: string | undefined = undefined;
	// export let tooltipDescription: string | undefined = undefined;
	// export let tooltipShortcut: ActionShortcut | undefined = undefined;

	let markerTrack: LayoutRow | undefined = undefined;
	let positionRestore: number | undefined = undefined;
	let deletionRestore: boolean | undefined = undefined;
	let midpointRestore: number | undefined = undefined;
	let activeMarkerIndexRestore: number | undefined = undefined;
	let activeMarkerIsMidpointRestore = false;
	let midpointDragged = false;

	function markerPointerDown(e: PointerEvent, index: number) {
		if (disabled) return;

		// Left-click to select and begin potentially dragging
		if (e.button === BUTTON_LEFT) {
			activeMarkerIndexRestore = activeMarkerIndex;
			activeMarkerIsMidpointRestore = activeMarkerIsMidpoint;
			activeMarkerIndex = index;
			activeMarkerIsMidpoint = false;
			dispatch("activeMarkerIndexChange", { activeMarkerIndex, activeMarkerIsMidpoint });
			addEvents();
			return;
		}

		// Right-click to delete
		if (e.button === BUTTON_RIGHT && deletionRestore === undefined) {
			deleteStopByIndex(index);
			return;
		}
	}

	function markerPosition(e: MouseEvent): number | undefined {
		const markerTrackRect = markerTrack?.div()?.getBoundingClientRect();
		if (!markerTrackRect) return;

		const ratio = (e.clientX - markerTrackRect.left) / markerTrackRect.width;

		return Math.max(0, Math.min(1, ratio));
	}

	function midpointPointerDown(e: PointerEvent, index: number) {
		if (disabled) return;
		if (e.button !== BUTTON_LEFT) return;

		activeMarkerIndexRestore = activeMarkerIndex;
		activeMarkerIsMidpointRestore = activeMarkerIsMidpoint;
		activeMarkerIndex = index;
		activeMarkerIsMidpoint = true;
		midpointDragged = false;

		dispatch("activeMarkerIndexChange", { activeMarkerIndex, activeMarkerIsMidpoint });

		addEvents();
	}

	function resetMidpoint(index: number, force = false) {
		if (!force && (disabled || midpointDragged)) return;

		gradient.midpoint[index] = 0.5;
		dispatch("gradient", gradient);
	}

	function insertStop(e: MouseEvent) {
		if (disabled) return;
		if (e.button !== BUTTON_LEFT) return;

		let position = markerPosition(e);
		if (position === undefined) return;

		let before = gradient.position.findLastIndex((item) => item < position);
		let after = gradient.position.findIndex((item) => item > position);

		let color = Color.fromCSS("black") as Color;
		if (before !== -1 && after !== -1) {
			type ReturnedColor = { red: number; green: number; blue: number; alpha: number };
			const evaluated = evaluateGradientAtPosition(position, new Float64Array(gradient.position), new Float64Array(gradient.midpoint), gradient.color) as ReturnedColor;
			color = new Color(evaluated.red, evaluated.green, evaluated.blue, evaluated.alpha);
		} else if (before !== -1) {
			color = gradient.color[before];
		} else if (after !== -1) {
			color = gradient.color[after];
		}

		let index = gradient.position.findIndex((item) => item > position);
		if (index === -1) index = gradient.position.length;

		gradient.position.splice(index, 0, position);
		gradient.midpoint.splice(index, 0, gradient.midpoint[index - 1] ?? 0.5);
		gradient.color.splice(index, 0, color);

		activeMarkerIndexRestore = activeMarkerIndex;
		activeMarkerIsMidpointRestore = activeMarkerIsMidpoint;
		activeMarkerIndex = index;
		activeMarkerIsMidpoint = false;
		deletionRestore = true;

		dispatch("activeMarkerIndexChange", { activeMarkerIndex, activeMarkerIsMidpoint });
		dispatch("gradient", gradient);

		addEvents();
	}

	function deleteStop(e: KeyboardEvent) {
		if (disabled) return;

		if (e.key !== "Delete" && e.key !== "Backspace") return;
		if (activeMarkerIndex === undefined) return;

		if (positionRestore !== undefined) stopDrag();

		if (activeMarkerIsMidpoint) resetMidpoint(activeMarkerIndex, true);
		else deleteStopByIndex(activeMarkerIndex);
	}

	function deleteStopByIndex(index: number) {
		if (disabled) return;

		if (gradient.position.length <= 2) return;

		gradient.position.splice(index, 1);
		gradient.midpoint.splice(index, 1);
		gradient.color.splice(index, 1);

		if (gradient.position.length === 0) {
			activeMarkerIndex = undefined;
		} else {
			activeMarkerIndex = Math.max(0, Math.min(gradient.position.length - 1, index));
		}
		activeMarkerIsMidpoint = false;
		deletionRestore = undefined;

		dispatch("activeMarkerIndexChange", { activeMarkerIndex, activeMarkerIsMidpoint });
		dispatch("gradient", gradient);
	}

	function moveMarker(e: PointerEvent, index: number) {
		if (disabled) return;

		// Just in case the mouseup event is lost
		if (e.buttons === 0) stopDrag();

		let position = markerPosition(e);
		if (position === undefined) return;

		if (positionRestore === undefined) positionRestore = position;
		if (deletionRestore === undefined) {
			deletionRestore = false;

			dispatch("dragging", true);
		}

		setPosition(index, position, false);
	}

	function moveMidpoint(e: PointerEvent, index: number) {
		if (disabled) return;

		// Just in case the mouseup event is lost
		if (e.buttons === 0) {
			stopDrag();
			return;
		}

		let position = markerPosition(e);
		if (position === undefined) return;

		if (midpointRestore === undefined) {
			midpointRestore = gradient.midpoint[index];
			midpointDragged = true;
			dispatch("dragging", true);
		}

		const leftStop = gradient.position[index];
		const rightStop = gradient.position[index + 1];
		const range = rightStop - leftStop;
		if (range <= 0) return;

		gradient.midpoint[index] = Math.max(MIN_MIDPOINT, Math.min(MAX_MIDPOINT, (position - leftStop) / range));
		dispatch("gradient", gradient);
	}

	export function setPosition(index: number, position: number, isMidpoint: boolean) {
		if (disabled) return;

		const markers = toMarkers(gradient);
		const active = markers[index];

		if (isMidpoint) active.midpoint = position;
		else active.position = position;

		markers.sort((a, b) => a.position - b.position);
		if (markers.indexOf(active) !== activeMarkerIndex) {
			activeMarkerIndex = markers.indexOf(active);
			dispatch("activeMarkerIndexChange", { activeMarkerIndex, activeMarkerIsMidpoint });
		}

		gradient.position = markers.map((stop) => stop.position);
		gradient.midpoint = markers.map((stop) => stop.midpoint);
		gradient.color = markers.map((stop) => stop.color);
		dispatch("gradient", gradient);
	}

	function toMarkers(gradient: Gradient): { position: number; midpoint: number; color: Color }[] {
		return gradient.position.map((position, i) => ({
			position,
			midpoint: gradient.midpoint[i],
			color: gradient.color[i],
		}));
	}

	function toMidpoints(gradient: Gradient): number[] {
		if (gradient.position.length < 2) return [];

		return gradient.midpoint.slice(0, -1).map((midpoint, i) => {
			const leftMarker = gradient.position[i];
			const rightMarker = gradient.position[i + 1];
			return leftMarker + midpoint * (rightMarker - leftMarker);
		});
	}

	function abortDrag() {
		if (disabled) return;

		if (activeMarkerIndex !== undefined) {
			if (activeMarkerIsMidpoint && midpointRestore !== undefined) {
				gradient.midpoint[activeMarkerIndex] = midpointRestore;
				dispatch("gradient", gradient);
			} else {
				if (deletionRestore) deleteStopByIndex(activeMarkerIndex);
				else if (positionRestore !== undefined) setPosition(activeMarkerIndex, positionRestore, false);
			}
		}

		activeMarkerIndex = activeMarkerIndexRestore;
		activeMarkerIsMidpoint = activeMarkerIsMidpointRestore;
		dispatch("activeMarkerIndexChange", { activeMarkerIndex, activeMarkerIsMidpoint });

		stopDrag();
	}

	function stopDrag() {
		if (disabled) return;

		removeEvents();

		positionRestore = undefined;
		deletionRestore = undefined;
		midpointRestore = undefined;
		activeMarkerIndexRestore = undefined;
		activeMarkerIsMidpointRestore = false;

		dispatch("dragging", false);
	}

	function onPointerMove(e: PointerEvent) {
		if (disabled) return;

		if (activeMarkerIsMidpoint && activeMarkerIndex !== undefined) moveMidpoint(e, activeMarkerIndex);
		else if (activeMarkerIndex !== undefined) moveMarker(e, activeMarkerIndex);
	}

	function onPointerUp() {
		if (disabled) return;

		stopDrag();
	}

	function onMouseDown(e: MouseEvent) {
		if (disabled) return;

		const BUTTONS_RIGHT = 0b0000_0010;
		if (e.buttons & BUTTONS_RIGHT) abortDrag();
	}

	function onKeyDown(e: KeyboardEvent) {
		if (disabled) return;

		if (e.key === "Escape") {
			const element = markerTrack?.div();
			if (element) preventEscapeClosingParentFloatingMenu(element);

			abortDrag();
		}
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

	document.addEventListener("keydown", deleteStop);
	onDestroy(() => {
		removeEvents();
		document.removeEventListener("keydown", deleteStop);
	});

	// Future design notes:
	//
	// # Backend -> Frontend
	// Populate(gradient, { position, color }[], active) // The only way indexes get changed. Frontend drops marker if it's being dragged.
	// UpdateGradient(gradient)
	// UpdateMarkers({ index, position, color }[])
	//
	// # Frontend -> Backend
	// SendNewActive(index)
	// SendPositions({ index, position }[])
	// AddMarker(position)
	// RemoveMarkers(index[])
	// ResetMarkerToDefault(index)
	//
	// We need a way to encode constraints on some markers, like locking them in place or preventing reordering
	// We need a way to encode the allowability of adding new markers between certain markers, or preventing the deletion of certain markers
	// We need the ability to multi-select markers and move them all at once
</script>

<LayoutCol
	class="spectrum-input"
	classes={{ disabled }}
	styles={{
		"--gradient-start": gradient.firstColor()?.toHexOptionalAlpha() || "black",
		"--gradient-end": gradient.lastColor()?.toHexOptionalAlpha() || "black",
		"--gradient-stops": gradient.toLinearGradientCSS(),
	}}
>
	<LayoutRow class="gradient-strip" on:pointerdown={insertStop}></LayoutRow>
	<LayoutRow class="midpoint-track">
		{#each toMidpoints(gradient) as midpoint, index}
			<svg
				class="midpoint"
				class:active={index === activeMarkerIndex && activeMarkerIsMidpoint}
				style:--midpoint-position={midpoint}
				on:pointerdown={(e) => midpointPointerDown(e, index)}
				on:dblclick={() => resetMidpoint(index)}
				data-gradient-midpoint
				xmlns="http://www.w3.org/2000/svg"
				viewBox="0 0 8 8"
			>
				<polygon points="0,4 4,0 8,4 4,8" />
			</svg>
		{/each}
	</LayoutRow>
	<LayoutRow class="marker-track" bind:this={markerTrack}>
		{#each toMarkers(gradient) as marker, index}
			<svg
				class="marker"
				class:active={index === activeMarkerIndex && !activeMarkerIsMidpoint}
				style:--marker-position={marker.position}
				style:--marker-color={marker.color.toRgbCSS()}
				on:pointerdown={(e) => markerPointerDown(e, index)}
				data-gradient-marker
				xmlns="http://www.w3.org/2000/svg"
				viewBox="0 0 12 12"
			>
				<path class="inner-fill" d="M10,11.5H2c-0.8,0-1.5-0.7-1.5-1.5V6.8c0-0.4,0.2-0.8,0.4-1.1L6,0.7l5.1,5.1c0.3,0.3,0.4,0.7,0.4,1.1V10C11.5,10.8,10.8,11.5,10,11.5z" />
				{#if disabled}
					<path class="disabled-fill" d="M10,11.5H2c-0.8,0-1.5-0.7-1.5-1.5V6.8c0-0.4,0.2-0.8,0.4-1.1L6,0.7l5.1,5.1c0.3,0.3,0.4,0.7,0.4,1.1V10C11.5,10.8,10.8,11.5,10,11.5z" />
				{/if}
				<path
					class="outer-border"
					d="M6,1.4L1.3,6.1C1.1,6.3,1,6.6,1,6.8V10c0,0.6,0.4,1,1,1h8c0.6,0,1-0.4,1-1V6.8c0-0.3-0.1-0.5-0.3-0.7L6,1.4M6,0l5.4,5.4C11.8,5.8,12,6.3,12,6.8V10c0,1.1-0.9,2-2,2H2c-1.1,0-2-0.9-2-2V6.8c0-0.5,0.2-1,0.6-1.4L6,0z"
				/>
			</svg>
		{/each}
	</LayoutRow>
</LayoutCol>

<style lang="scss" global>
	.spectrum-input {
		position: relative;
		--marker-half-width: 6px;

		.gradient-strip {
			flex: 0 0 auto;
			height: 16px;
			background-image:
				var(--gradient-stops),
				// Solid start/end colors on either side so the gradient begins at the center of a marker
				linear-gradient(var(--gradient-start), var(--gradient-start)),
				linear-gradient(var(--gradient-end), var(--gradient-end)),
				var(--color-transparent-checkered-background);
			background-size:
				calc(100% - 2 * var(--marker-half-width)) 100%,
				// TODO: Find a solution that avoids visual artifacts where these end colors meet the gradient that appear when viewing with a non-integer zoom or display scaling factor
				var(--marker-half-width) 100%,
				var(--marker-half-width) 100%,
				var(--color-transparent-checkered-background-size);
			background-position:
				var(--marker-half-width) 0,
				left 0,
				right 0,
				var(--color-transparent-checkered-background-position);
			background-repeat: no-repeat, no-repeat, no-repeat, var(--color-transparent-checkered-background-repeat);
			border-radius: 2px;
		}

		&.disabled .gradient-strip {
			transition: opacity 0.1s;

			&:hover {
				opacity: 0.5;
			}
		}

		.midpoint-track {
			position: absolute;
			top: 0;
			left: var(--marker-half-width);
			right: var(--marker-half-width);

			.midpoint {
				position: absolute;
				margin-left: -4px;
				width: 8px;
				height: 8px;
				bottom: 0;
				left: calc(var(--midpoint-position) * 100%);

				polygon {
					stroke: var(--color-e-nearwhite);
					fill: var(--color-2-mildblack);
				}

				&.active {
					z-index: 1;

					polygon {
						fill: var(--color-e-nearwhite);
					}
				}
			}
		}

		.marker-track {
			margin-top: calc(24px - 16px - 12px);
			margin-left: var(--marker-half-width);
			width: calc(100% - 2 * var(--marker-half-width));
			position: relative;
			pointer-events: none;

			.marker {
				position: absolute;
				transform: translateX(-50%);
				left: calc(var(--marker-position) * 100%);
				width: 12px;
				height: 12px;
				pointer-events: auto;
				overflow: visible;
				padding-top: 12px;
				margin-top: -12px;

				.inner-fill {
					fill: var(--marker-color);
				}

				.outer-border {
					fill: var(--color-5-dullgray);
				}
			}
		}

		&.disabled .marker-track .marker {
			.disabled-fill {
				opacity: 0.5;
			}

			.outer-border {
				fill: var(--color-4-dimgray);
			}
		}

		&:not(.disabled) .marker-track .marker {
			&:not(.active) {
				.inner-fill:hover + .outer-border,
				.outer-border:hover {
					fill: var(--color-6-lowergray);
				}
			}

			&.active {
				z-index: 1;

				.inner-fill {
					filter: drop-shadow(0 0 1px var(--color-2-mildblack)) drop-shadow(0 0 1px var(--color-2-mildblack));
				}

				// Outer border when active
				.outer-border {
					fill: var(--color-e-nearwhite);
				}

				.inner-fill:hover + .outer-border,
				.outer-border:hover {
					fill: var(--color-f-white);
				}
			}
		}
	}
</style>
