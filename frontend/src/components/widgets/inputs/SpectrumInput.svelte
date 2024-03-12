<script lang="ts">
	import { onDestroy } from "svelte";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	// const dispatch = createEventDispatcher<{ checked: boolean }>();

	let activeMarkerIndex = 0;
	let markers = [
		{ position: 0, color: "#e25151" },
		{ position: 0.25, color: "#ffc86d" },
		{ position: 0.5, color: "#fbdca3" },
		{ position: 0.75, color: "#f8eadd" },
		{ position: 1, color: "#85cbda" },
	];

	let markerTrack: LayoutRow | undefined;

	// export let disabled = false;
	// export let tooltip: string | undefined = undefined;

	function markerPointerDown(e: PointerEvent, index: number) {
		activeMarkerIndex = index;

		addEvents();

		onPointerMove(e);
	}

	function onPointerMove(e: PointerEvent) {
		// Just in case the mouseup event is lost
		if (e.buttons === 0) removeEvents();

		const markerTrackRect = markerTrack?.div()?.getBoundingClientRect();
		if (!markerTrackRect) return;
		const ratio = (e.clientX - markerTrackRect.left) / markerTrackRect.width;

		markers[activeMarkerIndex].position = Math.max(0, Math.min(1, ratio));
	}

	function onPointerUp() {
		removeEvents();
	}

	function addEvents() {
		document.addEventListener("pointermove", onPointerMove);
		document.addEventListener("pointerup", onPointerUp);
	}

	function removeEvents() {
		document.removeEventListener("pointermove", onPointerMove);
		document.removeEventListener("pointerup", onPointerUp);
	}

	onDestroy(() => {
		removeEvents();
	});

	// # Backend -> Frontend
	// Populate(gradient, { position, color }[], active) // The only way indexes get changed. Frontend drops marker if it's being dragged.
	// UpdateGradient(gradient)
	// UpdateMarkers({ index, position, color }[])

	// # Frontend -> Backend
	// SendNewActive(index)
	// SendPositions({ index, position }[])
	// AddMarker(position)
	// RemoveMarkers(index[])

	// // We need a way to encode constraints on some markers, like locking them in place or preventing reordering
	// // We need a way to encode the allowability of adding new markers between certain markers, or preventing the deletion of certain markers
	// // We need a way to reset a marker to what can be considered its default position, by double clicking on it
</script>

<LayoutCol class="spectrum-input">
	<LayoutRow class="gradient-strip"></LayoutRow>
	<LayoutRow class="marker-track" bind:this={markerTrack}>
		{#each markers as marker, index}
			<svg
				style:--marker-position={marker.position}
				style:--marker-color={marker.color}
				class="marker"
				class:active={index === activeMarkerIndex}
				data-gradient-marker
				xmlns="http://www.w3.org/2000/svg"
				viewBox="0 0 12 12"
			>
				<path
					on:pointerdown={(e) => markerPointerDown(e, index)}
					d="M10,11.5H2c-0.8,0-1.5-0.7-1.5-1.5V6.8c0-0.4,0.2-0.8,0.4-1.1L6,0.7l5.1,5.1c0.3,0.3,0.4,0.7,0.4,1.1V10C11.5,10.8,10.8,11.5,10,11.5z"
				/>
				<path
					on:pointerdown={(e) => markerPointerDown(e, index)}
					d="M6,1.4L1.3,6.1C1.1,6.3,1,6.6,1,6.8V10c0,0.6,0.4,1,1,1h8c0.6,0,1-0.4,1-1V6.8c0-0.3-0.1-0.5-0.3-0.7L6,1.4M6,0l5.4,5.4C11.8,5.8,12,6.3,12,6.8V10c0,1.1-0.9,2-2,2H2c-1.1,0-2-0.9-2-2V6.8c0-0.5,0.2-1,0.6-1.4L6,0z"
				/>
			</svg>
		{/each}
	</LayoutRow>
</LayoutCol>

<style lang="scss" global>
	.spectrum-input {
		--marker-half-width: 6px;
		--gradient-stops: #e25151 0%, #ffc86d 25%, #fbdca3 50%, #f8eadd 75%, #85cbda 100%;
		--gradient-start: #e25151;
		--gradient-end: #85cbda;

		.gradient-strip {
			flex: 0 0 auto;
			height: 16px;
			background-image:
				linear-gradient(to right, var(--gradient-stops)),
				// Solid start/end colors on either side so the gradient begins at the center of a marker
				linear-gradient(var(--gradient-start), var(--gradient-start)),
				linear-gradient(var(--gradient-end), var(--gradient-end));
			background-size:
				calc(100% - 2 * var(--marker-half-width)) 100%,
				// 2px fudge factor to avoid rendering artifacts
				calc(var(--marker-half-width) + 2px) 100%,
				calc(var(--marker-half-width) + 2px) 100%;
			background-position:
				var(--marker-half-width) 0,
				left 0,
				right 0;
			background-repeat: no-repeat;
			border-radius: 2px;
		}

		.marker-track {
			margin-top: calc(24px - 16px - 12px);
			margin-left: var(--marker-half-width);
			width: calc(100% - 2 * var(--marker-half-width));
			position: relative;

			.marker {
				position: absolute;
				transform: translateX(-50%);
				left: calc(var(--marker-position) * 100%);
				width: 12px;
				height: 12px;

				// Inner fill
				path:first-child {
					fill: var(--marker-color);
				}

				// Outer border
				path:last-child {
					fill: var(--color-5-dullgray);
				}

				&:not(.active) path:first-child:hover + path:last-child,
				&:not(.active) path:last-child:hover {
					fill: var(--color-6-lowergray);
				}

				// Outer border when active
				&.active path:last-child {
					fill: var(--color-e-nearwhite);
				}

				&.active path:first-child:hover + path:last-child,
				&.active path:last-child:hover {
					fill: var(--color-f-white);
				}
			}
		}
	}
</style>
