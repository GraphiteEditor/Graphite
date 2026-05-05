<script lang="ts">
	import { createEventDispatcher, onMount, onDestroy } from "svelte";
	import { preventEscapeClosingParentFloatingMenu } from "/src/components/layout/FloatingMenu.svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import type { SpectrumInputUpdate, SpectrumMarker } from "/wrapper/pkg/graphite_wasm_wrapper";

	const BUTTON_LEFT = 0;
	const BUTTON_RIGHT = 2;

	const dispatch = createEventDispatcher<{ update: SpectrumInputUpdate; dragging: boolean }>();

	export let trackCSS: string;
	export let trackStartCSS: string;
	export let trackEndCSS: string;
	export let markers: SpectrumMarker[];
	export let activeMarkerIndex: number | undefined = 0;
	export let activeMarkerIsMidpoint = false;
	export let showMidpoints = true;
	export let allowInsert = true;
	export let allowDelete = true;
	export let allowSwap = true;
	export let disabled = false;

	// Reference to the marker track DOM element so we can convert pointer coordinates to a 0..1 position along the track.
	let markerTrackElement: LayoutRow | undefined = undefined;

	// Drag state — only TS-local; Rust owns authoritative marker data.
	// Position the dragged marker (or midpoint) had at drag start, restored if the drag is cancelled.
	let dragRestorePosition: number | undefined = undefined;
	// True if this drag began with an insertion (so cancel must delete the inserted marker).
	let dragInsertedMarker = false;
	// Active marker selection at drag start, restored if the drag is cancelled.
	let activeMarkerIndexRestore: number | undefined = undefined;
	let activeMarkerIsMidpointRestore = false;
	// Tracks whether a midpoint drag has actually moved by at least one frame, to distinguish click-to-select from drag.
	let midpointDragged = false;

	function emit(intent: SpectrumInputUpdate) {
		dispatch("update", intent);
	}

	function setActive(index: number | undefined, isMidpoint: boolean) {
		activeMarkerIndex = index;
		activeMarkerIsMidpoint = isMidpoint;
		emit({ ActiveMarker: { activeMarkerIndex: index, activeMarkerIsMidpoint: isMidpoint } });
	}

	function pointerPosition(e: MouseEvent): number | undefined {
		const rect = markerTrackElement?.div()?.getBoundingClientRect();
		if (!rect) return undefined;
		const ratio = (e.clientX - rect.left) / rect.width;
		return Math.max(0, Math.min(1, ratio));
	}

	function clampToNeighbors(index: number, position: number): number {
		const lower = markers[index - 1]?.position ?? 0;
		const upper = markers[index + 1]?.position ?? 1;
		return Math.max(lower, Math.min(upper, position));
	}

	function markerPointerDown(e: PointerEvent, index: number) {
		if (disabled) return;

		if (e.button === BUTTON_LEFT) {
			activeMarkerIndexRestore = activeMarkerIndex;
			activeMarkerIsMidpointRestore = activeMarkerIsMidpoint;
			dragRestorePosition = markers[index].position;
			dragInsertedMarker = false;
			setActive(index, false);
			addEvents();
			return;
		}

		if (e.button === BUTTON_RIGHT && allowDelete) {
			emit({ DeleteMarker: { index } });
		}
	}

	function midpointPointerDown(e: PointerEvent, index: number) {
		if (disabled) return;
		if (e.button !== BUTTON_LEFT) return;

		midpointDragged = false;
		activeMarkerIndexRestore = activeMarkerIndex;
		activeMarkerIsMidpointRestore = activeMarkerIsMidpoint;
		dragRestorePosition = markers[index].midpoint;
		setActive(index, true);
		addEvents();
	}

	function midpointDoubleClick(index: number) {
		if (disabled || midpointDragged) return;
		emit({ ResetMidpoint: { index } });
	}

	function trackPointerDown(e: PointerEvent) {
		if (disabled) return;
		if (e.button !== BUTTON_LEFT) return;
		if (!allowInsert) return;

		const position = pointerPosition(e);
		if (position === undefined) return;

		// Compute the index this marker will land at after Rust inserts it (matches Rust's `insert_stop` logic).
		let insertIndex = markers.findIndex((m) => m.position > position);
		if (insertIndex === -1) insertIndex = markers.length;

		emit({ InsertMarker: { position } });

		activeMarkerIndexRestore = activeMarkerIndex;
		activeMarkerIsMidpointRestore = activeMarkerIsMidpoint;
		dragRestorePosition = position;
		dragInsertedMarker = true;
		// Don't dispatch an `ActiveMarker` here — the Rust handler already updates the active marker in response to `InsertMarker` and a duplicate `ActiveMarker` would race the layout update.
		activeMarkerIndex = insertIndex;
		activeMarkerIsMidpoint = false;
		addEvents();
	}

	function deleteShortcut(e: KeyboardEvent) {
		if (disabled) return;
		if (e.key !== "Delete" && e.key !== "Backspace") return;
		if (activeMarkerIndex === undefined) return;

		stopDrag();
		if (activeMarkerIsMidpoint) emit({ ResetMidpoint: { index: activeMarkerIndex } });
		else if (allowDelete) emit({ DeleteMarker: { index: activeMarkerIndex } });
	}

	function moveActiveMarker(e: PointerEvent) {
		if (disabled || activeMarkerIndex === undefined) return;
		if (e.buttons === 0) {
			stopDrag();
			return;
		}

		let position = pointerPosition(e);
		if (position === undefined) return;
		if (!allowSwap) position = clampToNeighbors(activeMarkerIndex, position);

		if (!dragInsertedMarker) dispatch("dragging", true);
		emit({ MoveMarker: { index: activeMarkerIndex, position } });
	}

	function moveActiveMidpoint(e: PointerEvent) {
		if (disabled || activeMarkerIndex === undefined) return;
		if (e.buttons === 0) {
			stopDrag();
			return;
		}

		const absolute = pointerPosition(e);
		if (absolute === undefined) return;

		const left = markers[activeMarkerIndex]?.position;
		const right = markers[activeMarkerIndex + 1]?.position;
		if (left === undefined || right === undefined) return;
		const range = right - left;
		if (range <= 0) return;

		midpointDragged = true;
		dispatch("dragging", true);
		emit({ MoveMidpoint: { index: activeMarkerIndex, position: (absolute - left) / range } });
	}

	function abortDrag() {
		if (disabled || activeMarkerIndex === undefined) return;

		// Restore the dragged value, or delete the marker if it was inserted as part of this drag.
		if (dragInsertedMarker) {
			emit({ DeleteMarker: { index: activeMarkerIndex } });
		} else if (dragRestorePosition !== undefined) {
			if (activeMarkerIsMidpoint) emit({ MoveMidpoint: { index: activeMarkerIndex, position: dragRestorePosition } });
			else emit({ MoveMarker: { index: activeMarkerIndex, position: dragRestorePosition } });
		}

		setActive(activeMarkerIndexRestore, activeMarkerIsMidpointRestore);
		stopDrag();
	}

	function stopDrag() {
		removeEvents();
		dragRestorePosition = undefined;
		dragInsertedMarker = false;
		activeMarkerIndexRestore = undefined;
		activeMarkerIsMidpointRestore = false;
		midpointDragged = false;
		dispatch("dragging", false);
	}

	function onPointerMove(e: PointerEvent) {
		if (activeMarkerIsMidpoint) moveActiveMidpoint(e);
		else moveActiveMarker(e);
	}

	function onPointerUp() {
		stopDrag();
	}

	function onMouseDown(e: MouseEvent) {
		const BUTTONS_RIGHT = 0b0000_0010;
		if (e.buttons & BUTTONS_RIGHT) abortDrag();
	}

	function onKeyDown(e: KeyboardEvent) {
		if (e.key === "Escape") {
			const element = markerTrackElement?.div();
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

	// Map midpoint pairs to absolute track positions for rendering the diamond markers.
	$: midpointPositions = !showMidpoints || markers.length < 2 ? [] : markers.slice(0, -1).map((marker, i) => marker.position + marker.midpoint * (markers[i + 1].position - marker.position));

	onMount(() => {
		document.addEventListener("keydown", deleteShortcut);
	});
	onDestroy(() => {
		removeEvents();
		document.removeEventListener("keydown", deleteShortcut);
	});
</script>

<LayoutCol
	class="spectrum-input"
	classes={{ disabled }}
	styles={{
		"--gradient-start": trackStartCSS,
		"--gradient-end": trackEndCSS,
		"--gradient-stops": trackCSS,
	}}
>
	<LayoutRow class="gradient-strip" on:pointerdown={trackPointerDown}></LayoutRow>
	<LayoutRow class="midpoint-track">
		{#each midpointPositions as midpoint, index}
			<svg
				class="midpoint"
				class:active={index === activeMarkerIndex && activeMarkerIsMidpoint}
				style:--midpoint-position={midpoint}
				on:pointerdown={(e) => midpointPointerDown(e, index)}
				on:dblclick={() => midpointDoubleClick(index)}
				data-gradient-midpoint
				xmlns="http://www.w3.org/2000/svg"
				viewBox="0 0 8 8"
			>
				<polygon points="0,4 4,0 8,4 4,8" />
			</svg>
		{/each}
	</LayoutRow>
	<LayoutRow class="marker-track" bind:this={markerTrackElement}>
		{#each markers as marker, index}
			<svg
				class="marker"
				class:active={index === activeMarkerIndex && !activeMarkerIsMidpoint}
				style:--marker-position={marker.position}
				style:--marker-color={marker.handleColorCSS}
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

<style lang="scss">
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

		&.disabled .midpoint-track .midpoint polygon {
			stroke: var(--color-4-dimgray);
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
