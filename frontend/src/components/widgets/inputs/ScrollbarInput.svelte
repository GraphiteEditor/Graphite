<script lang="ts" context="module">
	export type ScrollbarDirection = "Horizontal" | "Vertical";
</script>

<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import { PRESS_REPEAT_DELAY_MS, PRESS_REPEAT_INTERVAL_MS, PRESS_REPEAT_INTERVAL_RAPID_MS } from "@graphite/io-managers/input";

	const ARROW_CLICK_DISTANCE = 0.05;
	const ARROW_REPEAT_DISTANCE = 0.01;

	// Convert the position of the thumb (0-1) to the position on the track (0-1).
	// This includes the 1/2 thumb length gap of the possible thumb position each side so the end of the thumb doesn't go off the track.
	const lerp = (a: number, b: number, t: number): number => a * (1 - t) + b * t;
	const thumbToTrack = (thumbLength: number, thumbPosition: number): number => lerp(thumbLength / 2, 1 - thumbLength / 2, thumbPosition);

	const pointerPosition = (e: PointerEvent): number => (direction === "Vertical" ? e.clientY : e.clientX);

	const clamp01 = (value: number): number => Math.min(Math.max(value, 0), 1);

	const dispatch = createEventDispatcher<{ trackShift: number; thumbPosition: number; thumbDragStart: undefined; thumbDragEnd: undefined; thumbDragAbort: undefined }>();

	export let direction: ScrollbarDirection = "Vertical";
	export let thumbPosition = 0.5;
	export let thumbLength = 0.5;

	let scrollTrack: HTMLDivElement | undefined;
	let dragging = false;
	let pressingTrack = false;
	let pressingArrow = false;
	let repeatTimeout: ReturnType<typeof setTimeout> | undefined = undefined;
	let pointerPositionLastFrame = 0;
	let thumbTop: string | undefined = undefined;
	let thumbBottom: string | undefined = undefined;
	let thumbLeft: string | undefined = undefined;
	let thumbRight: string | undefined = undefined;

	$: start = thumbToTrack(thumbLength, thumbPosition) - thumbLength / 2;
	$: end = 1 - thumbToTrack(thumbLength, thumbPosition) - thumbLength / 2;
	$: [thumbTop, thumbBottom, thumbLeft, thumbRight] = direction === "Vertical" ? [`${start * 100}%`, `${end * 100}%`, "0%", "0%"] : ["0%", "0%", `${start * 100}%`, `${end * 100}%`];

	function trackLength(): number | undefined {
		if (scrollTrack === undefined) return undefined;
		return direction === "Vertical" ? scrollTrack.clientHeight - thumbLength : scrollTrack.clientWidth;
	}

	function trackOffset(): number | undefined {
		if (scrollTrack === undefined) return undefined;
		return direction === "Vertical" ? scrollTrack.getBoundingClientRect().top : scrollTrack.getBoundingClientRect().left;
	}

	function dragThumb(e: PointerEvent) {
		if (dragging) return;

		dragging = true;
		dispatch("thumbDragStart");
		pointerPositionLastFrame = pointerPosition(e);

		addEvents();
	}

	function pressArrow(direction: number) {
		const sendMove = () => {
			if (!pressingArrow) return;

			const distance = afterInitialDelay ? ARROW_REPEAT_DISTANCE : ARROW_CLICK_DISTANCE;
			dispatch("trackShift", -direction * distance);

			if (afterInitialDelay) repeatTimeout = setTimeout(sendMove, PRESS_REPEAT_INTERVAL_RAPID_MS);
			afterInitialDelay = true;
		};

		pressingArrow = true;
		dispatch("thumbDragStart");
		let afterInitialDelay = false;
		sendMove();
		repeatTimeout = setTimeout(sendMove, PRESS_REPEAT_DELAY_MS);

		addEvents();
	}

	function pressTrack(e: PointerEvent) {
		if (dragging) return;

		const length = trackLength();
		const offset = trackOffset();
		if (length === undefined || offset === undefined) return;

		const sendMove = () => {
			if (!pressingTrack) return;

			const oldPointer = thumbToTrack(thumbLength, thumbPosition) * length + offset;
			const newPointer = pointerPosition(e);

			// Check if the thumb has reached the cursor position
			const proposedThumbPosition = (newPointer - offset) / length;
			if (proposedThumbPosition >= start && proposedThumbPosition <= 1 - end) {
				// End pressing the track
				pressingTrack = false;
				clearTimeout(repeatTimeout);

				// Begin dragging the thumb
				dragging = true;
				pointerPositionLastFrame = newPointer;

				return;
			}

			const move = newPointer - oldPointer < 0 ? 1 : -1;
			dispatch("trackShift", move);

			if (afterInitialDelay) repeatTimeout = setTimeout(sendMove, PRESS_REPEAT_INTERVAL_MS);
			afterInitialDelay = true;
		};

		dispatch("thumbDragStart");
		pressingTrack = true;
		let afterInitialDelay = false;
		sendMove();
		repeatTimeout = setTimeout(sendMove, PRESS_REPEAT_DELAY_MS);

		addEvents();
	}

	function abortInteraction() {
		if (pressingTrack || pressingArrow) {
			pressingTrack = false;
			pressingArrow = false;
			clearTimeout(repeatTimeout);
			dispatch("thumbDragAbort");
		}

		if (dragging) {
			dragging = false;
			dispatch("thumbDragAbort");
		}
	}

	function onPointerUp() {
		if (dragging) dispatch("thumbDragEnd");

		dragging = false;
		pressingTrack = false;
		pressingArrow = false;
		clearTimeout(repeatTimeout);
		removeEvents();
	}

	function onPointerMove(e: PointerEvent) {
		if (pressingTrack) {
			return;
		}

		if (pressingArrow) {
			const target = e.target || undefined;
			if (!target || !(target instanceof Element)) return;
			if (!target?.closest?.("[data-scrollbar-arrow]")) {
				pressingArrow = false;
				clearTimeout(repeatTimeout);
				removeEvents();
			}

			return;
		}

		if (dragging) {
			const length = trackLength();
			if (length === undefined) return;

			const positionPositionThisFrame = pointerPosition(e);
			const dragDelta = positionPositionThisFrame - pointerPositionLastFrame;
			const movement = dragDelta / (length * (1 - thumbLength));
			const newThumbPosition = clamp01(thumbPosition + movement);
			dispatch("thumbPosition", newThumbPosition);

			pointerPositionLastFrame = positionPositionThisFrame;

			return;
		}

		removeEvents();
	}

	function onMouseDown(e: MouseEvent) {
		const BUTTONS_RIGHT = 0b0000_0010;
		if (e.buttons & BUTTONS_RIGHT) abortInteraction();
	}

	function onKeyDown(e: KeyboardEvent) {
		if (e.key === "Escape") abortInteraction();
	}

	function addEvents() {
		window.addEventListener("pointerup", onPointerUp);
		window.addEventListener("pointermove", onPointerMove);
		window.addEventListener("mousedown", onMouseDown);
		window.addEventListener("keydown", onKeyDown);
	}

	function removeEvents() {
		window.removeEventListener("pointerup", onPointerUp);
		window.removeEventListener("pointermove", onPointerMove);
		window.removeEventListener("mousedown", onMouseDown);
		window.removeEventListener("keydown", onKeyDown);
	}
