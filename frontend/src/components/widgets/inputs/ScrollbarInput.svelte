<script lang="ts" context="module">
	export type ScrollbarDirection = "Horizontal" | "Vertical";
</script>

<script lang="ts">
	import { createEventDispatcher, onMount, onDestroy } from "svelte";

	// Linear Interpolation
	const lerp = (x: number, y: number, a: number): number => x * (1 - a) + y * a;

	// Convert the position of the handle (0-1) to the position on the track (0-1).
	// This includes the 1/2 handle length gap of the possible handle positionson each side so the end of the handle doesn't go off the track.
	const handleToTrack = (handleLen: number, handlePos: number): number => lerp(handleLen / 2, 1 - handleLen / 2, handlePos);

	const pointerPosition = (direction: ScrollbarDirection, e: PointerEvent): number => (direction === "Vertical" ? e.clientY : e.clientX);

	const dispatch = createEventDispatcher<{ handlePosition: number; pressTrack: number }>();

	export let direction: ScrollbarDirection = "Vertical";
	export let handlePosition = 0.5;
	export let handleLength = 0.5;

	let scrollTrack: HTMLDivElement | undefined;
	let dragging = false;
	let pointerPos = 0;
	let thumbTop: string | undefined = undefined;
	let thumbBottom: string | undefined = undefined;
	let thumbLeft: string | undefined = undefined;
	let thumbRight: string | undefined = undefined;

	$: start = handleToTrack(handleLength, handlePosition) - handleLength / 2;
	$: end = 1 - handleToTrack(handleLength, handlePosition) - handleLength / 2;
	$: [thumbTop, thumbBottom, thumbLeft, thumbRight] = direction === "Vertical" ? [`${start * 100}%`, `${end * 100}%`, "0%", "0%"] : ["0%", "0%", `${start * 100}%`, `${end * 100}%`];

	function trackLength(): number | undefined {
		if (scrollTrack === undefined) return undefined;
		return direction === "Vertical" ? scrollTrack.clientHeight - handleLength : scrollTrack.clientWidth;
	}

	function trackOffset(): number | undefined {
		if (scrollTrack === undefined) return undefined;
		return direction === "Vertical" ? scrollTrack.getBoundingClientRect().top : scrollTrack.getBoundingClientRect().left;
	}

	function clampHandlePosition(newPos: number) {
		const clampedPosition = Math.min(Math.max(newPos, 0), 1);
		dispatch("handlePosition", clampedPosition);
	}

	function updateHandlePosition(e: PointerEvent) {
		const length = trackLength();
		if (length === undefined) return;

		const position = pointerPosition(direction, e);

		clampHandlePosition(handlePosition + (position - pointerPos) / (length * (1 - handleLength)));
		pointerPos = position;
	}

	function grabHandle(e: PointerEvent) {
		if (!dragging) {
			dragging = true;
			pointerPos = pointerPosition(direction, e);
		}
	}

	function grabArea(e: PointerEvent) {
		if (!dragging) {
			const length = trackLength();
			const offset = trackOffset();
			if (length === undefined || offset === undefined) return;

			const oldPointer = handleToTrack(handleLength, handlePosition) * length + offset;
			const pointerPos = pointerPosition(direction, e);
			dispatch("pressTrack", pointerPos - oldPointer);
		}
	}

	function pointerUp() {
		dragging = false;
	}

	function pointerMove(e: PointerEvent) {
		if (dragging) updateHandlePosition(e);
	}

	function changePosition(difference: number) {
		const length = trackLength();
		if (length === undefined) return;

		clampHandlePosition(handlePosition + difference / length);
	}

	onMount(() => {
		window.addEventListener("pointerup", pointerUp);
		window.addEventListener("pointermove", pointerMove);
	});

	onDestroy(() => {
		window.removeEventListener("pointerup", pointerUp);
		window.removeEventListener("pointermove", pointerMove);
	});
</script>

<div class={`scrollbar-input ${direction.toLowerCase()}`}>
	<button class="arrow decrease" on:pointerdown={() => changePosition(-50)} tabindex="-1" />
	<div class="scroll-track" bind:this={scrollTrack} on:pointerdown={grabArea}>
		<div class="scroll-thumb" on:pointerdown={grabHandle} class:dragging style:top={thumbTop} style:bottom={thumbBottom} style:left={thumbLeft} style:right={thumbRight} />
	</div>
	<button class="arrow increase" on:click={() => changePosition(50)} tabindex="-1" />
</div>

<style lang="scss" global>
	.scrollbar-input {
		display: flex;
		flex: 1 1 100%;

		.arrow {
			flex: 0 0 auto;
			background: none;
			border: none;
			border-style: solid;
			width: 0;
			height: 0;
			margin: 0;
			padding: 0;
		}

		.scroll-track {
			flex: 1 1 100%;
			position: relative;

			.scroll-thumb {
				position: absolute;
				border-radius: 4px;
				background: var(--color-5-dullgray);

				&:hover,
				&.dragging {
					background: var(--color-6-lowergray);
				}
			}

			.scroll-click-area {
				position: absolute;
			}
		}

		&.vertical {
			flex-direction: column;

			.arrow.decrease {
				margin: 4px 3px;
				border-width: 0 5px 8px 5px;
				border-color: transparent transparent var(--color-5-dullgray) transparent;

				&:hover {
					border-color: transparent transparent var(--color-6-lowergray) transparent;
				}
				&:active {
					border-color: transparent transparent var(--color-c-brightgray) transparent;
				}
			}

			.arrow.increase {
				margin: 4px 3px;
				border-width: 8px 5px 0 5px;
				border-color: var(--color-5-dullgray) transparent transparent transparent;

				&:hover {
					border-color: var(--color-6-lowergray) transparent transparent transparent;
				}
				&:active {
					border-color: var(--color-c-brightgray) transparent transparent transparent;
				}
			}
		}

		&.horizontal {
			flex-direction: row;

			.arrow.decrease {
				margin: 3px 4px;
				border-width: 5px 8px 5px 0;
				border-color: transparent var(--color-5-dullgray) transparent transparent;

				&:hover {
					border-color: transparent var(--color-6-lowergray) transparent transparent;
				}
				&:active {
					border-color: transparent var(--color-c-brightgray) transparent transparent;
				}
			}

			.arrow.increase {
				margin: 3px 4px;
				border-width: 5px 0 5px 8px;
				border-color: transparent transparent transparent var(--color-5-dullgray);

				&:hover {
					border-color: transparent transparent transparent var(--color-6-lowergray);
				}
				&:active {
					border-color: transparent transparent transparent var(--color-c-brightgray);
				}
			}
		}
	}
</style>
