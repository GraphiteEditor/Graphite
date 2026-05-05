<script lang="ts">
	import { createEventDispatcher, getContext, onDestroy } from "svelte";
	import LayoutCol from "/src/components/layout/LayoutCol.svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import type { TooltipStore } from "/src/stores/tooltip";
	import { colorContrastingColor, colorOpaque, colorToHexNoAlpha, colorToRgbCSS, createColor, createColorFromHSVA } from "/src/utility-functions/colors";

	const dispatch = createEventDispatcher<{
		update: { hue: number; saturation: number; value: number; alpha: number };
		startHistoryTransaction: undefined;
		commitHistoryTransaction: undefined;
		dragStateChange: boolean;
	}>();
	const tooltip = getContext<TooltipStore>("tooltip");

	export let hue: number;
	export let saturation: number;
	export let value: number;
	export let alpha: number;
	export let isNone: boolean;
	export let disabled = false;

	// Transient drag state
	let draggingPickerTrack: HTMLDivElement | undefined = undefined;
	let shiftPressed = false;
	let alignedAxis: "saturation" | "value" | undefined = undefined;
	let hueBeforeDrag = 0;
	let saturationBeforeDrag = 0;
	let valueBeforeDrag = 0;
	let alphaBeforeDrag = 0;
	let saturationStartOfAxisAlign: number | undefined = undefined;
	let valueStartOfAxisAlign: number | undefined = undefined;
	let saturationRestoreWhenShiftReleased: number | undefined = undefined;
	let valueRestoreWhenShiftReleased: number | undefined = undefined;

	function emitUpdate(h: number, s: number, v: number, a: number) {
		dispatch("update", { hue: h, saturation: s, value: v, alpha: a });
	}

	function onPointerDown(e: PointerEvent) {
		if (disabled) return;

		const target = e.target instanceof HTMLElement ? e.target : undefined;
		draggingPickerTrack = target?.closest("[data-saturation-value-picker], [data-hue-picker], [data-alpha-picker]") || undefined;

		hueBeforeDrag = hue;
		saturationBeforeDrag = saturation;
		valueBeforeDrag = value;
		alphaBeforeDrag = alpha;

		saturationStartOfAxisAlign = undefined;
		valueStartOfAxisAlign = undefined;
		saturationRestoreWhenShiftReleased = undefined;
		valueRestoreWhenShiftReleased = undefined;

		addEvents();

		onPointerMove(e);
	}

	function onPointerMove(e: PointerEvent) {
		// Just in case the mouseup event is lost
		if (e.buttons === 0) removeEvents();

		let nextHue = hue;
		let nextSaturation = saturation;
		let nextValue = value;
		let nextAlpha = alpha;

		if (draggingPickerTrack?.hasAttribute("data-saturation-value-picker")) {
			const rectangle = draggingPickerTrack.getBoundingClientRect();

			nextSaturation = clamp((e.clientX - rectangle.left) / rectangle.width, 0, 1);
			nextValue = clamp(1 - (e.clientY - rectangle.top) / rectangle.height, 0, 1);
			dispatch("dragStateChange", true);

			if (shiftPressed) {
				const locked = applyAxisLock(nextSaturation, nextValue);
				nextSaturation = locked.saturation;
				nextValue = locked.value;
			}
		} else if (draggingPickerTrack?.hasAttribute("data-hue-picker")) {
			const rectangle = draggingPickerTrack.getBoundingClientRect();

			nextHue = clamp(1 - (e.clientY - rectangle.top) / rectangle.height, 0, 1);
			dispatch("dragStateChange", true);
		} else if (draggingPickerTrack?.hasAttribute("data-alpha-picker")) {
			const rectangle = draggingPickerTrack.getBoundingClientRect();

			nextAlpha = clamp(1 - (e.clientY - rectangle.top) / rectangle.height, 0, 1);
			dispatch("dragStateChange", true);
		}

		emitUpdate(nextHue, nextSaturation, nextValue, nextAlpha);

		if (!e.shiftKey) {
			shiftPressed = false;
			alignedAxis = undefined;
		} else if (!shiftPressed && draggingPickerTrack) {
			shiftPressed = true;
			saturationStartOfAxisAlign = saturationBeforeDrag;
			valueStartOfAxisAlign = valueBeforeDrag;
		}
	}

	function onPointerUp() {
		if (draggingPickerTrack) dispatch("commitHistoryTransaction");
		removeEvents();
	}

	function onMouseDown(e: MouseEvent) {
		const BUTTONS_RIGHT = 0b0000_0010;
		if (e.buttons & BUTTONS_RIGHT) abortDrag();
	}

	function onKeyDown(e: KeyboardEvent) {
		if (e.key === "Escape") abortDrag();
	}

	function onKeyUp(e: KeyboardEvent) {
		if (e.key === "Shift") {
			shiftPressed = false;
			alignedAxis = undefined;

			if (saturationRestoreWhenShiftReleased !== undefined && valueRestoreWhenShiftReleased !== undefined) {
				emitUpdate(hue, saturationRestoreWhenShiftReleased, valueRestoreWhenShiftReleased, alpha);
			}
		}
	}

	function addEvents() {
		document.addEventListener("pointermove", onPointerMove);
		document.addEventListener("pointerup", onPointerUp);
		document.addEventListener("mousedown", onMouseDown);
		document.addEventListener("keydown", onKeyDown);
		document.addEventListener("keyup", onKeyUp);

		dispatch("startHistoryTransaction");
	}

	function removeEvents() {
		draggingPickerTrack = undefined;
		// The setTimeout is necessary to prevent the FloatingMenu's `escapeCloses` from becoming true immediately upon pressing the Escape key, and thus closing
		setTimeout(() => dispatch("dragStateChange", false), 0);
		shiftPressed = false;
		alignedAxis = undefined;

		document.removeEventListener("pointermove", onPointerMove);
		document.removeEventListener("pointerup", onPointerUp);
		document.removeEventListener("mousedown", onMouseDown);
		document.removeEventListener("keydown", onKeyDown);
		document.removeEventListener("keyup", onKeyUp);
	}

	function applyAxisLock(s: number, v: number): { saturation: number; value: number } {
		if (saturationStartOfAxisAlign === undefined || valueStartOfAxisAlign === undefined) return { saturation: s, value: v };

		const deltaSaturation = s - saturationStartOfAxisAlign;
		const deltaValue = v - valueStartOfAxisAlign;

		saturationRestoreWhenShiftReleased = s;
		valueRestoreWhenShiftReleased = v;

		if (Math.abs(deltaSaturation) < Math.abs(deltaValue)) {
			alignedAxis = "saturation";
			return { saturation: saturationStartOfAxisAlign, value: v };
		} else {
			alignedAxis = "value";
			return { saturation: s, value: valueStartOfAxisAlign };
		}
	}

	function abortDrag() {
		removeEvents();

		emitUpdate(hueBeforeDrag, saturationBeforeDrag, valueBeforeDrag, alphaBeforeDrag);
	}

	function clamp(input: number, min = 0, max = 1): number {
		return Math.max(min, Math.min(input, max));
	}

	onDestroy(() => {
		removeEvents();
	});

	$: newColor = isNone ? undefined : createColorFromHSVA(hue, saturation, value, alpha);
	$: opaqueHueColor = createColorFromHSVA(hue, 1, 1, 1);
	$: opaqueColorOnly = newColor ? colorOpaque(newColor) : createColor(0, 0, 0, 1);