</script>

<div class={`scrollbar-input ${direction.toLowerCase()}`}>
	<button class="arrow decrease" on:pointerdown={() => pressArrow(-1)} tabindex="-1" data-scrollbar-arrow></button>
	<div class="scroll-track" on:pointerdown={pressTrack} bind:this={scrollTrack}>
		<div class="scroll-thumb" on:pointerdown={dragThumb} class:dragging style:top={thumbTop} style:bottom={thumbBottom} style:left={thumbLeft} style:right={thumbRight} />
	</div>
	<button class="arrow increase" on:pointerdown={() => pressArrow(1)} tabindex="-1" data-scrollbar-arrow></button>
</div>

<style lang="scss" global>
	.scrollbar-input {
		display: flex;
		flex: 1 1 100%;

		&.vertical {
			flex-direction: column;
		}

		&.horizontal {
			flex-direction: row;
		}

		.arrow {
			--arrow-color: var(--color-5-dullgray);
			flex: 0 0 auto;
			background: none;
			border: none;
			margin: 0;
			padding: 0;
			width: 16px;
			height: 16px;

			&:hover {
				--arrow-color: var(--color-6-lowergray);
			}

			&:hover:active {
				--arrow-color: var(--color-c-brightgray);
			}

			&::after {
				content: "";
				display: block;
				border-style: solid;
			}
		}

		&.vertical .arrow.decrease::after {
			margin: 4px 3px;
			border-width: 0 5px 8px 5px;
			border-color: transparent transparent var(--arrow-color) transparent;
		}

		&.vertical .arrow.increase::after {
			margin: 4px 3px;
			border-width: 8px 5px 0 5px;
			border-color: var(--arrow-color) transparent transparent transparent;
		}

		&.horizontal .arrow.decrease::after {
			margin: 3px 4px;
			border-width: 5px 8px 5px 0;
			border-color: transparent var(--arrow-color) transparent transparent;
		}

		&.horizontal .arrow.increase::after {
			margin: 3px 4px;
			border-width: 5px 0 5px 8px;
			border-color: transparent transparent transparent var(--arrow-color);
		}

		.scroll-track {
			position: relative;
			flex: 1 1 100%;

			.scroll-thumb {
				position: absolute;
				border-radius: 4px;
				background: var(--color-5-dullgray);

				&:hover,
				&.dragging {
					background: var(--color-6-lowergray);
				}
			}
		}
	}
</style>