</script>

<LayoutRow
	class="visual-color-pickers-input"
	classes={{ disabled }}
	styles={{
		"--hue-color": colorToRgbCSS(opaqueHueColor),
		"--hue-color-contrasting": colorContrastingColor(opaqueHueColor),
		"--opaque-color": colorToHexNoAlpha(opaqueColorOnly),
		"--opaque-color-contrasting": colorContrastingColor(opaqueColorOnly),
		"--new-color-contrasting": colorContrastingColor(newColor),
	}}
>
	{@const hueDescription = "The shade along the spectrum of the rainbow."}
	<LayoutCol
		class="saturation-value-picker"
		data-tooltip-label="Saturation and Value"
		data-tooltip-description={`To move only along the saturation (X) or value (Y) axis, perform the shortcut shown.${disabled ? "\n\nDisabled (read-only)." : ""}`}
		data-tooltip-shortcut={$tooltip.shiftClickShortcut?.shortcut ? JSON.stringify($tooltip.shiftClickShortcut.shortcut) : undefined}
		on:pointerdown={onPointerDown}
		data-saturation-value-picker
	>
		{#if alignedAxis}
			<div
				class="selection-circle-axis-snap-line"
				style:width={alignedAxis === "value" ? "100%" : undefined}
				style:height={alignedAxis === "saturation" ? "100%" : undefined}
				style:top={alignedAxis === "value" ? `${(1 - value) * 100}%` : undefined}
				style:left={alignedAxis === "saturation" ? `${saturation * 100}%` : undefined}
			></div>
			<div
				class="selection-circle-axis-snap-line"
				style:width={alignedAxis === "saturation" ? "100%" : undefined}
				style:height={alignedAxis === "value" ? "100%" : undefined}
				style:top={alignedAxis === "saturation" ? `${(1 - valueBeforeDrag) * 100}%` : undefined}
				style:left={alignedAxis === "value" ? `${saturationBeforeDrag * 100}%` : undefined}
			></div>
		{/if}
		{#if !isNone}
			<div class="selection-circle" style:top={`${(1 - value) * 100}%`} style:left={`${saturation * 100}%`}></div>
		{/if}
	</LayoutCol>
	<LayoutCol class="hue-picker" data-tooltip-label="Hue" data-tooltip-description={`${hueDescription}${disabled ? "\n\nDisabled (read-only)." : ""}`} on:pointerdown={onPointerDown} data-hue-picker>
		{#if !isNone}
			<div class="selection-needle" style:top={`${(1 - hue) * 100}%`}></div>
		{/if}
	</LayoutCol>
	<LayoutCol
		class="alpha-picker"
		data-tooltip-label="Alpha"
		data-tooltip-description={`The level of translucency.${disabled ? "\n\nDisabled (read-only)." : ""}`}
		on:pointerdown={onPointerDown}
		data-alpha-picker
	>
		{#if !isNone}
			<div class="selection-needle" style:top={`${(1 - alpha) * 100}%`}></div>
		{/if}
	</LayoutCol>
</LayoutRow>

<style lang="scss">
	.visual-color-pickers-input {
		--picker-size: 256px;
		--picker-circle-radius: 6px;

		.saturation-value-picker {
			width: var(--picker-size);
			background-blend-mode: multiply;
			background: linear-gradient(to bottom, #ffffff, #000000), linear-gradient(to right, #ffffff, var(--hue-color));
			position: relative;
		}

		.saturation-value-picker,
		.hue-picker,
		.alpha-picker {
			height: var(--picker-size);
			border-radius: 2px;
			position: relative;
			overflow: hidden;
		}

		.hue-picker,
		.alpha-picker {
			width: 24px;
			margin-left: 8px;
			position: relative;
		}

		.hue-picker {
			--selection-needle-color: var(--hue-color-contrasting);
			background-blend-mode: screen;
			background:
				// Reds
				linear-gradient(to top, #ff0000ff calc(100% / 6), #ff000000 calc(200% / 6), #ff000000 calc(400% / 6), #ff0000ff calc(500% / 6)),
				// Greens
				linear-gradient(to top, #00ff0000 0%, #00ff00ff calc(100% / 6), #00ff00ff 50%, #00ff0000 calc(400% / 6)),
				// Blues
				linear-gradient(to top, #0000ff00 calc(200% / 6), #0000ffff 50%, #0000ffff calc(500% / 6), #0000ff00 100%);
		}

		.alpha-picker {
			--selection-needle-color: var(--new-color-contrasting);
			background-image: linear-gradient(to bottom, var(--opaque-color), transparent), var(--color-transparent-checkered-background);
			background-size:
				100% 100%,
				var(--color-transparent-checkered-background-size);
			background-position:
				0 0,
				var(--color-transparent-checkered-background-position);
			background-repeat: no-repeat, var(--color-transparent-checkered-background-repeat);
		}

		.selection-circle {
			pointer-events: none;
			position: absolute;
			left: 0;
			top: 0;
			width: 0;
			height: 0;

			&::after {
				content: "";
				display: block;
				position: relative;
				left: calc(-1 * var(--picker-circle-radius));
				top: calc(-1 * var(--picker-circle-radius));
				width: calc(var(--picker-circle-radius) * 2 + 1px);
				height: calc(var(--picker-circle-radius) * 2 + 1px);
				border-radius: 50%;
				border: 2px solid var(--opaque-color-contrasting);
				background: var(--opaque-color);
				box-sizing: border-box;
			}
		}

		.selection-circle-axis-snap-line {
			pointer-events: none;
			position: absolute;
			width: 1px;
			height: 1px;
			top: 0;
			left: 0;
			background: var(--opaque-color-contrasting);

			+ .selection-circle-axis-snap-line {
				opacity: 0.25;
			}
		}

		.selection-needle {
			pointer-events: none;
			position: absolute;
			top: 0;
			width: 100%;
			height: 0;

			&::before {
				content: "";
				position: absolute;
				top: -4px;
				left: 0;
				border-style: solid;
				border-width: 4px 0 4px 4px;
				border-color: transparent transparent transparent var(--selection-needle-color);
			}

			&::after {
				content: "";
				position: absolute;
				top: -4px;
				right: 0;
				border-style: solid;
				border-width: 4px 4px 4px 0;
				border-color: transparent var(--selection-needle-color) transparent transparent;
			}
		}

		&.disabled :is(.saturation-value-picker, .hue-picker, .alpha-picker) {
			transition: opacity 0.1s;

			&:hover {
				opacity: 0.5;
			}
		}
	}

	// paddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpaddingpadding
</style>
